//! Producing the signer secret: an unlocked signer needs none; otherwise Touch ID (macOS, interactive), the
//! passphrase file (`CLAVE_PASSPHRASE_FILE`) or a no-echo prompt. Never argv or plain environment variables.
//! `--yes` is not an input here: it skips confirmations, never authentication.

#[cfg(target_os = "macos")]
mod touch_id;

use std::path::PathBuf;

use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{
    context::Ctx,
    error::{Error, Kind, Result},
    profile::Profile,
    rpc::Client,
    types::SignerStatus,
};

/// The secret that opens `signer.sealed` for one call.
pub(crate) enum Secret {
    Passphrase(Zeroizing<String>),
    /// The 32-byte sealing key as `0x` hex, from the Touch ID keychain item.
    Key(Zeroizing<String>),
}

impl Secret {
    /// The `Secret` param (`{ passphrase }` or `{ key }`).
    pub(crate) fn to_param(&self) -> Value {
        match self {
            Self::Passphrase(p) => json!({ "passphrase": p.as_str() }),
            Self::Key(k) => json!({ "key": k.as_str() }),
        }
    }
}

/// Where the secret comes from.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// The daemon holds an unlock window: send no secret.
    Unlocked,
    TouchId,
    PassphraseFile(PathBuf),
    Prompt,
}

/// The inputs that decide the source.
pub(crate) struct Situation {
    pub(crate) unlocked: bool,
    pub(crate) touch_id: bool,
    pub(crate) interactive: bool,
    pub(crate) passphrase_file: Option<PathBuf>,
}

/// Picks the source, or fails when none can work (non-interactive, locked, no passphrase file).
pub(crate) fn source(s: &Situation) -> Result<Source> {
    if s.unlocked {
        return Ok(Source::Unlocked);
    }
    if s.touch_id && s.interactive && cfg!(target_os = "macos") {
        return Ok(Source::TouchId);
    }
    if let Some(file) = &s.passphrase_file {
        return Ok(Source::PassphraseFile(file.clone()));
    }
    if s.interactive {
        return Ok(Source::Prompt);
    }
    let mut error = Error::new(
        Kind::Failed,
        "The signer is locked and no passphrase source is available.",
    )
    .hint("Set CLAVE_PASSPHRASE_FILE, or unlock first: clave unlock --for 15m");
    error.daemon_kind = Some("locked".into());
    Err(error)
}

fn passphrase_file() -> Option<PathBuf> {
    std::env::var_os("CLAVE_PASSPHRASE_FILE")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Reads the passphrase file, dropping one trailing newline.
fn read_file(path: &PathBuf) -> Result<Zeroizing<String>> {
    let mut text =
        Zeroizing::new(std::fs::read_to_string(path).map_err(|e| {
            Error::failed(format!("CLAVE_PASSPHRASE_FILE {}: {e}", path.display()))
        })?);
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    if text.is_empty() {
        return Err(Error::failed(format!(
            "CLAVE_PASSPHRASE_FILE {} is empty",
            path.display()
        )));
    }
    Ok(text)
}

fn prompt(label: &str) -> Result<Zeroizing<String>> {
    rpassword::prompt_password(label)
        .map(Zeroizing::new)
        .map_err(|e| Error::failed(format!("could not read the passphrase: {e}")))
}

/// The existing passphrase: from the file, else a prompt.
pub(crate) fn passphrase(ctx: &Ctx) -> Result<Zeroizing<String>> {
    match passphrase_file() {
        Some(file) => read_file(&file),
        None if ctx.interactive => prompt("Passphrase: "),
        None => Err(Error::usage(
            "A passphrase is needed: set CLAVE_PASSPHRASE_FILE (prompts are off)",
        )),
    }
}

/// A new passphrase: from the file, else typed twice. At least 8 characters.
pub(crate) fn new_passphrase(ctx: &Ctx) -> Result<Zeroizing<String>> {
    let passphrase = match passphrase_file() {
        Some(file) => read_file(&file)?,
        None if ctx.interactive => {
            let first = prompt("New passphrase: ")?;
            let second = prompt("Repeat passphrase: ")?;
            if first != second {
                return Err(Error::usage("The passphrases do not match."));
            }
            first
        }
        None => {
            return Err(Error::usage(
                "A passphrase is needed: set CLAVE_PASSPHRASE_FILE (prompts are off)",
            ));
        }
    };
    if passphrase.chars().count() < 8 {
        return Err(Error::usage("Use a passphrase of at least 8 characters."));
    }
    Ok(passphrase)
}

/// The secret for one signature, or `None` when the signer is unlocked.
pub(crate) fn signing_secret(ctx: &Ctx, client: &mut Client) -> Result<Option<Secret>> {
    let status: SignerStatus = client.call("signer.status", Value::Null)?;
    let situation = Situation {
        unlocked: !status.locked,
        touch_id: touch_id_enabled(ctx),
        interactive: ctx.interactive,
        passphrase_file: passphrase_file(),
    };
    secret_from(ctx, source(&situation)?)
}

/// The secret for `signer.unlock`, which always needs one.
pub(crate) fn unlock_secret(ctx: &Ctx) -> Result<Secret> {
    let situation = Situation {
        unlocked: false,
        touch_id: touch_id_enabled(ctx),
        interactive: ctx.interactive,
        passphrase_file: passphrase_file(),
    };
    secret_from(ctx, source(&situation)?)?.ok_or_else(|| Error::failed("no secret"))
}

fn secret_from(ctx: &Ctx, source: Source) -> Result<Option<Secret>> {
    match source {
        Source::Unlocked => Ok(None),
        Source::PassphraseFile(file) => read_file(&file).map(|p| Some(Secret::Passphrase(p))),
        Source::Prompt => prompt("Passphrase: ").map(|p| Some(Secret::Passphrase(p))),
        Source::TouchId => match read_touch_id(ctx) {
            Ok(key) => Ok(Some(Secret::Key(key))),
            Err(error) => {
                ctx.note(&format!("{error}; using the passphrase instead"));
                prompt("Passphrase: ").map(|p| Some(Secret::Passphrase(p)))
            }
        },
    }
}

/// Whether this profile enabled Touch ID (a marker file: checking it never prompts).
pub(crate) fn touch_id_enabled(ctx: &Ctx) -> bool {
    ctx.profile.touch_id_marker().exists()
}

/// The keychain account of a profile: its directory, unique per profile and home.
fn keychain_account(profile: &Profile) -> String {
    profile.dir.display().to_string()
}

#[cfg(target_os = "macos")]
fn read_touch_id(ctx: &Ctx) -> Result<Zeroizing<String>> {
    touch_id::read(
        &keychain_account(&ctx.profile),
        &format!("sign as Clave profile {}", ctx.profile.name),
    )
}

#[cfg(not(target_os = "macos"))]
fn read_touch_id(_: &Ctx) -> Result<Zeroizing<String>> {
    Err(Error::failed("Touch ID is only available on macOS"))
}

/// `clave signer enable-touch-id`: passphrase → `signer.sealingKey` → keychain, after one Touch ID check.
#[cfg(target_os = "macos")]
pub(crate) fn enable_touch_id(ctx: &Ctx, client: &mut Client, known: Option<&str>) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Sealing {
        key: String,
    }
    if !ctx.interactive {
        return Err(Error::usage("Touch ID needs an interactive terminal."));
    }
    touch_id::authenticate("enable Touch ID for Clave")?;
    let passphrase = match known {
        Some(p) => Zeroizing::new(p.to_owned()),
        None => passphrase(ctx)?,
    };
    let sealing: Sealing = client.call(
        "signer.sealingKey",
        json!({ "passphrase": passphrase.as_str() }),
    )?;
    let key = Zeroizing::new(sealing.key);
    touch_id::store(&keychain_account(&ctx.profile), &key)?;
    crate::profile::write_private(&ctx.profile.touch_id_marker(), b"keychain\n")
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn enable_touch_id(_: &Ctx, _: &mut Client, _: Option<&str>) -> Result<()> {
    Err(Error::failed(
        "Touch ID is not available in this build: it needs macOS.",
    ))
}

/// `clave signer disable-touch-id`: removes the keychain item and the marker.
pub(crate) fn disable_touch_id(profile: &Profile) -> Result<()> {
    #[cfg(target_os = "macos")]
    touch_id::remove(&keychain_account(profile))?;
    match std::fs::remove_file(profile.touch_id_marker()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn situation() -> Situation {
        Situation {
            unlocked: false,
            touch_id: false,
            interactive: true,
            passphrase_file: None,
        }
    }

    #[test]
    fn unlocked_signer_needs_no_secret() {
        let s = Situation {
            unlocked: true,
            ..situation()
        };
        assert_eq!(source(&s).unwrap(), Source::Unlocked);
    }

    #[test]
    fn locked_signer_always_authenticates() {
        assert_eq!(source(&situation()).unwrap(), Source::Prompt);
        let file = Situation {
            passphrase_file: Some("/p".into()),
            ..situation()
        };
        assert_eq!(source(&file).unwrap(), Source::PassphraseFile("/p".into()));
        let headless_file = Situation {
            interactive: false,
            passphrase_file: Some("/p".into()),
            ..situation()
        };
        assert_eq!(
            source(&headless_file).unwrap(),
            Source::PassphraseFile("/p".into())
        );
    }

    #[test]
    fn headless_without_a_passphrase_file_fails_instead_of_signing() {
        let s = Situation {
            interactive: false,
            touch_id: true,
            ..situation()
        };
        let error = source(&s).err().unwrap();
        assert_eq!(error.exit_code(), 1);
        assert_eq!(error.to_json()["error"]["code"], "locked");
    }

    #[test]
    fn touch_id_only_when_interactive_on_macos() {
        let s = Situation {
            touch_id: true,
            ..situation()
        };
        let expected = if cfg!(target_os = "macos") {
            Source::TouchId
        } else {
            Source::Prompt
        };
        assert_eq!(source(&s).unwrap(), expected);
    }

    #[test]
    fn passphrase_file_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p");
        std::fs::write(&path, "correct horse\n").unwrap();
        assert_eq!(read_file(&path).unwrap().as_str(), "correct horse");
        std::fs::write(&path, "\n").unwrap();
        assert!(read_file(&path).is_err());
    }
}
