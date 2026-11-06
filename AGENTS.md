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
contracts/            Nargo workspace (Aztec.nr v6, Noir 1.0)
  vault_contract/       the vault account contract (TecpanVault)
  vault_test/           TestEnvironment tests
  test_signer_contract/ test-only helper exposing TXE signatures; never deploy
crates/               Cargo workspace
  crypto/               tecpan-crypto: Grumpkin, Aztec key-derivation compatibility, DKG, resharing, threshold ECDH
  wire/                 tecpan-wire: wire types and canonical encoding shared by keeper, relay and app
  keeper/               tecpan-keeper: member daemon (viewing share, ECDH responder, DKG/reshare, p2p, proposals)
  relay/                tecpan-relay: optional untrusted store-and-forward relay (ciphertext only)
  napi/                 @tecpan/napi: Node bindings of tecpan-crypto and tecpan-wire (secret helpers only in /testing)
  cli/                  tecpan-cli: the `tecpan` command line, a client of the member's daemon
packages/             TypeScript, only where Aztec requires it
  vault-wallet/         PXE host: vault account contract class, witness encoder, threshold key store
  engine/               @tecpan/engine: `tecpand`, the member's daemon (PXE, signer, founding, proposals, keeper)
apps/
  web/                  the Tecpan site (Vite, React, TanStack Router): pages and isometric figures
patches/              minimal pnpm patch of @aztec-labs/pxe for the threshold key store
architecture/         maintained design docs (verified decisions only)
research/             gitignored plans and investigation; never a build or doc input
```

Members use the `tecpan` CLI, a client of `tecpand`, a per-profile daemon that hosts the PXE and the passphrase-sealed
signer and supervises the member's keeper (`architecture/cli.md`). The Electron desktop app was removed after Phase 12
and can be restored from git (`3eb2d7e`). Signing keys never leave the member's device. Viewing shares live only in
`tecpan-keeper`, which may run locally or on the member's own server.

Folders inside `contracts/` and `crates/` carry no `tecpan` prefix; Rust package names keep it
(`crates/keeper` builds `tecpan-keeper`).

## Toolchain

| Concern         | Tool                                                                                |
| --------------- | ----------------------------------------------------------------------------------- |
| Contracts       | Aztec 6.0.0 (`aztec compile`, `aztec test`, `aztec codegen`), Noir 1.0              |
| Rust            | Stable toolchain, Cargo workspace, `rustfmt`, `clippy -D warnings`, `cargo-deny`    |
| TypeScript      | Node 24, pnpm workspaces, Turborepo, TypeScript 7, Klarity presets (oxfmt, oxlint)  |
| Site            | React 19, Vite, TanStack Router, Tailwind 4, Motion                                 |
| Hooks / commits | lefthook, commitlint (`klarity/commitlint`)                                         |
| Tests           | `aztec test` (TXE), `cargo test` + property tests, Vitest, local-network end-to-end |

Do not add overlapping tools (Prettier, ESLint, Husky, lint-staged, Tauri) or a central coordinator server.

## Working rules

- Inspect branch, working tree and recent commits before changing anything; preserve unrelated work.
- Commit after each coherent change with a Conventional Commit, e.g. `feat(contracts): verify k-of-n witness`.
  Scopes are kebab-case package or area names (`contracts`, `crypto`, `wire`, `keeper`, `relay`, `napi`,
  `vault-wallet`, `engine`, `cli`, `web`, `repo`).
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
- Config notes are delivered with constrained delivery everywhere except the constructor (an undelivered config note
  would lock members out). Signer notes are discovery-only and may use unconstrained delivery.
- Config changes are `#[only_self]` functions that bump the config version; membership and the viewing committee change
  together in `rotate_membership`, which removes at most `t_v - 1` members.
- The PXE patch (`patches/`) stays minimal: each patch site is marked `// tecpan:` and guarded so a stock key store
  behaves as stock. Port it on every Aztec upgrade following `architecture/pxe-integration.md`.
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

Prerequisites: Node 24 (`.node-version`), pnpm 12 (`packageManager`), Rust 1.96.0 (`rust-toolchain.toml`), Aztec
6.0.0 (`.aztecrc`, install with `aztec-up install 6.0.0`), `cargo-deny`.

```sh
pnpm install        # also installs git hooks
pnpm check          # format check, lint (oxlint, clippy, nargo fmt), typecheck, build, cargo-deny
pnpm test           # aztec test, cargo test, vitest
pnpm format         # oxfmt, rustfmt, nargo fmt
```

Narrow lanes:

```sh
pnpm --filter @tecpan/contracts build|test|profile   # aztec compile / aztec test / gate counts
pnpm --filter @tecpan/crates lint|test|deny          # cargo clippy / cargo test / cargo deny
cargo test --manifest-path crates/Cargo.toml -p tecpan-keeper   # keeper: custody, ceremonies, multi-keeper, rpc, p2p
cargo test --manifest-path crates/Cargo.toml -p tecpan-relay    # relay: mailbox store
pnpm --filter @tecpan/napi build|test                # napi build (+ testing binary) / Vitest vs live Aztec
pnpm --filter @tecpan/vault-wallet typecheck|test|build
pnpm --filter @tecpan/engine build|test|typecheck    # tecpand: contract, signer, socket server
cargo test --manifest-path crates/Cargo.toml -p tecpan-cli      # CLI: parsing, references, renderers, fake daemon
pnpm cli:install    # tecpan, tecpan-keeper and tecpand into ~/.tecpan (TECPAN_HOME); prints the PATH line
pnpm test:cli       # three members through tecpan only, against `aztec start --local-network` (not part of pnpm test)
pnpm --filter @tecpan/web dev|build|preview|typecheck|lint   # the site on http://localhost:3000
pnpm test:e2e       # vault wallet against `aztec start --local-network` (not part of pnpm test)
pnpm test:network   # Docker: relay + 3 keepers on separate networks (not part of pnpm test)
```

Hooks: pre-commit runs oxfmt, oxlint, rustfmt and nargo fmt on staged files; commit-msg runs commitlint; pre-push runs
`turbo run format:check lint typecheck test`. CI (`.github/workflows/ci.yml`) runs TypeScript, Rust and Noir jobs.

Run the narrowest relevant check first, then the full suite, and report skipped lanes and pre-existing failures rather
than hiding them.
