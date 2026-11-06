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

v0, on a local Aztec network. The contract, threshold cryptography, PXE integration, keeper and relay, proposals and
the `tecpan` CLI are built and tested; the membership drill and a testnet release come next. See
[`architecture/`](architecture/) for what is built.

## Layout

| Path                     | Component                                                         |
| ------------------------ | ----------------------------------------------------------------- |
| `contracts/`             | Vault account contract (Noir, Aztec.nr 6.0.0)                     |
| `crates/`                | Rust: threshold crypto, wire format, keeper, relay, CLI, bindings |
| `packages/vault-wallet/` | TypeScript host for the Aztec PXE with a threshold key store      |
| `packages/engine/`       | `tecpand`, the member's daemon behind the CLI                     |
| `apps/web/`              | The Tecpan site (Vite, React): mechanisms explained with figures  |

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

## Using a vault

```sh
aztec start --local-network          # in another terminal
pnpm cli:install                     # tecpan, its daemon and the keeper into ~/.tecpan
export PATH="$HOME/.tecpan/bin:$PATH"
tecpan init                          # a profile per member; -P <name> selects one
tecpan lobby open --name Ops -k 2 --tv 2 --members 3   # prints an invite; others: tecpan lobby join <invite>
tecpan balance
tecpan propose transfer 25 ttt --to bea
tecpan approve <id>                  # shows what you sign, then asks for your passphrase
tecpan execute <id>
```

Run several members on one machine with `-P ana`, `-P bea`, … `pnpm test:cli` drives three members through the whole
flow.
