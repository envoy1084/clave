//! `lobby open|inspect|join|start|status|list|leave` and the live founding steps.

use std::{
    collections::HashSet,
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
    render::{
        self, founding, layout,
        spinner::{self, Spinner},
    },
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
                table.render_with(ctx.style)
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
    let spin = ctx.spin("Opening a lobby on your keeper");
    let raw: Value = client.call(
        "lobby.create",
        json!({ "name": name, "threshold": threshold, "viewingThreshold": viewing_threshold }),
    )?;
    let lobby: LobbyView = decode(&raw)?;
    spin.ok(
        &format!("Lobby {} open", ctx.style.bold(name)),
        &format!("spend {threshold} of n · view {viewing_threshold} of n"),
    );
    let invite = lobby.invite.clone().unwrap_or_default();
    ctx.emit(&raw, || {
        let style = ctx.style;
        format!(
            "\n{}  {}\n\n{}\n\n{}  {}\n",
            style.bold("Invite"),
            style.dim("send it to the other members"),
            style.cyan(&invite),
            style.dim("They join with"),
            style.cyan("clave lobby join <invite>")
        )
    });
    if args.no_wait {
        return Ok(());
    }
    wait_for_members(ctx, client, &lobby, args.members)?;
    let spin = ctx.spin("Sending the plan to every member");
    let started: Value = client.call(
        "lobby.start",
        json!({ "lobby": lobby.lobby, "threshold": threshold, "viewingThreshold": viewing_threshold }),
    )?;
    spin.ok("Founding started", "");
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
        .hint("Point the daemon at the same network: clave config set node-url <url>"));
    }
    ctx.confirm(&format!(
        "Join {} hosted by {}?",
        preview.name, preview.host_name
    ))?;
    let spin = ctx.spin(&format!("Joining {}", preview.name));
    let joined: LobbyView = client.call("lobby.join", json!({ "invite": invite }))?;
    spin.ok(
        &format!("Joined {}", ctx.style.bold(&preview.name)),
        &match my_index(&joined) {
            0 => String::new(),
            index => format!("member {index}"),
        },
    );
    wait_until_started(ctx, client, &joined.lobby, &preview.host_name)?;
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
    let style = ctx.style;
    let network = if preview.same_network {
        format!("{}  {}", preview.network, style.green("matches your node"))
    } else {
        style.red(&format!("{}  differs from your node", preview.network))
    };
    let mut out = String::from("\n");
    out.push_str(&layout::title(
        style,
        &format!("Invitation to {}", preview.name),
        &[&format!("from {}", preview.host_name)],
    ));
    out.push_str(&layout::fields(
        style,
        &[
            (
                "Spend",
                format!(
                    "{} approvals needed for every transaction",
                    style.bold(&preview.threshold.to_string())
                ),
            ),
            (
                "View",
                format!(
                    "{} keepers needed to read the vault",
                    style.bold(&preview.viewing_threshold.to_string())
                ),
            ),
            ("Network", network),
            ("Lobby", style.dim(&short_hex(&preview.lobby))),
        ],
    ));
    out.push('\n');
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
    let hint = if enter.is_some() {
        " · Enter starts early"
    } else {
        ""
    };
    let waiting = |count: usize| match target {
        Some(n) => format!("Waiting for members  {count}/{n}{hint}"),
        None => format!("Waiting for members  {count} joined · Enter starts founding"),
    };
    let spin = ctx.spin(&waiting(1));
    let mut seen = HashSet::new();
    loop {
        let raw: Value = client.call("lobby.get", json!({ "lobby": lobby.lobby }))?;
        let view: LobbyView = decode(&raw)?;
        for member in &view.members {
            if seen.insert(member.index) && !member.you {
                if ctx.json {
                    Ctx::json_line(
                        &json!({ "type": "joined", "index": member.index, "name": member.name }),
                    );
                }
                spin.println(&spinner::line(
                    ctx.style,
                    true,
                    &format!("{} joined", ctx.style.bold(&member.name)),
                    &format!("member {}", member.index),
                    "",
                ));
            }
        }
        let count = view.members.len();
        spin.set(&waiting(count));
        if target.is_some_and(|n| count >= n) {
            spin.clear();
            return Ok(());
        }
        if enter.as_ref().is_some_and(|rx| rx.try_recv().is_ok()) {
            if count >= 2 {
                spin.clear();
                return Ok(());
            }
            spin.println(
                &ctx.style
                    .yellow("At least one other member must join first."),
            );
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn wait_until_started(ctx: &Ctx, client: &mut Client, lobby: &str, host: &str) -> Result<()> {
    let spin = ctx.spin(&format!("Waiting for {host} to start the founding"));
    let mut seen = HashSet::new();
    loop {
        let view: LobbyView = client.call("lobby.get", json!({ "lobby": lobby }))?;
        for member in &view.members {
            if seen.insert(member.index) && !member.you {
                spin.println(&format!(
                    "{} {}",
                    ctx.style.dim(&format!("member {}", member.index)),
                    member.name
                ));
            }
        }
        match view.status.as_str() {
            "started" => {
                spin.ok(&format!("{host} started the founding"), "");
                return Ok(());
            }
            "closed" => {
                spin.fail("The host closed the lobby");
                return Err(Error::new(Kind::NotReady, "The host closed the lobby."));
            }
            _ => std::thread::sleep(Duration::from_secs(1)),
        }
    }
}

/// Polls `founding.get` every 500 ms: the active step spins, finished steps print with their duration.
fn follow_founding(ctx: &Ctx, client: &mut Client, lobby: &str, alias: Option<&str>) -> Result<()> {
    let style = ctx.style;
    let mut finished: HashSet<String> = HashSet::new();
    let mut active: Option<(String, Spinner)> = None;
    let started = Instant::now();
    loop {
        let raw: Value = client.call("founding.get", json!({ "lobby": lobby }))?;
        if raw.is_null() {
            if started.elapsed() > Duration::from_secs(30) {
                return Err(Error::failed("Founding did not start.").hint("clave lobby status"));
            }
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let state: FoundingState = decode(&raw)?;
        for step in &state.steps {
            let detail = step.detail.clone().unwrap_or_default();
            match step.status.as_str() {
                "done" | "failed" if !finished.contains(&step.id) => {
                    finished.insert(step.id.clone());
                    if active.as_ref().is_some_and(|(id, _)| *id == step.id)
                        && let Some((_, spin)) = active.take()
                    {
                        spin.clear();
                    }
                    if ctx.json {
                        Ctx::json_line(
                            &json!({ "type": "step", "step": step.id, "status": step.status, "detail": step.detail }),
                        );
                    }
                    let took = match (step.started_at, step.finished_at) {
                        (Some(a), Some(b)) => render::elapsed(b.saturating_sub(a)),
                        _ => String::new(),
                    };
                    ctx.note(&spinner::line(
                        style,
                        step.status == "done",
                        founding::label(&step.id),
                        &short_detail(&detail),
                        &took,
                    ));
                }
                "active" if active.as_ref().is_none_or(|(id, _)| *id != step.id) => {
                    if let Some((_, spin)) = active.take() {
                        spin.clear();
                    }
                    let text = format!(
                        "{}  {}",
                        founding::label(&step.id),
                        style.dim(&short_detail(&detail))
                    );
                    active = Some((step.id.clone(), ctx.spin(&text)));
                }
                "active" => {
                    if let Some((_, spin)) = &active {
                        spin.set(&format!(
                            "{}  {}",
                            founding::label(&step.id),
                            style.dim(&short_detail(&detail))
                        ));
                    }
                }
                _ => {}
            }
        }
        if let Some(error) = &state.error {
            drop(active.take());
            let mut message = format!("Founding failed at {}: {}", error.step, error.message);
            if !error.blamed.is_empty() {
                let _ = write!(message, " (blamed: {})", error.blamed.join(", "));
            }
            return Err(Error::failed(message)
                .hint("Retry: clave lobby start (host) or clave lobby join (guests)"));
        }
        if state.done {
            drop(active.take());
            if let (Some(alias), Some(vault)) = (alias, &state.vault) {
                client.call::<Value>("vault.alias", json!({ "vault": vault, "alias": alias }))?;
            }
            ctx.emit(&json!({ "type": "founded", "vault": state.vault }), || {
                let mut out = String::from("\n");
                out.push_str(&layout::title(style, "Vault founded", &[]));
                let mut rows = vec![(
                    "Address",
                    style.cyan(state.vault.as_deref().unwrap_or_default()),
                )];
                if let Some(alias) = alias {
                    rows.push(("Alias", format!("vaults:{alias}")));
                }
                out.push_str(&layout::fields(style, &rows));
                out
            });
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Step details can be long (addresses, transcripts): keep one short line.
fn short_detail(detail: &str) -> String {
    if detail.len() > 60 && detail.starts_with("0x") {
        short_hex(detail)
    } else if detail.chars().count() > 70 {
        format!("{}…", detail.chars().take(69).collect::<String>())
    } else {
        detail.to_owned()
    }
}
