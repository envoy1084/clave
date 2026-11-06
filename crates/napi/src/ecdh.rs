//! Threshold ECDH: verifies members' partials (DLEQ) and combines them into `address_secret · P`.

use napi::Result;
use napi_derive::napi;
use tecpan_crypto::{
    aztec,
    ecdh::{self, DleqProof, EcdhContext, Partial},
};

use crate::convert::{Point, base, invalid, point, point_out, points, scalar};

/// One partial `D_i = x_i · P` with its DLEQ proof.
#[napi(object)]
#[derive(Clone)]
pub struct EcdhPartial {
    /// `D_i`.
    pub point: Point,
    /// DLEQ challenge.
    pub challenge: String,
    /// DLEQ response.
    pub response: String,
}

/// A member's partials, one per input point in request order.
#[napi(object)]
pub struct MemberPartials {
    /// 1-based member index.
    pub member: u32,
    /// Partials.
    pub partials: Vec<EcdhPartial>,
}

/// A threshold ECDH batch to verify and combine.
#[napi(object)]
pub struct ThresholdEcdhRequest {
    /// Committee epoch (vault config version) the partials are bound to.
    pub epoch: u32,
    /// Vault preaddress.
    pub preaddress: String,
    /// Group incoming viewing key.
    pub ivpk: Point,
    /// Verification shares `Y_i` in member order (member `i` at position `i - 1`).
    pub verification_shares: Vec<Point>,
    /// Viewing threshold `t_v`.
    pub threshold: u32,
    /// Input points.
    pub inputs: Vec<Point>,
    /// Members' responses.
    pub responses: Vec<MemberPartials>,
}

/// Combined results.
#[napi(object)]
pub struct ThresholdEcdhResult {
    /// `address_secret · input` for each input, in order.
    pub points: Vec<Point>,
    /// Members whose response was malformed or carried an invalid proof.
    pub invalid_members: Vec<u32>,
}

/// Verifies every partial and combines `t_v` valid ones per input. Fails if any input lacks `t_v` valid partials.
#[napi]
pub fn verify_and_combine(request: ThresholdEcdhRequest) -> Result<ThresholdEcdhResult> {
    let context = aztec::AddressContext::new(base(&request.preaddress)?, &point(&request.ivpk)?)
        .map_err(invalid)?;
    let ecdh_context = EcdhContext {
        vault: context.address(),
        epoch: request.epoch,
    };
    let publics = points(&request.verification_shares)?
        .iter()
        .map(|share| context.address_verification_share(share).map_err(invalid))
        .collect::<Result<Vec<_>>>()?;
    let inputs = points(&request.inputs)?;
    let threshold = usize::try_from(request.threshold).map_err(invalid)?;

    let mut invalid_members = Vec::new();
    let mut responses: Vec<Vec<Partial>> = Vec::new();
    for response in &request.responses {
        match decode_partials(response, inputs.len()) {
            Some(partials) => responses.push(partials),
            None => invalid_members.push(response.member),
        }
    }
    let member_public = |index: u32| {
        publics
            .get(usize::try_from(index).ok()?.checked_sub(1)?)
            .copied()
    };

    let mut combined = Vec::with_capacity(inputs.len());
    for (position, input) in inputs.iter().enumerate() {
        let partials: Vec<Partial> = responses.iter().map(|r| r[position]).collect();
        let (shared, rejected) =
            ecdh::verify_and_combine(&partials, input, member_public, threshold, &ecdh_context)
                .map_err(invalid)?;
        for member in rejected {
            if !invalid_members.contains(&member) {
                invalid_members.push(member);
            }
        }
        combined.push(point_out(&shared));
    }
    invalid_members.sort_unstable();
    Ok(ThresholdEcdhResult {
        points: combined,
        invalid_members,
    })
}

fn decode_partials(response: &MemberPartials, expected: usize) -> Option<Vec<Partial>> {
    if response.partials.len() != expected {
        return None;
    }
    response
        .partials
        .iter()
        .map(|p| {
            Some(Partial {
                index: response.member,
                point: point(&p.point).ok()?,
                proof: DleqProof {
                    challenge: scalar(&p.challenge).ok()?,
                    response: scalar(&p.response).ok()?,
                },
            })
        })
        .collect()
}
