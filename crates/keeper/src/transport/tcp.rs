use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex, PoisonError, RwLock},
    time::Duration,
};

use tecpan_crypto::envelope::TransportSecret;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::timeout,
};
use zeroize::Zeroizing;

use super::{Channel, Inbound, Peer, SendFuture, Transport, noise};
use crate::{Error, Result};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const QUEUE: usize = 256;

/// Noise-encrypted TCP between keepers. Only peers whose transport key is in the allowlist (current committees and
/// ceremonies) complete a connection; an outbound connection must reach exactly the expected key. One connection per
/// peer, used in both directions. Addresses use the form `tcp://host:port`.
pub struct TcpTransport {
    key: Zeroizing<[u8; 32]>,
    local: SocketAddr,
    allowed: RwLock<HashSet<[u8; 32]>>,
    connections: Mutex<HashMap<[u8; 32], mpsc::Sender<Vec<u8>>>>,
    inbound: mpsc::UnboundedSender<Inbound>,
}

impl TcpTransport {
    /// Listens on `addr` and returns the transport and its inbound frames.
    ///
    /// # Errors
    /// Bind failures.
    pub async fn bind(
        addr: SocketAddr,
        secret: &TransportSecret,
    ) -> Result<(Arc<Self>, mpsc::UnboundedReceiver<Inbound>)> {
        let listener = TcpListener::bind(addr).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let transport = Arc::new(Self {
            key: secret.to_bytes(),
            local: listener.local_addr()?,
            allowed: RwLock::new(HashSet::new()),
            connections: Mutex::new(HashMap::new()),
            inbound: tx,
        });
        let accepting = Arc::clone(&transport);
        tokio::spawn(async move {
            while let Ok((stream, from)) = listener.accept().await {
                let transport = Arc::clone(&accepting);
                tokio::spawn(async move {
                    if let Err(error) = transport.accept(stream).await {
                        tracing::debug!(%from, %error, "inbound peer connection closed");
                    }
                });
            }
        });
        Ok((transport, rx))
    }

    /// The bound address (advertise it as `tcp://<addr>`).
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    fn is_allowed(&self, key: &[u8; 32]) -> bool {
        self.allowed
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(key)
    }

    async fn accept(self: Arc<Self>, mut stream: TcpStream) -> Result<()> {
        let (channel, remote) = timeout(HANDSHAKE_TIMEOUT, noise::respond(&mut stream, &self.key))
            .await
            .map_err(|_| Error::Transport("handshake timed out".into()))??;
        if !self.is_allowed(&remote) {
            return Err(Error::Transport("peer not in any committee".into()));
        }
        self.run(stream, channel, remote);
        Ok(())
    }

    async fn connect(self: &Arc<Self>, peer: &Peer) -> Result<mpsc::Sender<Vec<u8>>> {
        let address = peer
            .addresses
            .iter()
            .find_map(|a| a.strip_prefix("tcp://"))
            .ok_or_else(|| Error::Transport("peer has no tcp address".into()))?;
        let mut stream = timeout(HANDSHAKE_TIMEOUT, TcpStream::connect(address))
            .await
            .map_err(|_| Error::Transport("connect timed out".into()))??;
        stream.set_nodelay(true)?;
        let (channel, remote) = timeout(HANDSHAKE_TIMEOUT, noise::initiate(&mut stream, &self.key))
            .await
            .map_err(|_| Error::Transport("handshake timed out".into()))??;
        if remote != peer.transport {
            return Err(Error::Transport(
                "peer presented an unexpected transport key".into(),
            ));
        }
        Ok(self.run(stream, channel, remote))
    }

    /// Spawns the reader and writer of an established connection and registers it for `remote`.
    fn run(
        self: &Arc<Self>,
        stream: TcpStream,
        channel: noise::Channel,
        remote: [u8; 32],
    ) -> mpsc::Sender<Vec<u8>> {
        let (mut reader, mut writer) = stream.into_split();
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(QUEUE);
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(remote, tx.clone());

        let write_channel = channel.clone();
        tokio::spawn(async move {
            let mut nonce = 0u64;
            while let Some(frame) = rx.recv().await {
                if write_channel
                    .write_frame(&mut writer, &mut nonce, &frame)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        let transport = Arc::clone(self);
        tokio::spawn(async move {
            let mut nonce = 0u64;
            while let Ok(bytes) = channel.read_frame(&mut reader, &mut nonce).await {
                if transport
                    .inbound
                    .send(Inbound {
                        channel: Channel::Key(remote),
                        bytes,
                    })
                    .is_err()
                {
                    break;
                }
            }
            transport
                .connections
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&remote);
        });
        tx
    }
}

impl Transport for Arc<TcpTransport> {
    fn send(&self, peer: Peer, frame: Vec<u8>) -> SendFuture<'_> {
        Box::pin(async move {
            let existing = self
                .connections
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&peer.transport)
                .cloned();
            if let Some(tx) = existing
                && tx.send(frame.clone()).await.is_ok()
            {
                return Ok(());
            }
            let tx = self.connect(&peer).await?;
            tx.send(frame)
                .await
                .map_err(|_| Error::Transport("connection closed".into()))
        })
    }

    fn set_allowed(&self, peers: Vec<Peer>) {
        *self.allowed.write().unwrap_or_else(PoisonError::into_inner) =
            peers.iter().map(|p| p.transport).collect();
    }
}
