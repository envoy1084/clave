# CLI and daemon

A member operates Clave from the terminal. `clave` (`crates/cli`, Rust) is a short-lived client; `claved`
(`packages/engine`, Node 24) is a per-profile background daemon that hosts the member's PXE, signer and founding and
proposal workflows, and supervises the member's `clave-keeper`. Node is a prerequisite and is not bundled.

```text
clave (Rust) ──JSON-RPC, unix socket──▶ claved (Node) ──JSON-RPC, unix socket──▶ clave-keeper (Rust)
 per command                              one per profile                           child of claved
 args, refs, rendering, prompts           PXE, signer, founding, proposals          viewing share, DKG, ECDH, p2p
```

## Profiles and files

`CLAVE_HOME` (default `~/.clave`) holds `profiles/<name>/`; `-P, --profile` / `CLAVE_PROFILE` selects one
(`default` when unset). Profile names match `^[a-z0-9][a-z0-9_-]{0,31}$`. Directories are `0700`, files `0600`.

| Path                                     | Contents                                                                                  |
| ---------------------------------------- | ----------------------------------------------------------------------------------------- |
| `profile.json`                           | Public profile, settings, contacts, vault aliases, default vault                          |
| `secrets.json`                           | The member's own Aztec account secrets and the keeper master key (protected by file mode) |
| `signer.sealed`                          | The approval signing key, sealed under a passphrase-derived key (see Signer)              |
| `vaults/<address>.json`                  | Founded vault records: setup, members, shared protocol secrets (protected by file mode)   |
| `engine/`                                | PXE store (`pxe/`) and tracked tokens                                                     |
| `keeper/`                                | The keeper's data directory (its own sealed custody)                                      |
| `daemon.addr`, `daemon.token`            | The daemon's socket path and session token, written at start (read `daemon.addr` first)   |
| `daemon.pid`, `daemon.log`, `keeper.log` | Process id and logs                                                                       |

The daemon's socket is `<profile>/daemon.sock` unless that path is longer than 100 bytes (macOS limits socket paths to
104), in which case it is `$TMPDIR/claved-<hash>.sock`; either way the path is written to `daemon.addr`. The keeper's
socket always lives in the temp directory (`clave-<hash>.sock`).

## Daemon

`claved --profile-dir <dir> [--foreground]` serves until `daemon.stop` or a signal. `clave daemon start` spawns it
detached (stdout and stderr to `daemon.log`), waits for the socket to answer `daemon.info`, and returns; `clave`
commands that need the daemon fail with exit code 3 when it is not running (they never start it implicitly, except
`clave init`). The engine binary is found at `CLAVE_ENGINE` (a `.js` entry run with `node`), else
`$CLAVE_HOME/lib/engine/claved.js`, else the repository build (`packages/engine/dist/claved.js`). The keeper binary
is `CLAVE_KEEPER_BIN`, else `$CLAVE_HOME/bin/clave-keeper`, else `crates/target/{release,debug}/clave-keeper`.

### Transport

Newline-delimited JSON-RPC 2.0 over `daemon.sock`, the keeper's framing: the first call of every connection is
`session.auth { token }` with the contents of `daemon.token` (compared in constant time); events arrive as
notifications `{"jsonrpc":"2.0","method":"event","params":<event>}` after `events.subscribe`. Results and params are
plain JSON: field elements and bytes are `0x` hex, points `{ x, y }`, amounts decimal strings of base units. Times are
unix milliseconds (`startedAt`, `foundedAt`, `createdAt`, `unlockedUntil`, `checkedAt`) except the chain's: a proposal's
`expiresAt` and an execution's `at` are unix seconds, and a received transfer's `at` is its block number. Validation
beyond the shapes: contact labels and vault aliases match `^[a-z0-9][a-z0-9_-]{0,31}$`, `viewingThreshold` is at least
2, unlock windows are at least 60 s and at most `unlockTimeoutSecs`. `events.subscribe` is handled by the server itself
and is not in the engine's method table.

Errors are `{ code, message, data?: { kind } }`. `kind` maps to the CLI's exit codes:

| `kind`      | Meaning                                                                   | Exit |
| ----------- | ------------------------------------------------------------------------- | ---- |
| `invalid`   | Params failed validation, unknown reference                               | 2    |
| `not-ready` | Not possible yet: below threshold, keepers unreachable, lobby not started | 4    |
| `locked`    | The signer needs a secret and none was given                              | 1    |
| `auth`      | Wrong passphrase or sealing key                                           | 1    |
| `failed`    | Anything else (node, keeper, chain, proof)                                | 1    |

### Methods

| Method                            | Params                                   | Result                               |
| --------------------------------- | ---------------------------------------- | ------------------------------------ |
| `daemon.info`                     | –                                        | `DaemonInfo`                         |
| `daemon.stop`                     | –                                        | `null` (then exits)                  |
| `status`                          | –                                        | `Status`                             |
| `profile.create`                  | `{ name, nodeUrl, passphrase }`          | `Status`                             |
| `config.get`                      | –                                        | `Settings`                           |
| `config.set`                      | `Partial<Settings>`                      | `Settings`                           |
| `signer.status`                   | –                                        | `SignerStatus`                       |
| `signer.unlock`                   | `{ secret: Secret, ttlSecs }`            | `SignerStatus`                       |
| `signer.lock`                     | –                                        | `SignerStatus`                       |
| `signer.sealingKey`               | `{ passphrase }`                         | `{ key }` (for Touch ID)             |
| `node.status`                     | –                                        | `NodeStatus`                         |
| `lobby.inspect`                   | `{ invite }`                             | `InvitePreview`                      |
| `lobby.create`                    | `{ name, threshold, viewingThreshold }`  | `LobbyView` (with invite)            |
| `lobby.join`                      | `{ invite }`                             | `LobbyView`                          |
| `lobby.get` / `lobby.leave`       | `{ lobby }`                              | `LobbyView` / `null`                 |
| `lobby.list`                      | –                                        | `LobbyView[]`                        |
| `lobby.start`                     | `{ lobby, threshold, viewingThreshold }` | `LobbyView`                          |
| `founding.begin` / `founding.get` | `{ lobby }`                              | `FoundingState`                      |
| `vault.list`                      | –                                        | `VaultSummary[]`                     |
| `vault.details`                   | `{ vault }`                              | `VaultDetails`                       |
| `vault.peers`                     | `{ vault }`                              | `MemberReachability[]`               |
| `vault.audit`                     | `{ vault, limit }`                       | `AuditEntry[]`                       |
| `vault.activity`                  | `{ vault }`                              | `ActivityEntry[]`                    |
| `vault.alias`                     | `{ vault, alias }`                       | `VaultSummary[]`                     |
| `vault.use`                       | `{ vault }`                              | `VaultSummary[]`                     |
| `tokens.list`                     | `{ vault }`                              | `TokenBalance[]`                     |
| `tokens.add`                      | `{ vault, token }`                       | `TokenInfo`                          |
| `tokens.deployTest`               | `{ vault }`                              | `TokenInfo`                          |
| `proposals.list`                  | `{ vault }`                              | `ProposalView[]`                     |
| `proposals.get`                   | `{ vault, id }`                          | `ProposalView`                       |
| `proposals.preview`               | `{ vault, intent, expiresInHours }`      | `ProposalPreview`                    |
| `proposals.create`                | `{ vault, intent, expiresInHours }`      | `ProposalView`                       |
| `proposals.review`                | `{ vault, id }`                          | `Review`                             |
| `proposals.approve`               | `{ vault, id, secret? }`                 | `ProposalView`                       |
| `proposals.reject`                | `{ vault, id, reason?, secret? }`        | `ProposalView`                       |
| `proposals.execute`               | `{ vault, id }`                          | `ProposalView`                       |
| `proposals.cancel`                | `{ vault, id }`                          | `ProposalView` (the cancel proposal) |
| `contracts.functions`             | `{ contract }`                           | `ContractFunction[]`                 |
| `contacts.list`                   | –                                        | `Contact[]`                          |
| `contacts.put`                    | `{ address, label }`                     | `Contact[]`                          |
| `contacts.remove`                 | `{ address }`                            | `Contact[]`                          |
| `relays.set`                      | `{ relays }` (restarts the keeper)       | `Settings`                           |
| `events.subscribe`                | –                                        | `null`, then events                  |

`Secret` is `{ passphrase: string }` or `{ key: string }` (the 32-byte sealing key as hex, from the Touch ID keychain
item). `intent` is `{ kind: "transfer", token, to, amount, memo? }` or `{ kind: "calls", calls: [{ to, fn, args }],
memo? }` (at most 8 calls, string arguments coerced from the ABI).

### Shapes

```ts
DaemonInfo  { version, pid, profile, profileDir, startedAt, node: string }
Settings    { nodeUrl, relays: string[], proverEnabled: boolean, unlockTimeoutSecs: number }
Status      { profile: Profile | null, settings: Settings, keeper: KeeperState, engine: EngineState,
              node: NodeStatus, pxe: { syncedBlock?: number }, signer: SignerStatus,
              vaults: VaultSummary[], defaultVault?: string }
Profile     { name, signerKey: Point, account: string, createdAt }
SignerStatus { locked: boolean, unlockedUntil?: number }
KeeperState { status: "stopped"|"starting"|"running"|"restarting"|"failed", peerId?, identity?, transport?,
              addresses?: string[], version?, error? }
EngineState { status: same, error? }
NodeStatus  { url, reachable, nodeVersion?, l1ChainId?, rollupVersion?, blockNumber?, checkedAt, error? }
VaultSummary { address, name, alias?, threshold, viewingThreshold, members, myIndex, foundedAt, isDefault }
ProposalView { id, vault, intent, proposer: { index, name }, status, threshold, members,
               approvals: Vote[], rejections: Vote[], mySlot, approvedByMe, rejectedByMe,
               configVersion, currentConfigVersion, expiresAt, txNonce, outerHash, approvalMessage,
               feeMethod, calls: DecodedCall[], token?: TokenInfo, execution?: { txHash, at }, cancelledBy? }
Vote        { slot, index, name, valid, reason? }
Review      { proposal: ProposalView, vaultName, myName,
              checks: { rebuilt: boolean, outerHash: string, configCurrent: boolean, notExpired: boolean,
                        failure?: string },
              action: string,               // "transfer 25 JADE (25000000000000000000 base units)"
              recipient?: { address, label?, source: "address book" | "member" | null },
              simulation: { ok: boolean, gas?: { da, l2 }, error? },
              signs: { message: string, formula: string },     // what an approval signs
              rejects: { message: string, formula: string } }   // what a rejection signs
```

Status words: `collecting`, `ready`, `blocked`, `executed`, `cancelled`, `expired`, `stale`. A proposal is `blocked`
when its valid rejections exceed `members − threshold`; an approval outranks a rejection from the same slot, and the
daemon refuses to reject after approving.

`InvitePreview`, `LobbyView`, `FoundingState`, `VaultDetails`, `MemberReachability`, `AuditEntry`, `ActivityEntry`,
`TokenInfo`, `TokenBalance`, `ProposalPreview`, `DecodedCall`, `ContractFunction`, `Contact` and the events are
defined in `packages/engine/src/types.ts`.

Events: `{ type: "vault-activity", vault }`, `{ type: "lobby", lobby: LobbyView }`, `{ type: "founding", founding }`,
`{ type: "node", node }`, `{ type: "execution", execution: { vault, proposal, phase, txHash?, error? } }`,
`{ type: "keeper", keeper: KeeperState }`.

## Signer

The approval signing key (Grumpkin) exists at rest only as `signer.sealed`:

```text
sealing key = Argon2id(passphrase, salt, memory 64 MiB, passes 3, parallelism 1, 32 bytes)
signer.sealed = { v: 1, kdf: { salt }, nonce, ciphertext }      ChaCha20-Poly1305, AAD "clave.signer.v1"
```

It is decrypted only inside the daemon, for one signature or for an unlock window (`signer.unlock { ttlSecs }`), and
the plaintext is dropped afterwards. The passphrase reaches the daemon only over the owner-only socket, typed at a
no-echo prompt or read from `CLAVE_PASSPHRASE_FILE`, never from argv or plain environment variables.

Touch ID (macOS, optional): `clave signer enable-touch-id` asks for the passphrase, obtains the sealing key with
`signer.sealingKey`, and stores it as a login-keychain item of the `clave` binary. Reading it requires a successful
`LocalAuthentication` check (Touch ID, or the account password as fallback) in the CLI, which then passes
`{ key }` as the secret. The passphrase always remains the recovery path.

`--yes` skips confirmations, never authentication. `clave approve` shows the review screen before asking.

## Trust boundaries

- The daemon's socket accepts only the profile owner (file mode) and the token holder.
- The member's account secrets, keeper master key and vault records are protected by file permissions on the member's
  device. The approval signing key additionally needs the passphrase (or Touch ID).
- Viewing shares never leave the keeper; the daemon never sees them.
