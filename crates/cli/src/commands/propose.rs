//! `propose transfer|call|batch` and `simulate …` (`proposals.preview` / `proposals.create`).

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::{
    ProposeCmd, ProposeOpts,
    context::Ctx,
    error::{Error, Result},
    refs::{self, short_hex, short_id},
    render::proposals::describe,
    rpc::Client,
    types::{
        Contact, ContractFunction, ProposalPreview, ProposalView, TokenBalance, TokenInfo,
        VaultSummary,
    },
    units::{group_thousands, parse_amount, parse_expiry_hours},
};

/// At most this many calls in one proposal.
const MAX_CALLS: usize = 8;

fn tracked_tokens(client: &mut Client, vault: &VaultSummary) -> Result<Vec<TokenInfo>> {
    let balances: Vec<TokenBalance> =
        client.call("tokens.list", json!({ "vault": vault.address }))?;
    Ok(balances.into_iter().map(|b| b.token).collect())
}

/// Vault members as (name, account address), where the daemon reports an account.
fn member_accounts(client: &mut Client, vault: &VaultSummary) -> Vec<(String, String)> {
    let Ok(details) = client.call::<Value>("vault.details", json!({ "vault": vault.address }))
    else {
        return Vec::new();
    };
    details
        .pointer("/committee/members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let name = m.get("name")?.as_str()?;
            let account = m.get("account")?.as_str()?;
            Some((name.to_owned(), account.to_owned()))
        })
        .collect()
}

/// A contract address, or a tracked token's.
fn contract_address(client: &mut Client, vault: &VaultSummary, reference: &str) -> Result<String> {
    if refs::is_address(reference) {
        return Ok(reference.to_owned());
    }
    let tokens = tracked_tokens(client, vault)?;
    refs::resolve_token(&tokens, reference)
        .map(|t| t.address.clone())
        .map_err(|e| e.hint("Use the contract's 0x address, or a tracked token symbol"))
}

/// Reads a batch file: `[{ "to", "fn", "args" }]` or `{ "calls": [...], "memo"? }`.
fn read_batch(
    client: &mut Client,
    vault: &VaultSummary,
    path: &std::path::Path,
) -> Result<(Vec<Value>, Option<String>)> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Error::usage(format!("{}: {e}", path.display())))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| Error::usage(format!("{}: {e}", path.display())))?;
    let (calls, memo) = match &value {
        Value::Array(calls) => (calls.clone(), None),
        Value::Object(o) => (
            o.get("calls")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            o.get("memo").and_then(Value::as_str).map(str::to_owned),
        ),
        _ => (Vec::new(), None),
    };
    if calls.is_empty() || calls.len() > MAX_CALLS {
        return Err(Error::usage(format!(
            "{}: expected 1 to {MAX_CALLS} calls",
            path.display()
        )));
    }
    let mut out = Vec::with_capacity(calls.len());
    for (i, call) in calls.iter().enumerate() {
        let field = |key: &str| call.get(key).and_then(Value::as_str).map(str::to_owned);
        let (Some(to), Some(function)) = (field("to"), field("fn")) else {
            return Err(Error::usage(format!(
                "{}: call {i} needs \"to\" and \"fn\"",
                path.display()
            )));
        };
        let args: Vec<String> = match call.get("args") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(args)) => args
                .iter()
                .map(|a| a.as_str().map_or_else(|| a.to_string(), str::to_owned))
                .collect(),
            Some(_) => {
                return Err(Error::usage(format!(
                    "{}: call {i}: \"args\" must be an array",
                    path.display()
                )));
            }
        };
        let to = contract_address(client, vault, &to)?;
        out.push(json!({ "to": to, "fn": function, "args": args }));
    }
    Ok((out, memo))
}

/// Checks `function` exists with `arity` parameters, when the daemon knows the contract's artifact.
fn check_function(client: &mut Client, contract: &str, function: &str, arity: usize) -> Result<()> {
    let Ok(functions) = client
        .call::<Vec<ContractFunction>>("contracts.functions", json!({ "contract": contract }))
    else {
        // Unknown artifact: the daemon refuses the proposal itself, with its own reason.
        return Ok(());
    };
    let Some(found) = functions.iter().find(|f| f.name == function) else {
        let mut names: Vec<&str> = functions
            .iter()
            .filter(|f| f.kind != "utility")
            .map(|f| f.name.as_str())
            .collect();
        names.sort_unstable();
        return Err(Error::usage(format!(
            "{} has no function \"{function}\".",
            short_hex(contract)
        ))
        .hint(format!("Functions: {}", names.join(", "))));
    };
    if found.params.len() != arity {
        let params: Vec<String> = found
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect();
        return Err(Error::usage(format!(
            "{function} takes {} argument{}, {arity} given: {function}({})",
            found.params.len(),
            if found.params.len() == 1 { "" } else { "s" },
            params.join(", ")
        )));
    }
    Ok(())
}

fn with_memo(mut intent: Value, memo: Option<String>) -> Value {
    if let Some(memo) = memo.filter(|m| !m.trim().is_empty()) {
        intent["memo"] = json!(memo);
    }
    intent
}

pub(crate) fn run(ctx: &Ctx, cmd: ProposeCmd, simulate: bool) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let (intent, opts) = match cmd {
        ProposeCmd::Transfer {
            amount,
            token,
            to,
            opts,
        } => {
            let tokens = tracked_tokens(&mut client, &vault)?;
            let token = refs::resolve_token(&tokens, &token)?.clone();
            let units = parse_amount(&amount, token.decimals)?;
            if units == 0 {
                return Err(Error::usage("The amount must be more than zero."));
            }
            let contacts: Vec<Contact> = client.call("contacts.list", Value::Null)?;
            let members = member_accounts(&mut client, &vault);
            let to = refs::resolve_recipient(&to, &contacts, &members)?;
            let intent = json!({ "kind": "transfer", "token": token.address, "to": to, "amount": units.to_string() });
            (with_memo(intent, opts.memo.clone()), opts)
        }
        ProposeCmd::Call {
            contract,
            function,
            args,
            opts,
        } => {
            let to = contract_address(&mut client, &vault, &contract)?;
            check_function(&mut client, &to, &function, args.len())?;
            let intent =
                json!({ "kind": "calls", "calls": [{ "to": to, "fn": function, "args": args }] });
            (with_memo(intent, opts.memo.clone()), opts)
        }
        ProposeCmd::Batch { file, opts } => {
            let (calls, memo) = read_batch(&mut client, &vault, &file)?;
            let intent = json!({ "kind": "calls", "calls": calls });
            (with_memo(intent, opts.memo.clone().or(memo)), opts)
        }
    };
    submit(
        ctx,
        &mut client,
        &vault,
        &intent,
        &opts,
        simulate || opts.dry_run,
    )
}

fn submit(
    ctx: &Ctx,
    client: &mut Client,
    vault: &VaultSummary,
    intent: &Value,
    opts: &ProposeOpts,
    dry_run: bool,
) -> Result<()> {
    let hours = parse_expiry_hours(&opts.expires)?;
    if !(1..=720).contains(&hours) {
        return Err(Error::usage("--expires must be between 1h and 30d."));
    }
    let params = json!({ "vault": vault.address, "intent": intent, "expiresInHours": hours });
    if dry_run {
        ctx.note("Simulating with the stub account…");
        let raw: Value = client.call("proposals.preview", params)?;
        let preview: ProposalPreview = serde_json::from_value(raw.clone()).unwrap_or_default();
        ctx.emit(&raw, || render_preview(&preview, ctx));
        if !preview.ok {
            return Err(Error::failed(format!(
                "Simulation failed: {}",
                preview.error.as_deref().unwrap_or("unknown error")
            )));
        }
        return Ok(());
    }
    ctx.note("Building the transaction and storing the proposal…");
    let raw: Value = client.call("proposals.create", params)?;
    let proposal: ProposalView = serde_json::from_value(raw.clone()).unwrap_or_default();
    let contacts: Vec<Contact> = client
        .call("contacts.list", Value::Null)
        .unwrap_or_default();
    ctx.emit(&raw, || {
        let id = short_id(&proposal.id);
        format!(
            "{} Proposed {id}: {}\n  Approvals {}/{} · expires in {} · replicated to the members' keepers\n\nNext: tecpan approve {id}\n",
            ctx.style.ok(),
            describe(&proposal, &contacts),
            proposal.valid_approvals(),
            proposal.threshold,
            if hours.is_multiple_of(24) { format!("{}d", hours / 24) } else { format!("{hours}h") }
        )
    });
    Ok(())
}

fn render_preview(preview: &ProposalPreview, ctx: &Ctx) -> String {
    let mut out = String::new();
    let mark = if preview.ok {
        ctx.style.ok()
    } else {
        ctx.style.fail()
    };
    let _ = writeln!(
        out,
        "{mark} Simulation {}",
        if preview.ok { "ok" } else { "failed" }
    );
    for (i, call) in preview.calls.iter().enumerate() {
        let to = call.to_label.clone().unwrap_or_else(|| short_hex(&call.to));
        let _ = writeln!(
            out,
            "  {i}  {to}.{}({})",
            call.function,
            call.args.join(", ")
        );
    }
    if let Some(gas) = &preview.gas {
        let _ = writeln!(
            out,
            "  gas l2 {} · da {}",
            group_thousands(&gas.l2.to_string()),
            group_thousands(&gas.da.to_string())
        );
    }
    if let Some(error) = &preview.error {
        let _ = writeln!(out, "  {}", ctx.style.red(error));
    }
    if preview.ok {
        out.push_str("\nNothing was posted. Propose it without --dry-run.\n");
    }
    out
}
