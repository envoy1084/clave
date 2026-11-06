# Keeper

`crates/keeper` (`tecpan-keeper`) is the daemon each member runs, on their own machine (supervised by the desktop app)
or on their own server. It is the only place a viewing share exists. It serves threshold ECDH to its owner's engine and
to the other members' keepers, runs the DKG and reshare ceremonies, keeps the proposal store, and activates or destroys
shares as the vault's on-chain committee changes.

Status: Phases 8–9. Keepers reach each other over a libp2p mesh limited to the committee, with optional relays for
NAT traversal and offline mailboxes (see `networking.md`).

## Responsibilities and trust

| The keeper…                                                           | …never                                                  |
| --------------------------------------------------------------------- | ------------------------------------------------------- |
| holds one viewing share per vault and committee epoch                 | exports a share (no RPC, CLI or log path returns one)   |
| answers ECDH for its engine and for current committee members         | answers anyone else, another epoch, or a removed member |
| joins ceremonies its owner starts, with the parameters its owner gave | joins a ceremony because a peer asked                   |
| activates a reshared committee when its engine observes it on chain   | activates on a peer's word                              |
| verifies every peer partial (DLEQ) before returning it                | trusts a partial it has not verified                    |
| hears strangers only on the lobby lane while it hosts an open lobby   | starts a DKG from a lobby plan without its owner        |

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
    lobby-<id>.sealed              invite secret of an open lobby (destroyed when it starts or closes)
  keeper.db         SQLite (WAL, secure_delete): manifests, ceremonies, proposals, approvals, receipts, audit, observations,
                    lobbies
  keeper.toml       optional configuration
  keeper.json       public identity, peer id, addresses, RPC socket (written at start)
  keeper.sock       RPC socket (owner-only); rpc.token when no token is supplied
  salt              Argon2id salt (passphrase mode)
```

`tecpan-keeper run` prints one JSON line `{"ready": {identity, transport, peerId, addresses, rpcSocket}}` on stdout when
the socket is listening; logs go to stderr (`--log-format json` for structured logs). SIGTERM or Ctrl-C shuts it down.
Network flags (all repeatable): `--listen <multiaddr>` (default loopback QUIC and TCP on random ports), `--relay
<multiaddr>/p2p/<id>`, `--external <multiaddr>`. `tecpan-keeper rpc --socket --token-file <method> [params]` calls one
RPC method (for operators of server keepers and for scripted tests).

`keeper.toml` may set `listen`, `external`, `relays`, `rpc_socket`, `round_timeout_secs` (120), `ecdh_timeout_ms`
(15 000), `ecdh_points_per_minute` (60 000), `max_skew_secs`, `sync_interval_secs` (60), `mailbox_interval_secs` (30)
and `mailbox_ttl_days` (7); command-line flags take precedence.

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
transcript. A joining member's keeper receives the current manifest from its engine.

**Echo.** Every participant forwards each dealer's signed `DkgRound1` / `ReshareDeal` once to the others; two
different signed packages from one dealer abort the ceremony and blame it. Ceremony sends are retried with backoff
until the round deadline, so members that start a session a little later still receive everything. The new share waits in
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

## Lobbies

Founding a vault needs every member's keeper identity, transport key, addresses and signing key before the DKG. A
lobby collects them without a coordinator: the founder's keeper hosts it, co-founders' keepers join it with an invite,
and the founder's engine fixes the plan. Wire types are in `wire.md` (`tecpan_wire::lobby`).

```text
founder engine ── lobby.open ──▶ host keeper            invite (out of band: chat, QR) ──▶ co-founder engine
                                     ▲  │                                                   │ lobby.inspect, lobby.join
                                     │  └──── LobbyState (every roster change) ────▶ guest keeper
                                     └─────── LobbyJoin { proof } ◀──────────────────┘
founder engine ── lobby.start ─▶ host keeper ── LobbyPlan (secrets sealed per member) ──▶ guest keepers
every engine:  lobby.status → plan → ceremony.dkg { session = plan.session, threshold = t_v, members = plan order }
```

- **Open.** The host is member 1. The invite carries the lobby id, a random 32-byte join secret, the host's keeper
  contact and display name, the vault name, proposed `k` / `t_v` and the engine's opaque network label. The secret is
  sealed in custody (`lobby-<id>`); the record (roster, plan) is in `lobbies`.
- **Join** (host side). A `LobbyJoin` is accepted only if the lobby is open, the frame is signed by the joiner's keeper
  identity on a channel bound to it (libp2p peer id, or the claimed transport key), the HMAC proof verifies in constant
  time (`tecpan_crypto::lobby`), the signing key is a valid point, and the roster stays at ≤ 8 members with distinct
  identities, signing keys and transport keys. A re-join from the same identity replaces its entry. Each change bumps
  the revision and is sent as a signed `LobbyState` to every guest (retried for 30 s, then the relay mailbox).
- **Guest.** Accepts `LobbyState` / `LobbyPlan` / closing `LobbyLeave` only when signed by the host identity from the
  invite, on a channel bound to it, and applies only newer roster revisions. A plan is accepted only if its first member
  is the host, thresholds fit (`1 ≤ k ≤ n`, `2 ≤ t_v ≤ n`), members are distinct, this keeper's entry matches the
  signing and transport keys it joined with, and its sealed secrets open.
- **Start** (host only, open lobby): a fresh random DKG session, members in roster order, `k`, `t_v`, the salt (a
  canonical field element) and one opaque secrets blob (≤ 1 KiB, identical for every member, never interpreted)
  sealed to each member including the host (envelope kind 4, `viewing-keys.md`). Admission closes; the invite secret
  is destroyed.
- **Secrets at rest.** Members keep only the plan with their sealed entry; `lobby.status` opens it with the keeper's
  transport key on demand, so the secrets survive restarts without a second copy.
- **Leave / close.** A guest's `LobbyLeave` removes it from an open roster (the others get the new state); the host's
  `LobbyLeave` closes the lobby for every guest. Started and closed lobbies refuse joins.
- **No ceremony on a peer's word.** The keeper never starts the DKG from a plan: each member's engine reads the plan
  and calls `ceremony.dkg` with it, after its user confirmed.
- Lobby frames use scope = lobby id zero-padded to 32 bytes, epoch 0, and the usual freshness and replay rules.

## Proposals

`proposals.put`, `approvals.put` and `receipts.put` store records and push them to the other members, who store them
after checking the sender is a member. Approvals are verified against the slot's signing key and the proposal's
approval message before they are stored; an approval that arrives before its proposal is held until it does.
`vault.observeTx` raises `unexpectedSpend` for a vault transaction that matches no stored receipt.

Every newly stored record raises an event, whether it was put locally or arrived from a member by push, sync or
mailbox: `proposal`, `approval` and `receipt`, each naming its vault. Storing the same record again raises nothing; a
receipt that names a different transaction for the same proposal replaces the old one and is reported again.

Members that were offline catch up two ways: a push that fails is sealed into the recipient's relay mailbox, and every
minute each keeper sends the others a `SyncRequest` inventory and receives what it lacks (`networking.md`).

## Local RPC

JSON-RPC 2.0, one JSON object per line, on an owner-only Unix socket (a named pipe rejecting remote clients on
Windows). The first call must be `session.auth { token }` (compared in constant time); everything else is refused
until then. Requests on a session run concurrently.

| Method                                                         | Params → result                                                                                                                                                             |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `keeper.info`                                                  | → `{ identity, transport, addresses, version }`                                                                                                                             |
| `peers.status`                                                 | `{ vault }` → reachability of every committee member (below)                                                                                                                |
| `vault.sync`                                                   | `{ vault }`: sync proposals, approvals and receipts with the other members now                                                                                              |
| `mailbox.poll`                                                 | → `{ delivered }`: fetch and process relay mailboxes now                                                                                                                    |
| `vault.list`, `vault.status`                                   | `{ vault }` → `{ manifest, status, index, serving }`                                                                                                                        |
| `vault.register`                                               | `{ session, manifest }` → vault                                                                                                                                             |
| `vault.observeConfig`                                          | `{ vault, version, committeeCommitment }` → `{ observation }`                                                                                                               |
| `vault.observeTx`                                              | `{ vault, txHash }` → `{ known }`                                                                                                                                           |
| `ecdh.batch`                                                   | `{ vault, epoch, purpose, points }` → `{ epoch, responses: MemberPartials[] }` (napi shape)                                                                                 |
| `ceremony.dkg`                                                 | `{ session, threshold, members }`                                                                                                                                           |
| `ceremony.reshare`                                             | `{ session, vault, current?, dealers, newThreshold, newSpendingThreshold, newMembers }`                                                                                     |
| `ceremony.status`                                              | `{ session }` → `{ kind, status, failure, result }`                                                                                                                         |
| `proposals.put/list/get`, `approvals.put/list`, `receipts.put` | canonical wire bodies as hex                                                                                                                                                |
| `receipts.list`                                                | `{ vault }` → `[{ proposal, txHash, at }]`, newest first (`at`: unix seconds the receipt was stored)                                                                        |
| `audit.list`                                                   | `{ vault, limit }` → audit entries                                                                                                                                          |
| `lobby.open`                                                   | `{ name, threshold, viewingThreshold, network, member: { name, signerKey } }` → `{ lobby, invite }`                                                                         |
| `lobby.inspect`                                                | `{ invite }` → `{ lobby, name, hostName, threshold, viewingThreshold, network, host: { identity, transport, addresses } }` (decode only)                                    |
| `lobby.join`                                                   | `{ invite, member: { name, signerKey } }` → `{ lobby }` once the host acknowledged (up to ~30 s)                                                                            |
| `lobby.status`                                                 | `{ lobby }` → lobby (below), with `plan.secrets`                                                                                                                            |
| `lobby.list`                                                   | → lobbies, without `plan.secrets`                                                                                                                                           |
| `lobby.start`                                                  | `{ lobby, threshold, viewingThreshold, salt, secrets }` (host; `secrets` one hex blob for every member) → lobby                                                             |
| `lobby.close`, `lobby.leave`                                   | `{ lobby }` (host closes / guest leaves an open lobby) → lobby                                                                                                              |
| `events.subscribe`                                             | then `event` notifications: `ceremony`, `invalidPartials`, `committeeActivated`, `removed`, `configMismatch`, `unexpectedSpend`, `proposal`, `approval`, `receipt`, `lobby` |

A lobby: `{ lobby, role: "host" | "guest", status: "open" | "started" | "closed", name, hostName, threshold,
viewingThreshold, network, host: { identity, transport, addresses }, members: [{ index, name, signerKey, identity,
transport, addresses, you }], plan?: { session, threshold, viewingThreshold, salt, secrets? }, invite? }`. Lobby ids
are 16 bytes of `0x` hex, `signerKey` is the `{ x, y }` point shape, `invite` is present while this keeper hosts the
open lobby. A guest's roster is empty until the host's first `LobbyState` arrives. The `lobby` event is
`{ "type": "lobby", "lobby": <hex, no 0x>, "status": "open" | "started" | "closed", "members": n }`, emitted on open or
join, every roster change, start and close.

Record events carry unprefixed hex like the other events: `{ "type": "proposal", "vault", "id" }`,
`{ "type": "approval", "vault", "proposal", "slot" }` and `{ "type": "receipt", "vault", "proposal", "txHash" }`.

`peers.status` returns one entry per member of the vault's current committee, in index order, this keeper included:
`{ index, identity, peerId, you, connected, relayed, lastSeen, rttMs }`. `connected` means the transport has an open
connection to the member, `relayed` that every open connection is a relay circuit, `lastSeen` the unix seconds of the
last established connection or received frame, `rttMs` the last ping round trip; the last two are `null` when unknown.
The entry with `you: true` is always `connected` with the other fields `false` / `null`. An unknown vault is the usual
`-32000` keeper error. How the transport tracks this is in `networking.md`.

Errors: `-32001` unauthenticated, `-32601` unknown method, `-32700` parse error, `-32000` keeper error with its message.

`@tecpan/vault-wallet` provides `launchKeeper` (spawn with the master key and a fresh token on stdin), `KeeperClient`,
`KeeperPartialsSource` and `keeperManifest`.

## Peer transport

libp2p (QUIC and TCP with Noise, relay circuits, DCUtR); the peer id is derived from the keeper's Ed25519 identity. The
allow list holds the members of the keeper's active committees and running ceremonies and the peers of its open
lobbies (recomputed whenever they change) plus its relays; every other connection is closed, except while the keeper
hosts an open lobby, when strangers may connect but are heard only on the lobby lane, rate limited. Inbound frames are
otherwise delivered only from members, and the inner `SignedMessage` signer must be that member, so authenticity never
depends on the transport. Protocols, relay and
mailbox details are in `networking.md`.

## Known limits

- An ECDH oracle is inherent: any current member can obtain `address_secret · P` for any point. That is what membership
  means (members can read the vault). The budget, the audit log and purpose tags bound and record its use.
- The ECDH purpose is self-reported; keepers cannot tell a note's ephemeral key from any other point.
- Liveness: any member can stall a ceremony by staying silent or disputing; the ceremony aborts and blames when it can.
- Without an engine reporting configs, a keeper keeps serving a superseded committee until told. The old shares no
  longer combine with the new committee's, and the contract no longer names them.

## Tests

| Suite                   | Covers                                                                                                                                                                                                                                                                                                                                                        |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/custody.rs`      | Round trip, file permissions, wrong master key, tampered and swapped files, rename and destroy, passphrase salt                                                                                                                                                                                                                                               |
| `tests/ceremony.rs`     | DKG agreement and ECDH, blame for a dealer standing by a bad share, no blame for a valid justification, equivocation, timeout, reshare correctness and old/new share separation, bad dealer                                                                                                                                                                   |
| `tests/keeper.rs`       | In-memory multi-keeper: ECDH agreement and audit, lying member detected and routed around, budget, reshare activation and removal, forged request from a removed member, timeouts, restart recovery, proposal, approval and receipt replication with their events, member reachability (a dropped member shows unreachable), unexpected spend, unknown config |
| `tests/catch_up.rs`     | Equivocating dealer caught by echo, offline member catches up through sync, offline member reads its mailbox hours later (relay sees ciphertext and unlinkable ids only)                                                                                                                                                                                      |
| `tests/p2p.rs`          | Real libp2p: DKG, ECDH and replication, members connected, seen and pinged afterwards; outsiders refused; a keeper reachable only through the relay; mailbox delivery after a restart; strangers join a lobby, receive the plan and run the DKG, outsiders refused again after the start                                                                      |
| `tests/network/run.sh`  | Docker: three keepers on separate internal networks, relay only (`networking.md`)                                                                                                                                                                                                                                                                             |
| `tests/lobby.rs`        | In-memory lobbies: roster agreement, identical plan and secrets for every guest, wrong proof / lobby / damaged invite / reused signing key refused, no join after start or close, leave and close, guest restart keeps its plan                                                                                                                               |
| `tests/rpc.rs`          | Socket permissions, authentication, methods, errors, event subscription, the `lobby.*` flow and `lobby` events, `peers.status`, `receipts.list` and `receipt` events                                                                                                                                                                                          |
| vault-wallet end to end | The Phase 7 suite against four keeper processes: DKG, ECDH for both engines, reshare, activation, removal                                                                                                                                                                                                                                                     |
