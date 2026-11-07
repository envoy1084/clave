//! `unlock`, `lock`, `signer status|enable-touch-id|disable-touch-id`.

use serde_json::{Value, json};

use crate::{
    SignerCmd,
    context::Ctx,
    error::Result,
    render::{self, layout, status::signer as render_signer},
    secret,
    types::SignerStatus,
    units::parse_duration,
};

fn show(ctx: &Ctx, raw: &Value) {
    let status: SignerStatus = serde_json::from_value(raw.clone()).unwrap_or_default();
    let mut value = raw.clone();
    value["touchId"] = json!(secret::touch_id_enabled(ctx));
    ctx.emit(&value, || {
        let style = ctx.style;
        let touch = if secret::touch_id_enabled(ctx) {
            style.green("enabled")
        } else {
            style.dim("off")
        };
        let state = render_signer(&status, render::now_secs() * 1000, style);
        let mut rows = vec![("Signer", state.trim_start_matches("signer ").to_owned())];
        rows.push(("Touch ID", touch));
        layout::fields(style, &rows)
    });
}

pub(crate) fn unlock(ctx: &Ctx, duration: &str) -> Result<()> {
    let ttl = parse_duration(duration)?;
    let mut client = ctx.client()?;
    let secret = secret::unlock_secret(ctx)?;
    let raw: Value = client.call(
        "signer.unlock",
        json!({ "secret": secret.to_param(), "ttlSecs": ttl }),
    )?;
    drop(secret);
    show(ctx, &raw);
    Ok(())
}

pub(crate) fn lock(ctx: &Ctx) -> Result<()> {
    let raw: Value = ctx.client()?.call("signer.lock", Value::Null)?;
    show(ctx, &raw);
    Ok(())
}

pub(crate) fn run(ctx: &Ctx, cmd: &SignerCmd) -> Result<()> {
    match cmd {
        SignerCmd::Status => {
            let raw: Value = ctx.client()?.call("signer.status", Value::Null)?;
            show(ctx, &raw);
            Ok(())
        }
        SignerCmd::EnableTouchId => {
            let mut client = ctx.client()?;
            secret::enable_touch_id(ctx, &mut client, None)?;
            ctx.emit(&json!({ "touchId": true }), || {
                format!(
                    "{} Touch ID enabled. The passphrase keeps working as the recovery path.\n",
                    ctx.style.ok()
                )
            });
            Ok(())
        }
        SignerCmd::DisableTouchId => {
            secret::disable_touch_id(&ctx.profile)?;
            ctx.emit(&json!({ "touchId": false }), || {
                "Touch ID disabled.\n".to_owned()
            });
            Ok(())
        }
    }
}
