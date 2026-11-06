# PXE integration and the vault wallet

`packages/vault-wallet` (`@tecpan/vault-wallet`) hosts an Aztec 6.0.0 PXE that operates Tecpan vaults without ever
holding a vault's incoming viewing secret. A 53-line pnpm patch of `@aztec-labs/pxe` routes every use of that secret
through the key store; `ThresholdKeyStore` answers with threshold ECDH (see `viewing-keys.md`). On top of it,
`VaultWallet` deploys vaults, verifies their config and runs k-of-n transactions and authwits.

Status: Phase 7. Partials come from in-process test shares; the keeper RPC replaces them in Phase 8.

## The patch (`patches/@aztec-labs__pxe@6.0.0.patch`)

Applied by pnpm (`patchedDependencies` in `pnpm-workspace.yaml`) to the compiled `dest/` files. Every changed line is
marked `// tecpan:`.

| Site                                                           | Before                                               | After, for threshold accounts                         |
| -------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------------------- |
| `createPXE` → `PXE.create` → `openPxeStores`                   | `new KeyStore(store)`                                | `options.createKeyStore(store)` when given            |
| `utility_execution_oracle#getSharedSecrets`                    | `ivsk` via `getMasterSecretKey(ivpkHash)`, per point | one `addressSecretMul(account, points)` for the batch |
| `pxe#deriveHandshakeSecret`                                    | `getMasterIncomingViewingSecretKey` + address secret | `addressSecretMul(recipient, [ephPk])`                |
| `log_service#getPointDerivedSecrets` / `#getSecretsForSenders` | recipient `ivsk`, ECDH per sender                    | `addressSecretMul(recipient, [senderPoint], cache)`   |
| `private_execution_oracle#getAppTaggingSecret`                 | sender `ivsk`, ECDH with the recipient               | `addressSecretMul(sender, [recipientPoint], cache)`   |
| `private_execution_oracle#isUnconstrainedSelfSend`             | forces `address-derived` for every own account       | defers to the wallet's tagging hook                   |

The check is `await this.keyStore.isThresholdAccount?.(account)`: with the stock `KeyStore` the method is absent and
the stock code runs unchanged. `test/pxe-patch.test.ts` fails if a marker is missing or if any
`getMasterIncomingViewingSecretKey(` call is not guarded.

**Upgrade procedure** (every Aztec release): `pnpm patch @aztec-labs/pxe@<new>`, re-apply each row above (search the
new `dest/` for `getMasterIncomingViewingSecretKey`, `getMasterSecretKey`, `computeAddressSecret`,
`computeSharedTaggingSecret`, `computeViaEcdh`, `isUnconstrainedSelfSend`), `pnpm patch-commit`, then run the unit
suite, `pnpm test:e2e` and the golden vectors (`tools/aztec-vectors`).

## `ThresholdKeyStore`

Extends Aztec's `KeyStore` over the same `key_store` map.

- **Registration.** `addAccount` accepts `ThresholdAccountKeys`: the nullifier-hiding, outgoing-viewing and tagging
  secrets, the group key `Ivpk`, and the public message-signing and fallback keys. It stores the stock layout minus
  `ivsk_m`, plus a `threshold` marker; `getAccounts` and `hasAccount` include marked accounts. Registration goes through
  `pxe.registerAccount`, so the PXE's address store is filled as for any account.
- **Guard.** `getMasterIncomingViewingSecretKey`, `getAccountSecretKeys` and `getMasterSecretKey(ivpkHash)` throw
  `ThresholdViewingError` for threshold accounts. The end-to-end suite runs with this guard; any unpatched read fails
  the test.
- **`addressSecretMul(account, points, { cache })`.** Identity inputs are rejected (as stock); off-curve inputs map to a
  random point so a malformed ephemeral key in a foreign log fails that decryption, not the whole sync. Valid points go
  to the account's `EcdhProvider`.
- **Batching and caching.** Requests made in the same tick become one provider call per account and purpose; a point
  already in flight is not requested twice. Results are kept **in memory for the session only**: tagging results
  (counterparty address points) without bound, note-decryption results in an LRU of 4096. Both are dropped when the
  account's provider changes (a new committee epoch).

**Decision: cache decryption results in memory.** The phase plan said never to cache one-shot log points. Measured,
the PXE re-derives the same message secrets on every sync, simulation and proof: the end-to-end suite asked keepers
for 125 points of which 21 were distinct (116 requests). A cached value is the shared secret of a message this engine
has already decrypted, so keeping it in memory reveals nothing new; it is never written to disk and is cleared at each
epoch. With the cache: 32 requests for 35 points.

## Threshold ECDH provider

`ThresholdEcdhProvider` holds a `ViewingCommittee` (vault, epoch, preaddress, `Ivpk`, verification shares, `t_v`) and a
`PartialsSource`. For each batch it asks the source for partials, then `@tecpan/napi.verifyAndCombine` checks every DLEQ
proof against `A_i` and combines `t_v` valid partials per point. Members with bad proofs are reported and skipped; fewer
than `t_v` valid partials for any point fails the batch.

| `PartialsSource`          | Where                                          | Status  |
| ------------------------- | ---------------------------------------------- | ------- |
| `InProcessPartialsSource` | `@tecpan/vault-wallet/testing`                 | Phase 7 |
| Keeper RPC                | the member's `tecpan-keeper` (local or remote) | Phase 8 |

A request carries vault, epoch, purpose (`decryption` / `tagging`) and points. Keepers take the address context from
their own manifest, never from the request.

## Vault keys and deployment

| Key                                 | Held by                       | Notes                                                    |
| ----------------------------------- | ----------------------------- | -------------------------------------------------------- |
| Incoming viewing `Ivpk` / shares    | Public / one share per keeper | From the DKG; the secret is never assembled              |
| Nullifier hiding, outgoing, tagging | Every member's engine         | `VaultProtocolSecrets`; shared over the keeper channel   |
| Message-signing, fallback           | Public keys only              | Secrets discarded at generation                          |
| Epoch tagging secret                | Current members' engines      | Tags the vault's unconstrained self-sends; new per epoch |
| Signing keys (Grumpkin)             | Each member's device          | Sign approvals; never in the PXE                         |

- `deriveVault(setup)` computes the instance with universal deployment (no deployer in the address), the constructor
  arguments (8-slot signer and committee arrays), and checks that Aztec's address equals the one the threshold crypto
  derives from `preaddress` and `Ivpk`.
- `deployVault` sends the private constructor from a member's own account with the vault in scope. No class or instance
  publication: the vault has no public functions.
- `verifyVaultConfig` reads `get_config` and compares both commitments, thresholds and signer count with the agreed
  setup. Every member runs it before funding or using the vault.

## Vault transactions

1. **Prepare** (`prepareVaultTx`): the app calls with the fee payment calls merged in, a pinned `txNonce`, and the
   current config version. The outer hash is captured from Aztec's own `DefaultAccountEntrypoint`, so it is exactly
   what the entrypoint will ask the witness for. The approval message is
   `Poseidon2("tecpan.approval.v0", [vault, outerHash, version, expiresAt])`.
2. **Preview** (`previewVaultTx`): simulation with the vault's class overridden by Aztec's stub Schnorr account, whose
   authorization always passes. Shows the effects of app calls; calls into the vault itself (governance) cannot be
   previewed this way.
3. **Approve**: each signer signs the approval message (`signApproval`, Aztec Schnorr).
4. **Execute** (`executeVaultTx`): every approval is verified, the 49-field witness is encoded through `@tecpan/napi`,
   and the transaction is proven with the pinned nonce. Vault transactions are always cancellable: a second approved
   transaction with the same nonce burns it.

Authwits (`prepareVaultAuthWit` / `createVaultAuthWit`) use the same approval message over the authwit message hash;
the consuming transaction attaches the witness and puts the vault in scope.

**Fees.** The entrypoint's fee-payment option is not covered by the outer hash, so an executor can choose it. Vault
transactions use an external payer (a sponsored FPC on the local network); the vault never holds Fee Juice, so the
`PREEXISTING_FEE_JUICE` path would fail rather than pay publicly. Fee design is Phase 14.

**Tagging.** The wallet's `resolveTaggingSecretStrategy` hook tags the vault's unconstrained self-sends with the epoch
tagging secret (address-derived when none is set), so members removed at a reshare stop finding them. Everything else
uses the configured strategy (Aztec's default, a non-interactive handshake). Constrained self-sends (config notes,
`transfer_in_private` change) still use handshakes; see Phase 12.

## Tests and measurements

| Suite                              | Covers                                                                                           |
| ---------------------------------- | ------------------------------------------------------------------------------------------------ |
| `test/pxe-patch.test.ts`           | Patch applied; every viewing-secret read guarded                                                 |
| `test/threshold-key-store.test.ts` | Registration without `ivsk`, guards, ordinary accounts, batching, de-duplication, caches, inputs |
| `test/threshold-ecdh.test.ts`      | Combination equals Aztec's `deriveEcdhSharedSecretPoint`; bad member reported; below threshold   |
| `test/deployment.test.ts`          | Address derivation, setup validation, witness encoding and approval checks                       |
| `test/e2e/vault.test.ts`           | Local network, two engines (below)                                                               |

End to end (`pnpm test:e2e`, against `aztec start --local-network`): a 2-of-3 vault with `t_v = 2`, members' keepers
simulated in-process, two engines reaching different keeper pairs. Deploy and verify in both engines; both discover the
vault's notes; preview; below-threshold and expired approvals rejected with the contract's reasons; a 2-of-3 private
transfer seen by both engines; cancellation burns the nonce; `transfer_in_private` through `verify_private_authwit` and
`lookup_validity`; `invalidate_pending` voids collected approvals; `rotate_membership` with a reshared committee rejects
stale approvals, accepts the new member's and both engines keep reading the vault at the new epoch.

Measured on macOS arm64: 10 tests in about 40 s with fake proofs, 62 s with client-side proving
(`TECPAN_E2E_PROVE=1`); a vault transfer takes 4.3 s (fake) / 6.9 s (proven) including mining.
