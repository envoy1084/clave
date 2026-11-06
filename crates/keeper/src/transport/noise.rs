//! Noise `XX` channel over a byte stream, keyed with the keepers' X25519 transport keys. Frames are split into Noise
//! messages (each sent as a big-endian `u16` length and the ciphertext); a frame starts with a message holding its
//! total length.

use std::sync::Arc;

use snow::{Builder, HandshakeState, StatelessTransportState};
use tecpan_wire::MAX_MESSAGE_BYTES;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

use crate::{Error, Result};

const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"tecpan.keeper.channel.v0";
const MAX_NOISE: usize = 65_535;
const TAG: usize = 16;
const CHUNK: usize = MAX_NOISE - TAG;

fn transport_error(error: impl std::fmt::Display) -> Error {
    Error::Transport(error.to_string())
}

/// Runs the handshake as initiator; returns the channel and the responder's static key.
pub(super) async fn initiate<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    key: &Zeroizing<[u8; 32]>,
) -> Result<(Channel, [u8; 32])> {
    let params = PATTERN.parse().map_err(transport_error)?;
    let mut state = Builder::new(params)
        .local_private_key(key.as_slice())
        .map_err(transport_error)?
        .prologue(PROLOGUE)
        .map_err(transport_error)?
        .build_initiator()
        .map_err(transport_error)?;
    let mut buffer = vec![0u8; MAX_NOISE];
    let n = state
        .write_message(&[], &mut buffer)
        .map_err(transport_error)?;
    write_raw(stream, &buffer[..n]).await?;
    let message = read_raw(stream).await?;
    state
        .read_message(&message, &mut buffer)
        .map_err(transport_error)?;
    let n = state
        .write_message(&[], &mut buffer)
        .map_err(transport_error)?;
    write_raw(stream, &buffer[..n]).await?;
    finish(state)
}

/// Runs the handshake as responder; returns the channel and the initiator's static key.
pub(super) async fn respond<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    key: &Zeroizing<[u8; 32]>,
) -> Result<(Channel, [u8; 32])> {
    let params = PATTERN.parse().map_err(transport_error)?;
    let mut state = Builder::new(params)
        .local_private_key(key.as_slice())
        .map_err(transport_error)?
        .prologue(PROLOGUE)
        .map_err(transport_error)?
        .build_responder()
        .map_err(transport_error)?;
    let mut buffer = vec![0u8; MAX_NOISE];
    let message = read_raw(stream).await?;
    state
        .read_message(&message, &mut buffer)
        .map_err(transport_error)?;
    let n = state
        .write_message(&[], &mut buffer)
        .map_err(transport_error)?;
    write_raw(stream, &buffer[..n]).await?;
    let message = read_raw(stream).await?;
    state
        .read_message(&message, &mut buffer)
        .map_err(transport_error)?;
    finish(state)
}

fn finish(state: HandshakeState) -> Result<(Channel, [u8; 32])> {
    let remote: [u8; 32] = state
        .get_remote_static()
        .and_then(|k| k.try_into().ok())
        .ok_or_else(|| Error::Transport("peer sent no static key".into()))?;
    let state = state
        .into_stateless_transport_mode()
        .map_err(transport_error)?;
    Ok((Channel(Arc::new(state)), remote))
}

/// An established channel; each direction keeps its own nonce counter.
#[derive(Clone)]
pub(super) struct Channel(Arc<StatelessTransportState>);

impl Channel {
    /// Encrypts and writes one frame.
    pub(super) async fn write_frame<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        nonce: &mut u64,
        frame: &[u8],
    ) -> Result<()> {
        let length = u32::try_from(frame.len())
            .map_err(transport_error)?
            .to_be_bytes();
        self.write_message(writer, nonce, &length).await?;
        for chunk in frame.chunks(CHUNK) {
            self.write_message(writer, nonce, chunk).await?;
        }
        writer.flush().await?;
        Ok(())
    }

    /// Reads and decrypts one frame.
    pub(super) async fn read_frame<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
        nonce: &mut u64,
    ) -> Result<Vec<u8>> {
        let header = self.read_message(reader, nonce).await?;
        let length: [u8; 4] = header
            .as_slice()
            .try_into()
            .map_err(|_| Error::Transport("bad frame header".into()))?;
        let length = usize::try_from(u32::from_be_bytes(length)).map_err(transport_error)?;
        if length > MAX_MESSAGE_BYTES {
            return Err(Error::Transport("frame too large".into()));
        }
        let mut frame = Vec::with_capacity(length);
        while frame.len() < length {
            frame.extend_from_slice(&self.read_message(reader, nonce).await?);
        }
        if frame.len() != length {
            return Err(Error::Transport("frame length mismatch".into()));
        }
        Ok(frame)
    }

    async fn write_message<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        nonce: &mut u64,
        payload: &[u8],
    ) -> Result<()> {
        let mut message = vec![0u8; payload.len() + TAG];
        let n = self
            .0
            .write_message(*nonce, payload, &mut message)
            .map_err(transport_error)?;
        *nonce += 1;
        write_raw(writer, &message[..n]).await
    }

    async fn read_message<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
        nonce: &mut u64,
    ) -> Result<Vec<u8>> {
        let message = read_raw(reader).await?;
        let mut payload = vec![0u8; message.len()];
        let n = self
            .0
            .read_message(*nonce, &message, &mut payload)
            .map_err(transport_error)?;
        *nonce += 1;
        payload.truncate(n);
        Ok(payload)
    }
}

async fn write_raw<W: AsyncWrite + Unpin>(writer: &mut W, message: &[u8]) -> Result<()> {
    let length = u16::try_from(message.len()).map_err(transport_error)?;
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(message).await?;
    Ok(())
}

async fn read_raw<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut length = [0u8; 2];
    reader.read_exact(&mut length).await?;
    let mut message = vec![0u8; usize::from(u16::from_be_bytes(length))];
    reader.read_exact(&mut message).await?;
    Ok(message)
}
