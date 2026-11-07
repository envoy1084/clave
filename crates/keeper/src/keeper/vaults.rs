use std::sync::Arc;

use clave_crypto::ecdh::AddressSecretShare;
use clave_wire::{messages::Manifest, types::WirePoint};
use serde::Serialize;

use super::{Keeper, State, VaultState, ceremonies::ReshareResult};
use crate::{
    Error, Result,
    committee::Committee,
    custody::{ShareSlot, StoredShare},
    events::Event,
    store::{ManifestStatus, from_json},
};

/// A vault as this keeper sees it.
#[derive(Clone, Debug, Serialize)]
pub struct VaultInfo {
    /// Current manifest.
    pub manifest: Manifest,
    /// `active`, `retired` or `removed`.
    pub status: &'static str,
    /// This keeper's member index, if a member.
    pub index: Option<u32>,
    /// Whether this keeper holds its share for the current epoch.
    pub serving: bool,
}

/// What an observed on-chain config meant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Observation {
    /// The config names the committee this keeper serves.
    Current,
    /// A completed reshare's committee became active.
    Activated,
    /// The config names a committee without this keeper; its share was destroyed.
    Removed,
    /// The config names a committee this keeper knows nothing about.
    Unknown,
}

impl Keeper {
    pub(super) fn load_vaults(&self) -> Result<()> {
        let mut vaults = Vec::new();
        for (manifest, status) in self.0.store.vaults()? {
            let committee = Committee::new(manifest)?;
            let share = if status == ManifestStatus::Active {
                self.load_share(&committee)?
            } else {
                None
            };
            vaults.push((
                committee.vault_bytes(),
                VaultState {
                    committee,
                    status,
                    share,
                },
            ));
        }
        let mut state = self
            .0
            .state
            .try_lock()
            .map_err(|_| Error::Storage("state busy at start".into()))?;
        state.vaults.extend(vaults);
        self.refresh_allowlist(&state);
        Ok(())
    }

    fn load_share(&self, committee: &Committee) -> Result<Option<Arc<AddressSecretShare>>> {
        let Some(member) = committee.member(&self.identity()) else {
            return Ok(None);
        };
        let slot = ShareSlot::Active(committee.vault_bytes(), committee.epoch());
        let share = StoredShare::load(&self.0.secrets, slot)?;
        if share.index() != member.index
            || share.group_public_key()
                != &clave_crypto::Point::try_from(&committee.manifest.group_key)?
        {
            return Err(Error::Invalid(
                "stored share does not belong to this committee".into(),
            ));
        }
        Ok(Some(Arc::new(AddressSecretShare::new(
            member.index,
            share.share(),
            &committee.context,
        )?)))
    }

    /// Binds a finished DKG to its vault: checks the manifest (structure, address, verification shares) against the
    /// DKG result and activates the share.
    ///
    /// # Errors
    /// [`Error::Invalid`] if the manifest does not match the DKG, custody or storage errors.
    pub async fn register_vault(&self, session: [u8; 32], manifest: Manifest) -> Result<VaultInfo> {
        let committee = Committee::new(manifest)?;
        let me = committee
            .member(&self.identity())
            .ok_or_else(|| Error::Invalid("this keeper is not in the manifest".into()))?;
        let share = StoredShare::load(&self.0.secrets, ShareSlot::Dkg(session))?;
        let shares: Vec<WirePoint> = share
            .verification_shares()?
            .iter()
            .map(WirePoint::from)
            .collect();
        let manifest_shares: Vec<WirePoint> = committee
            .manifest
            .members
            .iter()
            .map(|m| m.verification_share)
            .collect();
        if share.index() != me.index
            || share.threshold() != committee.threshold()
            || WirePoint::from(share.group_public_key()) != committee.manifest.group_key
            || shares != manifest_shares
        {
            return Err(Error::Invalid(
                "manifest does not match the DKG result".into(),
            ));
        }
        let vault = committee.vault_bytes();
        let mut state = self.0.state.lock().await;
        if state.vaults.contains_key(&vault) {
            return Err(Error::Invalid("vault already registered".into()));
        }
        self.0.secrets.rename(
            &ShareSlot::Dkg(session).name(),
            &ShareSlot::Active(vault, committee.epoch()).name(),
        )?;
        self.0
            .store
            .put_manifest(&committee.manifest, ManifestStatus::Active, self.now())?;
        let address_share = AddressSecretShare::new(me.index, share.share(), &committee.context)?;
        drop(share);
        let epoch = committee.epoch();
        state.vaults.insert(
            vault,
            VaultState {
                committee,
                status: ManifestStatus::Active,
                share: Some(Arc::new(address_share)),
            },
        );
        self.refresh_allowlist(&state);
        let info = Self::info(&state, &vault, &self.identity()).ok_or(Error::UnknownVault)?;
        drop(state);
        self.emit(Event::CommitteeActivated {
            vault: hex::encode(vault),
            epoch,
        });
        Ok(info)
    }

    /// Reacts to the vault config its engine observed on chain (after every sync): activates a completed reshare whose
    /// committee the config now names, or destroys this keeper's share if the config names a committee without it.
    ///
    /// # Errors
    /// Custody or storage errors.
    pub async fn observe_config(
        &self,
        vault: [u8; 32],
        version: u32,
        committee_commitment: [u8; 32],
    ) -> Result<Observation> {
        let now = self.now();
        self.0
            .store
            .observe(&vault, version, &committee_commitment, now)?;
        let mut state = self.0.state.lock().await;
        if let Some(current) = state.vaults.get(&vault)
            && current.status == ManifestStatus::Active
            && current.committee.commitment()? == committee_commitment
        {
            return Ok(Observation::Current);
        }
        for (session, result) in self.0.store.completed_reshares(&vault)? {
            let result: ReshareResult = from_json(&result)?;
            let Some(mut manifest) = result.manifest else {
                continue;
            };
            manifest.epoch = version;
            let committee = Committee::new(manifest)?;
            if committee.commitment()? == committee_commitment {
                self.activate(&mut state, session, committee)?;
                return Ok(Observation::Activated);
            }
        }
        let superseded = state
            .vaults
            .get(&vault)
            .is_some_and(|v| v.status == ManifestStatus::Active && version > v.committee.epoch());
        if superseded {
            self.remove(&mut state, vault)?;
            return Ok(Observation::Removed);
        }
        drop(state);
        self.emit(Event::ConfigMismatch {
            vault: hex::encode(vault),
            version,
        });
        Ok(Observation::Unknown)
    }

    fn activate(&self, state: &mut State, session: [u8; 32], committee: Committee) -> Result<()> {
        let vault = committee.vault_bytes();
        let epoch = committee.epoch();
        let me = committee
            .member(&self.identity())
            .ok_or_else(|| Error::Invalid("not in the new committee".into()))?;
        self.0.secrets.rename(
            &ShareSlot::Pending(session).name(),
            &ShareSlot::Active(vault, epoch).name(),
        )?;
        let share = StoredShare::load(&self.0.secrets, ShareSlot::Active(vault, epoch))?;
        let address_share = AddressSecretShare::new(me.index, share.share(), &committee.context)?;
        drop(share);
        self.0
            .store
            .put_manifest(&committee.manifest, ManifestStatus::Active, self.now())?;
        self.0
            .store
            .set_manifest_status(&vault, epoch, ManifestStatus::Active, self.now())?;
        self.0
            .store
            .update_ceremony(&session, "activated", None::<&()>, self.now())?;
        if let Some(old) = state.vaults.insert(
            vault,
            VaultState {
                committee,
                status: ManifestStatus::Active,
                share: Some(Arc::new(address_share)),
            },
        ) {
            self.0
                .secrets
                .destroy(&ShareSlot::Active(vault, old.committee.epoch()).name())?;
        }
        self.refresh_allowlist(state);
        self.emit(Event::CommitteeActivated {
            vault: hex::encode(vault),
            epoch,
        });
        Ok(())
    }

    fn remove(&self, state: &mut State, vault: [u8; 32]) -> Result<()> {
        let Some(current) = state.vaults.get_mut(&vault) else {
            return Ok(());
        };
        let epoch = current.committee.epoch();
        current.share = None;
        current.status = ManifestStatus::Removed;
        self.0
            .secrets
            .destroy(&ShareSlot::Active(vault, epoch).name())?;
        self.0
            .store
            .set_manifest_status(&vault, epoch, ManifestStatus::Removed, self.now())?;
        self.refresh_allowlist(state);
        self.emit(Event::Removed {
            vault: hex::encode(vault),
        });
        Ok(())
    }

    /// A vault's current state.
    ///
    /// # Errors
    /// [`Error::UnknownVault`].
    pub async fn vault(&self, vault: [u8; 32]) -> Result<VaultInfo> {
        let state = self.0.state.lock().await;
        Self::info(&state, &vault, &self.identity()).ok_or(Error::UnknownVault)
    }

    /// Every vault this keeper knows.
    pub async fn vaults(&self) -> Vec<VaultInfo> {
        let state = self.0.state.lock().await;
        let me = self.identity();
        state
            .vaults
            .keys()
            .filter_map(|v| Self::info(&state, v, &me))
            .collect()
    }

    fn info(state: &State, vault: &[u8; 32], me: &clave_wire::IdentityPublic) -> Option<VaultInfo> {
        let v = state.vaults.get(vault)?;
        Some(VaultInfo {
            manifest: v.committee.manifest.clone(),
            status: match v.status {
                ManifestStatus::Active => "active",
                ManifestStatus::Retired => "retired",
                ManifestStatus::Removed => "removed",
            },
            index: v.committee.member(me).map(|m| m.index),
            serving: v.share.is_some(),
        })
    }
}
