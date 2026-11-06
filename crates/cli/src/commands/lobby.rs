//! `lobby open|inspect|join|start|status|list|leave` and the live founding steps.

use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    sync::mpsc,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{
    LobbyCmd, LobbyOpen,
    context::Ctx,
    error::{Error, Kind, Result},
    refs::{self, short_hex},
    render::{self, founding},
    rpc::Client,
    types::{FoundingState, InvitePreview, LobbyView},
};

pub(crate) fn run(ctx: &Ctx, cmd: LobbyCmd) -> Result<()> {
    let mut client = ctx.client()?;
    match cmd {
        LobbyCmd::Open(args) => open(ctx, &mut client, &args),
        LobbyCmd::Inspect { invite } => {
            let raw: Value = client.call("lobby.inspect", json!({ "invite": invite.trim() }))?;
            let preview: InvitePreview = decode(&raw)?;
            ctx.emit(&raw, || plan(&preview, ctx));
            Ok(())
        }
        LobbyCmd::Join { invite, alias } => join(ctx, &mut client, &invite, alias.as_deref()),
        LobbyCmd::Start {
            lobby,
            threshold,
            viewing_threshold,
        } => {
            let lobby = pick(&mut client, lobby.as_deref())?;
            let view: LobbyView = client.call("lobby.get", json!({ "lobby": lobby }))?;
            let started: Value = client.call(
                "lobby.start",
                json!({
                    "lobby": lobby,
                    "threshold": threshold.unwrap_or(view.threshold),
                    "viewingThreshold": viewing_threshold.unwrap_or(view.viewing_threshold),
                }),
            )?;
            if ctx.json {
                Ctx::json_line(&started);
            }
            follow_founding(ctx, &mut client, &lobby, None)
        }
        LobbyCmd::Status { lobby, watch } => {
            let lobby = pick(&mut client, lobby.as_deref())?;
            let mut last = String::new();
            loop {
                let raw: Value = client.call("lobby.get", json!({ "lobby": lobby }))?;
                let view: LobbyView = decode(&raw)?;
                let mut text = founding::roster(&view, ctx.style);
                if view.status == "started" {
                    let state: Value = client.call("founding.get", json!({ "lobby": lobby }))?;
                    if let Ok(state) = serde_json::from_value::<FoundingState>(state) {
                        text.push('\n');
                        text.push_str(&founding::steps(
                            &state,
                            render::now_secs() * 1000,
                            ctx.style,
                        ));
                    }
                }
                if text != last {
                    ctx.emit(&raw, || text.clone());
                    last = text;
                }
                if !watch || view.status != "open" {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        LobbyCmd::List => {
            let raw: Value = client.call("lobby.list", Value::Null)?;
            let lobbies: Vec<LobbyView> = decode(&raw)?;
            ctx.emit(&raw, || {
                if lobbies.is_empty() {
                    return "No lobbies.\n".to_owned();
                }
                let mut table = render::table::Table::new(&[
                    "LOBBY", "NAME", "ROLE", "MEMBERS", "K", "T_V", "STATUS",
                ]);
                for l in &lobbies {
                    table.row(vec![
                        refs::short_id(&l.lobby),
                        l.name.clone(),
                        l.role.clone(),
                        l.members.len().to_string(),
                        l.threshold.to_string(),
                        l.viewing_threshold.to_string(),
                        ctx.style.status(&l.status),
                    ]);
                }
                table.render()
            });
            Ok(())
        }
        LobbyCmd::Leave { lobby } => {
            let lobby = pick(&mut client, lobby.as_deref())?;
            ctx.confirm(&format!("Leave lobby {}?", refs::short_id(&lobby)))?;
            client.call::<Value>("lobby.leave", json!({ "lobby": lobby }))?;
            ctx.emit(&json!({ "left": lobby }), || "Left the lobby.\n".to_owned());
            Ok(())
        }
    }
}

fn open(ctx: &Ctx, client: &mut Client, args: &LobbyOpen) -> Result<()> {
    let (name, threshold, viewing_threshold) = (&args.name, args.threshold, args.viewing_threshold);
    if !args.no_wait && args.members.is_none() && !ctx.interactive {
        return Err(Error::usage(
            "Without a terminal, pass --members <n> (or --no-wait).",
        ));
    }
    let raw: Value = client.call(
        "lobby.create",
        json!({ "name": name, "threshold": threshold, "viewingThreshold": viewing_threshold }),
    )?;
    let lobby: LobbyView = decode(&raw)?;
    let invite = lobby.invite.clone().unwrap_or_default();
    ctx.emit(&raw, || {
        format!(
            "Lobby {} open  (spend {threshold}, view {viewing_threshold})\n\nInvite (send it to the other members):\n\n  {invite}\n\nThey join with: tecpan lobby join <invite>\n",
            ctx.style.bold(name)
        )
    });
    if args.no_wait {
        ctx.out(&format!(
            "Start founding later: tecpan {}lobby start\n",
            ctx.profile.flag()
        ));
        return Ok(());
    }
    wait_for_members(ctx, client, &lobby, args.members)?;
    let started: Value = client.call(
        "lobby.start",
        json!({ "lobby": lobby.lobby, "threshold": threshold, "viewingThreshold": viewing_threshold }),
    )?;
    if ctx.json {
        Ctx::json_line(&started);
    }
    follow_founding(ctx, client, &lobby.lobby, args.alias.as_deref())
}

fn join(ctx: &Ctx, client: &mut Client, invite: &str, alias: Option<&str>) -> Result<()> {
    let invite = invite.trim().to_owned();
    let raw: Value = client.call("lobby.inspect", json!({ "invite": invite }))?;
    let preview: InvitePreview = decode(&raw)?;
    if ctx.json {
        Ctx::json_line(&raw);
    } else {
        ctx.out(&plan(&preview, ctx));
    }
    if !preview.same_network {
        return Err(Error::usage(format!(
            "The invite is for network {}, but this daemon uses another node.",
            preview.network
        ))
        .hint("Point the daemon at the same network: tecpan config set node-url <url>"));
    }
    ctx.confirm(&format!(
        "Join {} hosted by {}?",
        preview.name, preview.host_name
    ))?;
    let joined: LobbyView = client.call("lobby.join", json!({ "invite": invite }))?;
    ctx.note(&format!(
        "Joined{}. Waiting for the host to start…",
        match my_index(&joined) {
            0 => String::new(),
            index => format!(" as member {index}"),
        }
    ));
    wait_until_started(ctx, client, &joined.lobby)?;
    client.call::<Value>("founding.begin", json!({ "lobby": joined.lobby }))?;
    follow_founding(ctx, client, &joined.lobby, alias)
}

fn decode<T: serde::de::DeserializeOwned>(raw: &Value) -> Result<T> {
    serde_json::from_value(raw.clone())
        .map_err(|e| Error::failed(format!("unexpected daemon result: {e}")))
}

fn my_index(lobby: &LobbyView) -> u32 {
    lobby.members.iter().find(|m| m.you).map_or(0, |m| m.index)
}

/// The open lobby a prefix (or nothing) names.
fn pick(client: &mut Client, prefix: Option<&str>) -> Result<String> {
    let lobbies: Vec<LobbyView> = client.call("lobby.list", Value::Null)?;
    let ids: Vec<&str> = lobbies
        .iter()
        .filter(|l| prefix.is_some() || l.status == "open")
        .map(|l| l.lobby.as_str())
        .collect();
    refs::resolve_lobby(&ids, prefix).map(str::to_owned)
}

fn plan(preview: &InvitePreview, ctx: &Ctx) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<10}{}", "Vault", ctx.style.bold(&preview.name));
    let _ = writeln!(out, "{:<10}{}", "Host", preview.host_name);
    let _ = writeln!(
        out,
        "{:<10}{} approvals to spend",
        "Spend", preview.threshold
    );
    let _ = writeln!(
        out,
        "{:<10}{} keepers to read (t_v)",
        "View", preview.viewing_threshold
    );
    let network = if preview.same_network {
        format!("{} (matches this daemon)", preview.network)
    } else {
        ctx.style.red(&format!(
            "{} (differs from this daemon's node)",
            preview.network
        ))
    };
    let _ = writeln!(out, "{:<10}{network}", "Network");
    let _ = writeln!(out, "{:<10}{}", "Lobby", short_hex(&preview.lobby));
    out
}

/// Shows members as they join; returns when `target` joined or the host pressed Enter.
fn wait_for_members(
    ctx: &Ctx,
    client: &mut Client,
    lobby: &LobbyView,
    target: Option<usize>,
) -> Result<()> {
    let enter = if ctx.interactive {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            while std::io::stdin().read_line(&mut line).is_ok_and(|n| n > 0) && tx.send(()).is_ok()
            {
                line.clear();
            }
        });
        Some(rx)
    } else {
        None
    };
    let prompt = match target {
        Some(n) => {
            format!("Waiting for {n} members (Enter starts early, Ctrl-C leaves the lobby open)…")
        }
        None => {
            "Waiting for members; press Enter to start founding (Ctrl-C leaves the lobby open)…"
                .to_owned()
        }
    };
    ctx.note(&prompt);
    let mut seen = HashSet::new();
    loop {
        let raw: Value = client.call("lobby.get", json!({ "lobby": lobby.lobby }))?;
        let view: LobbyView = decode(&raw)?;
        for member in &view.members {
            if seen.insert(member.index) {
                let you = if member.you { " (you)" } else { "" };
                if ctx.json {
                    Ctx::json_line(
                        &json!({ "type": "joined", "index": member.index, "name": member.name }),
                    );
                } else {
                    ctx.out(&format!(
                        "  {} {} {}{you}\n",
                        ctx.style.ok(),
                        member.index,
                        member.name
                    ));
                }
            }
        }
        let count = view.members.len();
        if target.is_some_and(|n| count >= n) {
            return Ok(());
        }
        if enter.as_ref().is_some_and(|rx| rx.try_recv().is_ok()) {
            if count >= 2 {
                return Ok(());
            }
            ctx.note("At least one other member must join first.");
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn wait_until_started(ctx: &Ctx, client: &mut Client, lobby: &str) -> Result<()> {
    let mut seen = HashSet::new();
    loop {
        let view: LobbyView = client.call("lobby.get", json!({ "lobby": lobby }))?;
        for member in &view.members {
            if seen.insert(member.index) && !member.you {
                ctx.note(&format!("  member {} {}", member.index, member.name));
            }
        }
        match view.status.as_str() {
            "started" => return Ok(()),
            "closed" => return Err(Error::new(Kind::NotReady, "The host closed the lobby.")),
            _ => std::thread::sleep(Duration::from_secs(1)),
        }
    }
}

/// Polls `founding.get` every 500 ms, printing steps as they finish, until done or failed.
fn follow_founding(ctx: &Ctx, client: &mut Client, lobby: &str, alias: Option<&str>) -> Result<()> {
    let mut printed: HashMap<String, String> = HashMap::new();
    let started = Instant::now();
    loop {
        let raw: Value = client.call("founding.get", json!({ "lobby": lobby }))?;
        if raw.is_null() {
            if started.elapsed() > Duration::from_secs(30) {
                return Err(Error::failed("Founding did not start.").hint("tecpan lobby status"));
            }
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let state: FoundingState = decode(&raw)?;
        let now_ms = render::now_secs() * 1000;
        for step in &state.steps {
            if printed.get(&step.id) == Some(&step.status) || step.status == "pending" {
                continue;
            }
            printed.insert(step.id.clone(), step.status.clone());
            if ctx.json {
                Ctx::json_line(
                    &json!({ "type": "step", "step": step.id, "status": step.status, "detail": step.detail }),
                );
            } else if step.status == "active" {
                ctx.note(&founding::started_line(step, ctx.style));
            } else {
                ctx.out(&format!(
                    "{}\n",
                    founding::step_line(step, now_ms, ctx.style)
                ));
            }
        }
        if let Some(error) = &state.error {
            ctx.out(&founding::outcome(&state, ctx.style));
            let mut message = format!("Founding failed at {}: {}", error.step, error.message);
            if !error.blamed.is_empty() {
                let _ = write!(message, " (blamed: {})", error.blamed.join(", "));
            }
            return Err(Error::failed(message)
                .hint("Retry: tecpan lobby start (host) or tecpan lobby join (guests)"));
        }
        if state.done {
            if let (Some(alias), Some(vault)) = (alias, &state.vault) {
                client.call::<Value>("vault.alias", json!({ "vault": vault, "alias": alias }))?;
            }
            ctx.emit(&json!({ "type": "founded", "vault": state.vault }), || {
                founding::outcome(&state, ctx.style)
            });
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
