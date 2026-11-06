use crate::{Error, SignedMessage};

/// Wire version this build speaks. New message kinds are appended to [`crate::Message`]; incompatible changes bump it.
pub const WIRE_VERSION: u16 = 0;

/// Largest accepted encoded message.
pub const MAX_MESSAGE_BYTES: usize = 1 << 20;

/// Canonical encoding of a signed message.
///
/// # Errors
/// [`Error::Malformed`] if serialisation fails (not expected for well-formed values), [`Error::TooLarge`] above
/// [`MAX_MESSAGE_BYTES`].
pub fn encode(message: &SignedMessage) -> Result<Vec<u8>, Error> {
    let bytes = postcard::to_allocvec(message).map_err(|_| Error::Malformed)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

/// Decodes and bounds-checks a signed message. Does **not** verify the signature: call [`SignedMessage::verify`].
///
/// # Errors
/// [`Error::TooLarge`], [`Error::Malformed`], [`Error::UnsupportedVersion`] or [`Error::OutOfBounds`].
pub fn decode(bytes: &[u8]) -> Result<SignedMessage, Error> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    let (message, rest): (SignedMessage, &[u8]) =
        postcard::take_from_bytes(bytes).map_err(|_| Error::Malformed)?;
    if !rest.is_empty() {
        return Err(Error::Malformed);
    }
    if message.version != WIRE_VERSION {
        return Err(Error::UnsupportedVersion(message.version));
    }
    message.body.validate()?;
    Ok(message)
}

/// Canonical bytes of any serialisable value (signing payloads, proposal ids).
pub(crate) fn canonical<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    postcard::to_allocvec(value).map_err(|_| Error::Malformed)
}
