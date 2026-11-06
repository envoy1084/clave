# Vault contract

`contracts/vault_contract` defines `TecpanVault`, an Aztec account contract (Aztec.nr 6.0.0, Noir 1.0). It holds the
vault's spending policy (signer set, threshold `k`) and a commitment to its viewing committee (threshold `t_v`,
verification shares), and it authorizes `entrypoint` and `verify_private_authwit` only with valid signatures from at
least `k` signers before an approval deadline.

Viewing is enforced off-chain by keepers (threshold ECDH over shares of the vault's incoming viewing secret);
the contract only stores the committee commitment those keepers check against.

Status: Phases 3–4 (authorization and governance).

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

**Freshness:** a transaction can be authorized against a superseded config only if its prover deliberately serves the
old config note with an anchor block from before the change: an honest PXE serves the current note, and the kernel
rejects a note that did not exist at the anchor. Even then the transaction needs `threshold` approvals signed for the
old version and must be included within `CONFIG_MAX_AGE` of its anchor. Re-delivering the config note on every
`entrypoint` would close the window but costs about 25k gates per transaction (constrained delivery, Phase 2 S4), so
the bounded window is accepted.

## Governance

Config changes are `#[only_self]` private functions: they run only as calls the vault makes to itself, i.e. inside an
`entrypoint` payload that already passed k-of-n authorization (`"Function <name> can only be called by the same
contract"` otherwise). Every change replaces the config note with `version + 1`, which invalidates every approval
collected so far.

| Function                                                                                                                    | Effect                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `set_threshold(threshold)`                                                                                                  | New `k` (`1 <= k <= n`); signer set and committee unchanged                              |
| `invalidate_pending()`                                                                                                      | Version bump only; emergency cancellation of all collected approvals                     |
| `rotate_membership(current_signer_keys, new_signer_keys, new_signer_count, threshold, viewing_threshold, committee_shares)` | New signer set, `k`, `t_v` and committee in one atomic change; rewrites the signer notes |

`rotate_membership` asserts, in order:

1. `current_signer_keys` hash to the stored `signer_set_commitment` (`"current signer set mismatch"`): the rotation was
   built against the current config.
2. The new signer set is valid, `1 <= k <= n'` and the new committee is valid (`2 <= t_v' <= n'`).
3. Fewer than the current `t_v` members are absent from the new set (`"too many members removed"`), so the viewing
   reshare that accompanies the rotation keeps the incoming viewing secret safe. Larger exits run as several rotations
   or move funds to a new vault.
4. Exactly the current number of signer notes is popped (`"stale signer notes"`), then the new set is inserted.

Member `i` of the new set is signer slot `i` and committee index `i`; adding, removing, replacing a device key and
reordering are all expressed as a new ordered set.

### Rotation protocol

The committee commitment covers the **new** members' verification shares, which exist only after the keepers reshare,
so the reshare is prepared before the rotation executes:

1. A member proposes the new membership.
2. The staying keepers run the reshare and hold the resulting shares as pending; this yields the new verification
   shares and their commitment.
3. The `rotate_membership` proposal carries those shares; members approve it.
4. When the transaction is included, every keeper sees the new version with a matching `committee_commitment`,
   activates its pending share and erases the old one. If the transaction expires, pending shares are discarded.

`set_threshold` and `invalidate_pending` bump the version without changing `committee_commitment`; keepers reshare only
when the committee commitment changes.

## Functions

| Function                                                                                 | Kind                                         | Notes                                                                                           |
| ---------------------------------------------------------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `constructor(signer_keys, signer_count, threshold, viewing_threshold, committee_shares)` | private, initializer                         | Validates both sets, `1 <= k <= n`, `2 <= t_v <= n`; stores config (version 0) and signer notes |
| `entrypoint(app_payload, fee_payment_method, cancellable)`                               | private, `noinitcheck`, `allow_phase_change` | `AccountActions` with `is_authorized`; cancellable txs emit the nonce nullifier                 |
| `verify_private_authwit(inner_hash)`                                                     | private, `noinitcheck`, `view`               | `AccountActions` with `is_authorized`                                                           |
| `set_threshold`, `invalidate_pending`, `rotate_membership`                               | private, `only_self`                         | See Governance                                                                                  |
| `get_config()`                                                                           | utility                                      | Current `ConfigNote`                                                                            |
| `get_signer_set()`                                                                       | utility                                      | Ordered `SignerSet` from signer notes                                                           |
| `lookup_validity(consumer, inner_hash)`                                                  | utility                                      | Approved, unexpired and unconsumed; reverts on a malformed witness                              |

## Note delivery

| Note                          | Constructor   | Config changes | Why                                                                                                                                |
| ----------------------------- | ------------- | -------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Config note                   | unconstrained | constrained    | Members need its plaintext to prove; after deployment one executor acts for everyone, so a withheld note would lock the others out |
| Signer notes (discovery only) | unconstrained | unconstrained  | Authorization checks the commitment; members also hold the set in their committee manifest                                         |

All notes are tagged with the vault as sender. The constructor may deliver its config note unconstrained because the
deployer is a member and every member verifies the on-chain config against the agreed manifest before funding;
constrained delivery of `MAX_SIGNERS + 1` notes would also exceed per-call limits.

## Gate profile

`aztec profile gates` (MAX_SIGNERS = 8 unless noted):

| Circuit                  | TecpanVault | Aztec `SchnorrAccount` (single key) |
| ------------------------ | ----------- | ----------------------------------- |
| `constructor`            | 25,351      | 7,905                               |
| `entrypoint`             | 27,176      | 15,002                              |
| `verify_private_authwit` | 18,665      | 6,489                               |
| `set_threshold`          | 37,443      | —                                   |
| `invalidate_pending`     | 37,385      | —                                   |
| `rotate_membership`      | 85,734      | —                                   |

| MAX_SIGNERS | constructor | entrypoint | verify_private_authwit | rotate_membership |
| ----------- | ----------- | ---------- | ---------------------- | ----------------- |
| 4           | 17,674      | 21,829     | 13,318                 | 77,395            |
| 8           | 25,351      | 27,176     | 18,665                 | 85,734            |
| 16          | 41,757      | 37,845     | 29,334                 | 104,280           |

- `verify_private_authwit`: Schnorr verification ≈10.3k (55%, one two-point Grumpkin MSM per slot, ≈1.2k each),
  config freshness proof ≈2.6k (14%), the rest macro and kernel-interface overhead. The deadline check and committee
  commitment add well under 1k. Each extra slot costs about 1,340 gates.
- `rotate_membership`: constrained config delivery ≈30.6k (36%), popping the signer notes ≈28.5k (34%), unconstrained
  re-insertion ≈13.4k (16%). A full 8-out / 8-in rewrite fits the per-call limits.
- `set_threshold` / `invalidate_pending`: dominated by the constrained config delivery.

## Tests

`contracts/vault_contract` (34 unit tests): point, signer set and committee validation and commitments, witness
decoding and deadline range check, approval message binding, governance transitions (threshold change, rotation
invariants, removal counting).

`contracts/vault_test` (50 TXE tests):

- Deployment (14): 2-of-3 and 8-of-8 vaults; `get_config` / `get_signer_set`; rejections for `k = 0`, `k > n`,
  `n = 0`, `n > MAX`, duplicate / off-curve / out-of-range signer keys, `t_v = 1`, `t_v > n`, a single-member vault,
  off-curve and out-of-range committee shares.
- Authorization (14): threshold met, any subset, all 8 signers, deadline equal to the anchor timestamp; rejections
  below threshold, with no approvals, expired, deadline different from the signed one, signature over another hash, in
  another signer's slot, in an empty slot, substituted signer, approval for another vault, approval for another config
  version. Every rejection asserts its message.
- Governance (8): `set_threshold` and `invalidate_pending` succeed from the vault, invalidate collected approvals,
  reject out-of-range thresholds and outside callers.
- Rotation (14): add, remove, replace a key, reorder, full 8-member rewrite, successive rotations; removed members
  cannot approve and earlier approvals are rejected; rejections for too many removals, a stale current set, invalid
  thresholds, a duplicate new signer and outside callers.

Harness: `contracts/test_signer_contract` is a **test-only** helper (never deploy) that returns TXE-produced Schnorr
signatures to tests. Tests assemble the witness, mock `aztec_utl_getAuthWitness` (one answer per check), and run
`is_authorized` in a private context at the vault; vaults are deployed with the vault in the deployer's scope, notes are
synced with a utility call first, and config changes are called with the vault as caller.

Not covered by TXE (Phase 7 end-to-end on a local network): full `entrypoint` and `verify_private_authwit` with a real
witness, the expiration cap inside a static call, `lookup_validity`, cancellation, and the freshness window (inline
TXE contexts do not run the kernel, so they cannot reproduce a prover serving a superseded note at an old anchor).
