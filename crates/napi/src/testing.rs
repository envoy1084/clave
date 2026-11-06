//! Test-only helpers that create or use secret shares (enabled by the `testing` feature, never in the app build).

use napi::{Result, bindgen_prelude::Buffer};
use napi_derive::napi;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::{
    SecretScalar, aztec,
    ecdh::{AddressSecretShare, EcdhContext},
    schnorr,
    shamir::{Polynomial, random_scalar},
};

use crate::{
    convert::{Point, base, base_hex, invalid, point, point_out, points, scalar, scalar_hex},
    ecdh::EcdhPartial,
};

/// One dealt viewing share.
#[napi(object)]
pub struct DealtShare {
    /// 1-based member index.
    pub index: u32,
    /// The viewing share `s_i`.
    pub share: String,
    /// `Y_i = s_i · G`.
    pub verification_share: Point,
}

/// A trusted-dealer committee (tests only; production committees come from the DKG).
#[napi(object)]
pub struct DealtCommittee {
    /// Group incoming viewing key.
    pub group_key: Point,
    /// Members' shares.
    pub shares: Vec<DealtShare>,
}

fn rng(seed: &Buffer) -> Result<ChaCha20Rng> {
    let seed: [u8; 32] = seed
        .as_ref()
        .try_into()
        .map_err(|_| invalid("seed must be 32 bytes"))?;
    Ok(ChaCha20Rng::from_seed(seed))
}

/// Deals a `threshold`-of-`members` sharing of `secret` (random if absent), deterministically from `seed`.
#[napi]
pub fn testing_deal_shares(
    threshold: u32,
    members: u32,
    seed: Buffer,
    secret: Option<String>,
) -> Result<DealtCommittee> {
    let mut rng = rng(&seed)?;
    let constant = match secret {
        Some(secret) => SecretScalar::new(scalar(&secret)?),
        None => random_scalar(&mut rng),
    };
    let polynomial = Polynomial::random(
        constant,
        usize::try_from(threshold).map_err(invalid)?,
        &mut rng,
    )
    .map_err(invalid)?;
    let commitments = polynomial.commitments().map_err(invalid)?;
    let shares = (1..=members)
        .map(|index| {
            Ok(DealtShare {
                index,
                share: scalar_hex(polynomial.evaluate(index).map_err(invalid)?.expose()),
                verification_share: point_out(&commitments.evaluate(index).map_err(invalid)?),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DealtCommittee {
        group_key: point_out(commitments.constant_term()),
        shares,
    })
}

/// A member's partials for `inputs`, as its keeper would compute them.
#[napi]
pub fn testing_partials(
    index: u32,
    share: String,
    epoch: u32,
    preaddress: String,
    ivpk: Point,
    inputs: Vec<Point>,
    seed: Buffer,
) -> Result<Vec<EcdhPartial>> {
    let mut rng = rng(&seed)?;
    let context =
        aztec::AddressContext::new(base(&preaddress)?, &point(&ivpk)?).map_err(invalid)?;
    let member = AddressSecretShare::new(index, &SecretScalar::new(scalar(&share)?), &context)
        .map_err(invalid)?;
    let ecdh_context = EcdhContext {
        vault: context.address(),
        epoch,
    };
    Ok(member
        .partials(&points(&inputs)?, &ecdh_context, &mut rng)
        .map_err(invalid)?
        .iter()
        .map(|p| EcdhPartial {
            point: point_out(&p.point),
            challenge: scalar_hex(&p.proof.challenge),
            response: scalar_hex(&p.proof.response),
        })
        .collect())
}

/// A Schnorr signature over `message` by `secretKey`, as limbs.
#[napi]
pub fn testing_sign(secret_key: String, message: String, seed: Buffer) -> Result<Vec<String>> {
    let mut rng = rng(&seed)?;
    let key = SecretScalar::new(scalar(&secret_key)?);
    let signature = schnorr::sign(&key, base(&message)?, &mut rng).map_err(invalid)?;
    Ok(signature.to_limbs().iter().map(base_hex).collect())
}

/// `secret · G` for a test secret key.
#[napi]
pub fn testing_public_key(secret_key: String) -> Result<Point> {
    let key = SecretScalar::new(scalar(&secret_key)?);
    Ok(point_out(
        &tecpan_crypto::Point::generator_mul_secret(&key).map_err(invalid)?,
    ))
}
