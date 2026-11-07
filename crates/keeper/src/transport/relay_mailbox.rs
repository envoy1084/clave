use std::{collections::HashSet, sync::Arc};

use clave_wire::{
    mailbox::{self, MailboxRequest, MailboxResponse, StoredEnvelope},
    messages::RelayEnvelope,
};

use super::P2pTransport;
use crate::{
    Error, Result,
    mailbox::{MailboxFuture, MailboxService},
};

/// Mailboxes at the keeper's configured relays: deposits go to every relay (stored if any accepts), fetches merge
/// every relay's answer, deletes go everywhere.
pub struct RelayMailbox {
    transport: Arc<P2pTransport>,
}

impl RelayMailbox {
    /// Mailboxes at `transport`'s relays.
    #[must_use]
    pub fn new(transport: Arc<P2pTransport>) -> Self {
        Self { transport }
    }

    async fn ask(&self, request: &MailboxRequest) -> Vec<Result<MailboxResponse>> {
        let Ok(bytes) = mailbox::encode(request) else {
            return vec![Err(Error::Invalid("mailbox request".into()))];
        };
        let mut answers = Vec::new();
        for relay in self.transport.relays() {
            let answer = self
                .transport
                .mailbox(*relay, bytes.clone())
                .await
                .and_then(|response| Ok(mailbox::decode_response(&response)?));
            answers.push(answer);
        }
        answers
    }
}

impl MailboxService for RelayMailbox {
    fn deposit(&self, envelope: RelayEnvelope) -> MailboxFuture<'_, ()> {
        Box::pin(async move {
            let answers = self.ask(&MailboxRequest::Deposit(envelope)).await;
            if answers
                .iter()
                .any(|a| matches!(a, Ok(MailboxResponse::Stored { .. })))
            {
                Ok(())
            } else {
                Err(Error::Transport(format!(
                    "no relay stored the envelope: {answers:?}"
                )))
            }
        })
    }

    fn fetch(&self, mailboxes: Vec<[u8; 32]>) -> MailboxFuture<'_, Vec<StoredEnvelope>> {
        Box::pin(async move {
            let mut seen = HashSet::new();
            let mut envelopes = Vec::new();
            for answer in self.ask(&MailboxRequest::Fetch { mailboxes }).await {
                match answer {
                    Ok(MailboxResponse::Envelopes(found)) => {
                        envelopes.extend(found.into_iter().filter(|e| seen.insert(e.id)));
                    }
                    Ok(other) => tracing::debug!(?other, "unexpected mailbox answer"),
                    Err(error) => tracing::debug!(%error, "mailbox fetch failed"),
                }
            }
            Ok(envelopes)
        })
    }

    fn delete(&self, ids: Vec<[u8; 32]>) -> MailboxFuture<'_, ()> {
        Box::pin(async move {
            self.ask(&MailboxRequest::Delete { ids }).await;
            Ok(())
        })
    }
}
