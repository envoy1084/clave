//! libp2p request-response protocols between keepers and relays. Requests and responses are length-prefixed (`u32`
//! big-endian) byte strings: keeper protocols carry one encoded `SignedMessage` per request and answer with an empty
//! acknowledgement; the mailbox protocol carries [`crate::mailbox`] messages.

use std::io;

use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::{StreamProtocol, request_response::Codec};

use crate::{MAX_MESSAGE_BYTES, mailbox::MAX_MAILBOX_MESSAGE_BYTES};

/// Threshold ECDH requests and responses.
pub const ECDH: StreamProtocol = StreamProtocol::new("/clave/ecdh/0");
/// DKG and reshare rounds, complaints and echoes.
pub const CEREMONY: StreamProtocol = StreamProtocol::new("/clave/ceremony/0");
/// Proposals, approvals, receipts and sync requests.
pub const SYNC: StreamProtocol = StreamProtocol::new("/clave/sync/0");
/// Vault-founding lobbies: the only lane on which a keeper hosting an open lobby accepts frames from non-members.
pub const LOBBY: StreamProtocol = StreamProtocol::new("/clave/lobby/0");
/// Keeper to relay mailbox.
pub const MAILBOX: StreamProtocol = StreamProtocol::new("/clave/mailbox/0");
/// libp2p identify protocol version of Clave nodes.
pub const IDENTIFY_VERSION: &str = "/clave/0";

/// Length-prefixed byte codec with per-direction size limits.
#[derive(Clone, Copy, Debug)]
pub struct FrameCodec {
    max_request: usize,
    max_response: usize,
}

impl FrameCodec {
    /// Keeper protocols: one wire message per request, an empty acknowledgement back.
    #[must_use]
    pub const fn keeper() -> Self {
        Self {
            max_request: MAX_MESSAGE_BYTES,
            max_response: 64,
        }
    }

    /// The mailbox protocol.
    #[must_use]
    pub const fn mailbox() -> Self {
        Self {
            max_request: MAX_MAILBOX_MESSAGE_BYTES,
            max_response: MAX_MAILBOX_MESSAGE_BYTES,
        }
    }
}

async fn read<T: AsyncRead + Unpin + Send>(io: &mut T, max: usize) -> io::Result<Vec<u8>> {
    let mut length = [0u8; 4];
    io.read_exact(&mut length).await?;
    let length =
        usize::try_from(u32::from_be_bytes(length)).map_err(|_| io::ErrorKind::InvalidData)?;
    if length > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut bytes = vec![0u8; length];
    io.read_exact(&mut bytes).await?;
    Ok(bytes)
}

async fn write<T: AsyncWrite + Unpin + Send>(
    io: &mut T,
    bytes: &[u8],
    max: usize,
) -> io::Result<()> {
    if bytes.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame too large",
        ));
    }
    let length = u32::try_from(bytes.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
    io.write_all(&length.to_be_bytes()).await?;
    io.write_all(bytes).await?;
    io.close().await
}

impl Codec for FrameCodec {
    type Protocol = StreamProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T: AsyncRead + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
    ) -> io::Result<Vec<u8>> {
        read(io, self.max_request).await
    }

    async fn read_response<T: AsyncRead + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
    ) -> io::Result<Vec<u8>> {
        read(io, self.max_response).await
    }

    async fn write_request<T: AsyncWrite + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
        request: Vec<u8>,
    ) -> io::Result<()> {
        write(io, &request, self.max_request).await
    }

    async fn write_response<T: AsyncWrite + Unpin + Send>(
        &mut self,
        _: &StreamProtocol,
        io: &mut T,
        response: Vec<u8>,
    ) -> io::Result<()> {
        write(io, &response, self.max_response).await
    }
}
