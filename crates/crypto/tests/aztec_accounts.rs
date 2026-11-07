//! Byte-compatibility with Aztec 6.0.0 key derivation: preaddress, address, address-secret sign, ECDH, app siloing.

mod common;

use clave_crypto::{Point, SecretScalar, aztec};
use common::{assert_point, base, point, scalar, vectors};

#[test]
fn account_derivation_matches_aztec_for_both_signs() {
    let v = vectors();
    let accounts = v["accounts"].as_array().unwrap();
    assert_eq!(
        accounts
            .iter()
            .filter(|a| a["addressSecretNegated"].as_bool().unwrap())
            .count(),
        3
    );

    for a in accounts {
        let ivsk = SecretScalar::new(scalar(&a["ivsk"]));
        let group_key = Point::generator_mul_secret(&ivsk).unwrap();
        assert_point(&group_key, &a["ivpk"]);

        let preaddress = aztec::preaddress(base(&a["publicKeysHash"]), base(&a["partialAddress"]));
        assert_eq!(preaddress, base(&a["preaddress"]));

        let context = aztec::AddressContext::new(preaddress, &group_key).unwrap();
        assert_eq!(context.address(), base(&a["address"]));
        assert_point(&context.address_point, &a["addressPoint"]);
        let negated = a["addressSecretNegated"].as_bool().unwrap();
        assert_eq!(context.sign == -clave_crypto::Scalar::from(1u64), negated);

        let address_secret = aztec::single_key_address_secret(preaddress, &ivsk).unwrap();
        assert_eq!(*address_secret.expose(), scalar(&a["addressSecret"]));
        assert_eq!(
            Point::generator_mul_secret(&address_secret).unwrap(),
            context.address_point
        );

        let eph_pk = point(&a["ecdh"]["ephPk"]);
        let shared = eph_pk.mul_secret(&address_secret).unwrap();
        assert_point(&shared, &a["ecdh"]["sharedPoint"]);
        assert_eq!(
            aztec::app_siloed_shared_secret(&shared, base(&a["ecdh"]["app"])),
            base(&a["ecdh"]["appSiloedSecret"])
        );
    }
}
