//! Message catalogue. Variants of [`Message`] are only ever appended (their order is the encoding); every body has
//! structural bounds checked on decode and conversions to `tecpan-crypto` types that validate the cryptographic
//! content.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tecpan_crypto::{
    Base, Point,
    aztec::AddressContext,
    dkg::{ProofOfKnowledge, Round1Package},
    ecdh::{DleqProof, Partial},
    envelope::Envelope,
    reshare::DealerPackage,
    schnorr,
    shamir::{Commitments, MemberIndex},
    vault::{self, MAX_SIGNERS},
};

use crate::{
    Error, IdentityPublic,
    codec::canonical,
    types::{
        Bytes64, WireField, WirePoint, WireScalar, field, parse_field, parse_points, parse_scalar,
        scalar,
    },
};

/// Most points in one ECDH request (a sync batch).
pub const MAX_ECDH_POINTS: usize = 4096;
/// Largest opaque Aztec execution payload in a proposal.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;
/// Largest human-readable proposal summary.
pub const MAX_SUMMARY_BYTES: usize = 4 * 1024;
/// Most network addresses per member, and the longest address.
pub const MAX_ADDRESSES: usize = 8;
/// Longest network address string.
pub const MAX_ADDRESS_BYTES: usize = 256;
/// Largest sealed share ciphertext (a 32-byte scalar plus the AEAD tag, with headroom).
pub const MAX_SEALED_SHARE_BYTES: usize = 256;
/// Largest relay payload.
pub const MAX_RELAY_BYTES: usize = 512 * 1024;
/// Largest forwarded frame in an [`Echo`].
pub const MAX_ECHO_BYTES: usize = 64 * 1024;
/// Most records of each kind in one [`SyncRequest`].
pub const MAX_SYNC_ITEMS: usize = 4096;
/// Domain tag of proposal ids.
pub const PROPOSAL_DOMAIN: &[u8] = b"tecpan.proposal.v0";

/// Every message kind. Append new variants at the end only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// Committee manifest.
    Manifest(Manifest),
    /// DKG round 1 broadcast.
    DkgRound1(DkgRound1),
    /// DKG round 2 private share.
    DkgRound2(SealedShare),
    /// DKG transcript confirmation.
    DkgConfirm(Confirm),
    /// Reshare dealer commitments.
    ReshareDeal(ReshareDeal),
    /// Reshare private sub-share.
    ReshareSubShare(SealedShare),
    /// Reshare transcript confirmation.
    ReshareConfirm(Confirm),
    /// A member accuses a dealer of a bad share.
    Complaint(Complaint),
    /// The accused reveals the disputed share.
    Justification(Justification),
    /// A ceremony is abandoned.
    Abort(Abort),
    /// Threshold ECDH request.
    EcdhRequest(EcdhRequest),
    /// Threshold ECDH partials.
    EcdhResponse(EcdhResponse),
    /// Spending proposal.
    Proposal(Proposal),
    /// A member's approval of a proposal.
    Approval(Approval),
    /// A proposal was executed.
    ExecutionReceipt(ExecutionReceipt),
    /// Ciphertext for an offline recipient, via a relay.
    RelayEnvelope(RelayEnvelope),
    /// A participant forwards a dealer's signed broadcast so every participant sees every version of it.
    Echo(Echo),
    /// A member lists what it holds so peers send what it is missing.
    SyncRequest(SyncRequest),
}

impl Message {
    /// Structural bounds (counts and lengths). Cryptographic validity is checked by the conversions.
    ///
    /// # Errors
    /// [`Error::OutOfBounds`] naming the field.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Manifest(m) => m.validate(),
            Self::DkgRound1(m) => bounded("commitments", m.commitments.len(), 1, MAX_SIGNERS),
            Self::ReshareDeal(m) => bounded("commitments", m.commitments.len(), 1, MAX_SIGNERS),
            Self::DkgRound2(m) | Self::ReshareSubShare(m) => {
                bounded("ciphertext", m.ciphertext.len(), 1, MAX_SEALED_SHARE_BYTES)
            }
            Self::EcdhRequest(m) => bounded("points", m.points.len(), 1, MAX_ECDH_POINTS),
            Self::EcdhResponse(m) => bounded("partials", m.partials.len(), 1, MAX_ECDH_POINTS),
            Self::Proposal(m) => {
                bounded("payload", m.payload.len(), 0, MAX_PAYLOAD_BYTES)?;
                bounded("summary", m.summary.len(), 0, MAX_SUMMARY_BYTES)
            }
            Self::RelayEnvelope(m) => bounded("sealed", m.sealed.len(), 1, MAX_RELAY_BYTES),
            Self::Echo(m) => bounded("frame", m.frame.len(), 1, MAX_ECHO_BYTES),
            Self::SyncRequest(m) => {
                bounded("proposals", m.proposals.len(), 0, MAX_SYNC_ITEMS)?;
                bounded("approvals", m.approvals.len(), 0, MAX_SYNC_ITEMS)?;
                bounded("receipts", m.receipts.len(), 0, MAX_SYNC_ITEMS)
            }
            Self::DkgConfirm(_)
            | Self::ReshareConfirm(_)
            | Self::Complaint(_)
            | Self::Justification(_)
            | Self::Abort(_)
            | Self::Approval(_)
            | Self::ExecutionReceipt(_) => Ok(()),
        }
    }
}

fn bounded(field: &'static str, len: usize, min: usize, max: usize) -> Result<(), Error> {
    if (min..=max).contains(&len) {
        Ok(())
    } else {
        Err(Error::OutOfBounds { field })
    }
}

// ----------------------------------------------------------------------------------------------------------------------
// Committee

/// The committee as every member's keeper knows it. Checked against the vault's on-chain commitments before use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Vault address.
    pub vault: WireField,
    /// Committee epoch: the vault config version at which this committee became active.
    pub epoch: u32,
    /// Spending threshold `k`.
    pub threshold: u32,
    /// Viewing threshold `t_v`.
    pub viewing_threshold: u32,
    /// Vault preaddress.
    pub preaddress: WireField,
    /// Group incoming viewing key `Ivpk`.
    pub group_key: WirePoint,
    /// Members in slot order; member `i` (1-based) is signer slot `i - 1`.
    pub members: Vec<Member>,
}

/// One committee member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// 1-based member index.
    pub index: MemberIndex,
    /// Grumpkin key the member signs approvals with.
    pub signer_key: WirePoint,
    /// Viewing verification share `Y_i`.
    pub verification_share: WirePoint,
    /// The member's keeper identity.
    pub identity: IdentityPublic,
    /// The keeper's X25519 transport key.
    pub transport: [u8; 32],
    /// Where to reach the keeper (multiaddrs or relay hints).
    pub addresses: Vec<String>,
}

impl Manifest {
    /// Structural checks: 1..=8 members indexed `1..=n` in order, distinct identities, thresholds in range, bounded
    /// addresses.
    ///
    /// # Errors
    /// [`Error::OutOfBounds`] naming the field.
    pub fn validate(&self) -> Result<(), Error> {
        let n = self.members.len();
        bounded("members", n, 1, MAX_SIGNERS)?;
        let n = u32::try_from(n).map_err(|_| Error::OutOfBounds { field: "members" })?;
        if !(1..=n).contains(&self.threshold) {
            return Err(Error::OutOfBounds { field: "threshold" });
        }
        if !(2..=n).contains(&self.viewing_threshold) {
            return Err(Error::OutOfBounds {
                field: "viewing_threshold",
            });
        }
        for (position, member) in self.members.iter().enumerate() {
            if usize::try_from(member.index).ok() != Some(position + 1) {
                return Err(Error::OutOfBounds {
                    field: "members.index",
                });
            }
            if self.members[..position]
                .iter()
                .any(|m| m.identity == member.identity)
            {
                return Err(Error::OutOfBounds {
                    field: "members.identity",
                });
            }
            bounded(
                "members.addresses",
                member.addresses.len(),
                0,
                MAX_ADDRESSES,
            )?;
            if member.addresses.iter().any(|a| a.len() > MAX_ADDRESS_BYTES) {
                return Err(Error::OutOfBounds {
                    field: "members.addresses",
                });
            }
        }
        Ok(())
    }

    /// Signer keys in slot order.
    ///
    /// # Errors
    /// [`Error::Crypto`] for an invalid point.
    pub fn signer_keys(&self) -> Result<Vec<Point>, Error> {
        self.members
            .iter()
            .map(|m| Point::try_from(&m.signer_key))
            .collect()
    }

    /// Verification shares in member order.
    ///
    /// # Errors
    /// [`Error::Crypto`] for an invalid point.
    pub fn verification_shares(&self) -> Result<Vec<Point>, Error> {
        self.members
            .iter()
            .map(|m| Point::try_from(&m.verification_share))
            .collect()
    }

    /// The vault's `signer_set_commitment` for this manifest (compare with the on-chain config).
    ///
    /// # Errors
    /// Point or size errors.
    pub fn signer_set_commitment(&self) -> Result<Base, Error> {
        Ok(vault::signer_set_commitment(&self.signer_keys()?)?)
    }

    /// The vault's `committee_commitment` for this manifest (compare with the on-chain config).
    ///
    /// # Errors
    /// Point or size errors.
    pub fn committee_commitment(&self) -> Result<Base, Error> {
        Ok(vault::committee_commitment(
            self.viewing_threshold,
            &self.verification_shares()?,
        )?)
    }

    /// The address context, after checking the vault address matches `preaddress` and `group_key`.
    ///
    /// # Errors
    /// [`Error::OutOfBounds`] (`vault`) on a mismatch, or point errors.
    pub fn address_context(&self) -> Result<AddressContext, Error> {
        let context = AddressContext::new(
            parse_field(&self.preaddress)?,
            &Point::try_from(&self.group_key)?,
        )?;
        if field(&context.address()) != self.vault {
            return Err(Error::OutOfBounds { field: "vault" });
        }
        Ok(context)
    }

    /// The member at `index`.
    #[must_use]
    pub fn member(&self, index: MemberIndex) -> Option<&Member> {
        self.members.iter().find(|m| m.index == index)
    }
}

// ----------------------------------------------------------------------------------------------------------------------
// Ceremonies

/// Which ceremony a complaint, justification or abort refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ceremony {
    /// Distributed key generation.
    Dkg,
    /// Committee resharing.
    Reshare,
}

/// DKG round 1 broadcast.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DkgRound1 {
    /// Dealer index.
    pub dealer: MemberIndex,
    /// Feldman commitments, constant term first.
    pub commitments: Vec<WirePoint>,
    /// Proof-of-knowledge commitment `R`.
    pub proof_commitment: WirePoint,
    /// Proof-of-knowledge response `mu`.
    pub proof_response: WireScalar,
}

impl From<&Round1Package> for DkgRound1 {
    fn from(package: &Round1Package) -> Self {
        Self {
            dealer: package.dealer,
            commitments: package
                .commitments
                .points()
                .iter()
                .map(WirePoint::from)
                .collect(),
            proof_commitment: WirePoint::from(&package.proof.commitment),
            proof_response: scalar(&package.proof.response),
        }
    }
}

impl TryFrom<&DkgRound1> for Round1Package {
    type Error = Error;

    fn try_from(value: &DkgRound1) -> Result<Self, Error> {
        Ok(Self {
            dealer: value.dealer,
            commitments: Commitments::new(parse_points(&value.commitments)?)?,
            proof: ProofOfKnowledge {
                commitment: Point::try_from(&value.proof_commitment)?,
                response: parse_scalar(&value.proof_response)?,
            },
        })
    }
}

/// A share sealed from dealer to recipient (DKG round 2 or reshare sub-share).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedShare {
    /// Dealer index.
    pub dealer: MemberIndex,
    /// Recipient index.
    pub recipient: MemberIndex,
    /// HPKE encapsulated key.
    pub encapsulated_key: [u8; 32],
    /// AEAD ciphertext.
    pub ciphertext: Vec<u8>,
}

impl SealedShare {
    /// Wraps an envelope.
    #[must_use]
    pub fn new(dealer: MemberIndex, recipient: MemberIndex, envelope: Envelope) -> Self {
        Self {
            dealer,
            recipient,
            encapsulated_key: envelope.encapsulated_key,
            ciphertext: envelope.ciphertext,
        }
    }

    /// The envelope to open with `tecpan_crypto::envelope::open_scalar`.
    #[must_use]
    pub fn envelope(&self) -> Envelope {
        Envelope {
            encapsulated_key: self.encapsulated_key,
            ciphertext: self.ciphertext.clone(),
        }
    }
}

/// Transcript confirmation: every member must report the same hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confirm {
    /// Transcript hash.
    pub transcript: Bytes64,
}

/// Reshare dealer commitments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReshareDeal {
    /// Dealer index in the current committee.
    pub dealer: MemberIndex,
    /// Feldman commitments; the constant term must be `λ_dealer · Y_dealer`.
    pub commitments: Vec<WirePoint>,
}

impl From<&DealerPackage> for ReshareDeal {
    fn from(package: &DealerPackage) -> Self {
        Self {
            dealer: package.dealer,
            commitments: package
                .commitments
                .points()
                .iter()
                .map(WirePoint::from)
                .collect(),
        }
    }
}

impl TryFrom<&ReshareDeal> for DealerPackage {
    type Error = Error;

    fn try_from(value: &ReshareDeal) -> Result<Self, Error> {
        Ok(Self {
            dealer: value.dealer,
            commitments: Commitments::new(parse_points(&value.commitments)?)?,
        })
    }
}

/// A member accuses `accused` of dealing it a share that does not match the accused's commitments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Complaint {
    /// Ceremony.
    pub ceremony: Ceremony,
    /// Accused dealer.
    pub accused: MemberIndex,
}

/// The accused's answer: the share it dealt to `complainant`, revealed for everyone to check against its commitments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Justification {
    /// Ceremony.
    pub ceremony: Ceremony,
    /// The complaining member.
    pub complainant: MemberIndex,
    /// The share the accused dealt to the complainant.
    pub share: WireScalar,
}

impl Justification {
    /// `true` if the revealed share matches the accused's commitments (the complaint was false); `false` if the
    /// accused dealt a bad share.
    ///
    /// # Errors
    /// [`Error::Crypto`] for a non-canonical share.
    pub fn holds(&self, accused_commitments: &Commitments) -> Result<bool, Error> {
        let share = tecpan_crypto::SecretScalar::new(parse_scalar(&self.share)?);
        Ok(accused_commitments.verify_share(self.complainant, &share))
    }
}

/// Why a ceremony was abandoned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortReason {
    /// A round timed out.
    Timeout,
    /// A complaint was upheld.
    Complaint,
    /// A member sent conflicting messages.
    Equivocation,
    /// A member cancelled.
    Cancelled,
}

/// A ceremony is abandoned; members discard its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abort {
    /// Ceremony.
    pub ceremony: Ceremony,
    /// Reason.
    pub reason: AbortReason,
}

// ----------------------------------------------------------------------------------------------------------------------
// Threshold ECDH

/// What the requested `address_secret · P` is for (audit logs and rate limits).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EcdhPurpose {
    /// Note or message decryption (ephemeral keys).
    NoteDecryption,
    /// Handshake shared secret.
    Handshake,
    /// Recipient-side tagging secret (sender address points).
    RecipientTag,
    /// Sender-side tagging secret (recipient address points).
    SenderTag,
}

/// A batch of points to multiply by the vault's address secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcdhRequest {
    /// Purpose.
    pub purpose: EcdhPurpose,
    /// Input points.
    pub points: Vec<WirePoint>,
}

/// One partial with its DLEQ proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WirePartial {
    /// `D_i`.
    pub point: WirePoint,
    /// DLEQ challenge.
    pub challenge: WireScalar,
    /// DLEQ response.
    pub response: WireScalar,
}

/// A member's partials for a request, in request order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcdhResponse {
    /// The request's header id.
    pub request: [u8; 16],
    /// Responding member.
    pub member: MemberIndex,
    /// Partials, one per requested point.
    pub partials: Vec<WirePartial>,
}

impl EcdhResponse {
    /// Wraps a member's partials.
    #[must_use]
    pub fn new(request: [u8; 16], member: MemberIndex, partials: &[Partial]) -> Self {
        let partials = partials
            .iter()
            .map(|p| WirePartial {
                point: WirePoint::from(&p.point),
                challenge: scalar(&p.proof.challenge),
                response: scalar(&p.proof.response),
            })
            .collect();
        Self {
            request,
            member,
            partials,
        }
    }

    /// The partials, validated as points and scalars (proofs are checked with `ecdh::verify_partial`).
    ///
    /// # Errors
    /// [`Error::Crypto`] for invalid encodings.
    pub fn partials(&self) -> Result<Vec<Partial>, Error> {
        self.partials
            .iter()
            .map(|p| {
                Ok(Partial {
                    index: self.member,
                    point: Point::try_from(&p.point)?,
                    proof: DleqProof {
                        challenge: parse_scalar(&p.challenge)?,
                        response: parse_scalar(&p.response)?,
                    },
                })
            })
            .collect()
    }
}

// ----------------------------------------------------------------------------------------------------------------------
// Spending

/// A spending proposal. Its id is derived from its content, so an approval cannot point at a different proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    /// Config version the approvals are collected under.
    pub config_version: u32,
    /// The hash members approve: the entrypoint payload hash computed by the proposer's engine (each approver
    /// recomputes it from `payload` before signing).
    pub outer_hash: WireField,
    /// Approval deadline (unix seconds).
    pub expires_at: u64,
    /// Entrypoint `tx_nonce` (cancellation burns it).
    pub tx_nonce: WireField,
    /// Fee payment method passed to `entrypoint`.
    pub fee_method: u8,
    /// Opaque Aztec execution payload, serialised by the engine.
    pub payload: Vec<u8>,
    /// Human-readable summary of the calls.
    pub summary: String,
    /// Proposing member.
    pub proposer: MemberIndex,
}

impl Proposal {
    /// `SHA-256("tecpan.proposal.v0" ‖ canonical(proposal))`.
    ///
    /// # Errors
    /// [`Error::Malformed`] if the proposal cannot be encoded.
    pub fn id(&self) -> Result<[u8; 32], Error> {
        let mut hasher = Sha256::new();
        hasher.update(PROPOSAL_DOMAIN);
        hasher.update(canonical(self)?);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hasher.finalize());
        Ok(out)
    }

    /// The message signers sign for this proposal on `vault`.
    ///
    /// # Errors
    /// [`Error::Crypto`] for a non-canonical hash.
    pub fn approval_message(&self, vault: Base) -> Result<Base, Error> {
        Ok(vault::approval_message(
            vault,
            parse_field(&self.outer_hash)?,
            self.config_version,
            self.expires_at,
        ))
    }
}

/// A member's approval: the Schnorr signature over the proposal's approval message, as contract limbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    /// Approved proposal id.
    pub proposal: [u8; 32],
    /// Signer slot (member index - 1).
    pub slot: u32,
    /// `[s.lo, s.hi, e.lo, e.hi]`.
    pub signature: [WireField; 4],
}

impl Approval {
    /// Wraps a signature.
    #[must_use]
    pub fn new(proposal: [u8; 32], slot: u32, signature: &schnorr::Signature) -> Self {
        let limbs = signature.to_limbs();
        Self {
            proposal,
            slot,
            signature: [
                field(&limbs[0]),
                field(&limbs[1]),
                field(&limbs[2]),
                field(&limbs[3]),
            ],
        }
    }

    /// Whether this approval is for `proposal` and carries a valid signature by `signer_key` over its approval
    /// message on `vault`.
    ///
    /// # Errors
    /// [`Error::Crypto`] for malformed limbs or hashes.
    pub fn verify(
        &self,
        proposal: &Proposal,
        vault: Base,
        signer_key: &Point,
    ) -> Result<bool, Error> {
        if self.proposal != proposal.id()? {
            return Ok(false);
        }
        let limbs = [
            parse_field(&self.signature[0])?,
            parse_field(&self.signature[1])?,
            parse_field(&self.signature[2])?,
            parse_field(&self.signature[3])?,
        ];
        let signature = schnorr::Signature::from_limbs(&limbs)?;
        Ok(schnorr::verify(
            signer_key,
            proposal.approval_message(vault)?,
            &signature,
        ))
    }
}

/// A proposal was executed in `tx_hash`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Executed proposal id.
    pub proposal: [u8; 32],
    /// Aztec transaction hash.
    pub tx_hash: WireField,
}

// ----------------------------------------------------------------------------------------------------------------------
// Relay

/// Ciphertext held by a relay for an offline recipient. The relay sees only the opaque recipient id and expiry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayEnvelope {
    /// Opaque recipient mailbox id.
    pub recipient: [u8; 32],
    /// Drop after this unix time.
    pub expires_at: u64,
    /// Sealed bytes (an encoded, sealed `SignedMessage`).
    pub sealed: Vec<u8>,
}

// ----------------------------------------------------------------------------------------------------------------------
// Echo broadcast and sync

/// A dealer's signed ceremony broadcast (DKG round 1 or reshare deal), forwarded verbatim by another participant.
/// Two different packages signed by the same dealer prove equivocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Echo {
    /// The dealer's encoded `SignedMessage`.
    pub frame: Vec<u8>,
}

/// An approval a member holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ApprovalRef {
    /// Proposal id.
    pub proposal: [u8; 32],
    /// Signer slot.
    pub slot: u32,
}

/// What a member already holds for a vault; peers answer with the records it lacks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Proposal ids.
    pub proposals: Vec<[u8; 32]>,
    /// Approvals by proposal and slot.
    pub approvals: Vec<ApprovalRef>,
    /// Executed proposal ids.
    pub receipts: Vec<[u8; 32]>,
}
