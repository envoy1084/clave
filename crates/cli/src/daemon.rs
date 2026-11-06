//! Finding, starting and stopping `tecpand` (`architecture/cli.md`, Daemon).

use std::{
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::Value;

use crate::{
    error::{Error, Kind, Result},
    profile::Profile,
    rpc::Client,
    types::DaemonInfo,
};

/// How long `daemon start` waits for `daemon.info` (the PXE can take a while to open).
const START_TIMEOUT: Duration = Duration::from_mins(1);
const STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// The repository root, from this crate's manifest (`crates/cli` → `..` → `..`), for development builds.
fn repo_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

/// `TECPAN_ENGINE`, else `$TECPAN_HOME/lib/engine/tecpand.js`, else the repository build.
pub(crate) fn engine_path(home: &Path) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("TECPAN_ENGINE").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let installed = home.join("lib/engine/tecpand.js");
    let candidates = [
        installed.clone(),
        repo_root().join("packages/engine/dist/tecpand.js"),
    ];
    candidates.into_iter().find(|p| p.is_file()).ok_or_else(|| {
        Error::failed(format!(
            "The engine (tecpand.js) was not found at {}.",
            installed.display()
        ))
        .hint("Build it: pnpm --filter @tecpan/engine build, or set TECPAN_ENGINE")
    })
}

/// `TECPAN_KEEPER_BIN`, else `$TECPAN_HOME/bin/tecpan-keeper`, else `crates/target/{release,debug}/tecpan-keeper`.
pub(crate) fn keeper_path(home: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("TECPAN_KEEPER_BIN").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let target = repo_root().join("crates/target");
    [
        home.join("bin/tecpan-keeper"),
        target.join("release/tecpan-keeper"),
        target.join("debug/tecpan-keeper"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// `daemon.info` from a running daemon, if one answers.
pub(crate) fn info(profile: &Profile) -> Option<DaemonInfo> {
    Client::connect(profile)
        .and_then(|mut client| client.call("daemon.info", Value::Null))
        .ok()
}

/// Starts the daemon detached (output to `daemon.log`) and waits until it answers.
pub(crate) fn start(home: &Path, profile: &Profile) -> Result<DaemonInfo> {
    if let Some(info) = info(profile) {
        return Err(Error::usage(format!(
            "The daemon for profile \"{}\" is already running (pid {}).",
            profile.name, info.pid
        )));
    }
    profile.create_dir()?;
    let engine = engine_path(home)?;
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(profile.daemon_log())?;
    let mut child = Command::new("node")
        .arg(&engine)
        .arg("--profile-dir")
        .arg(&profile.dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        // Its own process group: a Ctrl-C in this terminal must not stop the daemon.
        .process_group(0)
        .spawn()
        .map_err(|e| {
            Error::failed(format!("could not run node: {e}"))
                .hint("Install Node 24 and check it is on PATH: tecpan doctor")
        })?;
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if let Some(info) = info(profile) {
            return Ok(info);
        }
        if let Some(status) = child.try_wait()? {
            return Err(Error::failed(format!(
                "The daemon exited during start ({status}).{}",
                log_tail(&profile.daemon_log(), 5)
            ))
            .hint(format!("Full log: tecpan {}daemon logs", profile.flag())));
        }
        if Instant::now() > deadline {
            return Err(Error::failed(format!(
                "The daemon did not answer within {}s (pid {}).",
                START_TIMEOUT.as_secs(),
                child.id()
            ))
            .hint(format!(
                "Check its log: tecpan {}daemon logs",
                profile.flag()
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Runs the daemon in this terminal until it exits.
pub(crate) fn run_foreground(home: &Path, profile: &Profile) -> Result<()> {
    if let Some(info) = info(profile) {
        return Err(Error::usage(format!(
            "The daemon is already running (pid {}).",
            info.pid
        )));
    }
    profile.create_dir()?;
    let engine = engine_path(home)?;
    let status = Command::new("node")
        .arg(&engine)
        .arg("--profile-dir")
        .arg(&profile.dir)
        .arg("--foreground")
        .status()
        .map_err(|e| Error::failed(format!("could not run node: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::failed(format!("the daemon exited ({status})")))
    }
}

/// `daemon.stop`, then waits until the socket stops answering and the pid file is gone.
pub(crate) fn stop(profile: &Profile) -> Result<Option<u32>> {
    let pid = profile.pid();
    let mut client = Client::connect(profile)?;
    client.call::<Value>("daemon.stop", Value::Null)?;
    drop(client);
    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline {
        if !profile.pid_file().exists() && !answers(profile) {
            return Ok(pid);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(Error::new(
        Kind::Failed,
        format!(
            "The daemon (pid {}) did not exit within {}s.",
            pid.unwrap_or_default(),
            STOP_TIMEOUT.as_secs()
        ),
    ))
}

/// Whether anything accepts connections on the profile's socket.
fn answers(profile: &Profile) -> bool {
    std::os::unix::net::UnixStream::connect(profile.socket()).is_ok()
}

/// The last `lines` lines of a log, indented, for error messages.
pub(crate) fn log_tail(path: &Path, lines: usize) -> String {
    let Ok(text) = fs::read_to_string(path) else {
        return String::new();
    };
    let tail: Vec<&str> = text.lines().rev().take(lines).collect();
    tail.iter().rev().fold(String::new(), |mut out, line| {
        out.push_str("\n  ");
        out.push_str(line);
        out
    })
}
