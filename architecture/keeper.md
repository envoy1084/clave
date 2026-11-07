# Keeper

## Purpose

`clave-keeper` (`crates/keeper`) is the daemon each member runs, as a child of the member's `claved` or standalone on
the member's own server. It is the only place a viewing share exists. It serves threshold ECDH to its owner's engine
and to the other members' keepers, runs the DKG and reshare ceremonies, hosts and joins founding lobbies, keeps the
proposal store, and activates or destroys shares as its owner reports the vault's on-chain committee.

## Responsibilities and trust

| The keeper…                                                           | …never                                                  |
| --------------------------------------------------------------------- | ------------------------------------------------------- |
| holds one viewing share per vault and committee epoch                 | exports a share (no RPC, CLI or log path returns one)   |
| answers ECDH for its engine and for current committee members         | answers anyone else, another epoch, or a removed member |
| joins ceremonies its owner starts, with the parameters its owner gave | joins a ceremony because a peer asked                   |
| activates a reshared committee when its engine reports it on chain    | activates on a peer's word                              |
| verifies every peer partial (DLEQ) before returning it                | trusts a partial it has not verified                    |
| hears strangers only on the lobby lane while it hosts an open lobby   | starts a DKG from a lobby plan without its owner        |

The engine (`claved`, the PXE host) is the keeper's owner: it holds the RPC token and is trusted to report on-chain
configs honestly. Other members' keepers are not trusted beyond their signatures and proofs.

## Process and data directory

```text
<data>/
  secrets/          one sealed file per secret (0700 dir, 0600 files)
    check.sealed                   master-key check
    identity.sealed                Ed25519 keeper identity seed
    transport.sealed               X25519 transport key
    share-<vault>-<epoch>.sealed   active viewing share
    dkg-<session>.sealed           finished DKG, waiting for vault registration
    pending-<session>.sealed       finished reshare, waiting for on-chain activation
    lobby-<id>.sealed              invite secret of an open lobby (destroyed when it starts or closes)
  keeper.db         SQLite (WAL, secure_delete): manifests, ceremonies, proposals, approvals, rejections, receipts,
                    audit, observations, lobbies
  keeper.toml       optional configuration
  keeper.json       public identity, peer id, addresses, RPC socket (written at start)
  keeper.sock       RPC socket (owner-only); rpc.token when no token is supplied
  salt              Argon2id salt (passphrase mode)
```

`clave-keeper run` prints one JSON line `{"ready": {identity, transport, peerId, addresses, rpcSocket}}` on stdout when
the socket is listening; logs go to stderr (`--log-format json` for structured logs). SIGTERM or Ctrl-C shuts it down.
The data directory is `--data-dir` or `CLAVE_KEEPER_DATA`. Network flags (all repeatable): `--listen <multiaddr>`
(default loopback QUIC and TCP on random ports), `--relay <multiaddr>/p2p/<id>`, `--external <multiaddr>`.
`clave-keeper rpc --socket --token-file <method> [params]` calls one RPC method (for operators of server keepers and
for scripted tests).

`keeper.toml` may set the following; command-line flags take precedence.

| Key                      | Default       | Meaning                                         |
| ------------------------ | ------------- | ----------------------------------------------- |
| `listen`, `external`     | loopback      | Listen and advertised multiaddrs                |
| `relays`                 | none          | Relay multiaddrs with `/p2p/<id>`               |
| `rpc_socket`             | `keeper.sock` | RPC socket path                                 |
| `round_timeout_secs`     | 120           | Ceremony round deadline                         |
| `ecdh_timeout_ms`        | 15 000        | Gatherer timeout                                |
| `ecdh_points_per_minute` | 60 000        | Per-member ECDH budget (token bucket)           |
| `max_skew_secs`          | 300           | Clock skew accepted on signed frames            |
| `sync_interval_secs`     | 60            | Proposal-store sync period                      |
| `mailbox_interval_secs`  | 30            | Relay mailbox poll period                       |
| `mailbox_ttl_days`       | 7             | Days of mailbox ids fetched and envelope expiry |

## Custody

- **Master key.** Seals every secret. Either supplied by the supervisor on stdin (`--secrets-stdin`, one JSON line
  `{"masterKey", "rpcToken"}`; `claved` keeps the key in the profile's `secrets.json`, see [cli.md](cli.md)), or
  derived from `CLAVE_KEEPER_PASSPHRASE` with Argon2id (64 MiB, 3 passes, 1 lane) over a per-keeper salt. A sealed
  check value detects a wrong key before any secret is touched. The keeper has no keychain dependency.
- **Sealed files.** XChaCha20-Poly1305 with a random 24-byte nonce; the associated data is
  `"clave.keeper.sealed.v0:" ‖ secret name`, so files cannot be swapped. Writes go to a temporary file, are fsynced,
  renamed, and the directory fsynced. Destruction overwrites the file before unlinking it. Share values are zeroized in
  memory after use.
- **Shares.** Stored with their index, threshold, participants, the committee's Feldman commitments and transcript; on
  load the share must match its committed verification share (`KeyShare::restore`).
- **Ceremony secrets** (polynomials, received sub-shares) are never persisted. A keeper restarted mid-ceremony marks it
  aborted ("keeper restarted during the ceremony"); members start a new session.

## Committee model

A manifest (wire `Manifest`) becomes a `Committee` only if the structure is valid (1..=8 members indexed in slot order,
`1 ≤ k ≤ n`, `2 ≤ t_v ≤ n`, distinct identities), the vault address equals `x(preaddress · G + Ivpk)`, and the
verification shares lie on one degree-`t_v - 1` polynomial whose constant term is `Ivpk` (`Commitments::interpolate`).
The committee's `committee_commitment` is what the vault stores on chain.

**Committee epoch** = the vault config version at which the committee became active (0 after the DKG). Partials and
envelopes are bound to it. Config changes that keep the committee do not change it.

## Threshold ECDH

**Responder** (a peer's `EcdhRequest`): the frame's channel key must belong to the signer; the signature, epoch, clock
skew and message id pass the `ReplayGuard`; the signer must be a current member; the batch must fit the member's budget
(token bucket refilling `per_minute / 60` points per second, 60 000 per minute by default); every point must be on the
curve and not the identity. Partials are computed off the async runtime with the share held behind an `Arc`, so
concurrent requests do not serialise on the state lock. Every request, served or refused, is written to the audit log
(time, requester, purpose, point count, outcome). A refused request gets no answer.

**Gatherer** (`ecdh.batch` from the engine): computes this keeper's partials, sends one signed request to every other
member, and verifies each response's DLEQ proofs (off the runtime) until `t_v` members are collected or the timeout
passes (15 s by default). A member whose proofs fail is reported (`invalidPartials` event) and skipped; the batch fails
with `Unavailable` if fewer than `t_v` answer validly. The engine verifies the returned partials again before
combining. ECDH requests are not retried.

## Ceremonies

Pure state machines (`ceremony::dkg`, `ceremony::reshare`): inputs are signed messages from known participants and
clock ticks, outputs are messages and a status. The keeper persists results and status; messages that arrive before
the owner starts a session are buffered (16 sessions × 64 messages) and replayed when it does.

- **DKG.** Participants in agreed order; round 1 commitments and proof of knowledge (broadcast), round 2 shares sealed
  to each recipient's transport key (envelope kind 1, epoch 0), finalisation, then every member must confirm the same
  transcript hash. Result: the share in `dkg-<session>`, and `{index, threshold, groupKey, verificationShares,
  transcript}` for the engine.
- **Vault registration** (`vault.register`): after deriving the vault, each engine registers the manifest with its
  keeper. The keeper checks it against its DKG result (index, threshold, group key and every verification share) and
  activates the share as `share-<vault>-0`.
- **Reshare** (`ceremony.reshare`). Exactly `t_v` current members deal (sub-share envelopes, kind 2, at the old epoch)
  to the new committee; new members check each deal's constant term against `λ_i · Y_i` from the current manifest,
  combine, and confirm one transcript. A joining member's keeper receives the current manifest from its engine. The new
  share waits in `pending-<session>` and the old committee keeps serving.
- **Echo.** Every participant forwards each dealer's signed `DkgRound1` / `ReshareDeal` once to the others; two
  different signed packages from one dealer abort the ceremony and blame it.
- **Retries.** Ceremony sends are retried with exponential backoff (200 ms doubling to 10 s, plus up to 255 ms jitter)
  until the round deadline, so members that start a session a little later still receive everything.
- **Activation** (`vault.observeConfig { vault, version, committeeCommitment }`): the keeper compares the reported
  commitment with its active committee and its completed reshares, then does nothing, activates the pending share at
  epoch = version and destroys the old one, or destroys its share and marks the vault removed. The full table is in
  [membership.md](membership.md#how-it-works).

**Failure handling.** Any malformed or inconsistent message aborts with the sender blamed. An invalid proof of knowledge
or a deal that is not the dealer's real share blames the dealer publicly. A bad share triggers complaint →
justification: an invalid justification (or none before the deadline) blames the dealer; a valid one blames nobody,
because others cannot tell what was sent. Either way the revealed share is public and the ceremony aborts. Differing
transcripts abort as equivocation. A round past its deadline (120 s by default) aborts, blaming the participants whose
messages are missing.

## Lobbies

Founding a vault needs every member's keeper identity, transport key, addresses and signing key before the DKG. A
lobby collects them without a coordinator: the founder's keeper hosts it, co-founders' keepers join it with an invite,
and the founder's engine fixes the plan. Wire types are in [wire.md](wire.md#lobbies-clave_wirelobby).

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
  time (`clave_crypto::lobby`), the signing key is a valid point, and the roster stays at ≤ 8 members with distinct
  identities, signing keys and transport keys. A re-join from the same identity replaces its entry. Each change bumps
  the revision and is sent as a signed `LobbyState` to every guest (retried for 30 s, then the relay mailbox).
- **Guest.** Accepts `LobbyState` / `LobbyPlan` / closing `LobbyLeave` only when signed by the host identity from the
  invite, on a channel bound to it, and applies only newer roster revisions. A plan is accepted only if its first
  member is the host, thresholds fit (`1 ≤ k ≤ n`, `2 ≤ t_v ≤ n`), members are distinct, this keeper's entry matches
  the signing and transport keys it joined with, and its sealed secrets open.
- **Start** (host only, open lobby): a fresh random DKG session, members in roster order, `k`, `t_v`, the salt (a
  canonical field element) and one opaque secrets blob (≤ 1 KiB, identical for every member, never interpreted) sealed
  to each member including the host (envelope kind 4). Admission closes; the invite secret is destroyed.
- **Secrets at rest.** Members keep only the plan with their sealed entry; `lobby.status` opens it with the keeper's
  transport key on demand, so the secrets survive restarts without a second copy.
- **Leave / close.** A guest's `LobbyLeave` removes it from an open roster (the others get the new state); the host's
  `LobbyLeave` closes the lobby for every guest. Started and closed lobbies refuse joins.
- **No ceremony on a peer's word.** The keeper never starts the DKG from a plan: each member's engine reads the plan
  and calls `ceremony.dkg` with it.

## Proposal store

`proposals.put`, `approvals.put`, `rejections.put` and `receipts.put` store records and push them to the other members,
who store them after checking the sender is a member. Approvals and rejections are verified against the slot's signing
key (and the proposal's approval or rejection message) before they are stored, one of each per slot. An approval that
arrives before its proposal is held (up to 1024) until it does; a rejection is dropped and comes back with the next
sync. `vault.observeTx` raises `unexpectedSpend` for a vault transaction that matches no stored receipt.

Every newly stored record raises an event, whether it was put locally or arrived from a member by push, sync or
mailbox: `proposal`, `approval`, `rejection` and `receipt`, each naming its vault. Storing the same record again raises
nothing; a receipt that names a different transaction for the same proposal replaces the old one and is reported
again.

Members that were offline catch up two ways: a push that fails is sealed into the recipient's relay mailbox, and every
minute each keeper sends the others a `SyncRequest` inventory and receives what it lacks
([networking.md](networking.md#replication)).

## Local RPC

JSON-RPC 2.0, one JSON object per line (at most 8 MiB), on an owner-only Unix socket (a named pipe rejecting remote
clients on Windows). The first call must be `session.auth { token }` (compared in constant time); everything else is
refused until then. Requests on a session run concurrently.

| Method                                                                                | Params → result                                                                                                                          |
| ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `keeper.info`                                                                         | → `{ identity, transport, addresses, version }`                                                                                          |
| `peers.status`                                                                        | `{ vault }` → reachability of every committee member (below)                                                                             |
| `vault.sync`                                                                          | `{ vault }`: sync proposals, approvals, rejections and receipts with the other members now                                               |
| `mailbox.poll`                                                                        | → `{ delivered }`: fetch and process relay mailboxes now                                                                                 |
| `vault.list`, `vault.status`                                                          | `{ vault }` → `{ manifest, status, index, serving }`                                                                                     |
| `vault.register`                                                                      | `{ session, manifest }` → vault                                                                                                          |
| `vault.observeConfig`                                                                 | `{ vault, version, committeeCommitment }` → `{ observation }` (`current`, `activated`, `removed`, `unknown`)                             |
| `vault.observeTx`                                                                     | `{ vault, txHash }` → `{ known }`                                                                                                        |
| `ecdh.batch`                                                                          | `{ vault, epoch, purpose, points }` → `{ epoch, responses: MemberPartials[] }` (napi shape)                                              |
| `ceremony.dkg`                                                                        | `{ session, threshold, members }`                                                                                                        |
| `ceremony.reshare`                                                                    | `{ session, vault, current?, dealers, newThreshold, newSpendingThreshold, newMembers }`                                                  |
| `ceremony.status`                                                                     | `{ session }` → `{ kind, status, failure, result }` (`running`, `completed`, `aborted`, `activated`)                                     |
| `proposals.put/list/get`, `approvals.put/list`, `rejections.put/list`, `receipts.put` | canonical wire bodies as hex                                                                                                             |
| `receipts.list`                                                                       | `{ vault }` → `[{ proposal, txHash, at }]`, newest first (`at`: unix seconds the receipt was stored)                                     |
| `audit.list`                                                                          | `{ vault, limit }` → audit entries                                                                                                       |
| `lobby.open`                                                                          | `{ name, threshold, viewingThreshold, network, member: { name, signerKey } }` → `{ lobby, invite }`                                      |
| `lobby.inspect`                                                                       | `{ invite }` → `{ lobby, name, hostName, threshold, viewingThreshold, network, host: { identity, transport, addresses } }` (decode only) |
| `lobby.join`                                                                          | `{ invite, member: { name, signerKey } }` → `{ lobby }` once the host acknowledged (up to about 30 s)                                    |
| `lobby.status`                                                                        | `{ lobby }` → lobby (below), with `plan.secrets`                                                                                         |
| `lobby.list`                                                                          | → lobbies, without `plan.secrets`                                                                                                        |
| `lobby.start`                                                                         | `{ lobby, threshold, viewingThreshold, salt, secrets }` (host; `secrets` one hex blob for every member) → lobby                          |
| `lobby.close`, `lobby.leave`                                                          | `{ lobby }` (host closes / guest leaves an open lobby) → lobby                                                                           |
| `events.subscribe`                                                                    | then `event` notifications (below)                                                                                                       |

A lobby: `{ lobby, role: "host" | "guest", status: "open" | "started" | "closed", name, hostName, threshold,
viewingThreshold, network, host: { identity, transport, addresses }, members: [{ index, name, signerKey, identity,
transport, addresses, you }], plan?: { session, threshold, viewingThreshold, salt, secrets? }, invite? }`. Lobby ids are
16 bytes of `0x` hex, `signerKey` is the `{ x, y }` point shape, and `invite` is present while this keeper hosts the
open lobby. A guest's roster is empty until the host's first `LobbyState` arrives.

`peers.status` returns one entry per member of the vault's current committee, in index order, this keeper included:
`{ index, identity, peerId, you, connected, relayed, lastSeen, rttMs }`. `connected` means the transport has an open
connection to the member, `relayed` that every open connection is a relay circuit, `lastSeen` the unix seconds of the
last established connection or received frame, `rttMs` the last ping round trip; the last two are `null` when unknown.
The entry with `you: true` is always `connected` with the other fields `false` / `null`. How the transport tracks this
is in [networking.md](networking.md#reachability).

Events are `{ "type": <kind>, ... }` with unprefixed hex values:

| Event                | Fields                                        |
| -------------------- | --------------------------------------------- |
| `ceremony`           | `session`, `kind` (`dkg`/`reshare`), `status` |
| `invalidPartials`    | `vault`, `member`                             |
| `committeeActivated` | `vault`, `epoch`                              |
| `removed`            | `vault`                                       |
| `configMismatch`     | `vault`, `version`                            |
| `unexpectedSpend`    | `vault`, `txHash`                             |
| `proposal`           | `vault`, `id`                                 |
| `approval`           | `vault`, `proposal`, `slot`                   |
| `rejection`          | `vault`, `proposal`, `slot`                   |
| `receipt`            | `vault`, `proposal`, `txHash`                 |
| `lobby`              | `lobby`, `status`, `members` (count)          |

Errors: `-32001` unauthenticated, `-32601` unknown method, `-32600` invalid request, `-32700` parse error, `-32000`
keeper error with its message.

`@clave/vault-wallet` provides `launchKeeper` (spawn with the master key and a fresh token on stdin), `KeeperClient`,
`KeeperPartialsSource` and `keeperManifest`.

## Peer transport

libp2p (QUIC and TCP with Noise, relay circuits, DCUtR); the peer id is derived from the keeper's Ed25519 identity. The
allow list holds the members of the keeper's active committees and running ceremonies, the peers of its open lobbies
(recomputed whenever they change) and its relays; every other connection is closed, except while the keeper hosts an
open lobby, when strangers may connect but are heard only on the lobby lane, rate limited. Inbound frames are otherwise
delivered only from members, and the inner `SignedMessage` signer must be that member, so authenticity never depends on
the transport. Details are in [networking.md](networking.md).

## Guarantees and limits

- An ECDH oracle is inherent: any current member can obtain `address_secret · P` for any point. That is what membership
  means (members can read the vault). The budget, the audit log and purpose tags bound and record its use.
- The ECDH purpose is self-reported; keepers cannot tell a note's ephemeral key from any other point.
- **Equivocation attribution is local.** Sends to a peer are not ordered. A member that processes another member's
  `Abort` before the echoed conflicting package aborts with "aborted by member N" and no blame; the member that
  compared the two signed packages blames the dealer. Every honest member aborts.
- Liveness: any member can stall a ceremony by staying silent or disputing; the ceremony aborts and blames when it can.
- **Config and transaction reports come only from the owner.** Activation, removal and `unexpectedSpend` happen only
  when the engine calls `vault.observeConfig` / `vault.observeTx`. `claved` does not call either yet, so under the CLI a
  keeper keeps serving its founding committee and raises no `unexpectedSpend`. Superseded shares no longer combine with
  a new committee's, and the contract no longer names them.
- Pending reshare shares are kept until activated; nothing discards them if the rotation never lands.

## Code and tests

| Path                             | Contents                                                    |
| -------------------------------- | ----------------------------------------------------------- |
| `crates/keeper/src/main.rs`      | `run` and `rpc` commands, flags                             |
| `crates/keeper/src/config.rs`    | `keeper.toml`                                               |
| `crates/keeper/src/custody/`     | Master key, sealed files, shares, long-lived keys           |
| `crates/keeper/src/store/`       | SQLite schema and queries                                   |
| `crates/keeper/src/committee.rs` | Manifest → `Committee` validation                           |
| `crates/keeper/src/ceremony/`    | DKG and reshare state machines                              |
| `crates/keeper/src/keeper/`      | ECDH, ceremonies, lobbies, proposals, sync, mailbox, vaults |
| `crates/keeper/src/rpc/`         | Local RPC server and methods                                |
| `crates/keeper/src/transport/`   | libp2p transport, connection gate, in-memory test transport |
| `crates/keeper/src/events.rs`    | Events                                                      |
| `crates/keeper/src/limits.rs`    | ECDH token bucket                                           |

| Suite                  | Covers                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tests/custody.rs`     | Round trip, file permissions, wrong master key, tampered and swapped files, rename and destroy, passphrase salt                                                                                                                                                                                                                                               |
| `tests/ceremony.rs`    | DKG agreement and ECDH, blame for a dealer standing by a bad share, no blame for a valid justification, equivocation, timeout, reshare correctness and old/new share separation, bad dealer                                                                                                                                                                   |
| `tests/keeper.rs`      | In-memory multi-keeper: ECDH agreement and audit, lying member detected and routed around, budget, reshare activation and removal, forged request from a removed member, timeouts, restart recovery, proposal, approval and receipt replication with their events, member reachability (a dropped member shows unreachable), unexpected spend, unknown config |
| `tests/catch_up.rs`    | Equivocating dealer caught by echo, offline member catches up through sync, offline member reads its mailbox hours later (relay sees ciphertext and unlinkable ids only)                                                                                                                                                                                      |
| `tests/lobby.rs`       | In-memory lobbies: roster agreement, identical plan and secrets for every guest, wrong proof / lobby / damaged invite / reused signing key refused, no join after start or close, leave and close, guest restart keeps its plan                                                                                                                               |
| `tests/p2p.rs`         | Real libp2p: DKG, ECDH and replication, members connected, seen and pinged afterwards; outsiders refused; a keeper reachable only through the relay; mailbox delivery after a restart; strangers join a lobby, receive the plan and run the DKG, outsiders refused again after the start                                                                      |
| `tests/rpc.rs`         | Socket permissions, authentication, methods, errors, event subscription, the `lobby.*` flow and `lobby` events, `peers.status`, `receipts.list` and `receipt` events                                                                                                                                                                                          |
| `tests/network/run.sh` | Docker: three keepers on separate internal networks, relay only ([networking.md](networking.md#code-and-measurements))                                                                                                                                                                                                                                        |
