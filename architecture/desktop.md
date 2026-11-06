# Desktop app

`apps/desktop` (`@tecpan/desktop`) is the member's app: Electron 45 with a React 19 renderer. It holds the member's
approval signing key, supervises their keeper and runs their Aztec PXE. Members create a profile, found a vault
together with co-founders on other machines, and inspect everything the vault is made of.

Status: Phase 10 (shell, custody, onboarding, founding, vault overview and internals). Payments and approvals are
Phase 11.

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
(`state`, `lobby`, `founding`, `node`) flow the other way and update TanStack Query's cache in place.

| Main process                                                                                        | Engine                                                                                              |
| --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `app.state`, `profile.create`, `profile.rename`, `settings.update`, `node.probe`, `keeper.restart`  | `node.status`, `pxe.status`, `lobby.inspect/create/join/get/list/start/leave`, `founding.begin/get` |
| `clipboard.write`, `clipboard.readInvite` (returns only invite-shaped text), `shell.openDataFolder` | `vault.list`, `vault.details`, `vault.audit`                                                        |

Main and engine talk over the utility process port (`src/shared/engine-protocol.ts`): `init`, `call`/`reply`, `event`,
and `host-call` for the one thing the engine asks of main (`vault.save`, sealing a vault record).

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

## Interface

Light theme only, built on the brand (`brand.md`). Onboarding and founding are single-task screens; the vault lives in
an ink sidebar shell with Overview (balance placeholder, health checks from real state: on-chain config match, keeper
serving its share, PXE sync), Members (roles, expandable keys and keeper details) and Internals (contract instance and
public keys, on-chain config, viewing committee and verification shares, keeper identity and ECDH audit log, node and
PXE). "Show technical details" puts peer ids and keys inline everywhere.

Motion follows one set of tokens (`lib/motion.ts`): strong ease-out entrances under 300 ms, a quick fade for frequent
section switches, springs only where something settles (roster entries, the mark assembling during the ceremony), and
`MotionConfig reducedMotion="user"`.

## Tests and measurements

| Suite                   | Covers                                                                                                                                                                           |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `test/contract.test.ts` | Unknown and prototype method names refused, malformed params refused (URLs, invites, hex, relays, sizes, thresholds), well-formed calls accepted                                 |
| `test/peer-id.test.ts`  | Peer id derivation matches the keeper's own                                                                                                                                      |
| `test/secrets.test.ts`  | Protocol secrets codec round trip within the keeper's 1 KiB limit                                                                                                                |
| `e2e/security.spec.ts`  | Renderer has no `require`/`process`, only `invoke`/`onEvent`/`platform`; unknown and malformed calls refused; secure `app://tecpan` origin; navigation and `window.open` refused |
| `e2e/founding.spec.ts`  | Exit criterion: three app instances (separate profiles, keepers, engines) onboard, found a 2-of-3 / `t_v = 2` vault on a local network and all verify the same config            |

Measured on macOS arm64 against `aztec start --local-network`: the three-instance founding test runs in 12–20 s; the
DKG among three keepers takes about 0.4 s, deployment (simulated proof) about 4 s, verification about 0.2 s. Renderer
bundle 833 KB minified; packaged app 679 MB (mostly Aztec's protocol circuits needed for client-side proving).
