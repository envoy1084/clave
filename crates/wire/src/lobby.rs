//! Vault-founding lobbies. A founder's keeper hosts a lobby; co-founders join with an out-of-band [`Invite`], see the
//! roster ([`LobbyState`]) live, and receive the agreed [`LobbyPlan`] (with the engine's protocol secrets sealed to
//! each member) when the founder starts. Each member's engine then starts the DKG itself.
//!
//! Lobby messages are ordinary signed frames: header scope = the lobby id zero-padded to 32 bytes ([`scope`]), epoch 0.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    Error, IdentityPublic,
    codec::canonical,
    messages::{MAX_ADDRESS_BYTES, MAX_ADDRESSES},
    types::{WireField, WirePoint},
};

/// Prefix of an invite's string form.
pub const INVITE_PREFIX: &str = "tecpan-invite-v0:";
/// Longest accepted invite string.
pub const MAX_INVITE_CHARS: usize = 4096;
/// Longest lobby, member or network name, in bytes.
pub const MAX_NAME_BYTES: usize = 64;
/// Most members of a lobby (the vault's signer limit).
pub const MAX_LOBBY_MEMBERS: usize = 8;
/// Largest protocol-secrets blob the engine may hand the host.
pub const MAX_LOBBY_SECRETS_BYTES: usize = 1024;
/// Largest sealed secrets ciphertext (the blob plus the AEAD tag).
pub const MAX_SEALED_SECRETS_BYTES: usize = MAX_LOBBY_SECRETS_BYTES + 16;

/// The header scope of a lobby's messages: its id zero-padded to 32 bytes.
#[must_use]
pub fn scope(lobby: &[u8; 16]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(lobby);
    out
}

/// How to reach a keeper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeeperContact {
    /// Ed25519 identity (also its libp2p peer id).
    pub identity: IdentityPublic,
    /// X25519 transport key.
    pub transport: [u8; 32],
    /// Addresses (multiaddrs), at most 8 of at most 256 bytes.
    pub addresses: Vec<String>,
}

impl KeeperContact {
    fn validate(&self) -> Result<(), Error> {
        if self.addresses.len() > MAX_ADDRESSES
            || self.addresses.iter().any(|a| a.len() > MAX_ADDRESS_BYTES)
        {
            return Err(Error::OutOfBounds { field: "addresses" });
        }
        Ok(())
    }
}

/// A lobby member: display name, signing key and keeper.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyMember {
    /// Display name (at most 64 bytes).
    pub name: String,
    /// Grumpkin key the member will sign approvals with (validated by the keeper).
    pub signer_key: WirePoint,
    /// The member's keeper.
    pub participant: KeeperContact,
}

impl LobbyMember {
    fn validate(&self) -> Result<(), Error> {
        name("member.name", &self.name)?;
        self.participant.validate()
    }
}

fn name(field: &'static str, value: &str) -> Result<(), Error> {
    if value.len() > MAX_NAME_BYTES {
        return Err(Error::OutOfBounds { field });
    }
    Ok(())
}

fn members(list: &[LobbyMember]) -> Result<(), Error> {
    if list.is_empty() || list.len() > MAX_LOBBY_MEMBERS {
        return Err(Error::OutOfBounds { field: "members" });
    }
    list.iter().try_for_each(LobbyMember::validate)
}

/// An invitation to a lobby, passed out of band (chat, QR code). Whoever holds it can join while the lobby is open.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    /// Lobby id.
    pub lobby: [u8; 16],
    /// Join secret (keys the join proof); zeroized on drop.
    pub secret: Zeroizing<[u8; 32]>,
    /// The host's keeper.
    pub host: KeeperContact,
    /// The host's display name (at most 64 bytes).
    pub host_name: String,
    /// Vault name (at most 64 bytes).
    pub name: String,
    /// Proposed spending threshold `k`.
    pub threshold: u8,
    /// Proposed viewing threshold `t_v`.
    pub viewing_threshold: u8,
    /// Network label set by the engine (opaque to keepers, at most 64 bytes).
    pub network: String,
}

impl core::fmt::Debug for Invite {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Invite")
            .field("lobby", &self.lobby)
            .field("host", &self.host)
            .field("host_name", &self.host_name)
            .field("name", &self.name)
            .field("threshold", &self.threshold)
            .field("viewing_threshold", &self.viewing_threshold)
            .field("network", &self.network)
            .finish_non_exhaustive()
    }
}

impl Invite {
    /// Structural bounds: names, addresses and thresholds (`1 ≤ k ≤ 8`, `2 ≤ t_v ≤ 8`).
    ///
    /// # Errors
    /// [`Error::OutOfBounds`] naming the field.
    pub fn validate(&self) -> Result<(), Error> {
        name("name", &self.name)?;
        name("host_name", &self.host_name)?;
        name("network", &self.network)?;
        self.host.validate()?;
        let most = u8::try_from(MAX_LOBBY_MEMBERS).unwrap_or(u8::MAX);
        if !(1..=most).contains(&self.threshold) {
            return Err(Error::OutOfBounds { field: "threshold" });
        }
        if !(2..=most).contains(&self.viewing_threshold) {
            return Err(Error::OutOfBounds {
                field: "viewing_threshold",
            });
        }
        Ok(())
    }

    /// Canonical bytes.
    ///
    /// # Errors
    /// Bounds errors.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        canonical(self)
    }

    /// Decodes canonical bytes and checks bounds.
    ///
    /// # Errors
    /// [`Error::Malformed`] for non-canonical or trailing bytes, bounds errors.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_INVITE_CHARS {
            return Err(Error::TooLarge(bytes.len()));
        }
        let (invite, rest): (Self, &[u8]) =
            postcard::take_from_bytes(bytes).map_err(|_| Error::Malformed)?;
        if !rest.is_empty() || canonical(&invite)? != bytes {
            return Err(Error::Malformed);
        }
        invite.validate()?;
        Ok(invite)
    }

    /// The string form: `tecpan-invite-v0:` followed by base64url (no padding) of the canonical bytes.
    ///
    /// # Errors
    /// Bounds errors.
    pub fn encode(&self) -> Result<String, Error> {
        Ok(format!(
            "{INVITE_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(self.to_bytes()?)
        ))
    }

    /// Parses the string form (surrounding whitespace is ignored). Only the canonical encoding is accepted.
    ///
    /// # Errors
    /// [`Error::TooLarge`], [`Error::Malformed`] or bounds errors.
    pub fn decode(text: &str) -> Result<Self, Error> {
        let text = text.trim();
        if text.len() > MAX_INVITE_CHARS {
            return Err(Error::TooLarge(text.len()));
        }
        let body = text.strip_prefix(INVITE_PREFIX).ok_or(Error::Malformed)?;
        // The engine rejects non-zero trailing bits, so every invite has exactly one string form.
        let bytes = URL_SAFE_NO_PAD.decode(body).map_err(|_| Error::Malformed)?;
        Self::from_bytes(&bytes)
    }
}

/// A guest asks to join. `proof = HMAC-SHA256(secret, "tecpan.lobby.join.v0" ‖ lobby ‖ identity ‖ transport)` of
/// the joiner's keeper (`tecpan_crypto::lobby::join_proof`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyJoin {
    /// Lobby id.
    pub lobby: [u8; 16],
    /// Join proof.
    pub proof: [u8; 32],
    /// The joiner; its keeper identity must be the frame's signer.
    pub member: LobbyMember,
}

impl LobbyJoin {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        self.member.validate()
    }
}

/// The roster, sent by the host to every joined member whenever it changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyState {
    /// Lobby id.
    pub lobby: [u8; 16],
    /// Increases with every roster change; members apply only newer states.
    pub revision: u32,
    /// Vault name.
    pub name: String,
    /// Proposed spending threshold.
    pub threshold: u8,
    /// Proposed viewing threshold.
    pub viewing_threshold: u8,
    /// Members, host first.
    pub members: Vec<LobbyMember>,
    /// Whether the host started the lobby.
    pub started: bool,
}

impl LobbyState {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        name("name", &self.name)?;
        members(&self.members)
    }
}

/// The protocol secrets sealed for one member (`tecpan_crypto::lobby::seal_secrets`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedSecrets {
    /// Recipient's 1-based index in the plan.
    pub index: u8,
    /// HPKE encapsulated key.
    pub enc: [u8; 32],
    /// AEAD ciphertext.
    pub ciphertext: Vec<u8>,
}

/// What the founders agreed: the DKG session, thresholds, salt, members in slot order (host = slot 0) and the
/// engine's protocol secrets sealed to each member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyPlan {
    /// Lobby id.
    pub lobby: [u8; 16],
    /// DKG session id.
    pub session: [u8; 32],
    /// Spending threshold `k`.
    pub threshold: u8,
    /// Viewing threshold `t_v`.
    pub viewing_threshold: u8,
    /// Vault deployment salt.
    pub salt: WireField,
    /// Members in slot order; member `i` (1-based) is slot `i - 1`.
    pub members: Vec<LobbyMember>,
    /// Sealed protocol secrets, one per member.
    pub secrets: Vec<SealedSecrets>,
}

impl LobbyPlan {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        members(&self.members)?;
        if self.secrets.len() > MAX_LOBBY_MEMBERS {
            return Err(Error::OutOfBounds { field: "secrets" });
        }
        if self
            .secrets
            .iter()
            .any(|s| s.ciphertext.is_empty() || s.ciphertext.len() > MAX_SEALED_SECRETS_BYTES)
        {
            return Err(Error::OutOfBounds {
                field: "secrets.ciphertext",
            });
        }
        Ok(())
    }

    /// The sealed secrets for member `index` (1-based).
    #[must_use]
    pub fn sealed_for(&self, index: u8) -> Option<&SealedSecrets> {
        self.secrets.iter().find(|s| s.index == index)
    }
}

/// From a guest: it leaves an open lobby. From the host: the lobby is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyLeave {
    /// Lobby id.
    pub lobby: [u8; 16],
}
