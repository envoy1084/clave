# Networking

How keepers reach each other: a libp2p mesh limited to each vault's committee, with optional untrusted relays for
members behind NATs and for members that are offline. There is no coordinator server; any member (or anyone) can run a
relay, and a vault can name several.

Status: Phase 9.

## Topology

```text
 member A                      member B (behind NAT)            member C (offline for a day)
 engine ── RPC ── keeper ◀──── QUIC/TCP, Noise ────▶ keeper ── RPC ── engine
                    │  ▲                               │
                    │  └──── relay circuit (v2) ───────┘
                    │                ▲
                    └── deposit ──▶ relay ◀── fetch ── keeper (later)
                                   (ciphertext mailbox)
```

- **Routable addresses only.** A wildcard listener also reports `127.0.0.1`; a keeper with any routable address
  never advertises loopback, and a keeper never records a peer address equal to one of its own listen addresses.
  Either would make the dialer reach itself: libp2p settles a dial on its first transport success, so that
  self-connection (wrong peer id) fails the whole dial, relay circuit included. The namespace test caught this.
- **Direct first.** A keeper advertises its external and listen addresses first and its relay circuits
  (`<relay>/p2p-circuit`) last, at most 8 in total. Peers dial them in order; DCUtR upgrades a relayed connection to a
  direct one when hole punching works.
- **Relay as fallback.** A keeper started with `--relay <multiaddr>/p2p/<id>` (repeatable) reserves a slot on each
  relay and listens on its circuit, so members that cannot reach it directly still can.
- **Mailbox for offline members.** A frame that cannot be delivered is sealed into the recipient's pairwise mailbox on
  every configured relay; the recipient fetches it later. Periodic sync repairs anything still missing.

## Transport

libp2p 0.57: QUIC v1 and TCP (Noise XX + yamux), DNS, identify (`/tecpan/0`), ping, relay client, DCUtR,
request-response, and an allow list. The keeper's libp2p key is its Ed25519 identity, so its peer id is derived from
the identity in the manifest (`transport::peer_id`) and Noise authenticates it. Idle connections close after 2 minutes.

| Protocol             | Carries                                                                          | Limits                               |
| -------------------- | -------------------------------------------------------------------------------- | ------------------------------------ |
| `/tecpan/ecdh/0`     | `EcdhRequest`, `EcdhResponse`                                                    | request ≤ 1 MiB, empty ack           |
| `/tecpan/ceremony/0` | DKG and reshare messages, complaints, justifications, aborts, `Echo`             | request ≤ 1 MiB, empty ack           |
| `/tecpan/sync/0`     | `Proposal`, `Approval`, `ExecutionReceipt`, `SyncRequest`                        | request ≤ 1 MiB, empty ack           |
| `/tecpan/mailbox/0`  | `MailboxRequest` / `MailboxResponse` (keeper → relay only; keepers never accept) | 4 MiB per message, 256 KiB envelopes |

Every keeper frame is a `SignedMessage`; the lane is chosen by its body. Requests are one-way: the receiver
acknowledges with an empty response and answers, if at all, with its own request (an `EcdhResponse` is a request back
to the gatherer). Frames use a u32 length prefix (`tecpan_wire::p2p::FrameCodec`); request timeout 10 s, 30 s for
mailbox calls.

### Membership enforcement

- **Connections.** The allow list holds exactly the peer ids of current committees and running ceremonies (updated
  whenever the keeper's committees change) plus the configured relays. Any other peer's connection is closed before a
  protocol runs, so outsiders cannot even probe which protocols a keeper speaks.
- **Messages.** Inbound requests are accepted only from members, delivered as `Channel::Peer(identity)`, and the
  frame's signer must be that identity. The keeper then applies the same checks as before (signature, epoch, replay
  window, committee membership, budget). Authenticity never depends on the transport.
- **Outbound.** A keeper sends only to members of a committee or ceremony it belongs to.

### Retries

Ceremony messages are retried with exponential backoff (200 ms doubling to 10 s, with jitter) until the round deadline,
because members start a ceremony at slightly different times. ECDH requests are not retried: the gatherer collects the
first `t_v` valid answers within its timeout. Proposal records fall back to the mailbox.

## Replication

**Echo broadcast.** When a keeper receives a dealer's signed `DkgRound1` or `ReshareDeal`, it forwards the frame once
to every other participant inside an `Echo`. A recipient that sees two different signed packages from one dealer in
the same session aborts the ceremony and blames the dealer ("two different round 1" / deals). This turns a dealer that
sends different commitments to different members into a provable, attributable fault.

**Sync.** Every `sync_interval_secs` (60 s by default) and on `vault.sync`, a keeper sends each member a
`SyncRequest` with its inventory for the vault: proposal ids, `(proposal, slot)` approval references and receipted
proposals (the newest 4096 of each). The member answers with up to 1024 missing records, proposals first, as ordinary
signed frames; the requester syncs again for the rest. Approvals that arrive before their proposal are held (up to 1024)
and stored when it arrives. Replicated records are re-verified (membership, approval signatures) and skip the replay
window, since a record may legitimately be days old.

**Mailboxes.** For each ordered pair of members (sender `S`, recipient `R`, transport keys) and each UTC day:

```text
mailbox_id = SHA-256("tecpan.mailbox.v0" ‖ X25519(s, R) ‖ S_pub ‖ R_pub ‖ be32(day))   (day = unix time / 86400)
sealed     = HPKE Auth (X25519, HKDF-SHA256, ChaCha20-Poly1305), kind 3, session = mailbox_id, epoch = day
```

The sender deposits `RelayEnvelope { recipient: mailbox_id, expires_at, sealed }` at every relay (success if any
stored it). Every `mailbox_interval_secs` (30 s) and on `mailbox.poll`, a keeper computes the ids for every committee
peer over the last `mailbox_ttl_days` (7), fetches in chunks of 128 ids, opens each envelope with the sender's key,
processes the frame as direct traffic and deletes it at the relays.

## Relay (`tecpan-relay`)

One binary, two services on one libp2p identity:

- **Circuit relay v2.** 1024 reservations, 256 circuits, 16 per peer; circuits last up to 30 minutes and 64 MiB
  (libp2p's defaults of 2 minutes and 128 KiB are too small for a DKG or a large ECDH batch).
- **Mailbox store.** SQLite (`id, mailbox, expires_at, sealed, size, stored_at`): expiry at most 14 days ahead, 512
  envelopes and 32 MiB per mailbox, 1 GiB in total, 3600 deposits per peer per hour, fetches up to 3 MiB. Expired
  envelopes are purged every minute. The depositor's peer id is never stored.

`tecpan-relay run --data-dir --listen --external --health --allow-peer`; `/health` and `/metrics` (Prometheus: reservations,
circuits, deposits, fetches, rejections, stored bytes) on the health address. `--allow-peer` (repeatable) turns it into a
private relay. Deployment files: `crates/deploy/Dockerfile` (relay and keeper image) and systemd units with sandboxing.

### Trust model

The relay is untrusted for confidentiality and integrity and trusted only for availability.

| A relay can                                                                             | A relay cannot                                                      |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| see which peer ids reserve and which peer ids open circuits to which (circuit metadata) | read circuit traffic (Noise end to end between keepers)             |
| see mailbox ids, envelope sizes and timing, and the connection depositing or fetching   | read envelopes or link a mailbox id to a pair of keepers or a vault |
| drop, delay or delete traffic and envelopes                                             | forge or alter a frame (signed, and sealed with HPKE Auth)          |
| refuse service                                                                          | learn a share, a partial's meaning, a signer or a threshold         |

Mitigations: members can name several relays; mailbox ids rotate daily and are unlinkable without the pair's X25519
secret; anyone who knows a mailbox id (only the pair) can fetch and delete it. Members who need to hide even circuit
metadata run their own relay or keep direct connectivity.

### Metadata exposure

- Keepers of one vault learn each other's IP addresses (they are members; this is inherent to a peer mesh without a
  mixnet).
- Peer ids are derived from keeper identities, which appear only in the manifest shared among members, never on chain.
- Identify announces the protocol version and listen addresses to allowed peers only (the allow list closes other
  connections first).

## Measured

| Scenario                                                                                                                                    | Result                                         |
| ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `tests/p2p.rs` (loopback): DKG 2-of-3, ECDH, replication, reshare with a joiner, outsider refused, relay-only keeper, mailbox after restart | 5 tests, ~6 s                                  |
| Real binaries on one host (relay + 3 keepers, `network_scenario`)                                                                           | DKG 291 ms, ECDH 25 ms, total 710 ms           |
| Docker: 3 keepers on 3 internal networks, reachable only through the relay (3 runs)                                                         | DKG 258–262 ms, ECDH 7–12 ms, total 567–579 ms |

`crates/keeper/tests/network/run.sh` (`pnpm test:network`) builds the node image and starts a relay attached to three
internal Docker networks and one keeper on each. No keeper can route to another; the driver container has no network
and talks to each keeper over the RPC socket in its volume. It runs a 2-of-3 DKG, registers the vault, gathers
threshold ECDH from two keepers and checks they combine to the same point, puts a proposal on one keeper and waits for
all three to hold it.

## Known limits

- **Relay availability.** A vault whose members are all behind symmetric NATs depends on at least one reachable relay.
- **Mailbox discovery cost.** A keeper fetches `members × ttl_days` mailbox ids per poll (at most 8 × 7 = 56 ids).
- **Finished ceremonies linger.** A ceremony's participants stay on the allow list for one round timeout after it
  ends, so a member that finishes first still delivers its last messages to a joining member that is in none of its
  committees yet.
- **Echo is one hop.** It detects a dealer equivocating between honest members; it is not a full reliable broadcast
  under a malicious majority (ceremonies abort and blame instead of completing).
