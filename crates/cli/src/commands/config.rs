//! `config get|set` (the daemon's `Settings`, plus `default-vault` through `vault.use`) and `relay add|list|remove`.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::{
    ConfigCmd, RelayCmd,
    context::Ctx,
    error::{Error, Result},
    refs,
    types::{Settings, VaultSummary},
    units::parse_duration,
};

const KEYS: &str = "node-url, prover, unlock-timeout, relays, default-vault";

fn render(settings: &Settings, default_vault: Option<&VaultSummary>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<16}{}", "node-url", settings.node_url);
    let _ = writeln!(
        out,
        "{:<16}{}",
        "prover",
        if settings.prover_enabled { "on" } else { "off" }
    );
    let _ = writeln!(
        out,
        "{:<16}{}s",
        "unlock-timeout", settings.unlock_timeout_secs
    );
    let relays = if settings.relays.is_empty() {
        "none".to_owned()
    } else {
        settings.relays.join(", ")
    };
    let _ = writeln!(out, "{:<16}{relays}", "relays");
    let vault = default_vault.map_or_else(
        || "none".to_owned(),
        |v| v.alias.clone().unwrap_or_else(|| v.address.clone()),
    );
    let _ = writeln!(out, "{:<16}{vault}", "default-vault");
    out
}

pub(crate) fn config(ctx: &Ctx, cmd: ConfigCmd) -> Result<()> {
    let mut client = ctx.client()?;
    match cmd {
        ConfigCmd::Get { key } => {
            let raw: Value = client.call("config.get", Value::Null)?;
            let settings: Settings = serde_json::from_value(raw.clone()).unwrap_or_default();
            let vaults: Vec<VaultSummary> = client.call("vault.list", Value::Null)?;
            let default = vaults.iter().find(|v| v.is_default);
            let Some(key) = key else {
                let mut value = raw;
                value["defaultVault"] = json!(default.map(|v| &v.address));
                ctx.emit(&value, || render(&settings, default));
                return Ok(());
            };
            let value = match key.as_str() {
                "node-url" => json!(settings.node_url),
                "prover" => json!(settings.prover_enabled),
                "unlock-timeout" => json!(settings.unlock_timeout_secs),
                "relays" => json!(settings.relays),
                "default-vault" => json!(default.map(|v| &v.address)),
                other => {
                    return Err(Error::usage(format!(
                        "Unknown setting \"{other}\"; use one of {KEYS}"
                    )));
                }
            };
            ctx.emit(&value, || match &value {
                Value::String(s) => format!("{s}\n"),
                Value::Array(items) => {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .fold(String::new(), |mut out, s| {
                            let _ = writeln!(out, "{s}");
                            out
                        })
                }
                Value::Null => "none\n".to_owned(),
                other => format!("{other}\n"),
            });
            Ok(())
        }
        ConfigCmd::Set { key, value } => {
            let patch = match key.as_str() {
                "node-url" => json!({ "nodeUrl": value }),
                "prover" => json!({ "proverEnabled": parse_switch(&value)? }),
                "unlock-timeout" => json!({ "unlockTimeoutSecs": parse_duration(&value)? }),
                "default-vault" => {
                    let vaults: Vec<VaultSummary> = client.call("vault.list", Value::Null)?;
                    let vault = refs::resolve_vault(&vaults, Some(&value))?;
                    let updated: Value =
                        client.call("vault.use", json!({ "vault": vault.address }))?;
                    ctx.emit(&updated, || format!("default-vault = {}\n", vault.address));
                    return Ok(());
                }
                "relays" => return Err(Error::usage("Use tecpan relay add|remove for relays.")),
                other => {
                    return Err(Error::usage(format!(
                        "Unknown setting \"{other}\"; use one of {KEYS}"
                    )));
                }
            };
            let updated: Value = client.call("config.set", patch)?;
            ctx.emit(&updated, || format!("{key} = {value}\n"));
            Ok(())
        }
    }
}

fn parse_switch(value: &str) -> Result<bool> {
    match value {
        "on" | "true" | "native" | "wasm" => Ok(true),
        "off" | "false" | "none" => Ok(false),
        other => Err(Error::usage(format!(
            "prover: expected on or off, not \"{other}\""
        ))),
    }
}

pub(crate) fn relay(ctx: &Ctx, cmd: RelayCmd) -> Result<()> {
    let mut client = ctx.client()?;
    let settings: Settings = client.call("config.get", Value::Null)?;
    let mut relays = settings.relays.clone();
    match cmd {
        RelayCmd::List => {
            ctx.emit(&json!(relays), || {
                if relays.is_empty() {
                    return "No relays.\n".to_owned();
                }
                relays.iter().fold(String::new(), |mut out, r| {
                    let _ = writeln!(out, "{r}");
                    out
                })
            });
            return Ok(());
        }
        RelayCmd::Add { multiaddr } => {
            if relays.contains(&multiaddr) {
                return Err(Error::usage(format!("{multiaddr} is already a relay.")));
            }
            relays.push(multiaddr);
        }
        RelayCmd::Remove { multiaddr } => {
            let before = relays.len();
            relays.retain(|r| r != &multiaddr);
            if relays.len() == before {
                return Err(
                    Error::usage(format!("{multiaddr} is not a relay.")).hint("tecpan relay list")
                );
            }
        }
    }
    ctx.note("Updating relays (restarts the keeper)…");
    let updated: Value = client.call("relays.set", json!({ "relays": relays }))?;
    ctx.emit(&updated, || {
        format!(
            "{} {} relay{} configured\n",
            ctx.style.ok(),
            relays.len(),
            if relays.len() == 1 { "" } else { "s" }
        )
    });
    Ok(())
}
