# Desktop app

`apps/desktop` (`@tecpan/desktop`) is the member's app: Electron 45 with a React 19 renderer. It holds the member's
approval signing key, supervises their keeper and runs their Aztec PXE. Members create a profile, found a vault
together with co-founders on other machines, and inspect everything the vault is made of.

Status: Phases 10–11 (shell, custody, onboarding, founding, assets, proposals, approvals, execution, activity).

## Processes

```text
renderer (sandboxed, no Node)          React, TanStack Router + Query, Tailwind 4, shadcn/Base UI, Motion
   │  window.tecpan.invoke / onEvent   preload: two functions, nothing else
   ▼
main process                            IPC router (schema-validated), signer, profile + settings, sealed vault records,
   │                                    keeper and engine supervisors
   ├── tecpan-keeper (child process)   viewing share, ECDH, lobby, DKG; master key on stdin
   └── engine (utilityProcess)         VaultWallet: patched PXE (LMDB), Barretenberg, keeper RPC client, founding flow
```

| Process  | Holds                                                            | Never holds                              |
| -------- | ---------------------------------------------------------------- | ---------------------------------------- |
| Main     | Approval signing key (in memory), keychain access, vault records | Viewing shares, the PXE                  |
| Engine   | PXE state, the member's own account key, vault protocol secrets  | The approval signing key, viewing shares |
| Keeper   | Viewing shares, keeper identity and transport keys               | Signing keys                             |
| Renderer | What it is shown                                                 | Any key; Node; Electron objects          |

The engine starts after the profile exists and is restarted when the node URL, the prover setting or the keeper's
session changes (a keeper restart issues a new RPC token). Both supervisors restart crashed children with backoff and
report status to the renderer.

## IPC contract (`src/shared/contract.ts`)

Every renderer call names a method from one table, with a zod schema for its params and the process that serves it.
The main process refuses a call whose sender frame is not the app's origin, whose method is not in the table (including
`__proto__`-style names) or whose params fail the schema; then routes it to the main process or the engine. Events
(`state`, `lobby`, `founding`, `node`, `vault-activity`, `execution`) flow the other way and update TanStack Query's
cache in place; `vault-activity` (from keeper `proposal`, `approval` and `receipt` events) invalidates a vault's data.

| Main process                                                                                        | Engine                                                                                                          |
| --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `app.state`, `profile.create`, `profile.rename`, `settings.update`, `node.probe`, `keeper.restart`  | `node.status`, `pxe.status`, `lobby.inspect/create/join/get/list/start/leave`, `founding.begin/get`             |
| `clipboard.write`, `clipboard.readInvite` (returns only invite-shaped text), `shell.openDataFolder` | `vault.list`, `vault.details`, `vault.audit`, `vault.peers`, `vault.activity`                                   |
| `contacts.list/put/remove` (address book, profile-local)                                            | `tokens.list/add/deployTest`, `proposals.list/get/preview/create/approve/execute/cancel`, `contracts.functions` |

Main and engine talk over the utility process port (`src/shared/engine-protocol.ts`): `init`, `call`/`reply`, `event`,
and `host-call` for what the engine asks of main: `vault.save` (seal a vault record) and `signer.approve` (the member
confirms and the main process signs; only the signature limbs return).

## Custody

- **Keychain.** Every secret the app keeps is a JSON value sealed with Electron `safeStorage` (the OS keychain) in
  `<data>/secure/*.sealed`: the approval signing key, the keeper master key, the member's account secrets, and one
  record per vault (setup, protocol secrets, committee). The app refuses to start without a keychain. Development and
  release builds use different app names, so their keychain items never collide.
- **Signer.** The approval key is a Grumpkin scalar generated in the main process. Signing re-derives nothing from the
  caller and shows a native confirmation (and Touch ID on Macs that have it) before every signature (`Signer.signApproval`).
- **Keeper.** Its master key is passed on stdin with a fresh RPC token; its socket lives in the temp dir (macOS limits
  socket paths) and is owner-only. It listens on every interface (QUIC and TCP, OS-chosen ports) and advertises only
  routable addresses.
- **Profiles.** `--profile=<name>`, `TECPAN_PROFILE` or `TECPAN_USER_DATA` select the data directory, so several members
  can run on one machine; one instance per profile.

## Hardening

- Window: `sandbox`, `contextIsolation`, no `nodeIntegration`, no webviews, devtools only in development; `app.enableSandbox()`.
- Origin: the built renderer is served from `app://tecpan/` (a privileged standard scheme) with a strict CSP on every
  response: `default-src 'none'`, scripts, styles, fonts and images only from the app, no eval, no remote anything.
- Navigation away from the app and `window.open` are refused; permission requests and checks are denied; downloads are
  cancelled.
- Electron fuses (packaging): no `RunAsNode`, no `NODE_OPTIONS`, no inspector flags, ASAR integrity validation, app only
  from the ASAR, cookie encryption, no extra file-protocol privileges. Hardened runtime with only JIT and library
  validation exceptions.
- Packaging ships the keeper (`Resources/bin/tecpan-keeper`), unpacks native code (`bb`, `.node` addons) from the
  ASAR, excludes the napi testing build and drops other platforms' binaries and unused contract artifacts.

## Founding a vault

1. **Profile.** Name and node; the app creates the signing key, starts the keeper, starts the engine and registers the
   member's own initializerless Schnorr account (fees come from the sponsored FPC on a local network).
2. **Lobby** (`keeper.md`): the host's keeper opens a lobby and returns an invite (`tecpan-invite-v0:…`) carrying the
   lobby id, a join secret, the host's keeper and the proposed rules. Guests preview it (`lobby.inspect`: vault name,
   host, rules, same network or not) and join; everyone sees the roster live.
3. **Plan.** The host starts: its engine generates the vault's protocol secrets and epoch tagging secret, its keeper
   seals them to every member with the plan (members, thresholds, salt, DKG session). Each guest reviews the plan and
   confirms; their keeper never joins on the host's word.
4. **Ceremony** (`engine/founding.ts`, per member): DKG through the member's keeper → derive the vault from the plan
   and the DKG result → register with the keeper (`vault.register`) and the PXE → the host deploys (one private
   transaction from its account), guests wait for it on chain → `verifyVaultConfig` compares the on-chain commitments
   and thresholds with the plan → the vault record is sealed and the vault opens. All members derive the same address
   independently.

## Operations (`engine/operations.ts`)

```text
create    intent ─▶ payload (fee payment calls + app calls) ─▶ prepareVaultTx (pinned nonce, outer hash, config version,
                    expires_at) ─▶ wire Proposal { payload, summary = intent JSON, proposer } ─▶ keeper proposals.put
approve   proposals.get ─▶ decode payload ─▶ prepareVaultTx with the proposal's nonce and expiry ─▶ outer hash and config
                    version must equal the proposal's ─▶ signer.approve (native confirm) ─▶ keeper approvals.put
execute   rebuild as above ─▶ approvals verified against slot signer keys ─▶ executeVaultTx (49-field witness, proving)
                    ─▶ keeper receipts.put ─▶ receipt event on every member
cancel    same tx nonce, fee calls only ─▶ a proposal of its own; once executed the original's nonce is consumed
```

- **Status** per proposal: `executed` (receipt), `cancelled` (an executed cancel targets it), `expired` (`expires_at`
  ≤ latest block timestamp), `stale` (config version changed since proposing: approvals are void), `ready` (≥ k valid
  approvals), else `collecting`. Approvals are verified locally (`verifyProposalApproval`) before they count.
- **Payload encoding** (`vault-wallet/vault/payload.ts`): calls through `FunctionCall`'s schema, authwits, capsules and
  hashed args through their byte encodings, ≤ 64 KiB. A payload that does not hash to the proposed outer hash cannot
  be signed.
- **Assets.** Tokens are aztec-nr `Token` contracts tracked per vault in the engine's data directory. Balances are
  `balance_of_private(vault)` simulated through the PXE (so they need t_v keepers). A transfer proposal of an unknown
  token registers its instance from the node. On a local network (L1 31337) the app can deploy a test token and mint
  to the vault. Fees use the sponsored FPC.
- **Activity**: finished proposals plus `Transfer` events decrypted for the vault. **Committee**: `peers.status` per
  member (direct or relayed, last seen, ping RTT) and whether t_v keepers are reachable for ECDH.

## Running several members on one machine

`pnpm --filter @tecpan/desktop demo` builds and starts Ana, Bea and Cai (or any names after `--`) as separate profiles
(`--profile=<name>`, data under the OS app-data dir `Tecpan/profiles/<name>`), each with its own keychain-sealed keys,
keeper and PXE; windows cascade. `-- --fresh` wipes those profiles first. Profiles persist between runs.

## Interface

Light theme only, built on the brand (`brand.md`). Onboarding and founding are single-task screens; the vault lives in
an ink sidebar shell with Overview (private balances, open proposals, committee reachability, health), Proposals (list,
composer with simulation, detail with decoded calls, approvals and every hash a reviewer checks), Activity, Members
(roles, expandable keys and keeper details), Address book and Internals (contract instance and
public keys, on-chain config, viewing committee and verification shares, keeper identity and ECDH audit log, node and
PXE). "Show technical details" puts peer ids and keys inline everywhere.

Copy is technical (k, t_v, outer hash, tx nonce, config version, L1 chain by name) and limited to what an Aztec
developer needs to act or verify.

Motion follows one set of tokens (`lib/motion.ts`) and animates only `transform` and `opacity`: onboarding steps slide
by direction inside a frame that stays mounted, entrances use a strong ease-out under 300 ms, section switches are a
120 ms fade, springs only where something settles, and `MotionConfig reducedMotion="user"`. The generated art is
rasterised once into an image (an SVG turbulence filter repainted every frame made earlier builds lag).

## Tests and measurements

| Suite                                 | Covers                                                                                                                                                                           |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `test/contract.test.ts`               | Unknown and prototype method names refused, malformed params refused (URLs, invites, hex, relays, sizes, thresholds), well-formed calls accepted                                 |
| `test/peer-id.test.ts`                | Peer id derivation matches the keeper's own                                                                                                                                      |
| `test/secrets.test.ts`                | Protocol secrets codec round trip within the keeper's 1 KiB limit                                                                                                                |
| `e2e/security.spec.ts`                | Renderer has no `require`/`process`, only `invoke`/`onEvent`/`platform`; unknown and malformed calls refused; secure `app://tecpan` origin; navigation and `window.open` refused |
| `e2e/founding.spec.ts`                | Exit criterion: three app instances (separate profiles, keepers, engines) onboard, found a 2-of-3 / `t_v = 2` vault on a local network and all verify the same config            |
| `e2e/operations.spec.ts`              | Phase 11 exit criterion: test token minted to the vault; A proposes 25 TTT, A and B approve while C is offline; C restarts, catches up and executes; all three read 9,975        |
| `vault-wallet` `test/payload.test.ts` | Payload codec round trip of every part; size limit                                                                                                                               |

Measured on macOS arm64 against `aztec start --local-network`: the three-instance operations test (founding, token,
proposal, two approvals, offline catch-up, execution) runs in about 36 s; the founding test in 12–20 s; the
DKG among three keepers takes about 0.4 s, deployment (simulated proof) about 4 s, verification about 0.2 s. Renderer
bundle 833 KB minified; packaged app 679 MB (mostly Aztec's protocol circuits needed for client-side proving).
