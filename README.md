# Tecpan

A private k-of-n multisig vault on [Aztec](https://aztec.network).

- **Private spending.** The vault is an Aztec account contract. A transaction runs only with valid approvals from `k`
  of its `n` signers, and the signer set, thresholds and approvals never appear on-chain in plaintext.
- **Threshold viewing.** No member holds the vault's viewing key. Members' keepers hold shares and reshare them on every
  membership change, so a removed member cannot read anything that arrives afterwards, and the vault keeps its
  address.
- **No central server.** Keepers talk to each other over an encrypted peer-to-peer mesh; an optional relay only
  forwards ciphertext.

## Status

v0, under construction. Built phase by phase: the repository currently contains the toolchain and placeholders for
every component. See [`architecture/`](architecture/) for what is built.

## Layout

| Path                     | Component                                                         |
| ------------------------ | ----------------------------------------------------------------- |
| `contracts/`             | Vault account contract (Noir, Aztec.nr 6.0.0)                     |
| `crates/`                | Rust: threshold crypto, wire format, keeper, relay, Node bindings |
| `packages/vault-wallet/` | TypeScript host for the Aztec PXE with a threshold key store      |
| `apps/desktop/`          | Electron + React desktop app                                      |

## Prerequisites

- Node.js 24 and pnpm 12
- Rust 1.96.0 (picked up from `rust-toolchain.toml` by rustup) and `cargo-deny`
- Aztec 6.0.0: `aztec-up install 6.0.0` (version pinned in `.aztecrc`)

## Commands

```sh
pnpm install   # dependencies and git hooks
pnpm check     # formatting, lint, typecheck, build, dependency policy
pnpm test      # contract, Rust and TypeScript tests
```
