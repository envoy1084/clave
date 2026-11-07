//! `daemon start|stop|restart|status|logs`.

use std::{
    fmt::Write as _,
    io::{Read, Seek, SeekFrom},
    time::Duration,
};

use serde_json::{Value, json};

use crate::{
    DaemonCmd,
    context::{Ctx, write_stdout},
    daemon,
    error::{Error, Result},
    render,
    types::{DaemonInfo, Status},
};

pub(crate) fn run(ctx: &Ctx, cmd: &DaemonCmd) -> Result<()> {
    let profile = &ctx.profile;
    match cmd {
        DaemonCmd::Start { foreground: true } => daemon::run_foreground(&ctx.home, profile),
        DaemonCmd::Start { foreground: false } => start(ctx),
        DaemonCmd::Stop => {
            let spin = ctx.spin("Stopping the daemon and its keeper");
            let pid = daemon::stop(profile)?;
            spin.clear();
            ctx.emit(&json!({ "stopped": true, "pid": pid }), || {
                format!(
                    "{} Daemon stopped  {}\n",
                    ctx.style.ok(),
                    ctx.style
                        .dim(&pid.map(|p| format!("pid {p}")).unwrap_or_default())
                )
            });
            Ok(())
        }
        DaemonCmd::Restart => {
            if daemon::info(profile).is_some() {
                daemon::stop(profile)?;
            }
            start(ctx)
        }
        DaemonCmd::Status => {
            let mut client = ctx.client()?;
            let info: Value = client.call("daemon.info", Value::Null)?;
            let status: Value = client.call("status", Value::Null)?;
            ctx.emit(&json!({ "daemon": info, "status": status }), || {
                let info: DaemonInfo = serde_json::from_value(info.clone()).unwrap_or_default();
                let status: Status = serde_json::from_value(status.clone()).unwrap_or_default();
                let mut out = String::new();
                let _ = writeln!(
                    out,
                    "{:<10}claved {} (pid {})",
                    "Daemon", info.version, info.pid
                );
                let _ = writeln!(out, "{:<10}{}", "Profile", info.profile_dir);
                let _ = writeln!(
                    out,
                    "{:<10}{}",
                    "Started",
                    render::relative(render::secs(info.started_at), render::now_secs())
                );
                let _ = writeln!(
                    out,
                    "{:<10}{}",
                    "Engine",
                    ctx.style.status(&status.engine.status)
                );
                let _ = writeln!(
                    out,
                    "{:<10}{}",
                    "Keeper",
                    ctx.style.status(&status.keeper.status)
                );
                let _ = writeln!(out, "{:<10}{}", "Node", info.node);
                out
            });
            Ok(())
        }
        &DaemonCmd::Logs {
            follow,
            keeper,
            lines,
        } => {
            let path = if keeper {
                profile.keeper_log()
            } else {
                profile.daemon_log()
            };
            logs(&path, follow, lines)
        }
    }
}

fn start(ctx: &Ctx) -> Result<()> {
    let spin = ctx.spin("Starting the daemon and opening the PXE");
    let info = daemon::start(&ctx.home, &ctx.profile)?;
    spin.ok(
        "Daemon running",
        &format!("pid {} · profile {}", info.pid, info.profile),
    );
    if ctx.json {
        Ctx::json_line(
            &json!({ "pid": info.pid, "version": info.version, "profile": info.profile }),
        );
    }
    Ok(())
}

fn logs(path: &std::path::Path, follow: bool, lines: usize) -> Result<()> {
    let mut file = std::fs::File::open(path).map_err(|e| {
        Error::failed(format!("{}: {e}", path.display())).hint("The daemon writes it on start")
    })?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let tail: Vec<&str> = text.lines().rev().take(lines).collect();
    let mut out: String = tail.iter().rev().fold(String::new(), |mut out, l| {
        out.push_str(l);
        out.push('\n');
        out
    });
    write_stdout(&std::mem::take(&mut out));
    if !follow {
        return Ok(());
    }
    let mut position = file.seek(SeekFrom::End(0))?;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let length = std::fs::metadata(path).map_or(0, |m| m.len());
        if length < position {
            // Rotated or truncated: start over.
            position = 0;
            file = std::fs::File::open(path)?;
        }
        file.seek(SeekFrom::Start(position))?;
        let mut chunk = String::new();
        position += u64::try_from(file.read_to_string(&mut chunk)?).unwrap_or(0);
        if !chunk.is_empty() {
            write_stdout(&chunk);
        }
    }
}
