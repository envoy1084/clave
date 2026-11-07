//! A vault committee: a checked manifest and the crypto context derived from it.

use clave_crypto::{
    Base, Point,
    aztec::AddressContext,
    ecdh::EcdhContext,
    shamir::{Commitments, MemberIndex},
    vault,
};
use clave_wire::{
    IdentityPublic,
    messages::{Manifest, Member},
    types::{field, parse_field},
};

use crate::{Error, Result, transport::Peer};

/// A vault committee whose manifest has been checked: structure, vault address against `preaddress` and `Ivpk`, and
/// verification shares lying on one degree-`t_v - 1` polynomial whose constant term is `Ivpk`.
#[derive(Clone, Debug)]
pub struct Committee {
    /// The manifest.
    pub manifest: Manifest,
    /// Vault address as a field element.
    pub vault: Base,
    /// Address-secret context.
    pub context: AddressContext,
    /// Group commitments rebuilt from the verification shares.
    pub commitments: Commitments,
    /// Public address-secret shares `A_i`, in member order.
    pub address_shares: Vec<Point>,
}

impl Committee {
    /// Validates `manifest`.
    ///
    /// # Errors
    /// [`Error::Wire`] / [`Error::Crypto`] / [`Error::Invalid`] naming what is inconsistent.
    pub fn new(manifest: Manifest) -> Result<Self> {
        manifest.validate()?;
        let context = manifest.address_context()?;
        let shares = manifest.verification_shares()?;
        let evaluations: Vec<(MemberIndex, Point)> = manifest
            .members
            .iter()
            .map(|m| m.index)
            .zip(shares.iter().copied())
            .collect();
        let threshold = usize::try_from(manifest.viewing_threshold)
            .map_err(|_| Error::Invalid("viewing threshold".into()))?;
        let commitments = Commitments::interpolate(threshold, &evaluations)?;
        if commitments.constant_term() != &Point::try_from(&manifest.group_key)? {
            return Err(Error::Invalid(
                "verification shares do not match the group key".into(),
            ));
        }
        let address_shares = shares
            .iter()
            .map(|y| context.address_verification_share(y))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            vault: parse_field(&manifest.vault)?,
            manifest,
            context,
            commitments,
            address_shares,
        })
    }

    /// Vault address bytes.
    #[must_use]
    pub fn vault_bytes(&self) -> [u8; 32] {
        self.manifest.vault
    }

    /// Committee epoch.
    #[must_use]
    pub fn epoch(&self) -> u32 {
        self.manifest.epoch
    }

    /// Viewing threshold `t_v`.
    #[must_use]
    pub fn threshold(&self) -> usize {
        usize::try_from(self.manifest.viewing_threshold).unwrap_or(usize::MAX)
    }

    /// The member with `identity`.
    #[must_use]
    pub fn member(&self, identity: &IdentityPublic) -> Option<&Member> {
        self.manifest
            .members
            .iter()
            .find(|m| &m.identity == identity)
    }

    /// `A_i` of member `index`.
    #[must_use]
    pub fn address_share(&self, index: MemberIndex) -> Option<Point> {
        self.address_shares
            .get(usize::try_from(index).ok()?.checked_sub(1)?)
            .copied()
    }

    /// What partials are bound to.
    #[must_use]
    pub fn ecdh_context(&self) -> EcdhContext {
        EcdhContext {
            vault: self.vault,
            epoch: self.manifest.epoch,
        }
    }

    /// The contract's `committee_commitment` for this committee.
    ///
    /// # Errors
    /// Point errors (not expected for a validated manifest).
    pub fn commitment(&self) -> Result<[u8; 32]> {
        Ok(field(&vault::committee_commitment(
            self.manifest.viewing_threshold,
            &self.manifest.verification_shares()?,
        )?))
    }

    /// Every member except `me`, as transport peers.
    #[must_use]
    pub fn peers_except(&self, me: &IdentityPublic) -> Vec<Peer> {
        self.manifest
            .members
            .iter()
            .filter(|m| &m.identity != me)
            .map(Peer::from)
            .collect()
    }
}
