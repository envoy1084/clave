# Tecpan

Tecpan is a private k-of-n multisig vault on Aztec with threshold viewing.

- **Spending** is enforced on-chain: the vault is an Aztec account contract whose `entrypoint` runs a payload only when
  the auth witness carries valid signatures from at least `k` registered signers.
- **Viewing** is enforced off-chain: the account's incoming viewing secret never exists in one place. Members hold
  threshold shares, the PXE obtains `address_secret · P` through threshold ECDH, and shares are reshared on every
  membership change. A removed member cannot read anything that arrives after removal, and the vault keeps its address.

The repository is at **v0** and is being built phase by phase. The plan and design research live in `research/`
(gitignored); verified decisions are promoted into `architecture/` as each phase lands. Only describe components as
existing once they are in the tree.

## Target layout

```text
contracts/            Nargo workspace (Aztec.nr v6, Noir 1.0): vault contract, tests
crates/               Cargo workspace
  tecpan-crypto/        Grumpkin, Aztec key-derivation compatibility, DKG, resharing, threshold ECDH + DLEQ
  tecpan-wire/          wire types and canonical encoding shared by agent, relay and app (TS types generated)
  tecpan-agent/         member daemon: viewing-share custody, ECDH responder, DKG/reshare, p2p, proposal store
  tecpan-relay/         optional untrusted store-and-forward relay (ciphertext only)
  tecpan-napi/          Node bindings of tecpan-crypto for the Electron app
packages/             TypeScript, only where Aztec requires it
  vault-wallet/         PXE host: vault account contract class, witness encoder, threshold key store
apps/
  desktop/              Electron (main + PXE utility process) with a React renderer
patches/              minimal pnpm patch of @aztec-labs/pxe for the threshold key store
architecture/         maintained design docs (verified decisions only)
research/             gitignored plans and investigation; never a build or doc input
```

Process boundaries in the desktop app: the renderer is UI only (sandboxed, no Node). The PXE, signing keys and agent
supervision live in the main process or an Electron `utilityProcess`. Signing keys never leave the member's device.
Viewing shares live only in `tecpan-agent`, which may run locally or on the member's own server.

## Toolchain

| Concern         | Tool                                                                                |
| --------------- | ----------------------------------------------------------------------------------- |
| Contracts       | Aztec 6.0.0 (`aztec compile`, `aztec test`, `aztec codegen`), Noir 1.0              |
| Rust            | Stable toolchain, Cargo workspace, `rustfmt`, `clippy -D warnings`, `cargo-deny`    |
| TypeScript      | Node 24, pnpm workspaces, Turborepo, TypeScript 7, Klarity presets (oxfmt, oxlint)  |
| Desktop         | Electron, electron-vite, electron-builder                                           |
| UI              | React 19, Vite, TanStack Router, TanStack Query                                     |
| Hooks / commits | lefthook, commitlint (`klarity/commitlint`)                                         |
| Tests           | `aztec test` (TXE), `cargo test` + property tests, Vitest, local-network end-to-end |

Do not add overlapping tools (Prettier, ESLint, Husky, lint-staged, Tauri) or a central coordinator server.

## Working rules

- Inspect branch, working tree and recent commits before changing anything; preserve unrelated work.
- Commit after each coherent change with a Conventional Commit, e.g. `feat(contracts): verify k-of-n witness`.
  Scopes are kebab-case package or area names (`contracts`, `crypto`, `wire`, `agent`, `relay`, `napi`,
  `vault-wallet`, `desktop`, `repo`).
- Never commit `research/`, `.env` files, keys, shares, or generated `target/`, `out/` and artifact output.
- Keep files focused and split by responsibility. Reuse a helper only for real repeated use or a domain invariant.
- Update the owning `architecture/` page in the same change when a contract, wire format, key flow or trust boundary
  changes.
- After each phase, record what was delivered, decided and measured in `research/progress.md`.

## Aztec rules

- Pin everything to 6.0.0: `aztec` CLI, `aztec-nr` tag `v6.0.0`, `@aztec-labs/*` 6.0.0.
- Verify APIs against fetched source (`~/nargo/github.com/aztec-labs-eng/aztec-nr/v6.0.0`, installed
  `@aztec-labs/*` packages), not blog posts or pre-v6 examples.
- Profile circuits with `aztec profile gates` / `aztec profile flamegraph` before and after circuit changes and keep
  the gate tables in `architecture/` current.
- Authorization must stay valid in static calls (`verify_private_authwit`): never read state in a way that nullifies.
- Wire formats shared across Noir, Rust and TypeScript (witness layout, approval message, committee commitment) change
  only together with their `architecture/` page and every encoder.

## Cryptography rules

- All threshold code lives in `tecpan-crypto`: constant-time arithmetic, `zeroize` on secrets, no secret in logs.
- Every threshold ECDH partial carries a DLEQ proof and is verified before use.
- Byte-for-byte compatibility with Aztec (point encoding, address-secret derivation and its negation rule, ECDH-to-AES
  derivation, tag derivation) is proven by golden tests against real Aztec 6.0.0 output, and those tests run on every
  Aztec upgrade.
- Viewing shares are never backed up, exported, or sent anywhere except as protocol sub-shares during resharing.

## Privacy invariants

These are product requirements. Any change that weakens one needs an explicit decision recorded in `architecture/`.

- No single party, member or server ever holds the full incoming viewing secret.
- Signing keys never leave the signer's device.
- Relays and nodes see ciphertext and public data only.
- Signer identities, thresholds and the viewing committee are never written to public storage or public arguments.
- Fees are not paid from the vault in public.

## Commands

The phases add these as the components land. Run the narrowest relevant check first, then the full suite, and report
skipped lanes and pre-existing failures rather than hiding them.
