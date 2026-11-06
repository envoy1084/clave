# @tecpan/desktop

The Tecpan desktop app: Electron main process (signer, keeper and engine supervision), a `utilityProcess` engine
running the Aztec PXE, and a sandboxed React renderer. See `architecture/desktop.md` and `architecture/brand.md`.

```sh
cargo build --release --manifest-path ../../crates/Cargo.toml -p tecpan-keeper   # the keeper the app supervises
pnpm dev                     # run with hot reload
pnpm build && pnpm start     # run the production build
pnpm test                    # unit tests (IPC contract, codecs)
pnpm test:e2e                # Playwright: needs `aztec start --local-network`
pnpm package                 # unpacked app in dist/release (ad-hoc signed)
```

Run several members on one machine with `--profile=<name>` (or `TECPAN_PROFILE`, `TECPAN_USER_DATA`).
`TECPAN_KEEPER_BIN` points at a specific keeper binary.
