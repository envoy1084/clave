//! Mailbox protocol between keepers and an untrusted relay (`/tecpan/mailbox/0`). The relay stores sealed
//! [`RelayEnvelope`]s under opaque 32-byte mailbox ids that only the sender and recipient can derive; it never sees a
//! plaintext, an identity or a vault address. Messages use the same canonical `postcard` encoding as the wire format.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::{Error, codec::canonical, messages::RelayEnvelope};

/// Domain tag of envelope ids.
pub const ENVELOPE_DOMAIN: &[u8] = b"tecpan.mailbox.envelope.v0";
/// Most mailboxes in one fetch.
pub const MAX_FETCH_MAILBOXES: usize = 128;
/// Most ids in one delete.
pub const MAX_DELETE_IDS: usize = 1024;
/// Largest sealed payload a relay accepts.
pub const MAX_ENVELOPE_BYTES: usize = 256 * 1024;
/// Largest mailbox message (a fetch answer may carry several envelopes).
pub const MAX_MAILBOX_MESSAGE_BYTES: usize = 4 << 20;

/// A keeper's request to a relay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MailboxRequest {
    /// Store an envelope until it expires or is deleted.
    Deposit(RelayEnvelope),
    /// Return envelopes held for these mailboxes.
    Fetch {
        /// Mailbox ids.
        mailboxes: Vec<[u8; 32]>,
    },
    /// Delete delivered envelopes.
    Delete {
        /// Envelope ids.
        ids: Vec<[u8; 32]>,
    },
}

/// Why a relay refused a deposit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    /// Payload above [`MAX_ENVELOPE_BYTES`].
    TooLarge,
    /// Already expired, or expiring beyond the relay's maximum TTL.
    BadExpiry,
    /// The depositor or the mailbox is over quota.
    QuotaExceeded,
    /// The relay only serves allowlisted keepers.
    NotAllowed,
    /// The request was malformed or out of bounds.
    Malformed,
}

/// A stored envelope with its id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEnvelope {
    /// [`envelope_id`].
    pub id: [u8; 32],
    /// The envelope.
    pub envelope: RelayEnvelope,
}

/// A relay's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MailboxResponse {
    /// Stored under this id.
    Stored {
        /// Envelope id.
        id: [u8; 32],
    },
    /// Not stored.
    Rejected(Rejection),
    /// Envelopes for the requested mailboxes (possibly a prefix; fetch again after deleting).
    Envelopes(Vec<StoredEnvelope>),
    /// Number of envelopes deleted.
    Deleted(u32),
}

/// `SHA-256("tecpan.mailbox.envelope.v0" ‖ canonical(envelope))`.
///
/// # Errors
/// [`Error::Malformed`] if the envelope cannot be encoded.
pub fn envelope_id(envelope: &RelayEnvelope) -> Result<[u8; 32], Error> {
    let mut hasher = Sha256::new();
    hasher.update(ENVELOPE_DOMAIN);
    hasher.update(canonical(envelope)?);
    Ok(hasher.finalize().into())
}

impl MailboxRequest {
    fn validate(&self) -> Result<(), Error> {
        let within = |field, len: usize, max| {
            if len <= max {
                Ok(())
            } else {
                Err(Error::OutOfBounds { field })
            }
        };
        match self {
            Self::Deposit(envelope) => within("sealed", envelope.sealed.len(), MAX_ENVELOPE_BYTES),
            Self::Fetch { mailboxes } => within("mailboxes", mailboxes.len(), MAX_FETCH_MAILBOXES),
            Self::Delete { ids } => within("ids", ids.len(), MAX_DELETE_IDS),
        }
    }
}

/// Canonical encoding of a mailbox message.
///
/// # Errors
/// [`Error::Malformed`] or [`Error::TooLarge`].
pub fn encode<T: Serialize>(message: &T) -> Result<Vec<u8>, Error> {
    let bytes = canonical(message)?;
    if bytes.len() > MAX_MAILBOX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

fn decode_canonical<T: Serialize + DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    if bytes.len() > MAX_MAILBOX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    let (value, rest): (T, &[u8]) =
        postcard::take_from_bytes(bytes).map_err(|_| Error::Malformed)?;
    if !rest.is_empty() || canonical(&value)? != bytes {
        return Err(Error::Malformed);
    }
    Ok(value)
}

/// Decodes and bounds-checks a request (canonical encoding only).
///
/// # Errors
/// [`Error::Malformed`], [`Error::TooLarge`] or [`Error::OutOfBounds`].
pub fn decode_request(bytes: &[u8]) -> Result<MailboxRequest, Error> {
    let request: MailboxRequest = decode_canonical(bytes)?;
    request.validate()?;
    Ok(request)
}

/// Decodes a response (canonical encoding only).
///
/// # Errors
/// [`Error::Malformed`] or [`Error::TooLarge`].
pub fn decode_response(bytes: &[u8]) -> Result<MailboxResponse, Error> {
    decode_canonical(bytes)
}
