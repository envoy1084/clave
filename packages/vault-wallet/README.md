# @clave/vault-wallet

Node library that hosts an Aztec 6.0.0 PXE able to operate a Clave vault without its incoming viewing secret:

- a pnpm patch of `@aztec-labs/pxe` (`patches/`) routing every viewing-secret use through the key store;
- `ThresholdKeyStore` and `ThresholdEcdhProvider` (threshold ECDH with verified DLEQ partials);
- `VaultWallet`: deploy and verify vaults, prepare / preview / execute k-of-n transactions, vault authwits.

See `architecture/pxe-integration.md`.

```sh
pnpm --filter @clave/vault-wallet test        # unit tests
aztec start --local-network                    # in another terminal
pnpm --filter @clave/vault-wallet test:e2e    # end to end (CLAVE_E2E_PROVE=1 to prove client-side)
```

`@clave/vault-wallet/testing` deals viewing shares in-process for tests; the app never uses it.
