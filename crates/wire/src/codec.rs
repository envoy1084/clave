use crate::{Error, Message, SignedMessage};

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
/// Only the canonical encoding is accepted (postcard also parses overlong varints), so every message has exactly one
/// byte form and byte-level deduplication is sound.
///
/// # Errors
/// [`Error::TooLarge`], [`Error::Malformed`] (including non-canonical bytes), [`Error::UnsupportedVersion`] or
/// [`Error::OutOfBounds`].
pub fn decode(bytes: &[u8]) -> Result<SignedMessage, Error> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    let (message, rest): (SignedMessage, &[u8]) =
        postcard::take_from_bytes(bytes).map_err(|_| Error::Malformed)?;
    if !rest.is_empty() || canonical(&message)? != bytes {
        return Err(Error::Malformed);
    }
    if message.version != WIRE_VERSION {
        return Err(Error::UnsupportedVersion(message.version));
    }
    message.body.validate()?;
    Ok(message)
}

/// Canonical encoding of an unsigned message body, as passed between an engine and its own keeper before signing.
///
/// # Errors
/// [`Error::OutOfBounds`] from [`Message::validate`], [`Error::TooLarge`] above [`MAX_MESSAGE_BYTES`].
pub fn encode_body(body: &Message) -> Result<Vec<u8>, Error> {
    body.validate()?;
    let bytes = canonical(body)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    Ok(bytes)
}

/// Decodes and bounds-checks an unsigned message body; only the canonical encoding is accepted.
///
/// # Errors
/// [`Error::TooLarge`], [`Error::Malformed`] or [`Error::OutOfBounds`].
pub fn decode_body(bytes: &[u8]) -> Result<Message, Error> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    let (body, rest): (Message, &[u8]) =
        postcard::take_from_bytes(bytes).map_err(|_| Error::Malformed)?;
    if !rest.is_empty() || canonical(&body)? != bytes {
        return Err(Error::Malformed);
    }
    body.validate()?;
    Ok(body)
}

/// Canonical bytes of any serialisable value (signing payloads, proposal ids).
pub(crate) fn canonical<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    postcard::to_allocvec(value).map_err(|_| Error::Malformed)
}
