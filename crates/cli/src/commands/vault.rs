//! `vault list|use|alias|show|members|peers|inspect|audit`.

use std::{fmt::Write as _, time::Duration};

use serde_json::{Value, json};

use crate::{
    VaultCmd,
    context::Ctx,
    error::Result,
    refs::{self, short_hex},
    render::{self, layout, table::Table},
    types::{AuditEntry, MemberReachability, VaultSummary},
};

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or("–")
}

fn number(value: &Value, pointer: &str) -> String {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .map_or_else(|| "–".to_owned(), |n| n.to_string())
}

fn yes_no(value: &Value, pointer: &str) -> &'static str {
    match value.pointer(pointer).and_then(Value::as_bool) {
        Some(true) => "yes",
        Some(false) => "no",
        None => "–",
    }
}

fn vault_table(vaults: &[VaultSummary], style: crate::render::Style) -> String {
    if vaults.is_empty() {
        return "No vaults yet: clave lobby open, or clave lobby join <invite>\n".to_owned();
    }
    let mut table = Table::new(&["", "ALIAS", "NAME", "ADDRESS", "SPEND", "VIEW", "SLOT"]);
    for v in vaults {
        table.row(vec![
            if v.is_default {
                "*".into()
            } else {
                String::new()
            },
            v.alias.clone().unwrap_or_else(|| "–".into()),
            style.bold(&v.name),
            style.cyan(&short_hex(&v.address)),
            format!("{}/{}", v.threshold, v.members),
            format!("{}/{}", v.viewing_threshold, v.members),
            v.my_index.to_string(),
        ]);
    }
    table.render_with(style)
}

pub(crate) fn run(ctx: &Ctx, cmd: VaultCmd) -> Result<()> {
    let mut client = ctx.client()?;
    match cmd {
        VaultCmd::List => {
            let raw: Value = client.call("vault.list", Value::Null)?;
            let vaults: Vec<VaultSummary> = serde_json::from_value(raw.clone()).unwrap_or_default();
            ctx.emit(&raw, || vault_table(&vaults, ctx.style));
        }
        VaultCmd::Use { reference } => {
            let vaults: Vec<VaultSummary> = client.call("vault.list", Value::Null)?;
            let vault = refs::resolve_vault(&vaults, Some(&reference))?;
            let raw: Value = client.call("vault.use", json!({ "vault": vault.address }))?;
            ctx.emit(&raw, || {
                format!(
                    "Default vault: {} ({})\n",
                    vault.name,
                    short_hex(&vault.address)
                )
            });
        }
        VaultCmd::Alias { reference, alias } => {
            let vaults: Vec<VaultSummary> = client.call("vault.list", Value::Null)?;
            let vault = refs::resolve_vault(&vaults, Some(&reference))?;
            let raw: Value = client.call(
                "vault.alias",
                json!({ "vault": vault.address, "alias": alias }),
            )?;
            ctx.emit(&raw, || {
                format!(
                    "vaults:{alias} → {} ({})\n",
                    vault.name,
                    short_hex(&vault.address)
                )
            });
        }
        VaultCmd::Show | VaultCmd::Members | VaultCmd::Inspect => {
            let vault = ctx.vault(&mut client)?;
            let details: Value = client.call("vault.details", json!({ "vault": vault.address }))?;
            ctx.emit(&details, || match cmd {
                VaultCmd::Show => show(&vault, &details, ctx),
                VaultCmd::Members => members(&details, ctx.style),
                _ => inspect(&details),
            });
        }
        VaultCmd::Peers { watch } => {
            let vault = ctx.vault(&mut client)?;
            let mut last = String::new();
            loop {
                let raw: Value = client.call("vault.peers", json!({ "vault": vault.address }))?;
                let peers: Vec<MemberReachability> =
                    serde_json::from_value(raw.clone()).unwrap_or_default();
                let text = peers_table(&peers, &vault, ctx);
                if text != last {
                    ctx.emit(&raw, || text.clone());
                    last = text;
                }
                if !watch {
                    break;
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        }
        VaultCmd::Audit { limit } => {
            let vault = ctx.vault(&mut client)?;
            let raw: Value = client.call(
                "vault.audit",
                json!({ "vault": vault.address, "limit": limit }),
            )?;
            let entries: Vec<AuditEntry> = serde_json::from_value(raw.clone()).unwrap_or_default();
            ctx.emit(&raw, || {
                if entries.is_empty() {
                    return "No ECDH requests yet.\n".to_owned();
                }
                let mut table = Table::new(&["TIME", "REQUESTER", "PURPOSE", "POINTS", "OUTCOME"]);
                for e in &entries {
                    table.row(vec![
                        render::utc(render::secs(e.at)),
                        e.requester_name
                            .clone()
                            .unwrap_or_else(|| short_hex(&e.requester)),
                        e.purpose.clone(),
                        e.points.to_string(),
                        e.outcome.clone(),
                    ]);
                }
                table.render_with(ctx.style)
            });
        }
    }
    Ok(())
}

fn show(vault: &VaultSummary, d: &Value, ctx: &Ctx) -> String {
    let style = ctx.style;
    let mut out = crate::commands::assets::vault_title(ctx, vault);
    out.push('\n');
    let mut rows = vec![("Address", style.cyan(&vault.address))];
    if let Some(alias) = &vault.alias {
        rows.push(("Alias", format!("vaults:{alias}")));
    }
    rows.push((
        "Spend",
        format!(
            "{} of {} members approve every transaction",
            style.bold(&vault.threshold.to_string()),
            vault.members
        ),
    ));
    rows.push((
        "View",
        format!(
            "{} of {} keepers decrypt together",
            style.bold(&vault.viewing_threshold.to_string()),
            vault.members
        ),
    ));
    rows.push(("You", format!("member {}", vault.my_index)));
    let config = if d.get("config").is_some_and(|c| !c.is_null()) {
        let matches = d.pointer("/config/matchesSetup").and_then(Value::as_bool) == Some(true);
        format!(
            "v{}  {}",
            number(d, "/config/version"),
            if matches {
                style.green("on-chain config matches the founding plan")
            } else {
                style.red("on-chain config differs from the founding plan")
            }
        )
    } else {
        style.dim("not read yet")
    };
    rows.push(("Config", config));
    let serving = d.pointer("/committee/serving").and_then(Value::as_bool) == Some(true);
    rows.push((
        "Keeper",
        format!(
            "{}  {}",
            if serving {
                style.green("serving")
            } else {
                style.yellow("not serving")
            },
            style.dim(&format!(
                "committee epoch {}",
                number(d, "/committee/epoch")
            ))
        ),
    ));
    rows.push((
        "Founded",
        style.dim(&render::utc(render::secs(vault.founded_at))),
    ));
    out.push_str(&layout::fields(style, &rows));
    out.push('\n');
    out.push_str(&layout::heading(style, "Members"));
    out.push_str(&members(d, style));
    out
}

fn members(d: &Value, style: crate::render::Style) -> String {
    let list = d
        .pointer("/committee/members")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut table = Table::new(&[
        "#",
        "NAME",
        "SIGNER KEY",
        "VERIFICATION SHARE",
        "KEEPER",
        "PEER",
    ]);
    for m in &list {
        let you = if m.get("you").and_then(Value::as_bool) == Some(true) {
            format!("  {}", style.dim("you"))
        } else {
            String::new()
        };
        table.row(vec![
            number(m, "/index"),
            format!("{}{you}", style.bold(text(m, "/name"))),
            short_hex(text(m, "/signerKey/x")),
            short_hex(text(m, "/verificationShare/x")),
            short_hex(text(m, "/identity")),
            style.dim(text(m, "/peerId")),
        ]);
    }
    table.render_with(style)
}

fn inspect(d: &Value) -> String {
    let mut out = String::from("Contract instance\n");
    for (label, key) in [
        ("Address", "address"),
        ("Preaddress", "preaddress"),
        ("Partial address", "partialAddress"),
        ("Class id", "classId"),
        ("Salt", "salt"),
        ("Deployer", "deployer"),
        ("Init hash", "initializationHash"),
    ] {
        let _ = writeln!(out, "  {label:<20}{}", text(d, &format!("/contract/{key}")));
    }
    out.push_str("\nPublic keys\n");
    if let Some(keys) = d.pointer("/contract/publicKeys").and_then(Value::as_object) {
        for (name, value) in keys {
            let _ = writeln!(out, "  {name:<20}{}", value.as_str().unwrap_or("–"));
        }
    }
    out.push_str("\nOn-chain config\n");
    if d.get("config").is_some_and(|c| !c.is_null()) {
        let _ = writeln!(out, "  {:<20}v{}", "Version", number(d, "/config/version"));
        let _ = writeln!(
            out,
            "  {:<20}{}",
            "Threshold",
            number(d, "/config/threshold")
        );
        let _ = writeln!(
            out,
            "  {:<20}{}",
            "Viewing threshold",
            number(d, "/config/viewingThreshold")
        );
        let _ = writeln!(
            out,
            "  {:<20}{}",
            "Signers",
            number(d, "/config/signerCount")
        );
        let _ = writeln!(
            out,
            "  {:<20}{}",
            "Signer set",
            text(d, "/config/signerSetCommitment")
        );
        let _ = writeln!(
            out,
            "  {:<20}{}",
            "Committee",
            text(d, "/config/committeeCommitment")
        );
        let _ = writeln!(
            out,
            "  {:<20}{}",
            "Matches setup",
            yes_no(d, "/config/matchesSetup")
        );
    } else {
        out.push_str("  not read yet\n");
    }
    out.push_str("\nViewing committee\n");
    let _ = writeln!(out, "  {:<20}{}", "Epoch", number(d, "/committee/epoch"));
    let _ = writeln!(
        out,
        "  {:<20}{}",
        "Group key x",
        text(d, "/committee/groupKey/x")
    );
    let _ = writeln!(
        out,
        "  {:<20}{}",
        "Group key y",
        text(d, "/committee/groupKey/y")
    );
    let _ = writeln!(
        out,
        "  {:<20}{}",
        "Founding session",
        text(d, "/founding/session")
    );
    if let Some(t) = d.pointer("/founding/transcript").and_then(Value::as_str) {
        let _ = writeln!(out, "  {:<20}{t}", "Transcript");
    }
    out
}

fn peers_table(peers: &[MemberReachability], vault: &VaultSummary, ctx: &Ctx) -> String {
    let now = render::now_secs();
    let mut table = Table::new(&["SLOT", "NAME", "STATE", "RTT", "LAST SEEN", "PEER"]);
    for p in peers {
        let state = if p.you {
            ctx.style.green("you")
        } else if p.connected && p.relayed {
            ctx.style.yellow("relayed")
        } else if p.connected {
            ctx.style.green("direct")
        } else {
            ctx.style.red("unreachable")
        };
        table.row(vec![
            p.index.to_string(),
            p.name.clone(),
            state,
            p.rtt_ms.map_or_else(|| "–".into(), |r| format!("{r}ms")),
            p.last_seen
                .map_or_else(|| "–".into(), |t| render::relative(render::secs(t), now)),
            p.peer_id.clone(),
        ]);
    }
    let reachable = peers.iter().filter(|p| p.you || p.connected).count();
    let mut out = table.render_with(ctx.style);
    let enough = reachable >= vault.viewing_threshold as usize;
    let line = format!(
        "\nECDH available {reachable}/{} (t_v {})",
        peers.len(),
        vault.viewing_threshold
    );
    out.push_str(&if enough {
        ctx.style.green(&line)
    } else {
        ctx.style.red(&line)
    });
    out.push('\n');
    if !enough {
        out.push_str(
            "Balances and decryption need t_v keepers online: start another member's daemon.\n",
        );
    }
    out
}
