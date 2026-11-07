//! `proposals`, `proposal show`, `approve`, `reject`, `execute`, `cancel`, `watch`, `activity`, `tx`.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::{
    context::Ctx,
    error::{Error, Result},
    refs::{self, short_hex, short_id},
    render::{
        self, layout,
        proposals::{OPEN, describe, show as render_show, table, vault_header},
        review::{Verb, question, screen},
        spinner::{self, Spinner},
        table::Table,
    },
    rpc::Client,
    secret,
    types::{Contact, NodeStatus, ProposalView, Review, VaultSummary},
    units::format_amount,
};

const STATUSES: [&str; 7] = [
    "collecting",
    "ready",
    "blocked",
    "executed",
    "cancelled",
    "expired",
    "stale",
];

fn contacts(client: &mut Client) -> Vec<Contact> {
    client
        .call("contacts.list", Value::Null)
        .unwrap_or_default()
}

/// The vault and the full proposal id a prefix names.
fn resolve(ctx: &Ctx, client: &mut Client, prefix: &str) -> Result<(VaultSummary, String)> {
    let vault = ctx.vault(client)?;
    let proposals: Vec<ProposalView> =
        client.call("proposals.list", json!({ "vault": vault.address }))?;
    let ids: Vec<&str> = proposals.iter().map(|p| p.id.as_str()).collect();
    let id = refs::resolve_proposal(&ids, prefix)?.to_owned();
    Ok((vault, id))
}

pub(crate) fn list(ctx: &Ctx, status: &str, limit: usize) -> Result<()> {
    if status != "open" && status != "all" && !STATUSES.contains(&status) {
        return Err(Error::usage(format!(
            "Unknown status \"{status}\"; use open, all or one of {}",
            STATUSES.join(", ")
        )));
    }
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let raw: Value = client.call("proposals.list", json!({ "vault": vault.address }))?;
    let all: Vec<ProposalView> =
        serde_json::from_value(raw.clone()).map_err(|e| Error::failed(e.to_string()))?;
    let keep = |p: &ProposalView| match status {
        "all" => true,
        "open" => OPEN.contains(&p.status.as_str()),
        word => p.status == word,
    };
    if ctx.json {
        let items: Vec<Value> = raw
            .as_array()
            .into_iter()
            .flatten()
            .zip(&all)
            .filter(|(_, p)| keep(p))
            .take(limit)
            .map(|(v, _)| v.clone())
            .collect();
        Ctx::json_line(&json!(items));
        return Ok(());
    }
    let shown: Vec<&ProposalView> = all.iter().filter(|p| keep(p)).take(limit).collect();
    let config = all.first().map(|p| p.current_config_version);
    let block = client
        .call::<NodeStatus>("node.status", Value::Null)
        .ok()
        .and_then(|n| n.block_number);
    let contacts = contacts(&mut client);
    let header = vault_header(&vault, config, block, ctx.style);
    ctx.out(&table(
        &header,
        &shown,
        &contacts,
        render::now_secs(),
        ctx.style,
    ));
    Ok(())
}

pub(crate) fn show(ctx: &Ctx, prefix: &str, raw_output: bool) -> Result<()> {
    let mut client = ctx.client()?;
    let (vault, id) = resolve(ctx, &mut client, prefix)?;
    let raw: Value = client.call("proposals.get", json!({ "vault": vault.address, "id": id }))?;
    if ctx.json || raw_output {
        let text = serde_json::to_string_pretty(&raw).unwrap_or_default();
        if ctx.json {
            Ctx::json_line(&raw);
        } else {
            ctx.out(&format!("{text}\n"));
        }
        return Ok(());
    }
    let proposal: ProposalView =
        serde_json::from_value(raw).map_err(|e| Error::failed(e.to_string()))?;
    let contacts = contacts(&mut client);
    ctx.out(&render_show(
        &proposal,
        &contacts,
        render::now_secs(),
        ctx.style,
    ));
    Ok(())
}

/// What `approve` / `reject` sign.
pub(crate) enum Vote {
    Approve,
    Reject(Option<String>),
}

/// Review → clear-signing screen → refuse on failed checks → confirm → authenticate → sign.
pub(crate) fn vote(ctx: &Ctx, prefix: &str, vote: &Vote) -> Result<()> {
    let mut client = ctx.client()?;
    let (vault, id) = resolve(ctx, &mut client, prefix)?;
    let verb = match vote {
        Vote::Approve => Verb::Approve,
        Vote::Reject(_) => Verb::Reject,
    };
    let spin = ctx.spin("Rebuilding the transaction on this device and simulating it");
    let raw: Value = client.call(
        "proposals.review",
        json!({ "vault": vault.address, "id": id }),
    )?;
    let review: Review =
        serde_json::from_value(raw.clone()).map_err(|e| Error::failed(e.to_string()))?;
    if review.checks.passed() {
        spin.ok(
            "Rebuilt and checked",
            "nothing is taken from the proposer's description",
        );
    } else {
        spin.fail("Rebuilt, and the checks failed");
    }
    if ctx.json {
        Ctx::json_line(&json!({ "review": raw }));
    } else {
        ctx.out("\n");
        ctx.out(&screen(&review, verb, ctx.style));
        ctx.out("\n");
    }
    if !review.checks.passed() {
        let reason = review.checks.failure.clone().unwrap_or_else(|| {
            let failed: Vec<&str> = [
                (
                    review.checks.rebuilt,
                    "the rebuilt transaction does not match the proposal",
                ),
                (
                    review.checks.config_current,
                    "the vault's config changed since it was proposed",
                ),
                (review.checks.not_expired, "the proposal has expired"),
            ]
            .into_iter()
            .filter(|(ok, _)| !ok)
            .map(|(_, why)| why)
            .collect();
            failed.join("; ")
        });
        return Err(Error::failed(format!("Refusing to sign: {reason}."))
            .hint("Nothing was signed. Propose it again if it is still wanted."));
    }
    ctx.confirm(&question(&review, verb))?;
    // `--yes` answered the question above; authentication below is never skipped.
    let secret = secret::signing_secret(ctx, &mut client)?;
    let mut params = json!({ "vault": vault.address, "id": id });
    if let Some(secret) = &secret {
        params["secret"] = secret.to_param();
    }
    let method = match vote {
        Vote::Approve => "proposals.approve",
        Vote::Reject(reason) => {
            if let Some(reason) = reason {
                params["reason"] = json!(reason);
            }
            "proposals.reject"
        }
    };
    let spin = ctx.spin(match vote {
        Vote::Approve => "Signing the approval and replicating it to the members' keepers",
        Vote::Reject(_) => "Signing the rejection and replicating it to the members' keepers",
    });
    let result: Value = client.call(method, params)?;
    drop(secret);
    let proposal: ProposalView = serde_json::from_value(result.clone()).unwrap_or_default();
    let short = short_id(&proposal.id);
    let (have, k) = (proposal.valid_approvals(), proposal.threshold);
    let state = match proposal.status.as_str() {
        "ready" => ctx.style.green("ready to execute"),
        "blocked" => ctx.style.red("blocked by rejections"),
        other => ctx.style.status(other),
    };
    spin.ok(
        if matches!(vote, Vote::Approve) {
            "Approved"
        } else {
            "Rejected"
        },
        &format!("{have} of {k} approvals"),
    );
    ctx.emit(&result, || {
        let next = match proposal.status.as_str() {
            "ready" => format!("clave execute {short}"),
            "blocked" => format!("clave cancel {short}"),
            _ => "clave proposals".to_owned(),
        };
        let mut out = String::from("\n");
        out.push_str(&layout::fields(
            ctx.style,
            &[
                ("Proposal", format!("{}  {state}", ctx.style.bold(&short))),
                ("Approvals", format!("{have} of {k}")),
            ],
        ));
        out.push_str(&layout::next(ctx.style, &[&next]));
        out
    });
    Ok(())
}

fn phase_label(phase: &str) -> &str {
    match phase {
        "rebuilding" => "Rebuilding the transaction and verifying every approval",
        "proving" => "Proving the transaction on this device",
        "sending" => "Sending to the network",
        other => other,
    }
}

fn phase_done(phase: &str) -> &str {
    match phase {
        "rebuilding" => "Rebuilt, approvals verified",
        "proving" => "Proved",
        "sending" => "Sent",
        other => other,
    }
}

pub(crate) fn execute(ctx: &Ctx, prefix: &str) -> Result<()> {
    let mut client = ctx.client()?;
    let (vault, id) = resolve(ctx, &mut client, prefix)?;
    client.subscribe()?;
    let style = ctx.style;
    let mut current: Option<(String, Spinner)> = Some((
        "start".to_owned(),
        ctx.spin("Handing the proposal to the daemon"),
    ));
    let result: Result<Value> = client.call_with(
        "proposals.execute",
        json!({ "vault": vault.address, "id": id }),
        |event| {
            if event.get("type").and_then(Value::as_str) != Some("execution") {
                return;
            }
            let execution = &event["execution"];
            if execution
                .get("proposal")
                .and_then(Value::as_str)
                .is_some_and(|p| !p.eq_ignore_ascii_case(&id))
            {
                return;
            }
            if ctx.json {
                Ctx::json_line(execution);
                return;
            }
            let phase = execution
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if current.as_ref().is_some_and(|(p, _)| *p == phase) {
                return;
            }
            if let Some((previous, spin)) = current.take() {
                match previous.as_str() {
                    "start" => spin.clear(),
                    _ => spin.ok(phase_done(&previous), ""),
                }
            }
            match phase.as_str() {
                "failed" => {}
                "mined" => {
                    let tx_hash = execution
                        .get("txHash")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    ctx.note(&spinner::line(
                        style,
                        true,
                        "Mined",
                        &short_hex(tx_hash),
                        "",
                    ));
                }
                _ => current = Some((phase.clone(), ctx.spin(phase_label(&phase)))),
            }
        },
    );
    if let Some((_, spin)) = current.take() {
        spin.clear();
    }
    let raw = result?;
    let proposal: ProposalView = serde_json::from_value(raw.clone()).unwrap_or_default();
    ctx.emit(&raw, || {
        let mut out = String::from("\n");
        match &proposal.execution {
            Some(execution) => {
                out.push_str(&layout::title(
                    style,
                    "Executed",
                    &[
                        &short_id(&proposal.id),
                        "receipt replicated to every member",
                    ],
                ));
                out.push_str(&layout::fields(
                    style,
                    &[
                        ("Action", style.bold(&describe(&proposal, &[]))),
                        ("Tx", style.cyan(&execution.tx_hash)),
                    ],
                ));
                out.push_str(&layout::next(style, &["clave balance", "clave activity"]));
            }
            None => {
                let _ = writeln!(
                    out,
                    "  Proposal {} is {}",
                    short_id(&proposal.id),
                    style.status(&proposal.status)
                );
            }
        }
        out
    });
    Ok(())
}

pub(crate) fn cancel(ctx: &Ctx, prefix: &str) -> Result<()> {
    let mut client = ctx.client()?;
    let (vault, id) = resolve(ctx, &mut client, prefix)?;
    ctx.confirm(&format!(
        "Propose cancelling {} (a nonce burn that needs {} approvals itself)?",
        short_id(&id),
        vault.threshold
    ))?;
    let spin = ctx.spin("Building the nonce burn and storing it as a proposal");
    let raw: Value = client.call(
        "proposals.cancel",
        json!({ "vault": vault.address, "id": id }),
    )?;
    let proposal: ProposalView = serde_json::from_value(raw.clone()).unwrap_or_default();
    let short = short_id(&proposal.id);
    spin.ok(
        &format!("Cancel proposal {} created", ctx.style.bold(&short)),
        &format!("burns the nonce of {}", short_id(&id)),
    );
    ctx.emit(&raw, || {
        let mut out = String::from("\n");
        out.push_str(&layout::fields(
            ctx.style,
            &[("Approvals", format!("0 of {}", proposal.threshold))],
        ));
        out.push_str(&layout::next(
            ctx.style,
            &[&format!("clave approve {short}")],
        ));
        out
    });
    Ok(())
}

/// One human line for an event.
pub(crate) fn event_line(event: &Value) -> String {
    let text = |p: &str| event.pointer(p).and_then(Value::as_str).unwrap_or_default();
    match text("/type") {
        "vault-activity" => format!("vault     activity in {}", short_hex(text("/vault"))),
        "lobby" => format!(
            "lobby     {} {} ({} members)",
            text("/lobby/name"),
            text("/lobby/status"),
            event
                .pointer("/lobby/members")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        ),
        "founding" => {
            let steps = event
                .pointer("/founding/steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let active = steps
                .iter()
                .find(|s| s.get("status").and_then(Value::as_str) == Some("active"))
                .and_then(|s| s.get("id").and_then(Value::as_str))
                .unwrap_or(if event.pointer("/founding/done") == Some(&json!(true)) {
                    "done"
                } else {
                    "waiting"
                });
            format!("founding  {} {active}", short_hex(text("/founding/lobby")))
        }
        "node" => {
            let reachable = event.pointer("/node/reachable") == Some(&json!(true));
            let block = event.pointer("/node/blockNumber").and_then(Value::as_u64);
            format!(
                "node      {} {}{}",
                text("/node/url"),
                if reachable {
                    "reachable"
                } else {
                    "unreachable"
                },
                block.map(|b| format!(" block {b}")).unwrap_or_default()
            )
        }
        "execution" => {
            let mut line = format!(
                "execution {} {}",
                short_id(text("/execution/proposal")),
                text("/execution/phase")
            );
            for key in ["/execution/txHash", "/execution/error"] {
                if !text(key).is_empty() {
                    let _ = write!(line, " {}", text(key));
                }
            }
            line
        }
        "keeper" => format!("keeper    {}", text("/keeper/status")),
        other => format!("{other:<9} {event}"),
    }
}

pub(crate) fn watch(ctx: &Ctx) -> Result<()> {
    let mut client = ctx.client()?;
    client.subscribe()?;
    ctx.note("Watching events (Ctrl-C to stop)…");
    loop {
        let Some(event) = client.next_event(None)? else {
            continue;
        };
        if ctx.json {
            Ctx::json_line(&event);
        } else {
            let time = render::utc(render::now_secs());
            ctx.out(&format!(
                "{}  {}\n",
                ctx.style.dim(&time[11..16]),
                event_line(&event)
            ));
        }
    }
}

pub(crate) fn activity(ctx: &Ctx, limit: usize) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let raw: Value = client.call("vault.activity", json!({ "vault": vault.address }))?;
    let entries: Vec<Value> = raw
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(limit)
        .collect();
    if ctx.json {
        Ctx::json_line(&json!(entries));
        return Ok(());
    }
    if entries.is_empty() {
        ctx.out("No activity yet.\n");
        return Ok(());
    }
    let contacts = contacts(&mut client);
    let mut table = Table::new(&["TIME", "KIND", "DETAIL", "TX"]);
    for entry in &entries {
        let at = entry.get("at").and_then(Value::as_u64).unwrap_or(0);
        let (kind, detail, tx) = if entry.get("kind").and_then(Value::as_str) == Some("received") {
            let decimals = entry
                .pointer("/token/decimals")
                .and_then(Value::as_u64)
                .and_then(|d| u32::try_from(d).ok())
                .unwrap_or(0);
            let amount = format_amount(
                entry.get("amount").and_then(Value::as_str).unwrap_or("0"),
                decimals,
            );
            let symbol = entry
                .pointer("/token/symbol")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let from = entry
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (
                "received".to_owned(),
                format!(
                    "{amount} {symbol} from {}",
                    render::proposals::recipient(from, &contacts)
                ),
                entry
                    .get("txHash")
                    .and_then(Value::as_str)
                    .map(short_hex)
                    .unwrap_or_default(),
            )
        } else {
            let proposal: ProposalView = entry
                .get("proposal")
                .cloned()
                .and_then(|p| serde_json::from_value(p).ok())
                .unwrap_or_default();
            (
                proposal.status.clone(),
                format!(
                    "{}  {}",
                    short_id(&proposal.id),
                    describe(&proposal, &contacts)
                ),
                proposal
                    .execution
                    .as_ref()
                    .map(|e| short_hex(&e.tx_hash))
                    .unwrap_or_default(),
            )
        };
        let when = match entry.get("block").and_then(Value::as_u64) {
            Some(block) if kind == "received" => format!("block {block}"),
            _ => render::utc(render::secs(at)),
        };
        table.row(vec![when, kind, detail, tx]);
    }
    ctx.out(&table.render_with(ctx.style));
    Ok(())
}

/// The daemon has no transaction lookup yet; this finds the hash among this vault's executions and activity.
pub(crate) fn tx(ctx: &Ctx, hash: &str) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let proposals: Vec<ProposalView> =
        client.call("proposals.list", json!({ "vault": vault.address }))?;
    let found = proposals.iter().find(|p| {
        p.execution
            .as_ref()
            .is_some_and(|e| e.tx_hash.eq_ignore_ascii_case(hash))
    });
    match found {
        Some(p) => {
            let execution = p
                .execution
                .as_ref()
                .map(|e| (e.tx_hash.clone(), e.at))
                .unwrap_or_default();
            let value = json!({ "txHash": execution.0, "proposal": p.id, "status": "mined", "at": execution.1 });
            let contacts = contacts(&mut client);
            ctx.emit(&value, || {
                format!(
                    "{}  mined · executed proposal {} ({}) at {}\n",
                    execution.0,
                    short_id(&p.id),
                    describe(p, &contacts),
                    render::utc(render::secs(execution.1))
                )
            });
            Ok(())
        }
        None => Err(Error::failed(format!(
            "Transaction lookup is not supported yet: {} is not an execution of this vault.",
            short_hex(hash)
        ))
        .hint("Check it with the node, e.g. aztec get-tx <hash>")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_lines() {
        let execution = json!({ "type": "execution", "execution": { "vault": "0x01", "proposal": "0x7f3a1c99", "phase": "mined", "txHash": "0xabc" } });
        assert_eq!(event_line(&execution), "execution 7f3a1c mined 0xabc");
        let keeper = json!({ "type": "keeper", "keeper": { "status": "running" } });
        assert_eq!(event_line(&keeper), "keeper    running");
        let node = json!({ "type": "node", "node": { "url": "http://n", "reachable": true, "blockNumber": 7 } });
        assert_eq!(event_line(&node), "node      http://n reachable block 7");
    }
}
