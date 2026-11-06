# Tecpan

Tecpan is a private k-of-n multisig on Aztec. The multisig is an Aztec account contract whose
`entrypoint` executes a payload only when the aggregated auth witness carries valid Schnorr
signatures from at least `threshold` registered signers. The signer set, threshold and approvals
live in private notes; the chain sees one proof, nullifiers, encrypted logs and the fee payment.

Signers approve off-chain. The coordinator relays end-to-end encrypted proposals and signatures,
and one signer's PXE assembles the witness, proves and submits.

## Repository layout

```text
contracts/            Nargo workspace (Aztec.nr v6, Noir 1.0)
  tecpan_contract/      the multisig account contract
  tecpan_test/          TestEnvironment tests
  test_signer_contract/ test-only helper exposing TXE signatures; never deploy
packages/
  protocol/           Effect Schema models, IDs, DTOs and expected errors; no I/O
  api/                HttpApi declarations for the coordinator; no handlers
  sdk/                Aztec client: account contract class, witness packer, signing,
                      proposal envelope encryption, generated contract artifacts
apps/
  coordinator/        Effect HttpApi server, Drizzle + PostgreSQL, encrypted relay
  web/                Vite + React 19 + TanStack Router + Effect Atom signer app
architecture/         maintained design docs (verified decisions only)
research/             gitignored local investigation; never a build or doc input
```

Dependency direction: `api -> protocol`; `sdk -> protocol`; `coordinator -> api, protocol`;
`web -> api, protocol, sdk`. The coordinator never imports `sdk` and never holds keys. `web`
imports only browser-safe exports.

## Toolchain

| Concern         | Tool                                                                     |
| --------------- | ------------------------------------------------------------------------ |
| Runtime         | Node.js 24, pnpm workspaces, Turborepo                                   |
| Shared config   | `klarity` presets (tsconfig, oxlint, oxfmt, tsdown, vitest, lefthook)    |
| TypeScript      | TypeScript 7, strict Klarity presets; one environment preset per package |
| Format / lint   | oxfmt, oxlint                                                            |
| Hooks / commits | lefthook, commitlint (`klarity/commitlint`)                              |
| TS application  | Effect v4: HttpApi, Schema, Layers, Effect Atom (`@effect/atom-react`)   |
| Database        | Drizzle ORM 1.0 RC on PostgreSQL via Effect SQL                          |
| Contracts       | Aztec 6.0.0 toolchain (`aztec compile`, `aztec test`, `aztec codegen`)   |
| Tests           | Vitest + `@effect/vitest`; Aztec `TestEnvironment` for Noir              |

Do not add overlapping tools (Prettier, ESLint, Husky, lint-staged, tsup) or Effect v3 APIs.

## Working rules

- Inspect branch, working tree and recent commits before changing anything; preserve unrelated work.
- Commit after each coherent change with a Conventional Commit, e.g.
  `feat(contracts): verify k-of-n schnorr witness`. Scopes are kebab-case package or area names
  (`contracts`, `sdk`, `protocol`, `api`, `coordinator`, `web`, `repo`).
- Never commit `research/`, `.env` files, keys, or generated `target/` and artifact output.
- Keep files focused and split by responsibility. Reuse a helper only for real repeated use or a
  domain invariant; no generic utility layers.
- Update the owning `architecture/` page in the same change when a contract, schema, workflow or
  trust boundary changes.

## Aztec contracts

- Pin everything to 6.0.0: `aztec` CLI, `aztec-nr` tag `v6.0.0`, `@aztec-labs/*` 6.0.0.
- Verify Aztec.nr APIs against the fetched source in `~/nargo/github.com/aztec-labs-eng/aztec-nr/v6.0.0`
  and the reference account contracts embedded in `@aztec-labs/accounts` artifacts, not against blog
  posts or pre-v6 examples. v6 uses `#[external("private")]`, `self.storage`, `self.context`,
  `MessageDelivery`, and `entrypoint(app_payload, fee_payment_method: u8, cancellable)`.
- Noir has no early `return`; use `panic("msg")` for unconditional failures. Library asserts carry
  messages; test asserts do not. Use `#[test(should_fail_with = "...")]` for failure cases.
- Signature loops are bounded by the compile-time `MAX_SIGNERS`; proof time scales with it. Profile
  with `aztec profile gates target` (and `aztec profile flamegraph`) before and after any contract
  change that affects circuits, and update the gate tables in `architecture/contract.md`.
- Authorization must stay valid in static calls: read the config with `get_note` plus
  `assert_local_note_was_not_nullified_by`, never `SinglePrivateMutable::get_note` (it nullifies).
- The witness layout and approval message are a wire format shared with the SDK. Change them only
  together with `architecture/contract.md` and the SDK encoder.
- Config-changing functions deliver notes with constrained delivery; only the constructor may use
  unconstrained delivery (see `architecture/contract.md`).
- Signer and threshold changes must only run inside an approved payload
  (`msg_sender == this address`) and must keep `1 <= threshold <= signer_count`.

## Effect v4

Read `node_modules/effect/AGENTS.md` before writing Effect code and verify APIs against the
installed source and lockfile. Use `Effect.gen`, named `Effect.fn` for traced operations, class-based
`Context.Service` with Layers, Schema at every untrusted boundary, `Schema.TaggedError` for expected
failures, and Config/Redacted for configuration. Provide Node or browser implementations at the
runtime edge only.

Browser state flows through one `AtomHttpApi` client and one router-owned atom registry.

## Privacy invariants

These are product requirements, not style preferences. Any change that weakens one needs an explicit
decision recorded in `architecture/`.

- The coordinator stores and relays ciphertext only. It never sees plaintext payloads, signing keys or
  the account secret.
- Signing keys never leave the signer's device. The shared account secret is stored encrypted at rest
  in each signer's wallet, never on the coordinator.
- Signer identities and threshold are never written to public storage or public function arguments.
- Removing a signer is a treasury migration event, because the account viewing key cannot rotate.

## Commands

```sh
pnpm install
pnpm build          # turbo: contracts -> sdk -> apps
pnpm test           # turbo: aztec test + vitest
pnpm check          # format check, lint, typecheck, build
pnpm dev            # coordinator + web
```

Contracts alone: `pnpm --filter @tecpan/contracts build` / `test`. Local Aztec network:
`aztec start --local-network`.

Run the narrowest relevant check first, then `pnpm check`. Report skipped lanes and pre-existing
failures rather than hiding them.
