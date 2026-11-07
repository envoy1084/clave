//! What every command gets: the resolved profile, global flags, output and confirmations.

use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
};

use serde_json::Value;

use crate::{
    error::{Error, Kind, Result},
    profile::Profile,
    refs,
    render::{
        Style,
        spinner::{Mode, Spinner},
    },
    rpc::Client,
    types::VaultSummary,
};

/// Global options after resolution.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line flags"
)]
pub(crate) struct Ctx {
    pub(crate) home: PathBuf,
    pub(crate) profile: Profile,
    pub(crate) vault: Option<String>,
    pub(crate) node_url: Option<String>,
    pub(crate) json: bool,
    pub(crate) yes: bool,
    pub(crate) quiet: bool,
    pub(crate) style: Style,
    /// How progress lines are shown on stderr.
    pub(crate) progress: Mode,
    /// Prompts are allowed: not `--json` and stdin is a terminal.
    pub(crate) interactive: bool,
}

impl Ctx {
    pub(crate) fn new(global: &crate::Global) -> Result<Self> {
        let home = crate::profile::home()?;
        let profile = Profile::resolve(&home, &global.profile)?;
        let no_color =
            global.no_color || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        Ok(Self {
            home,
            profile,
            vault: global.vault.clone(),
            node_url: global.node_url.clone(),
            json: global.json,
            yes: global.yes,
            quiet: global.quiet,
            style: Style {
                color: !no_color && !global.json && std::io::stdout().is_terminal(),
            },
            progress: Mode::detect(global.json || global.quiet),
            interactive: !global.json && std::io::stdin().is_terminal(),
        })
    }

    /// Starts a progress line; finish it with `ok`, `fail` or `clear`.
    pub(crate) fn spin(&self, text: &str) -> Spinner {
        Spinner::start(self.progress, self.style, text)
    }

    pub(crate) fn client(&self) -> Result<Client> {
        Client::connect(&self.profile)
    }

    /// Writes human output to stdout (nothing under `--json`).
    pub(crate) fn out(&self, text: &str) {
        if !self.json {
            write_stdout(text);
        }
    }

    /// Writes a progress or informational line to stderr (nothing under `--json` or `-q`).
    pub(crate) fn note(&self, text: &str) {
        if !self.json && !self.quiet {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{text}");
        }
    }

    /// One JSON value on its own line on stdout.
    pub(crate) fn json_line(value: &Value) {
        write_stdout(&format!("{value}\n"));
    }

    /// Prints `value` under `--json`, otherwise the human rendering.
    pub(crate) fn emit(&self, value: &Value, human: impl FnOnce() -> String) {
        if self.json {
            Self::json_line(value);
        } else {
            write_stdout(&human());
        }
    }

    /// Asks `question [y/N]`; `--yes` answers yes. Never prompts under `--json` or without a terminal.
    pub(crate) fn confirm(&self, question: &str) -> Result<()> {
        if self.yes {
            return Ok(());
        }
        if !self.interactive {
            return Err(Error::usage(format!(
                "{question} Confirmation needed: pass --yes"
            )));
        }
        let mut stderr = std::io::stderr().lock();
        let _ = write!(
            stderr,
            "{} {} {} ",
            self.style.cyan("?"),
            self.style.bold(question),
            self.style.dim("[y/N]")
        );
        let _ = stderr.flush();
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
            Ok(())
        } else {
            Err(Error::new(Kind::Declined, "Declined."))
        }
    }

    /// The vault `-V` names, or the default one.
    pub(crate) fn vault(&self, client: &mut Client) -> Result<VaultSummary> {
        let vaults: Vec<VaultSummary> = client.call("vault.list", Value::Null)?;
        refs::resolve_vault(&vaults, self.vault.as_deref()).cloned()
    }
}

pub(crate) fn write_stdout(text: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}
