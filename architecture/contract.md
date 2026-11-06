# Vault contract

`contracts/vault_contract` defines `TecpanVault`, an Aztec account contract (Aztec.nr 6.0.0, Noir 1.0). It holds the
vault's spending policy (signer set, threshold `k`) and a commitment to its viewing committee (threshold `t_v`,
verification shares), and it authorizes `entrypoint` and `verify_private_authwit` only with valid signatures from at
least `k` signers before an approval deadline.

Viewing is enforced off-chain by member agents (threshold ECDH over shares of the vault's incoming viewing secret);
the contract only stores the committee commitment those agents check against.

Status: Phase 3 (core authorization). Config changes (`rotate_membership`, `set_threshold`, `invalidate_pending`)
arrive in Phase 4.

## Storage

| State variable | Type                                        | Contents                                                                                                     |
| -------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `config`       | `SinglePrivateMutable<ConfigNote>`          | `threshold`, `signer_count`, `version`, `signer_set_commitment`, `viewing_threshold`, `committee_commitment` |
| `signers`      | `Owned<PrivateSet<SignerNote>>` (the vault) | `{ x, y, slot }` per signer                                                                                  |

- Authorization reads only the config note (6 fields; a note carries at most about 8).
- Signer notes exist so members can rebuild the ordered signer set (`get_signer_set`). A `PrivateSet` cannot prove it
  is complete or that a key is absent, so authorization checks the witness against `signer_set_commitment` instead.
- All notes are owned by and encrypted to the vault address.
- Member `i` is signer slot `i` and viewing-committee index `i`.

## Wire formats

These are shared with the Rust (`tecpan-crypto`, `tecpan-wire`) and TypeScript (`vault-wallet`) encoders. Change them
only together with this page and every encoder.

### Constants

| Name                    | Value                                      | Meaning                                        |
| ----------------------- | ------------------------------------------ | ---------------------------------------------- |
| `MAX_SIGNERS`           | 8                                          | Member slots (hard ceiling 15: notes per call) |
| `WITNESS_LEN`           | `MAX_SIGNERS · 6 + 1` = 49                 | Auth witness length                            |
| `MIN_VIEWING_THRESHOLD` | 2                                          | No single member can read the vault            |
| `CONFIG_MAX_AGE`        | 3600 s                                     | Max anchor-to-inclusion time; public           |
| `DOM_SEP__APPROVAL`     | `0x74656370616e2e617070726f76616c2e7630`   | ASCII `tecpan.approval.v0`                     |
| `DOM_SEP__SIGNER_SET`   | `0x74656370616e2e7369676e6572732e7630`     | ASCII `tecpan.signers.v0`                      |
| `DOM_SEP__COMMITTEE`    | `0x74656370616e2e636f6d6d69747465652e7630` | ASCII `tecpan.committee.v0`                    |

Poseidon2 below is Aztec's `poseidon2_hash_with_separator`: the separator is prepended as a field, then the Barretenberg
sponge (rate 3, capacity initialised to `len · 2^64`) runs over the inputs.

### Signer set commitment

```
signer_set_commitment = Poseidon2(DOM_SEP__SIGNER_SET, [x_0, y_0, x_1, y_1, ..., x_7, y_7])
```

Valid when `1 <= n <= MAX_SIGNERS`, slots `[0, n)` hold distinct on-curve Grumpkin points, and slots `[n, MAX)` are
`(0, 0)`. Order is part of the commitment.

### Committee commitment

```
committee_commitment = Poseidon2(DOM_SEP__COMMITTEE, [t_v, Y_0.x, Y_0.y, ..., Y_7.x, Y_7.y])
```

`Y_i = s_i · G` is member `i`'s public verification share from the DKG. Valid when `2 <= t_v <= n`, slots `[0, n)`
hold distinct on-curve points and later slots are zero.

### Approval message

Each signer signs, with Schnorr over Grumpkin (`schnorr` v0.4.0, the scheme of Aztec's `SchnorrAccount`):

```
approval = Poseidon2(DOM_SEP__APPROVAL, [vault_address, outer_hash, config_version, expires_at])
```

| Input            | Purpose                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------- |
| `vault_address`  | No replay on another vault with the same signers (the authwit hash binds only the consumer) |
| `outer_hash`     | Entrypoint payload hash `H(vault, chain_id, version, payload)` or authwit message hash      |
| `config_version` | Any config change invalidates pending approvals                                             |
| `expires_at`     | Approval deadline in unix seconds; public once used, so clients round it                    |

### Auth witness

Served by the PXE for `outer_hash` (`get_auth_witness`), `WITNESS_LEN` = 49 fields:

| Fields     | Contents                                                                            |
| ---------- | ----------------------------------------------------------------------------------- |
| `[0, 16)`  | Signer keys in slot order (`x`, `y`), empty slots zero                              |
| `[16, 48)` | Per slot: `s.lo, s.hi, e.lo, e.hi` (`Signature.toLimbFields()`), zero = no approval |
| `48`       | `expires_at`; must fit in `u64` exactly (`"expires_at out of range"`)               |

## Authorization

`authorization::is_authorized(context, config_slot, outer_hash)`, used by both entry points:

1. Read the config note with `note_getter::get_note` and prove it was not nullified at the anchor block
   (`assert_local_note_was_not_nullified_by`). Nothing is nullified, so this is valid in the static
   `verify_private_authwit` call.
2. Decode the witness; its keys must hash to `signer_set_commitment` (`"signer set mismatch"`).
3. `anchor_timestamp <= expires_at` (`"approval expired"`).
4. For every slot: a non-empty signature must be in an occupied slot (`"signature for empty signer slot"`) and verify
   over `approval` (`"invalid signature"`). Empty slots are verified against the generator so the circuit is fixed.
5. Set the transaction expiration to `min(expires_at, anchor_timestamp + CONFIG_MAX_AGE)`.
6. Authorized iff the number of valid signatures is at least `threshold`.

Duplicate approvals are impossible: one signature per slot, distinct keys.

**Freshness:** a superseded config can authorize only transactions anchored before the change and included within
`CONFIG_MAX_AGE`, and only with approvals signed for the old version. Re-delivering the config note on every
`entrypoint` would close that window but costs about 25k gates per transaction (Phase 2, S4).

## Functions

| Function                                                                                 | Kind                                         | Notes                                                                                           |
| ---------------------------------------------------------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `constructor(signer_keys, signer_count, threshold, viewing_threshold, committee_shares)` | private, initializer                         | Validates both sets, `1 <= k <= n`, `2 <= t_v <= n`; stores config (version 0) and signer notes |
| `entrypoint(app_payload, fee_payment_method, cancellable)`                               | private, `noinitcheck`, `allow_phase_change` | `AccountActions` with `is_authorized`; cancellable txs emit the nonce nullifier                 |
| `verify_private_authwit(inner_hash)`                                                     | private, `noinitcheck`, `view`               | `AccountActions` with `is_authorized`                                                           |
| `get_config()`                                                                           | utility                                      | Current `ConfigNote`                                                                            |
| `get_signer_set()`                                                                       | utility                                      | Ordered `SignerSet` from signer notes                                                           |
| `lookup_validity(consumer, inner_hash)`                                                  | utility                                      | Approved, unexpired and unconsumed; reverts on a malformed witness                              |

## Note delivery

The constructor delivers notes **unconstrained**, tagged with the vault as sender, so members discover them by
registering the vault. This is safe because the deployer is a member and every member verifies the on-chain config
against the agreed manifest before funding; constrained delivery of `MAX_SIGNERS + 1` notes would also exceed per-call
limits. Config changes (Phase 4) use constrained delivery: there one executor acts for everyone, and an undelivered
config note would lock the others out.

## Gate profile

`aztec profile gates` (MAX_SIGNERS = 8 unless noted):

| Circuit                  | TecpanVault | Aztec `SchnorrAccount` (single key) |
| ------------------------ | ----------- | ----------------------------------- |
| `constructor`            | 25,351      | 7,905                               |
| `entrypoint`             | 27,176      | 15,002                              |
| `verify_private_authwit` | 18,665      | 6,489                               |

| MAX_SIGNERS | constructor | entrypoint | verify_private_authwit |
| ----------- | ----------- | ---------- | ---------------------- |
| 4           | 17,674      | 21,829     | 13,318                 |
| 8           | 25,351      | 27,176     | 18,665                 |
| 16          | 41,757      | 37,845     | 29,334                 |

`verify_private_authwit` flamegraph: Schnorr verification ≈10.3k (55%, one two-point Grumpkin MSM per slot, ≈1.2k
each), config freshness proof ≈2.6k (14%), the rest is macro and kernel-interface overhead. The deadline check and
committee commitment add well under 1k. Each extra slot costs about 1,340 gates.

## Tests

`contracts/vault_contract` (21 unit tests): point, signer set and committee validation and commitments, witness
decoding and deadline range check, approval message binding.

`contracts/vault_test` (28 TXE tests):

- Deployment: 2-of-3 and 8-of-8 vaults; `get_config` / `get_signer_set`; rejections for `k = 0`, `k > n`, `n = 0`,
  `n > MAX`, duplicate / off-curve / out-of-range signer keys, `t_v = 1`, `t_v > n`, a single-member vault, off-curve
  and out-of-range committee shares.
- Authorization: threshold met, any subset, all 8 signers, deadline equal to the anchor timestamp; rejections below
  threshold, with no approvals, expired, deadline different from the signed one, signature over another hash, in
  another signer's slot, in an empty slot, substituted signer, approval for another vault, approval for another config
  version. Every rejection asserts its message.

Harness: `contracts/test_signer_contract` is a **test-only** helper (never deploy) that returns TXE-produced Schnorr
signatures to tests. Tests assemble the witness, mock `aztec_utl_getAuthWitness`, and run `is_authorized` in a private
context at the vault; vaults are deployed with the vault in the deployer's scope, and notes are synced with a utility
call first.

Not covered by TXE (Phase 7 end-to-end on a local network): full `entrypoint` and `verify_private_authwit` with a real
witness, the expiration cap inside a static call, `lookup_validity`, cancellation.
