//! Progress on stderr: an animated line while the daemon works, then `✓ text  1.2s` (or `✗`). Animated only on a
//! terminal; under `--json` or `-q` nothing is printed; otherwise (logs, pipes) only the final line.

use std::{
    io::{IsTerminal, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use super::{Style, elapsed};

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// How progress is shown.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Nothing (`--json`, `-q`).
    Silent,
    /// Final lines only (not a terminal).
    Lines,
    /// Animated.
    Live,
}

impl Mode {
    pub(crate) fn detect(silent: bool) -> Self {
        if silent {
            Self::Silent
        } else if std::io::stderr().is_terminal() {
            Self::Live
        } else {
            Self::Lines
        }
    }
}

/// One running step.
pub(crate) struct Spinner {
    mode: Mode,
    style: Style,
    text: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    started: Instant,
}

impl Spinner {
    pub(crate) fn start(mode: Mode, style: Style, text: &str) -> Self {
        let text = Arc::new(Mutex::new(text.to_owned()));
        let stop = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let thread = (mode == Mode::Live).then(|| {
            let (text, stop) = (Arc::clone(&text), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut frame = 0;
                while !stop.load(Ordering::Relaxed) {
                    let line = text.lock().map(|t| t.clone()).unwrap_or_default();
                    let took = took(started);
                    let mut err = std::io::stderr().lock();
                    let _ = write!(
                        err,
                        "\r\x1b[2K{} {line}  {}",
                        style.cyan(FRAMES[frame % FRAMES.len()]),
                        style.dim(&took)
                    );
                    let _ = err.flush();
                    drop(err);
                    frame += 1;
                    std::thread::sleep(Duration::from_millis(80));
                }
            })
        });
        Self {
            mode,
            style,
            text,
            stop,
            thread,
            started,
        }
    }

    /// Changes the running text.
    pub(crate) fn set(&self, text: &str) {
        if let Ok(mut current) = self.text.lock() {
            text.clone_into(&mut current);
        }
    }

    /// Prints a finished line above the running one.
    pub(crate) fn println(&self, line: &str) {
        if self.mode == Mode::Silent {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "\r\x1b[2K{line}");
    }

    /// Ends with `✓ text  <elapsed>`; `detail` follows dimmed.
    pub(crate) fn ok(mut self, text: &str, detail: &str) {
        let mark = self.style.ok();
        self.finish(&mark, text, detail);
    }

    /// Ends with `✗ text`.
    pub(crate) fn fail(mut self, text: &str) {
        let mark = self.style.fail();
        self.finish(&mark, text, "");
    }

    /// Stops and erases the line (before a prompt, or when the caller prints its own result).
    pub(crate) fn clear(mut self) {
        self.halt();
    }

    fn finish(&mut self, mark: &str, text: &str, detail: &str) {
        self.halt();
        if self.mode == Mode::Silent {
            return;
        }
        let took = took(self.started);
        let mut line = format!("{mark} {text}");
        if !detail.is_empty() {
            line.push_str("  ");
            line.push_str(&self.style.dim(detail));
        }
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err, "{}  {}", line, self.style.dim(&took));
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
            let mut err = std::io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.halt();
    }
}

/// A finished progress line without a spinner: `✓ text  detail  took`.
pub(crate) fn line(style: Style, ok: bool, text: &str, detail: &str, took: &str) -> String {
    let mark = if ok { style.ok() } else { style.fail() };
    let mut line = format!("{mark} {text}");
    if !detail.is_empty() {
        line.push_str("  ");
        line.push_str(&style.dim(detail));
    }
    if !took.is_empty() {
        line.push_str("  ");
        line.push_str(&style.dim(took));
    }
    line
}

fn took(started: Instant) -> String {
    elapsed(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX))
}
