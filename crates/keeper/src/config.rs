//! Keeper configuration: `<data>/keeper.toml` (optional), overridden by command-line flags.

use std::{net::SocketAddr, path::Path, time::Duration};

use serde::Deserialize;

use crate::{Error, KeeperConfig, Result};

/// Contents of `keeper.toml`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct FileConfig {
    /// Peer listen address.
    pub listen: Option<SocketAddr>,
    /// Address advertised to other members (default `tcp://<bound address>`).
    pub advertise: Option<String>,
    /// RPC socket path (named pipe on Windows).
    pub rpc_socket: Option<String>,
    /// Ceremony round timeout, seconds.
    pub round_timeout_secs: Option<u64>,
    /// ECDH gathering timeout, milliseconds.
    pub ecdh_timeout_ms: Option<u64>,
    /// ECDH budget per member, points per minute.
    pub ecdh_points_per_minute: Option<u32>,
    /// Accepted clock skew, seconds.
    pub max_skew_secs: Option<u64>,
}

impl FileConfig {
    /// Reads `path` if it exists.
    ///
    /// # Errors
    /// [`Error::Invalid`] for malformed TOML.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        toml::from_str(&std::fs::read_to_string(path)?)
            .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))
    }

    /// The keeper tunables, defaults filled in.
    #[must_use]
    pub fn keeper_config(&self) -> KeeperConfig {
        let defaults = KeeperConfig::default();
        KeeperConfig {
            round_timeout_secs: self
                .round_timeout_secs
                .unwrap_or(defaults.round_timeout_secs),
            ecdh_timeout: self
                .ecdh_timeout_ms
                .map_or(defaults.ecdh_timeout, Duration::from_millis),
            ecdh_points_per_minute: self
                .ecdh_points_per_minute
                .unwrap_or(defaults.ecdh_points_per_minute),
            max_skew_secs: self.max_skew_secs.unwrap_or(defaults.max_skew_secs),
            replay_capacity: defaults.replay_capacity,
        }
    }
}
