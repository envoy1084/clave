//! Aztec address derivation for a vault whose incoming viewing key is shared.

use clave_crypto::{Scalar, aztec};
use napi::Result;
use napi_derive::napi;

use crate::convert::{Point, base, base_hex, invalid, point, point_out};

/// Public data fixing how viewing shares map to shares of the address secret.
#[napi(object)]
pub struct AddressContext {
    /// The vault address.
    pub address: String,
    /// `1` or `-1`: the sign Aztec applies to the address secret.
    pub sign: i32,
    /// `preaddress · G + Ivpk`, normalised to positive `y`.
    pub address_point: Point,
}

/// Aztec's preaddress `Poseidon2([publicKeysHash, partialAddress], 993442748)`.
#[napi]
pub fn preaddress(public_keys_hash: String, partial_address: String) -> Result<String> {
    Ok(base_hex(&aztec::preaddress(
        base(&public_keys_hash)?,
        base(&partial_address)?,
    )))
}

/// The address context of a preaddress and group incoming viewing key.
#[napi]
pub fn address_context(preaddress: String, ivpk: Point) -> Result<AddressContext> {
    let context =
        aztec::AddressContext::new(base(&preaddress)?, &point(&ivpk)?).map_err(invalid)?;
    Ok(AddressContext {
        address: base_hex(&context.address()),
        sign: if context.sign == Scalar::from(1u64) {
            1
        } else {
            -1
        },
        address_point: point_out(&context.address_point),
    })
}

/// Each member's public address-secret share `A_i` (what DLEQ proofs are checked against), from the verification
/// shares `Y_i` in member order.
#[napi]
pub fn address_public_shares(
    preaddress: String,
    ivpk: Point,
    verification_shares: Vec<Point>,
) -> Result<Vec<Point>> {
    let context =
        aztec::AddressContext::new(base(&preaddress)?, &point(&ivpk)?).map_err(invalid)?;
    verification_shares
        .iter()
        .map(|share| {
            Ok(point_out(
                &context
                    .address_verification_share(&point(share)?)
                    .map_err(invalid)?,
            ))
        })
        .collect()
}
