//! One member's terminal: an interactive `bash` in a pseudo-terminal, its screen parsed by `vt100`. A
//! `PROMPT_COMMAND` hook prints a private OSC sequence with the exit code of each finished command, which the reader
//! thread turns into completion events.

use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{self, Receiver, Sender},
    },
};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

/// The hook's marker: `ESC ] 777 ; clave-done ; <code> BEL`.
const MARKER: &[u8] = b"\x1b]777;clave-done;";

/// A running member terminal.
pub(crate) struct Pane {
    pub(crate) title: &'static str,
    parser: Arc<Mutex<vt100::Parser>>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    done: Receiver<i32>,
    size: (u16, u16),
    /// The shell printed its first prompt.
    pub(crate) ready: bool,
    /// A command was submitted and has not finished.
    pub(crate) busy: bool,
    /// Exit code of the last finished command.
    pub(crate) last: Option<i32>,
}

impl Pane {
    /// Starts `bash` for `profile` with `env`; raw output is also appended to `log` when given.
    pub(crate) fn spawn(
        title: &'static str,
        profile: &str,
        env: &[(String, String)],
        (rows, cols): (u16, u16),
        log: Option<PathBuf>,
    ) -> Result<Self, String> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("pty: {e}"))?;
        let mut cmd = CommandBuilder::new("/bin/bash");
        cmd.args(["--noprofile", "--norc", "-i"]);
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd.env("CLAVE_PROFILE", profile);
        cmd.env("TERM", "xterm-256color");
        cmd.env("HISTFILE", "/dev/null");
        cmd.env("BASH_SILENCE_DEPRECATION_WARNING", "1");
        cmd.env("PS1", format!("\\[\\e[36m\\]{profile} \\$\\[\\e[0m\\] "));
        cmd.env(
            "PROMPT_COMMAND",
            r#"printf '\033]777;clave-done;%s\007' "$?""#,
        );
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("bash: {e}"))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("pty reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("pty writer: {e}"))?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 2000)));
        let (tx, done) = mpsc::channel();
        let log = log.and_then(|path| File::create(path).ok());
        let shared = Arc::clone(&parser);
        std::thread::spawn(move || read_loop(reader, &shared, &tx, log));
        Ok(Self {
            title,
            parser,
            writer,
            master: pair.master,
            _child: child,
            done,
            size: (rows, cols),
            ready: false,
            busy: false,
            last: None,
        })
    }

    pub(crate) fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// Clears the screen of an idle shell (readline's Ctrl-L redraws the prompt at the top).
    pub(crate) fn clear(&mut self) {
        if !self.busy {
            self.send(b"\x0c");
        }
    }

    /// Submits a line the runner typed: the shell is now busy until its marker arrives.
    pub(crate) fn submit(&mut self) {
        self.busy = true;
        self.send(b"\r");
    }

    /// Drains completion markers; the first one only means the shell is ready.
    pub(crate) fn poll(&mut self) {
        while let Ok(code) = self.done.try_recv() {
            if self.ready {
                self.busy = false;
                self.last = Some(code);
            } else {
                self.ready = true;
            }
        }
    }

    pub(crate) fn resize(&mut self, rows: u16, cols: u16) {
        if (rows, cols) == self.size || rows == 0 || cols == 0 {
            return;
        }
        self.size = (rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        if let Ok(mut parser) = self.parser.lock() {
            parser.screen_mut().set_size(rows, cols);
        }
    }

    pub(crate) fn screen(&self) -> Option<MutexGuard<'_, vt100::Parser>> {
        self.parser.lock().ok()
    }
}

fn read_loop(
    mut reader: Box<dyn Read + Send>,
    parser: &Mutex<vt100::Parser>,
    done: &Sender<i32>,
    mut log: Option<File>,
) {
    let mut buffer = [0u8; 8192];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let bytes = &buffer[..n];
        if let Some(file) = log.as_mut() {
            let _ = file.write_all(bytes);
        }
        if let Ok(mut parser) = parser.lock() {
            parser.process(bytes);
        }
        pending.extend_from_slice(bytes);
        for code in take_markers(&mut pending) {
            let _ = done.send(code);
        }
    }
}

/// Removes complete markers from `pending` and returns their codes; keeps a possible partial marker.
fn take_markers(pending: &mut Vec<u8>) -> Vec<i32> {
    let mut codes = Vec::new();
    loop {
        let Some(start) = find(pending, MARKER) else {
            // Keep a tail that could be the start of a marker split across reads.
            let keep = pending.len().min(MARKER.len());
            pending.drain(..pending.len() - keep);
            return codes;
        };
        let body = start + MARKER.len();
        let Some(end) = pending[body..].iter().position(|b| *b == 0x07) else {
            pending.drain(..start);
            return codes;
        };
        let code = std::str::from_utf8(&pending[body..body + end])
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(-1);
        codes.push(code);
        pending.drain(..=body + end);
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_across_reads() {
        let mut pending = b"out\x1b]777;clave-done;0\x07more\x1b]777;clave".to_vec();
        assert_eq!(take_markers(&mut pending), vec![0]);
        pending.extend_from_slice(b"-done;3\x07");
        assert_eq!(take_markers(&mut pending), vec![3]);
        assert!(take_markers(&mut pending).is_empty());
    }
}
