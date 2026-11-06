# TecpanMultisig contract

`contracts/tecpan_contract` is an Aztec account contract (Aztec.nr v6, Noir 1.0) that executes a payload only when the
auth witness carries valid Schnorr signatures from at least `threshold` of its registered signers. Signer keys,
threshold and approvals never appear on-chain in plaintext.

## Storage

| Slot      | State variable                         | Contents                                                        |
| --------- | -------------------------------------- | --------------------------------------------------------------- |
| `config`  | `SinglePrivateMutable<ConfigNote>`     | `threshold`, `signer_count`, `version`, `signer_set_commitment` |
| `signers` | `Owned<PrivateSet<SignerNote>>` (self) | One `{x, y, slot}` note per signer                              |

All notes are owned by the multisig address and encrypted to it, so every holder of the account secret (every signer)
can read them.

- **The config note is authoritative.** `signer_set_commitment` commits to the ordered signer keys. Authorization only
  reads this note.
- **Signer notes are for discovery only.** They let a co-signer's PXE rebuild the ordered signer set
  (`get_signer_set`) from chain data. Authorization never reads them, because a `PrivateSet` cannot prove that a set of
  notes is complete or that a key is absent.

## Signer set commitment

```
signer_set_commitment = poseidon2_hash_with_separator(
    [x_0, y_0, x_1, y_1, ..., x_{MAX-1}, y_{MAX-1}],
    DOM_SEP__SIGNER_SET,            // ASCII "tecpan.signers.v1"
)
```

A signer set is valid when `1 <= count <= MAX_SIGNERS`, slots `[0, count)` hold distinct on-curve Grumpkin keys, and
every later slot is `(0, 0)`. Order matters: approvals are matched to signers by slot.

## Approval message

Each signer signs, with Schnorr over Grumpkin (`schnorr` v0.4.0, the same scheme as Aztec's `SchnorrAccount`):

```
approval_message = poseidon2_hash_with_separator(
    [account_address, outer_hash, config_version],
    DOM_SEP__APPROVAL,              // ASCII "tecpan.approval.v1"
)
```

- `outer_hash` is the hash Aztec asks the account to authorize: the entrypoint payload hash
  `H(account, chain_id, version, app_payload)`, or an authwit message hash `H(consumer, chain_id, version, inner_hash)`.
- `account_address` stops replay on another multisig with the same signers. The authwit hash binds the consumer, not
  the account.
- `config_version` invalidates approvals collected before a config change.

## Auth witness layout

`WITNESS_LEN = MAX_SIGNERS * 6` fields, fetched with `get_auth_witness(outer_hash)`:

| Range                        | Contents                                                                 |
| ---------------------------- | ------------------------------------------------------------------------ |
| `[0, 2 * MAX_SIGNERS)`       | Signer keys in slot order, `x` then `y`, empty slots zero                |
| `[2 * MAX_SIGNERS, 6 * MAX)` | Per slot: `s.lo, s.hi, e.lo, e.hi` (`Signature.toLimbFields()`), or zero |

A zero signature means "did not approve". With `MAX_SIGNERS = 8` the witness is 48 fields.

## Authorization algorithm

`authorization::is_authorized(context, config_slot, outer_hash)`, used by both `entrypoint` and
`verify_private_authwit`:

1. Read the config note with `get_note` and prove it was **not nullified at the anchor block**
   (`assert_local_note_was_not_nullified_by`). This makes no state changes, so it is valid inside the static
   `verify_private_authwit` call. `SinglePrivateMutable::get_note` would nullify, so it can't be used there.
2. Cap the transaction's expiration at `anchor_timestamp + CONFIG_MAX_AGE` (1 hour). This bounds how long a superseded
   config can still authorize after it is replaced.
3. Decode the witness. Assert its keys hash to `signer_set_commitment` (`"signer set mismatch"`).
4. For every slot: a non-empty signature must belong to an occupied slot (`"signature for empty signer slot"`) and must
   verify over `approval_message` (`"invalid signature"`). Count the non-empty ones.
5. Authorized iff `count >= threshold`.

Duplicate approvals are impossible by construction: each slot holds at most one signature, and keys are distinct.

## Functions

| Function                                   | Kind            | Notes                                                         |
| ------------------------------------------ | --------------- | ------------------------------------------------------------- |
| `constructor(signer_keys, count, thresh.)` | private, init   | Validates the set and `1 <= threshold <= count`; stores notes |
| `entrypoint(app_payload, fee_method, c.)`  | private, noinit | `AccountActions::entrypoint` with `is_authorized`             |
| `verify_private_authwit(inner_hash)`       | private, view   | `AccountActions::verify_private_authwit`                      |
| `get_config()`                             | utility         | Current `ConfigNote`                                          |
| `get_signer_set()`                         | utility         | Ordered `SignerSet` rebuilt from signer notes                 |
| `lookup_validity(consumer, inner_hash)`    | utility         | Approved and not yet consumed; reverts on a malformed witness |

Replay protection and cancellation come from `AccountActions`: with `cancellable = true` the payload's `tx_nonce` is
emitted as a nullifier, and a second transaction with the same nonce cancels the first.

## Note delivery

The constructor delivers notes **unconstrained**, tagged with the multisig itself as sender (as `SchnorrAccount` does),
so co-signers discover them by registering the account. This is safe because:

- the deployer is one of the signers;
- before funding, co-signers verify that the delivered config matches the agreed signer set and threshold, which the
  SDK enforces.

A bad delivery can only brick an empty account. Constrained delivery of `MAX_SIGNERS + 1` notes would also exceed the
per-call kernel limits: each constrained message spends a sequence nullifier plus read and key-validation requests.

Config changes (Phase 2) must use **constrained** delivery. There the executor is one signer acting for the others, and
an undelivered config note would lock them out.

## Gate profile (`aztec profile gates`, MAX_SIGNERS = 8)

| Circuit                  | TecpanMultisig | SchnorrAccount (single key) |
| ------------------------ | -------------- | --------------------------- |
| `constructor`            | 24,692         | 7,905                       |
| `entrypoint`             | 27,078         | 15,002                      |
| `verify_private_authwit` | 18,567         | 6,489                       |

Scaling with `MAX_SIGNERS` (≈1,340 gates per slot):

| MAX_SIGNERS | constructor | entrypoint | verify_private_authwit |
| ----------- | ----------- | ---------- | ---------------------- |
| 4           | 17,356      | 21,731     | 13,220                 |
| 8           | 24,692      | 27,078     | 18,567                 |
| 16          | 39,934      | 37,747     | 29,236                 |

Where the gates go in `verify_private_authwit` (from the flamegraph):

| Part                                                                 | Gates  | Share |
| -------------------------------------------------------------------- | ------ | ----- |
| Schnorr verification (one 2-point Grumpkin MSM per slot, ≈1.2k each) | ≈10.3k | 56%   |
| Config freshness proof (nullifier non-membership)                    | ≈2.6k  | 14%   |
| Macro and kernel interface overhead                                  | ≈5k    | —     |

Every slot is verified so that the circuit has a fixed size. That ≈1.2k-gate check per slot is the floor for this
signature scheme.

`MAX_SIGNERS = 8`, because:

- it keeps `entrypoint` within about 2× of a single-key account;
- the constructor's `MAX_SIGNERS + 1` notes must fit the 16-note per-call limit, so 15 is the hard ceiling.

## Testing

`contracts/tecpan_test` runs under the TXE:

- **Deployment:** valid configs, and every invalid-config rejection.
- **Authorization:** threshold met, any subset, every signer, below threshold, no approvals, wrong hash, signature in
  another signer's slot, signature in an empty slot, substituted signer, wrong account, wrong config version. Each
  failure case asserts its exact error message.

`contracts/test_signer_contract` is a **test-only** helper. The TXE can only sign with a test account's key into its
own witness store, so the helper returns those signatures for the tests to assemble multi-signer witnesses. The
aggregated witness is then served through an `OracleMock` of `aztec_utl_getAuthWitness`, and `is_authorized` runs
inside a private context at the multisig address. Never deploy the helper outside tests.

Not covered in TXE (deferred to the Phase 3 end-to-end suite against a local network):

- the full `entrypoint` / `verify_private_authwit` call path with a real aggregated witness;
- the expiration cap inside a static call;
- `lookup_validity`.

## Invariants

- `1 <= threshold <= signer_count <= MAX_SIGNERS` at all times.
- Exactly one current config note. Authorization proves it was current at the anchor block.
- Approvals are bound to account, `outer_hash` and config version.
- No public storage, and no public function arguments carrying signer or policy data.
