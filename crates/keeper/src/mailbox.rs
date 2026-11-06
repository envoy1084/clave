//! Store-and-forward for members who are offline: replication records a peer could not receive directly are sealed into
//! its pairwise mailbox at one or more untrusted relays (see `tecpan_crypto::mailbox`).

use std::{
    future::Future,
    pin::Pin,
    sync::{Mutex, PoisonError},
};

use tecpan_wire::{
    mailbox::{StoredEnvelope, envelope_id},
    messages::RelayEnvelope,
};

use crate::Result;

/// Boxed mailbox future.
pub type MailboxFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/// Where mailboxes live: relays over libp2p, or memory in tests.
pub trait MailboxService: Send + Sync + 'static {
    /// Stores an envelope (best effort across relays).
    fn deposit(&self, envelope: RelayEnvelope) -> MailboxFuture<'_, ()>;
    /// Envelopes held for `mailboxes`.
    fn fetch(&self, mailboxes: Vec<[u8; 32]>) -> MailboxFuture<'_, Vec<StoredEnvelope>>;
    /// Deletes delivered envelopes.
    fn delete(&self, ids: Vec<[u8; 32]>) -> MailboxFuture<'_, ()>;
}

/// In-memory mailbox shared by test keepers.
#[derive(Default)]
pub struct MemoryMailbox {
    envelopes: Mutex<Vec<StoredEnvelope>>,
}

impl MemoryMailbox {
    /// Number of envelopes held.
    pub fn len(&self) -> usize {
        self.envelopes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether the mailbox holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every stored envelope (to check that only ciphertext is held).
    pub fn snapshot(&self) -> Vec<StoredEnvelope> {
        self.envelopes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl MailboxService for MemoryMailbox {
    fn deposit(&self, envelope: RelayEnvelope) -> MailboxFuture<'_, ()> {
        let result = envelope_id(&envelope).map(|id| {
            let mut envelopes = self
                .envelopes
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !envelopes.iter().any(|e| e.id == id) {
                envelopes.push(StoredEnvelope { id, envelope });
            }
        });
        Box::pin(async move { Ok(result?) })
    }

    fn fetch(&self, mailboxes: Vec<[u8; 32]>) -> MailboxFuture<'_, Vec<StoredEnvelope>> {
        let found = self
            .envelopes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|e| mailboxes.contains(&e.envelope.recipient))
            .cloned()
            .collect();
        Box::pin(async move { Ok(found) })
    }

    fn delete(&self, ids: Vec<[u8; 32]>) -> MailboxFuture<'_, ()> {
        self.envelopes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|e| !ids.contains(&e.id));
        Box::pin(async { Ok(()) })
    }
}
