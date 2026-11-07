//! `balance`, `token add|list|deploy-test`, `receive`.

use std::fmt::Write as _;

use qrcode::{QrCode, render::unicode::Dense1x2};
use serde_json::{Value, json};

use crate::{
    TokenCmd,
    context::Ctx,
    error::{Error, Result},
    refs::{self, short_hex},
    render::table::Table,
    types::{TokenBalance, TokenInfo},
    units::format_amount,
};

pub(crate) fn balance(ctx: &Ctx, token: Option<&str>) -> Result<()> {
    let mut client = ctx.client()?;
    let vault = ctx.vault(&mut client)?;
    let raw: Value = client.call("tokens.list", json!({ "vault": vault.address }))?;
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
        let mut out = format!("{}  {}\n\n", ctx.style.bold(&vault.name), short_hex(&vault.address));
        if balances.is_empty() {
            out.push_str("No tracked tokens. Track one: clave token add <address> (local network: clave token deploy-test)\n");
            return out;
        }
        let mut table = Table::new(&["TOKEN", "BALANCE", "ADDRESS"]);
        for b in &balances {
            let amount = match (&b.balance, &b.error) {
                (Some(units), _) => format_amount(units, b.token.decimals),
                (None, Some(error)) => ctx.style.red(error),
                (None, None) => "–".to_owned(),
            };
            table.row(vec![b.token.symbol.clone(), amount, short_hex(&b.token.address)]);
        }
        out.push_str(&table.render());
        out
    });
    Ok(())
}

fn token_line(token: &TokenInfo) -> String {
    format!(
        "{}  {}  ({}, {} decimals)\n",
        token.symbol, token.address, token.name, token.decimals
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
            let raw: Value = client.call(
                "tokens.add",
                json!({ "vault": vault.address, "token": address }),
            )?;
            let token: TokenInfo = serde_json::from_value(raw.clone()).unwrap_or_default();
            ctx.emit(&raw, || {
                format!("{} Tracking {}", ctx.style.ok(), token_line(&token))
            });
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
                        return "No tracked tokens.\n".to_owned();
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
                    table.render()
                },
            );
        }
        TokenCmd::DeployTest => {
            ctx.note("Deploying a test token and minting to the vault (local network only)…");
            let raw: Value = client.call("tokens.deployTest", json!({ "vault": vault.address }))?;
            let token: TokenInfo = serde_json::from_value(raw.clone()).unwrap_or_default();
            ctx.emit(&raw, || {
                format!("{} Deployed {}", ctx.style.ok(), token_line(&token))
            });
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
    let mut out = format!("{}\n", vault.address);
    if show_qr {
        out.push('\n');
        out.push_str(&qr(&vault.address)?);
        out.push('\n');
    }
    if !ctx.quiet {
        let _ = write!(
            out,
            "\nSend private notes to this address. Balances need t_v keepers online: clave balance\n"
        );
    }
    ctx.out(&out);
    Ok(())
}
