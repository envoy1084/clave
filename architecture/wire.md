# Wire format and Node bindings

`crates/wire` (`tecpan-wire`) is the single definition of every message exchanged between engines, keepers and relays.
`crates/napi` (`@tecpan/napi`) exposes it and the public parts of `tecpan-crypto` to TypeScript, so the vault wallet
and the desktop app never reimplement cryptography or encoding.

Status: Phase 6 (library and bindings). Keepers send these messages from Phase 8; the vault wallet uses the bindings
from Phase 7.

## Encoding

- [`postcard`](https://docs.rs/postcard) over serde: varint integers, length-prefixed sequences, enum variants as a
  varint index. `WIRE_VERSION = 0`.
- **Canonical only.** postcard also parses overlong varints, so `decode` re-encodes and rejects any input that differs
  from its canonical form (found by fuzzing). Every message has exactly one byte form; byte-level hashing and dedup
  are sound.
- Decode rejects: input over `MAX_MESSAGE_BYTES` (1 MiB), trailing bytes, unknown variants, other wire versions, and
  bodies outside their bounds (below). Cryptographic validity (points on the curve, canonical scalars, proofs) is
  checked when a body is converted to `tecpan-crypto` types.

| Value          | Encoding                                                      |
| -------------- | ------------------------------------------------------------- |
| Field element  | 32-byte big-endian (`WireField`), canonical on conversion     |
| Scalar         | 32-byte big-endian (`WireScalar`), canonical on conversion    |
| Point          | `{ x, y }` (`WirePoint`), validated on the curve, no infinity |
| 64-byte values | two 32-byte halves (`Bytes64`): signatures, transcript hashes |

## Signed messages

```text
SignedMessage { version: u16, header: Header, body: Message, signer: [u8; 32], signature: [u8; 64] }
Header        { scope: [u8; 32], epoch: u32, id: [u8; 16], created_at: u64 }
signature     = Ed25519(keeper, "tecpan.wire.v0" ‖ postcard(version, header, body, signer))
```

- **Keeper identity.** Each keeper has an Ed25519 identity (`KeeperIdentity`), which is also its future libp2p peer
  id, and a separate X25519 transport key for envelopes. Verification uses `verify_strict` (no small-order keys,
  canonical `S`). The identity's `Debug` output is redacted.
- **Signer binding.** `verify_from(expected)` checks the signer is the manifest entry of the member the message
  claims to come from, before checking the signature.
- **Scope** is the vault address, or the ceremony session id before the vault exists. **Epoch** is the committee epoch:
  the vault config version at which the current viewing committee became active (0 from the DKG). Config changes that
  keep the committee (`set_threshold`, `invalidate_pending`) do not change it; DKG messages use 0 and reshare messages
  the old committee's epoch.

### Replay rules (`ReplayGuard`)

Applied after signature verification, in this order:

1. `header.epoch` must equal the receiver's current epoch (`WrongEpoch`).
2. `|created_at − now| ≤ max_skew` (`Stale`).
3. `(signer, id)` must not have been seen (`Replayed`). Remembered ids are bounded (oldest evicted); an evicted id is
   already outside the skew window, so it cannot be replayed.

## Message catalogue

Variants are append-only: the index is the encoding. Golden bytes for every kind are in
`crates/wire/tests/fixtures/wire.json`; changing one is a wire-format change.

| Tag | Message            | Contents                                                                                     | Bounds                            |
| --: | ------------------ | -------------------------------------------------------------------------------------------- | --------------------------------- |
|   0 | `Manifest`         | vault, epoch, `k`, `t_v`, preaddress, `Ivpk`, members                                        | 1..=8 members (see below)         |
|   1 | `DkgRound1`        | dealer, Feldman commitments, proof of knowledge `(R, μ)`                                     | 1..=8 commitments                 |
|   2 | `DkgRound2`        | dealer → recipient sealed share (HPKE encapsulated key, ciphertext)                          | ciphertext 1..=256 bytes          |
|   3 | `DkgConfirm`       | transcript hash                                                                              |                                   |
|   4 | `ReshareDeal`      | dealer, commitments (constant term must be `λ_i · Y_i`)                                      | 1..=8 commitments                 |
|   5 | `ReshareSubShare`  | dealer → recipient sealed sub-share                                                          | ciphertext 1..=256 bytes          |
|   6 | `ReshareConfirm`   | transcript hash                                                                              |                                   |
|   7 | `Complaint`        | ceremony, accused dealer                                                                     |                                   |
|   8 | `Justification`    | ceremony, complainant, the revealed share                                                    |                                   |
|   9 | `Abort`            | ceremony, reason (timeout, complaint, equivocation, cancelled)                               |                                   |
|  10 | `EcdhRequest`      | purpose (note decryption, handshake, recipient tag, sender tag), input points                | 1..=4096 points                   |
|  11 | `EcdhResponse`     | request id, member, partials `(D_i, c, z)` in request order                                  | 1..=4096 partials                 |
|  12 | `Proposal`         | config version, outer hash, `expires_at`, `tx_nonce`, fee method, payload, summary, proposer | payload ≤ 64 KiB, summary ≤ 4 KiB |
|  13 | `Approval`         | proposal id, slot, signature limbs `[s.lo, s.hi, e.lo, e.hi]`                                |                                   |
|  14 | `ExecutionReceipt` | proposal id, transaction hash                                                                |                                   |
|  15 | `RelayEnvelope`    | opaque recipient mailbox id, expiry, sealed bytes                                            | 1 byte..=512 KiB                  |

### Manifest

Members are listed in slot order with `index = slot + 1`, each with its signer key, verification share `Y_i`, keeper
identity, transport key and up to 8 addresses (≤ 256 bytes each). `validate` checks indices `1..=n`, distinct
identities, `1 ≤ k ≤ n` and `2 ≤ t_v ≤ n`. A manifest is trusted only after
`signer_set_commitment()` and `committee_commitment()` equal the vault's on-chain config and `address_context()`
confirms the vault address from `preaddress` and `Ivpk`.

### Ceremony complaints

Third parties cannot open another member's envelope, so a complaint is resolved by the accused:

1. Member `i` broadcasts `Complaint { accused: j }`.
2. Dealer `j` broadcasts `Justification { complainant: i, share: f_j(i) }`.
3. Everyone checks `Justification::holds(C_j)`. A share that does not match, or no justification before the round
   deadline, blames `j`. A share that matches is unattributable (nobody else saw what `j` sent), so nobody is blamed.
   Either way the ceremony aborts: the revealed share is now public, and members restart with a new session (without
   `j` if it was blamed). See `keeper.md`.

### Proposals and approvals

- `Proposal::id() = SHA-256("tecpan.proposal.v0" ‖ postcard(proposal))`, covering every field.
- `Proposal::approval_message(vault)` is the contract's `Poseidon2("tecpan.approval.v0", [vault, outer_hash, version,
  expires_at])` (see `contract.md`).
- `Approval::verify(proposal, vault, signer_key)` requires the approval to name that proposal id and the signature to
  verify exactly as the contract checks it. Approvers recompute `outer_hash` from `payload` before signing.

### Unsigned bodies

`encode_body` / `decode_body` carry a bare `Message` with the same canonical and bounds rules. They are used between an
engine and its own keeper (local RPC), which signs and sends the message.

## Node bindings (`@tecpan/napi`)

napi-rs 3, Node-API 8. Field elements and scalars cross the boundary as `0x`-prefixed 32-byte hex, points as
`{ x, y }`, byte strings as `Buffer`, timestamps as non-negative integers. Every function validates its input and
throws an `InvalidArg` error on failure.

| Area    | Functions                                                                                                                                                  |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Vault   | `approvalMessage`, `signerSetCommitment`, `committeeCommitment`, `encodeWitness`, `signatureLimbs`, `verifySignature`                                      |
| Address | `preaddress`, `addressContext` (address, sign, address point), `addressPublicShares` (`A_i` from `Y_i`)                                                    |
| Viewing | `verifyAndCombine`: verifies every DLEQ proof, combines `t_v` valid partials per input, reports bad members                                                |
| Wire    | `wireVersion`, `proposalId`, `proposalApprovalMessage`, `encodeProposal` / `decodeProposal`, `encodeApproval` / `decodeApproval`, `verifyProposalApproval` |

- **No secrets in the production addon.** Envelope sealing and opening stay in the keeper (which holds the transport
  key); signing keys stay in the desktop signer service. Helpers that deal or use viewing shares and a test Schnorr
  signer exist only in the separate `@tecpan/napi/testing` binary, built with the `testing` feature.
- `pnpm --filter @tecpan/napi build` writes `index.js`, `index.d.ts` and `tecpan.<platform>.node` to `dist/` (the
  testing build to `dist/testing/`). `dist/` is build output and is not committed.
- Node-API is ABI-stable: the same binary loads in Node 24 and in Electron 45 (verified with Electron 45.2.3,
  Node 24.22.2). Per-platform prebuilds are produced by the release pipeline.

## Tests

| Suite                              | Covers                                                                                                                                                                                    |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/wire/tests/codec.rs`       | Round trips, golden bytes, variant tags, truncation, trailing bytes, overlong varints, unknown variants, versions, size and count bounds                                                  |
| `crates/wire/tests/signed.rs`      | Signatures over every kind, signer binding, every flipped bit rejected, replay guard                                                                                                      |
| `crates/wire/tests/messages.rs`    | Manifest validation and commitments vs the contract vectors, approvals with real Schnorr, proposal id, DKG / reshare / ECDH conversions with real ceremonies, justifications              |
| `crates/napi/test/aztec.test.ts`   | Against live `@aztec-labs` 6.0.0: address and sign (both branches), preaddress, 2-of-3 threshold ECDH vs `deriveEcdhSharedSecretPoint`, Aztec Schnorr approvals, codecs, contract vectors |
| `crates/crypto/fuzz` `wire_decode` | Decoding signed messages and bare bodies never panics; accepted input re-encodes identically                                                                                              |

Regenerate golden bytes only for an intended format change:
`UPDATE_WIRE_FIXTURES=1 cargo test -p tecpan-wire --test codec`.
