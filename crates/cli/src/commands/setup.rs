//! `init`, `doctor`, `status`, `profile …`, `completions`.

use std::{fmt::Write as _, process::Command, time::Duration};

use clap::CommandFactory;
use serde_json::{Value, json};

use crate::{
    ProfileCmd,
    context::{Ctx, write_stdout},
    daemon,
    error::{Error, Result},
    http,
    profile::{self, Profile},
    refs::short_hex,
    render::{self, layout, proposals::OPEN, status::status as render_status, style},
    rpc::Client,
    secret,
    types::{ProposalView, Settings, Status},
};

const DEFAULT_NODE_URL: &str = "http://localhost:8080";

pub(crate) fn init(ctx: &Ctx, name: Option<String>, touch_id: bool) -> Result<()> {
    let profile = &ctx.profile;
    if profile.exists() {
        return Err(
            Error::usage(format!("Profile \"{}\" already exists.", profile.name))
                .hint(format!("Use it: clave {}status", profile.flag())),
        );
    }
    profile.create_dir()?;
    let name = name.unwrap_or_else(|| profile.name.clone());
    let node_url = ctx
        .node_url
        .clone()
        .unwrap_or_else(|| DEFAULT_NODE_URL.to_owned());
    let info = if let Some(info) = daemon::info(profile) {
        info
    } else {
        let spin = ctx.spin("Starting the daemon");
        let info = daemon::start(&ctx.home, profile)?;
        spin.ok("Daemon started", &format!("pid {}", info.pid));
        info
    };
    if ctx.interactive && secret::passphrase_file_set().is_none() {
        ctx.note(&format!(
            "\n  {}\n  {}\n",
            ctx.style.bold("Choose a passphrase"),
            ctx.style
                .dim("It seals your signing key on this device. Nobody can recover it for you.")
        ));
    }
    let passphrase = secret::new_passphrase(ctx)?;
    let mut client = ctx.client()?;
    let spin = ctx.spin("Creating your signing key, keeper identity and Aztec account");
    let status: Value = client.call(
        "profile.create",
        json!({ "name": name, "nodeUrl": node_url, "passphrase": passphrase.as_str() }),
    )?;
    spin.ok("Keys and account created", "");
    if touch_id {
        match secret::enable_touch_id(ctx, &mut client, Some(passphrase.as_str())) {
            Ok(()) => ctx.note(&format!("{} Touch ID enabled", ctx.style.ok())),
            Err(e) => ctx.note(&format!("{} Touch ID not enabled: {e}", ctx.style.fail())),
        }
    }
    drop(passphrase);
    let typed: Status = serde_json::from_value(status.clone()).unwrap_or_default();
    ctx.emit(&status, || {
        let style = ctx.style;
        let mut out = String::from("\n");
        out.push_str(&layout::title(
            style,
            &format!("Welcome, {name}"),
            &["profile created"],
        ));
        let mut rows = vec![("Profile", profile.name.clone())];
        if let Some(p) = &typed.profile {
            rows.push(("Signer key", style.cyan(&short_hex(&p.signer_key.x))));
            if !p.account.is_empty() {
                rows.push(("Account", style.cyan(&p.account)));
            }
        }
        rows.push(("Node", typed.settings.node_url.clone()));
        rows.push((
            "Daemon",
            format!("running {}", style.dim(&format!("pid {}", info.pid))),
        ));
        rows.push(("Directory", style.dim(&profile.dir.display().to_string())));
        out.push_str(&layout::fields(style, &rows));
        let f = profile.flag();
        out.push_str(&layout::next(
            style,
            &[
                &format!("clave {f}lobby open --name <vault> -k 2 --tv 2"),
                &format!("clave {f}lobby join <invite>"),
            ],
        ));
        out
    });
    Ok(())
}

struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
    fix: Option<String>,
}

fn check(name: &'static str, result: std::result::Result<String, (String, String)>) -> Check {
    match result {
        Ok(detail) => Check {
            name,
            ok: true,
            detail,
            fix: None,
        },
        Err((detail, fix)) => Check {
            name,
            ok: false,
            detail,
            fix: Some(fix),
        },
    }
}

fn node_version() -> std::result::Result<String, (String, String)> {
    let install =
        "Install Node 24 (https://nodejs.org, or `fnm install 24`) and put it on PATH".to_owned();
    let output = Command::new("node")
        .arg("--version")
        .output()
        .map_err(|e| (format!("node not found: {e}"), install.clone()))?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let major: u32 = version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|m| m.parse().ok())
        .unwrap_or(0);
    if major >= 24 {
        Ok(format!("node {version}"))
    } else {
        Err((format!("node {version} is older than 24"), install))
    }
}

/// The node URL to probe: `-n`, else the daemon's setting, else the default.
fn probe_url(ctx: &Ctx) -> String {
    if let Some(url) = &ctx.node_url {
        return url.clone();
    }
    ctx.client()
        .and_then(|mut c| c.call::<Settings>("config.get", Value::Null))
        .map(|s| s.node_url)
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| DEFAULT_NODE_URL.to_owned())
}

fn node_check(url: &str) -> std::result::Result<String, (String, String)> {
    if url.starts_with("https://") {
        return Ok(format!(
            "{url} (https: not probed here; the daemon checks it, see clave status)"
        ));
    }
    let info = http::json_rpc(url, "node_getNodeInfo", Duration::from_secs(5)).map_err(|e| {
        (
            format!("{url}: {e}"),
            "Start a node (aztec start --local-network) or pass its URL with -n".to_owned(),
        )
    })?;
    let mut detail = url.to_owned();
    if let Some(version) = info.get("nodeVersion").and_then(Value::as_str) {
        let _ = write!(detail, "  Aztec {version}");
    }
    if let Some(chain) = info.get("l1ChainId").and_then(Value::as_u64) {
        let _ = write!(detail, "  L1 {chain}");
    }
    Ok(detail)
}

fn private_path(
    path: &std::path::Path,
    want: &str,
    missing: &str,
) -> std::result::Result<String, (String, String)> {
    match profile::group_or_other_bits(path) {
        None => Ok(format!("{} {missing}", path.display())),
        Some(0) => Ok(format!("{} is owner-only", path.display())),
        Some(bits) => Err((
            format!(
                "{} is accessible to others (mode …{bits:o})",
                path.display()
            ),
            format!("chmod {want} {}", path.display()),
        )),
    }
}

pub(crate) fn doctor(ctx: &Ctx) -> Result<()> {
    let profile = &ctx.profile;
    let mut checks = vec![check("node", node_version())];
    checks.push(check(
        "keeper",
        daemon::keeper_path(&ctx.home).map(|p| p.display().to_string()).ok_or_else(|| {
            (
                "clave-keeper not found".to_owned(),
                "Build it: cargo build --release -p clave-keeper (in crates/), or set CLAVE_KEEPER_BIN".to_owned(),
            )
        }),
    ));
    checks.push(check(
        "engine",
        daemon::engine_path(&ctx.home)
            .map(|p| p.display().to_string())
            .map_err(|e| (e.message.clone(), e.hint.clone().unwrap_or_default())),
    ));
    checks.push(check("aztec node", node_check(&probe_url(ctx))));
    let dir = if profile.dir.exists() {
        private_path(&profile.dir, "700", "")
    } else {
        Err((
            format!("profile \"{}\" does not exist", profile.name),
            format!("Create it: clave {}init", profile.flag()),
        ))
    };
    checks.push(check("profile", dir));
    if profile.dir.exists() {
        checks.push(check(
            "socket",
            private_path(&profile.socket(), "600", "absent (daemon not running)"),
        ));
        checks.push(check(
            "token",
            private_path(&profile.token(), "600", "absent (daemon not running)"),
        ));
        let daemon = match daemon::info(profile) {
            Some(info) => format!("running (pid {}, claved {})", info.pid, info.version),
            None => format!(
                "not running (start it: clave {}daemon start)",
                profile.flag()
            ),
        };
        checks.push(check("daemon", Ok(daemon)));
    }
    let failed = checks.iter().filter(|c| !c.ok).count();
    let value = json!({
        "ok": failed == 0,
        "checks": checks.iter().map(|c| json!({ "name": c.name, "ok": c.ok, "detail": c.detail, "fix": c.fix })).collect::<Vec<_>>(),
    });
    ctx.emit(&value, || {
        let style = ctx.style;
        let mut out = String::new();
        for c in &checks {
            let mark = if c.ok { style.ok() } else { style.fail() };
            let _ = writeln!(
                out,
                "{mark} {}{}",
                style::pad(&style.bold(c.name), 12),
                c.detail
            );
            if let Some(fix) = &c.fix {
                let _ = writeln!(out, "  {}{} {fix}", " ".repeat(12), style.yellow("→"));
            }
        }
        out
    });
    if failed > 0 {
        return Err(Error::failed(format!(
            "{failed} check{} failed",
            if failed == 1 { "" } else { "s" }
        )));
    }
    Ok(())
}

/// Open proposals per vault; `None` where the list failed.
fn open_counts(client: &mut Client, status: &Status) -> Vec<Option<usize>> {
    status
        .vaults
        .iter()
        .map(|v| {
            client
                .call::<Vec<ProposalView>>("proposals.list", json!({ "vault": v.address }))
                .ok()
                .map(|list| {
                    list.iter()
                        .filter(|p| OPEN.contains(&p.status.as_str()))
                        .count()
                })
        })
        .collect()
}

pub(crate) fn status(ctx: &Ctx) -> Result<()> {
    let mut client = ctx.client()?;
    let raw: Value = client.call("status", Value::Null)?;
    if ctx.json {
        Ctx::json_line(&raw);
        return Ok(());
    }
    let status: Status = serde_json::from_value(raw).map_err(|e| Error::failed(e.to_string()))?;
    let pid = client
        .call::<crate::types::DaemonInfo>("daemon.info", Value::Null)
        .ok()
        .map(|i| i.pid);
    let open = open_counts(&mut client, &status);
    let now_ms = render::now_secs() * 1000;
    ctx.out(&render_status(&status, pid, &open, now_ms, ctx.style));
    if status.profile.is_none() {
        ctx.out(&format!(
            "\nThe profile is not created yet: clave {}init\n",
            ctx.profile.flag()
        ));
    }
    Ok(())
}

pub(crate) fn profile(ctx: &Ctx, cmd: ProfileCmd) -> Result<()> {
    match cmd {
        ProfileCmd::List => {
            let names = profile::list(&ctx.home);
            let rows: Vec<(String, bool)> = names
                .iter()
                .filter_map(|n| Profile::resolve(&ctx.home, n).ok())
                .map(|p| {
                    let running = daemon::info(&p).is_some();
                    (p.name, running)
                })
                .collect();
            let value = json!(rows.iter().map(|(name, running)| json!({ "name": name, "running": running, "current": *name == ctx.profile.name })).collect::<Vec<_>>());
            ctx.emit(&value, || {
                if rows.is_empty() {
                    return "No profiles yet: clave init\n".to_owned();
                }
                rows.iter().fold(String::new(), |mut out, (name, running)| {
                    let mark = if *name == ctx.profile.name { "*" } else { " " };
                    let state = if *running {
                        ctx.style.green("running")
                    } else {
                        ctx.style.dim("stopped")
                    };
                    let _ = writeln!(out, "{mark} {name:<20}{state}");
                    out
                })
            });
            Ok(())
        }
        ProfileCmd::Show => {
            let p = &ctx.profile;
            let info = daemon::info(p);
            let value = json!({
                "name": p.name,
                "dir": p.dir,
                "exists": p.exists(),
                "daemon": info.as_ref().map(|i| json!({ "pid": i.pid, "version": i.version })),
                "touchId": secret::touch_id_enabled(ctx),
            });
            ctx.emit(&value, || {
                let mut out = String::new();
                let _ = writeln!(out, "{:<11}{}", "Profile", p.name);
                let _ = writeln!(out, "{:<11}{}", "Directory", p.dir.display());
                let _ = writeln!(
                    out,
                    "{:<11}{}",
                    "Created",
                    if p.exists() { "yes" } else { "no (clave init)" }
                );
                let daemon = info.as_ref().map_or_else(
                    || "stopped".to_owned(),
                    |i| format!("running (pid {})", i.pid),
                );
                let _ = writeln!(out, "{:<11}{daemon}", "Daemon");
                let _ = writeln!(
                    out,
                    "{:<11}{}",
                    "Touch ID",
                    if secret::touch_id_enabled(ctx) {
                        "enabled"
                    } else {
                        "off"
                    }
                );
                out
            });
            Ok(())
        }
        ProfileCmd::Remove { name } => {
            let target = Profile::resolve(&ctx.home, &name)?;
            if !target.dir.exists() {
                return Err(Error::usage(format!("No profile \"{name}\".")));
            }
            ctx.confirm(&format!(
                "Delete profile \"{name}\" and everything in {}? Its keys and vault records cannot be recovered.",
                target.dir.display()
            ))?;
            if daemon::info(&target).is_some() {
                daemon::stop(&target)?;
            }
            if target.touch_id_marker().exists() {
                secret::disable_touch_id(&target)?;
            }
            std::fs::remove_dir_all(&target.dir)?;
            ctx.emit(&json!({ "removed": name }), || {
                format!("Removed profile {name}\n")
            });
            Ok(())
        }
    }
}

pub(crate) fn completions(shell: clap_complete::Shell) {
    let mut buffer = Vec::new();
    clap_complete::generate(shell, &mut crate::Cli::command(), "clave", &mut buffer);
    write_stdout(&String::from_utf8_lossy(&buffer));
}
