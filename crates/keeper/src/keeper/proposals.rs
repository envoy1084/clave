use tecpan_wire::{
    Header, IdentityPublic, Message, decode_body, encode_body,
    messages::{Approval, ExecutionReceipt, Proposal},
    types::parse_field,
};

use super::Keeper;

/// Approvals held while their proposal has not arrived yet.
const MAX_ORPHANS: usize = 1024;
use crate::{Error, Result, events::Event, store::ManifestStatus};

impl Keeper {
    /// Stores a proposal for `vault` and pushes it to the other members. Returns its id.
    ///
    /// # Errors
    /// [`Error::UnknownVault`], decoding errors.
    pub async fn put_proposal(&self, vault: [u8; 32], body: &[u8]) -> Result<[u8; 32]> {
        let Message::Proposal(proposal) = decode_body(body)? else {
            return Err(Error::Invalid("not a proposal".into()));
        };
        let id = self.store_proposal(vault, &proposal)?;
        self.adopt_orphans(vault, id).await;
        self.broadcast(vault, Message::Proposal(proposal)).await?;
        Ok(id)
    }

    /// Verifies and stores an approval (its signature must be the slot's signer over the proposal's approval message)
    /// and pushes it to the other members.
    ///
    /// # Errors
    /// [`Error::Invalid`] for an unknown proposal or invalid signature.
    pub async fn put_approval(&self, vault: [u8; 32], body: &[u8]) -> Result<()> {
        let Message::Approval(approval) = decode_body(body)? else {
            return Err(Error::Invalid("not an approval".into()));
        };
        self.store_approval(vault, &approval).await?;
        self.broadcast(vault, Message::Approval(approval)).await
    }

    /// Records that `proposal` executed in `tx_hash` and tells the other members.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn put_receipt(
        &self,
        vault: [u8; 32],
        proposal: [u8; 32],
        tx_hash: [u8; 32],
    ) -> Result<()> {
        self.0
            .store
            .put_receipt(&proposal, &vault, &tx_hash, self.now())?;
        self.broadcast(
            vault,
            Message::ExecutionReceipt(ExecutionReceipt { proposal, tx_hash }),
        )
        .await
    }

    /// Checks a vault transaction the engine observed against known executions; an unknown one raises
    /// [`Event::UnexpectedSpend`]. Returns whether it was known.
    ///
    /// # Errors
    /// Storage errors.
    pub fn observe_tx(&self, vault: [u8; 32], tx_hash: [u8; 32]) -> Result<bool> {
        let known = self.0.store.has_receipt(&vault, &tx_hash)?;
        if !known {
            self.emit(Event::UnexpectedSpend {
                vault: hex::encode(vault),
                tx_hash: hex::encode(tx_hash),
            });
        }
        Ok(known)
    }

    /// Proposal bodies of `vault`.
    ///
    /// # Errors
    /// Storage errors.
    pub fn proposals(&self, vault: [u8; 32]) -> Result<Vec<Vec<u8>>> {
        self.0.store.proposals(&vault)
    }

    /// A proposal body.
    ///
    /// # Errors
    /// Storage errors.
    pub fn proposal(&self, id: [u8; 32]) -> Result<Option<Vec<u8>>> {
        Ok(self.0.store.proposal(&id)?.map(|(_, body)| body))
    }

    /// Approval bodies of a proposal.
    ///
    /// # Errors
    /// Storage errors.
    pub fn approvals(&self, proposal: [u8; 32]) -> Result<Vec<Vec<u8>>> {
        self.0.store.approvals(&proposal)
    }

    /// The ECDH audit log of `vault`, newest first.
    ///
    /// # Errors
    /// Storage errors.
    pub fn audit_log(&self, vault: [u8; 32], limit: u32) -> Result<Vec<crate::store::AuditEntry>> {
        self.0.store.audit_log(&vault, limit)
    }

    pub(super) async fn on_replicated(&self, from: IdentityPublic, header: Header, body: Message) {
        let vault = header.scope;
        let member = {
            let state = self.0.state.lock().await;
            state
                .vaults
                .get(&vault)
                .filter(|v| v.status == ManifestStatus::Active)
                .is_some_and(|v| v.committee.member(&from).is_some())
        };
        if !member {
            return;
        }
        let result = match body {
            Message::Proposal(proposal) => match self.store_proposal(vault, &proposal) {
                Ok(id) => {
                    self.adopt_orphans(vault, id).await;
                    Ok(())
                }
                Err(error) => Err(error),
            },
            Message::Approval(approval) => {
                if self
                    .0
                    .store
                    .proposal(&approval.proposal)
                    .ok()
                    .flatten()
                    .is_none()
                {
                    // Arrived before its proposal (records are processed concurrently): keep it for later.
                    let mut state = self.0.state.lock().await;
                    if state.orphans.values().map(Vec::len).sum::<usize>() < MAX_ORPHANS {
                        state
                            .orphans
                            .entry(approval.proposal)
                            .or_default()
                            .push((vault, approval));
                    }
                    return;
                }
                self.store_approval(vault, &approval).await
            }
            Message::ExecutionReceipt(receipt) => {
                self.0
                    .store
                    .put_receipt(&receipt.proposal, &vault, &receipt.tx_hash, self.now())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            tracing::debug!(%error, "dropped a replicated record");
        }
    }

    /// Stores approvals that arrived before proposal `id`.
    async fn adopt_orphans(&self, vault: [u8; 32], id: [u8; 32]) {
        let waiting = self
            .0
            .state
            .lock()
            .await
            .orphans
            .remove(&id)
            .unwrap_or_default();
        for (orphan_vault, approval) in waiting {
            if orphan_vault == vault
                && let Err(error) = self.store_approval(vault, &approval).await
            {
                tracing::debug!(%error, "dropped an early approval");
            }
        }
    }

    fn store_proposal(&self, vault: [u8; 32], proposal: &Proposal) -> Result<[u8; 32]> {
        let id = proposal.id()?;
        let body = encode_body(&Message::Proposal(proposal.clone()))?;
        if self.0.store.put_proposal(&id, &vault, &body, self.now())? {
            self.emit(Event::Proposal {
                vault: hex::encode(vault),
                id: hex::encode(id),
            });
        }
        Ok(id)
    }

    async fn store_approval(&self, vault: [u8; 32], approval: &Approval) -> Result<()> {
        let (stored_vault, body) = self
            .0
            .store
            .proposal(&approval.proposal)?
            .ok_or_else(|| Error::Invalid("approval for an unknown proposal".into()))?;
        if stored_vault != vault {
            return Err(Error::Invalid(
                "approval for another vault's proposal".into(),
            ));
        }
        let Message::Proposal(proposal) = decode_body(&body)? else {
            return Err(Error::Storage("stored proposal is not a proposal".into()));
        };
        let signer_key = {
            let state = self.0.state.lock().await;
            let committee = &state
                .vaults
                .get(&vault)
                .ok_or(Error::UnknownVault)?
                .committee;
            let slot = usize::try_from(approval.slot).map_err(|_| Error::Invalid("slot".into()))?;
            let member = committee
                .manifest
                .members
                .get(slot)
                .ok_or_else(|| Error::Invalid("approval for an empty slot".into()))?;
            tecpan_crypto::Point::try_from(&member.signer_key)?
        };
        if !approval.verify(&proposal, parse_field(&vault)?, &signer_key)? {
            return Err(Error::Invalid("invalid approval signature".into()));
        }
        let body = encode_body(&Message::Approval(*approval))?;
        if self
            .0
            .store
            .put_approval(&approval.proposal, approval.slot, &body, self.now())?
        {
            self.emit(Event::Approval {
                proposal: hex::encode(approval.proposal),
                slot: approval.slot,
            });
        }
        Ok(())
    }

    /// Signs `body` for `vault` and sends it to every other member.
    async fn broadcast(&self, vault: [u8; 32], body: Message) -> Result<()> {
        let (peers, epoch) = {
            let state = self.0.state.lock().await;
            let v = state.vaults.get(&vault).ok_or(Error::UnknownVault)?;
            (
                v.committee.peers_except(&self.identity()),
                v.committee.epoch(),
            )
        };
        let (frame, _) = self.sign(vault, epoch, body)?;
        for peer in peers {
            let keeper = self.clone();
            let frame = frame.clone();
            tokio::spawn(async move {
                // A member that cannot be reached gets the record through its relay mailbox (and the next sync).
                if !keeper.send(peer.clone(), frame.clone()).await
                    && let Err(error) = keeper.deposit(&peer, &frame).await
                {
                    tracing::debug!(%error, "mailbox deposit failed");
                }
            });
        }
        Ok(())
    }
}
