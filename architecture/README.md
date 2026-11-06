# Architecture

Maintained design documentation for Tecpan. Pages describe what is built and verified; plans and investigation stay in
the gitignored `research/` folder until a phase lands.

| Page                 | Covers                                                     | Added in |
| -------------------- | ---------------------------------------------------------- | -------- |
| `contract.md`        | Vault contract storage, authorization, wire formats, gates | Phase 3  |
| `viewing-keys.md`    | Threshold viewing keys: DKG, threshold ECDH, resharing     | Phase 5  |
| `wire.md`            | Message catalogue, encoding, signing, replay protection    | Phase 6  |
| `pxe-integration.md` | PXE patch, threshold key store, upgrade procedure          | Phase 7  |
| `keeper.md`          | Keeper responsibilities, storage, RPC, ceremonies          | Phase 8  |
| `networking.md`      | Peer mesh, protocols, sync, mailboxes, relay trust model   | Phase 9  |
| `desktop.md`         | Electron processes, IPC contract, custody, founding flow   | Phase 10 |
| `brand.md`           | Mark, colour, type and imagery                             | Phase 10 |
| `membership.md`      | Membership changes and resharing end to end                | Phase 12 |

## Repository at a glance

| Path                     | What it is                                                                         |
| ------------------------ | ---------------------------------------------------------------------------------- |
| `contracts/`             | Nargo workspace: `vault_contract`, `vault_test`                                    |
| `crates/`                | Cargo workspace: `crypto`, `wire`, `keeper`, `relay`, `napi` (packages `tecpan-*`) |
| `packages/vault-wallet/` | TypeScript PXE host for vaults                                                     |
| `apps/desktop/`          | Electron + React desktop app                                                       |
