# Keeper

`crates/keeper` (`tecpan-keeper`) is the daemon each member runs, on their own machine (supervised by the desktop app)
or on their own server. It is the only place a viewing share exists. It serves threshold ECDH to its owner's engine and
to the other members' keepers, runs the DKG and reshare ceremonies, keeps the proposal store, and activates or destroys
shares as the vault's on-chain committee changes.

Status: Phase 8. Keepers reach each other over Noise-encrypted TCP; the libp2p mesh, relay and set reconciliation come
in Phase 9.

## Responsibilities and trust

| The keeper…                                                           | …never                                                  |
| --------------------------------------------------------------------- | ------------------------------------------------------- |
| holds one viewing share per vault and committee epoch                 | exports a share (no RPC, CLI or log path returns one)   |
| answers ECDH for its engine and for current committee members         | answers anyone else, another epoch, or a removed member |
| joins ceremonies its owner starts, with the parameters its owner gave | joins a ceremony because a peer asked                   |
| activates a reshared committee when its engine observes it on chain   | activates on a peer's word                              |
| verifies every peer partial (DLEQ) before returning it                | trusts a partial it has not verified                    |

The engine (PXE host) is the keeper's owner: it holds the RPC token and is trusted to report on-chain configs
honestly. Other members' keepers are not trusted beyond their signatures and proofs.

## Process and data directory

```text
<data>/
  secrets/          one sealed file per secret (0700 dir, 0600 files)
    check.sealed      master-key check
    identity.sealed   Ed25519 keeper identity seed
    transport.sealed  X25519 transport key
    share-<vault>-<epoch>.sealed   active viewing share
    dkg-<session>.sealed           finished DKG, waiting for vault registration
    pending-<session>.sealed       finished reshare, waiting for on-chain activation
  keeper.db         SQLite (WAL, secure_delete): manifests, ceremonies, proposals, approvals, receipts, audit, observations
  keeper.toml       optional configuration
  keeper.json       public identity, addresses, RPC socket (written at start)
  keeper.sock       RPC socket (owner-only); rpc.token when no token is supplied
  salt              Argon2id salt (passphrase mode)
```

`tecpan-keeper run` prints one JSON line `{"ready": {identity, transport, addresses, rpcSocket}}` on stdout when the
socket is listening; logs go to stderr (`--log-format json` for structured logs). SIGTERM or Ctrl-C shuts it down.

## Custody

- **Master key.** Seals every secret. Either supplied by the supervising app on stdin
  (`--secrets-stdin`, one JSON line `{"masterKey", "rpcToken"}`; the desktop keeps the key in the OS keychain through
  Electron `safeStorage`), or derived from `TECPAN_KEEPER_PASSPHRASE` with Argon2id (64 MiB, 3 passes) over a per-keeper
  salt. A sealed check value detects a wrong key before any secret is touched. The keeper itself has no keychain
  dependency.
- **Sealed files.** XChaCha20-Poly1305 with a random 24-byte nonce; the secret's name is authenticated as associated
  data, so files cannot be swapped. Writes go to a temporary file, are fsynced, renamed and the directory fsynced.
  Destruction overwrites the file before unlinking it. Share values are zeroized in memory after use.
- **Shares.** Stored with their index, threshold, participants, the committee's Feldman commitments and transcript; on
  load the share must match its committed verification share (`KeyShare::restore`).
- **Ceremony secrets** (polynomials, received sub-shares) are never persisted. A keeper restarted mid-ceremony marks it
  aborted ("keeper restarted during the ceremony"); members start a new session.

## Committee model

A manifest (wire `Manifest`) becomes a `Committee` only if: the structure is valid (1..=8 members indexed in slot
order, `1 ≤ k ≤ n`, `2 ≤ t_v ≤ n`, distinct identities); the vault address equals `x(preaddress · G + Ivpk)`; and the
verification shares lie on one degree-`t_v − 1` polynomial whose constant term is `Ivpk`
(`Commitments::interpolate`). The committee's `committee_commitment` is what the vault stores on chain.

**Committee epoch** = the vault config version at which the committee became active (0 after the DKG). Partials and
envelopes are bound to it. Config changes that keep the committee do not change it.

## Threshold ECDH

**Responder** (a peer's `EcdhRequest`): the frame's channel key must belong to the signer; the signature, epoch, clock
skew and message id pass the `ReplayGuard`; the signer must be a current member; the batch must fit the member's
budget (token bucket, 60 000 points per minute by default); every point must be on the curve and not the identity.
Partials are computed off the async runtime with the share held behind an `Arc`, so concurrent requests do not
serialise on the state lock. Every request, served or refused, is written to the audit log (time, requester, purpose,
point count, outcome). A refused request gets no answer.

**Gatherer** (`ecdh.batch` from the engine): computes this keeper's partials, sends one signed request to every other
member, and verifies each response's DLEQ proofs (off the runtime) until `t_v` members are collected or the timeout
passes (15 s by default). A member whose proofs fail is reported (`invalidPartials` event) and skipped; the batch fails
with `Unavailable` if fewer than `t_v` answer validly. The engine verifies the returned partials again before
combining.

## Ceremonies

Pure state machines (`ceremony::dkg`, `ceremony::reshare`): inputs are signed messages from known participants and
clock ticks, outputs are messages and a status. The keeper persists results and status; messages that arrive before
the owner starts a session are buffered (16 sessions × 64 messages) and replayed when it does.

**DKG.** Participants in agreed order; round 1 commitments and proof of knowledge (broadcast), round 2 shares sealed
to each recipient's transport key (envelope kind 1, epoch 0), finalisation, then every member must confirm the same
transcript hash. Result: the share in `dkg-<session>`, and `{index, threshold, groupKey, verificationShares,
transcript}` for the engine.

**Vault registration** (`vault.register`): after deriving the vault, each engine registers the manifest with its keeper.
The keeper checks it against its DKG result (index, threshold, group key and every verification share) and activates
the share.

**Reshare.** Exactly `t_v` current members deal (sub-share envelopes, kind 2, at the old epoch) to the new committee;
new members check each deal's constant term against `λ_i · Y_i` from the current manifest, combine, and confirm one
transcript. A joining member's keeper receives the current manifest from its engine. The new share waits in
`pending-<session>` and the old committee keeps serving.

**Activation** (`vault.observeConfig`, called by the engine after each sync with the config version and committee
commitment):

| Observed committee commitment                | Effect                                                                                               |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| equals the active committee's                | nothing (`current`)                                                                                  |
| equals a completed reshare's new committee   | new manifest active at that version, pending share becomes active, old share destroyed (`activated`) |
| differs, version newer than the active epoch | this keeper is not in the new committee: share destroyed, vault marked removed (`removed`)           |
| anything else                                | `configMismatch` event (`unknown`)                                                                   |

**Failure handling.** Any malformed or inconsistent message aborts with the sender blamed. An invalid proof of
knowledge or a deal that is not the dealer's real share blames the dealer publicly. A bad share triggers complaint →
justification: an invalid justification (or none before the deadline) blames the dealer; a valid one blames nobody,
because others cannot tell what was sent. Either way the revealed share is public and the ceremony aborts. Differing
transcripts abort as equivocation. A round past its deadline (120 s by default) aborts, blaming the participants whose
messages are missing.

## Proposals

`proposals.put`, `approvals.put` and `receipts.put` store records and push them to the other members, who store them
after checking the sender is a member. Approvals are verified against the slot's signing key and the proposal's
approval message before they are stored. `vault.observeTx` raises `unexpectedSpend` for a vault transaction that
matches no stored receipt. Set reconciliation for members that were offline comes in Phase 9.

## Local RPC

JSON-RPC 2.0, one JSON object per line, on an owner-only Unix socket (a named pipe rejecting remote clients on
Windows). The first call must be `session.auth { token }` (compared in constant time); everything else is refused
until then. Requests on a session run concurrently.

| Method                                                         | Params → result                                                                                                                                         |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `keeper.info`                                                  | → `{ identity, transport, addresses, version }`                                                                                                         |
| `vault.list`, `vault.status`                                   | `{ vault }` → `{ manifest, status, index, serving }`                                                                                                    |
| `vault.register`                                               | `{ session, manifest }` → vault                                                                                                                         |
| `vault.observeConfig`                                          | `{ vault, version, committeeCommitment }` → `{ observation }`                                                                                           |
| `vault.observeTx`                                              | `{ vault, txHash }` → `{ known }`                                                                                                                       |
| `ecdh.batch`                                                   | `{ vault, epoch, purpose, points }` → `{ epoch, responses: MemberPartials[] }` (napi shape)                                                             |
| `ceremony.dkg`                                                 | `{ session, threshold, members }`                                                                                                                       |
| `ceremony.reshare`                                             | `{ session, vault, current?, dealers, newThreshold, newSpendingThreshold, newMembers }`                                                                 |
| `ceremony.status`                                              | `{ session }` → `{ kind, status, failure, result }`                                                                                                     |
| `proposals.put/list/get`, `approvals.put/list`, `receipts.put` | canonical wire bodies as hex                                                                                                                            |
| `audit.list`                                                   | `{ vault, limit }` → audit entries                                                                                                                      |
| `events.subscribe`                                             | then `event` notifications: `ceremony`, `invalidPartials`, `committeeActivated`, `removed`, `configMismatch`, `unexpectedSpend`, `proposal`, `approval` |

Errors: `-32001` unauthenticated, `-32601` unknown method, `-32700` parse error, `-32000` keeper error with its message.

`@tecpan/vault-wallet` provides `launchKeeper` (spawn with the master key and a fresh token on stdin), `KeeperClient`,
`KeeperPartialsSource` and `keeperManifest`.

## Peer transport (Phase 8)

`Noise_XX_25519_ChaChaPoly_BLAKE2s` over TCP, static keys = the keepers' X25519 transport keys, prologue
`tecpan.keeper.channel.v0`. An inbound connection completes only if the peer's key belongs to a current committee or
ceremony; an outbound connection must reach exactly the expected key. Frames (signed wire messages, ≤ 1 MiB) are split
into Noise messages. One connection per peer serves both directions. Addresses are `tcp://host:port`. The inner
`SignedMessage` is still verified and its signer must map to the channel's key, so authenticity never depends on the
transport.

## Known limits

- An ECDH oracle is inherent: any current member can obtain `address_secret · P` for any point. That is what membership
  means (members can read the vault). The budget, the audit log and purpose tags bound and record its use.
- The ECDH purpose is self-reported; keepers cannot tell a note's ephemeral key from any other point.
- Liveness: any member can stall a ceremony by staying silent or disputing; the ceremony aborts and blames when it can.
- Without an engine reporting configs, a keeper keeps serving a superseded committee until told. The old shares no
  longer combine with the new committee's, and the contract no longer names them.

## Tests

| Suite                   | Covers                                                                                                                                                                                                                                                                   |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `tests/custody.rs`      | Round trip, file permissions, wrong master key, tampered and swapped files, rename and destroy, passphrase salt                                                                                                                                                          |
| `tests/ceremony.rs`     | DKG agreement and ECDH, blame for a dealer standing by a bad share, no blame for a valid justification, equivocation, timeout, reshare correctness and old/new share separation, bad dealer                                                                              |
| `tests/keeper.rs`       | In-memory multi-keeper: ECDH agreement and audit, lying member detected and routed around, budget, reshare activation and removal, forged request from a removed member, timeouts, restart recovery, proposal and approval replication, unexpected spend, unknown config |
| `tests/transport.rs`    | Noise TCP: 300 KB frames both ways, unknown peer refused, unexpected key refused                                                                                                                                                                                         |
| `tests/rpc.rs`          | Socket permissions, authentication, methods, errors, event subscription                                                                                                                                                                                                  |
| vault-wallet end to end | The Phase 7 suite against four keeper processes: DKG, ECDH for both engines, reshare, activation, removal                                                                                                                                                                |
