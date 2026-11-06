//! CLI errors: a message, an optional next step, and the exit code they map to.

use std::fmt;

use serde_json::json;

/// What went wrong, which decides the exit code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Engine, keeper, node or chain error; also a locked signer or a wrong passphrase (exit 1).
    Failed,
    /// Bad arguments or an unknown or ambiguous reference (exit 2).
    Usage,
    /// The profile's daemon is not running (exit 3).
    Daemon,
    /// Not possible yet: below threshold, keepers unreachable, lobby not started (exit 4).
    NotReady,
    /// The member answered no at a confirmation prompt (exit 5).
    Declined,
}

impl Kind {
    pub(crate) fn exit_code(self) -> u8 {
        match self {
            Self::Failed => 1,
            Self::Usage => 2,
            Self::Daemon => 3,
            Self::NotReady => 4,
            Self::Declined => 5,
        }
    }

    /// The daemon's `data.kind` → CLI kind, per `architecture/cli.md`.
    pub(crate) fn from_daemon(kind: Option<&str>) -> Self {
        match kind {
            Some("invalid") => Self::Usage,
            Some("not-ready") => Self::NotReady,
            _ => Self::Failed,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Usage => "usage",
            Self::Daemon => "daemon",
            Self::NotReady => "not-ready",
            Self::Declined => "declined",
        }
    }
}

/// A CLI error.
#[derive(Debug)]
pub(crate) struct Error {
    pub(crate) kind: Kind,
    pub(crate) message: String,
    /// The next command to run, printed on a second line.
    pub(crate) hint: Option<String>,
    /// The daemon's `data.kind`, when the error came from the daemon.
    pub(crate) daemon_kind: Option<String>,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub(crate) fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
            daemon_kind: None,
        }
    }

    pub(crate) fn failed(message: impl Into<String>) -> Self {
        Self::new(Kind::Failed, message)
    }

    pub(crate) fn usage(message: impl Into<String>) -> Self {
        Self::new(Kind::Usage, message)
    }

    #[must_use]
    pub(crate) fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub(crate) fn exit_code(&self) -> u8 {
        self.kind.exit_code()
    }

    /// The `--json` error object printed on stderr.
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let code = self.daemon_kind.as_deref().unwrap_or(self.kind.code());
        let mut error = json!({ "code": code, "message": self.message });
        if let Some(hint) = &self.hint {
            error["hint"] = json!(hint);
        }
        json!({ "error": error })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::failed(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_kinds_map_to_exit_codes() {
        let code = |k| Kind::from_daemon(Some(k)).exit_code();
        assert_eq!(code("invalid"), 2);
        assert_eq!(code("not-ready"), 4);
        assert_eq!(code("locked"), 1);
        assert_eq!(code("auth"), 1);
        assert_eq!(code("failed"), 1);
        assert_eq!(Kind::from_daemon(None).exit_code(), 1);
        assert_eq!(Kind::Daemon.exit_code(), 3);
        assert_eq!(Kind::Declined.exit_code(), 5);
    }

    #[test]
    fn json_error_shape() {
        let error = Error::usage("Ambiguous proposal \"7f\": 7f3a1c, 7f90d2.");
        assert_eq!(
            error.to_json(),
            json!({ "error": { "code": "usage", "message": "Ambiguous proposal \"7f\": 7f3a1c, 7f90d2." } })
        );
        let mut locked = Error::failed("signer locked").hint("tecpan unlock");
        locked.daemon_kind = Some("locked".into());
        assert_eq!(
            locked.to_json(),
            json!({ "error": { "code": "locked", "message": "signer locked", "hint": "tecpan unlock" } })
        );
    }
}
