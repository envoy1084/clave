//! `balance`, `token add|list|deploy-test`, `receive`.

use std::fmt::Write as _;

use qrcode::{QrCode, render::unicode::Dense1x2};
use serde_json::{Value, json};

use crate::{
    TokenCmd,
    context::Ctx,
    error::{Error, Result},
    refs::{self, short_hex},
    render::{layout, table::Table},
    types::{TokenBalance, TokenInfo, VaultSummary},
    units::format_amount,
};

pub(crate) fn balance(ctx: &Ctx, token: Option<&str>) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let spin = ctx.spin(&format!(
        "Decrypting the vault's notes with {} of {} keepers",
        vault.viewing_threshold, vault.members
    ));
    let raw: Value = client.call("tokens.list", json!({ "vault": vault.address }))?;
    spin.clear();
    let mut balances: Vec<TokenBalance> = serde_json::from_value(raw.clone()).unwrap_or_default();
    let mut value = raw;
    if let Some(reference) = token {
        let tokens: Vec<TokenInfo> = balances.iter().map(|b| b.token.clone()).collect();
        let wanted = refs::resolve_token(&tokens, reference)?.address.clone();
        balances.retain(|b| b.token.address == wanted);
        value = json!(value.as_array().map(|a| {
            a.iter()
                .filter(|b| b.pointer("/token/address").and_then(Value::as_str) == Some(&wanted))
                .cloned()
                .collect::<Vec<_>>()
        }));
    }
    ctx.emit(&value, || {
        let style = ctx.style;
        let mut out = vault_title(ctx, &vault);
        if balances.is_empty() {
            let _ = write!(out, "\n  {}\n", style.dim("No tracked tokens yet."));
            out.push_str(&layout::next(
                style,
                &["clave token add <address>", "clave token deploy-test"],
            ));
            return out;
        }
        let mut table = Table::new(&["TOKEN", "BALANCE", "", ""]);
        for b in &balances {
            let amount = match (&b.balance, &b.error) {
                (Some(units), _) => style.bold(&format_amount(units, b.token.decimals)),
                (None, Some(error)) => style.red(error),
                (None, None) => "–".to_owned(),
            };
            table.row(vec![
                style.bold(&b.token.symbol),
                amount,
                style.dim(&b.token.name),
                style.dim(&short_hex(&b.token.address)),
            ]);
        }
        out.push('\n');
        out.push_str(&table.render_with(style));
        out
    });
    Ok(())
}

/// `Treasury  · 0x2204…3fc0 · 2-of-2 · view 2`.
pub(crate) fn vault_title(ctx: &Ctx, vault: &VaultSummary) -> String {
    layout::title(
        ctx.style,
        &vault.name,
        &[
            &short_hex(&vault.address),
            &format!("{}-of-{}", vault.threshold, vault.members),
            &format!("view {}", vault.viewing_threshold),
        ],
    )
}

fn token_fields(ctx: &Ctx, token: &TokenInfo) -> String {
    layout::fields(
        ctx.style,
        &[
            (
                "Token",
                format!("{}  {}", ctx.style.bold(&token.symbol), token.name),
            ),
            ("Address", ctx.style.cyan(&token.address)),
            ("Decimals", token.decimals.to_string()),
        ],
    )
}

pub(crate) fn token(ctx: &Ctx, cmd: TokenCmd) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    match cmd {
        TokenCmd::Add { address } => {
            if !refs::is_address(&address) {
                return Err(Error::usage(format!(
                    "\"{address}\" is not an address (0x and 64 hex digits)"
                )));
            }
            let spin = ctx.spin("Registering the token contract in your PXE");
            let raw: Value = client.call(
                "tokens.add",
                json!({ "vault": vault.address, "token": address }),
            )?;
            let token: TokenInfo = serde_json::from_value(raw.clone()).unwrap_or_default();
            spin.ok(
                &format!("Tracking {}", ctx.style.bold(&token.symbol)),
                &token.name,
            );
            ctx.emit(&raw, String::new);
        }
        TokenCmd::List => {
            let raw: Value = client.call("tokens.list", json!({ "vault": vault.address }))?;
            let balances: Vec<TokenBalance> =
                serde_json::from_value(raw.clone()).unwrap_or_default();
            let tokens: Vec<&TokenInfo> = balances.iter().map(|b| &b.token).collect();
            ctx.emit(
                &json!(
                    raw.as_array()
                        .map(|a| a.iter().map(|b| b["token"].clone()).collect::<Vec<_>>())
                ),
                || {
                    if tokens.is_empty() {
                        return format!("  {}\n", ctx.style.dim("No tracked tokens."));
                    }
                    let mut table = Table::new(&["SYMBOL", "NAME", "DECIMALS", "ADDRESS"]);
                    for t in tokens {
                        table.row(vec![
                            t.symbol.clone(),
                            t.name.clone(),
                            t.decimals.to_string(),
                            t.address.clone(),
                        ]);
                    }
                    table.render_with(ctx.style)
                },
            );
        }
        TokenCmd::DeployTest => {
            let spin = ctx.spin("Deploying a test token and minting 10,000 privately to the vault");
            let raw: Value = client.call("tokens.deployTest", json!({ "vault": vault.address }))?;
            let token: TokenInfo = serde_json::from_value(raw.clone()).unwrap_or_default();
            spin.ok(
                &format!(
                    "Deployed {} and minted to the vault",
                    ctx.style.bold(&token.symbol)
                ),
                "local network",
            );
            ctx.emit(&raw, || format!("\n{}", token_fields(ctx, &token)));
        }
    }
    Ok(())
}

/// The QR code as text, two modules per character row.
pub(crate) fn qr(text: &str) -> Result<String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| Error::failed(format!("QR code: {e}")))?;
    Ok(code.render::<Dense1x2>().quiet_zone(true).build())
}

pub(crate) fn receive(ctx: &Ctx, show_qr: bool) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let value = json!({ "vault": vault.address, "name": vault.name });
    if ctx.json {
        Ctx::json_line(&value);
        return Ok(());
    }
    let style = ctx.style;
    let mut out = vault_title(ctx, &vault);
    let _ = write!(out, "\n  {}\n", style.cyan(&vault.address));
    if show_qr {
        out.push('\n');
        for line in qr(&vault.address)?.lines() {
            let _ = writeln!(out, "  {line}");
        }
    }
    if !ctx.quiet {
        let _ = write!(
            out,
            "\n  {}\n",
            style.dim(
                "Send private notes to this address. Only the members can read them, together."
            )
        );
    }
    ctx.out(&out);
    Ok(())
}
