//! Blocking JSON-RPC client for the daemon socket: newline-delimited JSON-RPC 2.0, `session.auth { token }` first,
//! events as `{"method":"event"}` notifications (`architecture/cli.md`, Transport).

use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, ErrorKind, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{
    error::{Error, Kind, Result},
    profile::Profile,
};

/// One authenticated connection to a daemon.
pub(crate) struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    next_id: u64,
    /// Bytes of a line not yet terminated when a read timed out.
    partial: Vec<u8>,
    /// Events that arrived while waiting for a response.
    events: VecDeque<Value>,
}

impl Client {
    /// Connects to the profile's daemon; exit code 3 when it is not running.
    pub(crate) fn connect(profile: &Profile) -> Result<Self> {
        let not_running = || {
            Error::new(
                Kind::Daemon,
                format!("Daemon for profile \"{}\" is not running.", profile.name),
            )
            .hint(format!("Start it: tecpan {}daemon start", profile.flag()))
        };
        let token = std::fs::read_to_string(profile.token()).map_err(|_| not_running())?;
        Self::connect_at(&profile.socket(), &Zeroizing::new(token)).map_err(|e| {
            if e.kind == Kind::Daemon {
                not_running()
            } else {
                e
            }
        })
    }

    /// Connects to `socket` and authenticates with `token`.
    pub(crate) fn connect_at(socket: &Path, token: &str) -> Result<Self> {
        let stream =
            UnixStream::connect(socket).map_err(|e| Error::new(Kind::Daemon, e.to_string()))?;
        let writer = stream.try_clone()?;
        let mut client = Self {
            reader: BufReader::new(stream),
            writer,
            next_id: 1,
            partial: Vec::new(),
            events: VecDeque::new(),
        };
        client
            .call::<Value>("session.auth", json!({ "token": token.trim() }))
            .map_err(|e| Error::failed(format!("daemon refused the session token: {e}")))?;
        Ok(client)
    }

    /// Calls `method` and decodes its result.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers build params inline with json!"
    )]
    pub(crate) fn call<T: DeserializeOwned>(&mut self, method: &str, params: Value) -> Result<T> {
        self.call_with(method, params, |_| {})
    }

    /// Calls `method`, handing every event that arrives meanwhile to `on_event`.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "callers build params inline with json!"
    )]
    pub(crate) fn call_with<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
        mut on_event: impl FnMut(&Value),
    ) -> Result<T> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.writer, "{request}")?;
        self.writer.flush()?;
        self.reader.get_ref().set_read_timeout(None)?;
        loop {
            let message = self.read_message()?.ok_or_else(closed)?;
            if is_event(&message) {
                on_event(&message["params"]);
                self.events.push_back(message["params"].clone());
                continue;
            }
            if message.get("id") != Some(&json!(id)) {
                continue;
            }
            if let Some(error) = message.get("error") {
                return Err(daemon_error(error));
            }
            let result = message.get("result").cloned().unwrap_or(Value::Null);
            return serde_json::from_value(result)
                .map_err(|e| Error::failed(format!("unexpected result from {method}: {e}")));
        }
    }

    /// `events.subscribe`: events then arrive through [`Client::next_event`].
    pub(crate) fn subscribe(&mut self) -> Result<()> {
        self.call::<Value>("events.subscribe", Value::Null)?;
        self.events.clear();
        Ok(())
    }

    /// The next event, waiting at most `timeout` (forever when `None`).
    pub(crate) fn next_event(&mut self, timeout: Option<Duration>) -> Result<Option<Value>> {
        if let Some(event) = self.events.pop_front() {
            return Ok(Some(event));
        }
        self.reader.get_ref().set_read_timeout(timeout)?;
        loop {
            match self.read_message() {
                Ok(Some(message)) if is_event(&message) => {
                    return Ok(Some(message["params"].clone()));
                }
                Ok(Some(_)) => {}
                Ok(None) => return Err(closed()),
                Err(e) if e.kind == Kind::NotReady => return Ok(None),
                Err(e) => return Err(e),
            }
        }
    }

    /// One message; `Err(NotReady)` signals a read timeout, `Ok(None)` a closed socket.
    fn read_message(&mut self) -> Result<Option<Value>> {
        loop {
            match self.reader.read_until(b'\n', &mut self.partial) {
                Ok(0) => return Ok(None),
                Ok(_) if !self.partial.ends_with(b"\n") => return Ok(None),
                Ok(_) => {
                    let line = std::mem::take(&mut self.partial);
                    if line.iter().all(u8::is_ascii_whitespace) {
                        continue;
                    }
                    return serde_json::from_slice(&line)
                        .map(Some)
                        .map_err(|e| Error::failed(format!("malformed daemon message: {e}")));
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Err(Error::new(Kind::NotReady, "timed out"));
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}

fn is_event(message: &Value) -> bool {
    message.get("id").is_none() && message.get("method").and_then(Value::as_str) == Some("event")
}

fn closed() -> Error {
    Error::new(Kind::Daemon, "the daemon closed the connection")
}

fn daemon_error(error: &Value) -> Error {
    let kind = error
        .pointer("/data/kind")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("daemon error")
        .to_owned();
    let mut out = Error::new(Kind::from_daemon(kind.as_deref()), message);
    out.hint = match kind.as_deref() {
        Some("locked") => Some("Unlock the signer (tecpan unlock) or pass the passphrase".into()),
        Some("auth") => Some("Check the passphrase and try again".into()),
        _ => None,
    };
    out.daemon_kind = kind;
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader},
        net::UnixListener,
    };

    use super::*;

    const TOKEN: &str = "secret-token";

    /// A fake daemon speaking the framing: checks the token, answers a few methods, sends events.
    fn fake_daemon() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("d.sock");
        let path = socket.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let listener = UnixListener::bind(&path).unwrap();
                ready_tx.send(()).unwrap();
                loop {
                    let (stream, _) = listener.accept().await.unwrap();
                    tokio::spawn(serve(stream));
                }
            });
        });
        ready_rx.recv().unwrap();
        (dir, socket)
    }

    async fn serve(stream: tokio::net::UnixStream) {
        let (read, mut write) = stream.into_split();
        let mut lines = TokioBufReader::new(read).lines();
        let mut authed = false;
        while let Some(line) = lines.next_line().await.unwrap() {
            let request: Value = serde_json::from_str(&line).unwrap();
            let id = request["id"].clone();
            let method = request["method"].as_str().unwrap().to_owned();
            let error = |kind: &str, message: &str| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": message, "data": { "kind": kind } } });
            let ok = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
            let mut out = Vec::new();
            if authed {
                match method.as_str() {
                    "daemon.info" => out.push(ok(json!({ "pid": 42, "profile": "ana" }))),
                    "events.subscribe" => {
                        out.push(ok(Value::Null));
                        out.push(json!({ "jsonrpc": "2.0", "method": "event", "params": { "type": "vault-activity", "vault": "0x01" } }));
                    }
                    "proposals.execute" => {
                        for phase in ["rebuilding", "proving"] {
                            out.push(json!({ "jsonrpc": "2.0", "method": "event", "params": { "type": "execution", "execution": { "phase": phase } } }));
                        }
                        out.push(ok(json!({ "id": "0x7f" })));
                    }
                    "balance" => out.push(error("not-ready", "1 of 3 keepers reachable")),
                    "proposals.get" => out.push(error("invalid", "unknown proposal")),
                    "proposals.approve" => out.push(error("locked", "signer locked")),
                    _ => out.push(error("failed", "no such method")),
                }
            } else if method == "session.auth" && request["params"]["token"] == TOKEN {
                authed = true;
                out.push(ok(Value::Null));
            } else {
                out.push(error("auth", "bad token"));
            }
            for message in out {
                write
                    .write_all(format!("{message}\n").as_bytes())
                    .await
                    .unwrap();
            }
        }
    }

    #[test]
    fn authenticates_and_calls() {
        let (_dir, socket) = fake_daemon();
        let mut client = Client::connect_at(&socket, &format!("{TOKEN}\n")).unwrap();
        let info: Value = client.call("daemon.info", Value::Null).unwrap();
        assert_eq!(info["pid"], 42);
    }

    #[test]
    fn wrong_token_is_refused() {
        let (_dir, socket) = fake_daemon();
        let error = Client::connect_at(&socket, "nope").err().unwrap();
        assert_eq!(error.exit_code(), 1);
        assert!(error.message.contains("session token"));
    }

    #[test]
    fn missing_socket_is_exit_3() {
        let dir = tempfile::tempdir().unwrap();
        let profile = Profile::resolve(dir.path(), "bea").unwrap();
        let error = Client::connect(&profile).err().unwrap();
        assert_eq!(error.exit_code(), 3);
        assert_eq!(
            error.hint.as_deref(),
            Some("Start it: tecpan -P bea daemon start")
        );
    }

    #[test]
    fn error_kinds_map_to_exit_codes() {
        let (_dir, socket) = fake_daemon();
        let mut client = Client::connect_at(&socket, TOKEN).unwrap();
        let code =
            |client: &mut Client, method| client.call::<Value>(method, Value::Null).err().unwrap();
        assert_eq!(code(&mut client, "balance").exit_code(), 4);
        assert_eq!(code(&mut client, "proposals.get").exit_code(), 2);
        let locked = code(&mut client, "proposals.approve");
        assert_eq!(locked.exit_code(), 1);
        assert_eq!(locked.daemon_kind.as_deref(), Some("locked"));
        assert_eq!(code(&mut client, "nope").exit_code(), 1);
    }

    #[test]
    fn events_during_a_call_and_after_subscribe() {
        let (_dir, socket) = fake_daemon();
        let mut client = Client::connect_at(&socket, TOKEN).unwrap();
        client.subscribe().unwrap();
        let first = client
            .next_event(Some(Duration::from_secs(5)))
            .unwrap()
            .unwrap();
        assert_eq!(first["type"], "vault-activity");
        let mut phases = Vec::new();
        let result: Value = client
            .call_with("proposals.execute", Value::Null, |e| {
                phases.push(e["execution"]["phase"].as_str().unwrap().to_owned());
            })
            .unwrap();
        assert_eq!(result["id"], "0x7f");
        assert_eq!(phases, ["rebuilding", "proving"]);
        // Events seen during a call stay queued for `next_event`.
        let queued = client
            .next_event(Some(Duration::from_millis(50)))
            .unwrap()
            .unwrap();
        assert_eq!(queued["execution"]["phase"], "rebuilding");
    }

    #[test]
    fn next_event_times_out() {
        let (_dir, socket) = fake_daemon();
        let mut client = Client::connect_at(&socket, TOKEN).unwrap();
        assert!(
            client
                .next_event(Some(Duration::from_millis(30)))
                .unwrap()
                .is_none()
        );
    }
}
