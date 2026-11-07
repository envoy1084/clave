# Membership

## Purpose

A vault's membership changes by one atomic step: `rotate_membership` replaces the signer set, the spending threshold
`k`, the viewing threshold `t_v` and the viewing committee together, under the vault's own k-of-n authorization. The
viewing committee is moved by resharing the incoming viewing secret, so the vault keeps its address and its notes, and
members who leave stop being able to read what arrives afterwards.

Adding a member, removing one, replacing a device key and reordering are all the same operation: a new ordered set.

## How it works

The committee commitment stored on chain covers the new members' verification shares, and those exist only after the
keepers reshare. So the reshare runs first, and the on-chain rotation activates its result.

```text
proposer engine            current keepers (t_v dealers)     new keepers            ClaveVault
agree new ordered set, k', t_v'
ceremony.reshare ─────────▶ deal λ_i·s_i to new committee ──▶ verify, combine,
  (same session, dealers,     (sub-share envelopes, kind 2,   confirm transcript
   new members, k', t_v')      at the old epoch)              share kept as pending-<session>
            ◀──────────────── ReshareResult { new manifest, new verification shares, transcript }
rotate_membership proposal: current keys, new keys, n', k', t_v', new verification shares
approve with k signatures under the current config version ───────────────────────────────▶ asserts, version + 1
each engine: vault.observeConfig { version, committee_commitment } ─▶ every keeper
  matches its completed reshare    → activate pending share at epoch = version, destroy old share
  newer, not one it belongs to     → destroy share, mark vault removed
engines: setViewing(new epoch, new verification shares, t_v', new epoch tagging secret)
```

1. **Agree.** Members agree off chain on the new ordered signer set, `k'` and `t_v'`. At most `t_v - 1` current members
   may be absent from it (the contract enforces this).
2. **Reshare.** Every participating member's engine calls `ceremony.reshare` on its keeper with the same parameters:
   `{ session, vault, current?, dealers, newThreshold, newSpendingThreshold, newMembers }`. `dealers` are exactly `t_v`
   current members. A joining member's keeper does not hold the vault yet, so its engine passes the current manifest
   as `current` after checking it against the on-chain committee commitment. The ceremony is described in
   [viewing-keys.md](viewing-keys.md#resharing) and [keeper.md](keeper.md#ceremonies). The old committee keeps serving
   ECDH throughout.
3. **Propose.** The `rotate_membership` call carries the current keys (checked against the stored commitment), the new
   keys, `n'`, `k'`, `t_v'` and the new verification shares in slot order. It is an ordinary vault transaction
   ([contract.md](contract.md#governance)), approved by `k` of the current signers.
4. **Activate.** After the transaction is included the config version is `v + 1`. Each member's engine reports
   `vault.observeConfig { vault, version, committeeCommitment }` to its keeper, which compares the observed commitment
   with its completed reshares:

   | Observed commitment                          | Keeper action                                                                               |
   | -------------------------------------------- | ------------------------------------------------------------------------------------------- |
   | equals the active committee's                | nothing (`current`)                                                                         |
   | equals a completed reshare's new committee   | new manifest active with epoch = version; pending share active; old destroyed (`activated`) |
   | differs, version newer than the active epoch | this keeper is not in the new committee: share destroyed, vault `removed`                   |
   | anything else                                | `configMismatch` event (`unknown`)                                                          |

5. **Switch the PXE.** Each staying engine points its `ThresholdKeyStore` at the new committee
   (`VaultWallet.setViewing`: epoch, verification shares, `t_v'`) and registers a fresh epoch tagging secret for the
   vault's unconstrained self-sends. The key store drops its ECDH caches when the provider changes.

The new committee epoch is the config version at which it became active. Partials and envelopes are bound to the
epoch, and keepers refuse requests for any other, so old and new shares can never be mixed.

## Parameters

| Value                        | Rule                                              |
| ---------------------------- | ------------------------------------------------- |
| Members removed per rotation | at most `t_v - 1` of the current members (by key) |
| Dealers                      | exactly `t_v` current members                     |
| New thresholds               | `1 ≤ k' ≤ n'`, `2 ≤ t_v' ≤ n'`, `n' ≤ 8`          |
| Committee epoch after        | the config version written by `rotate_membership` |
| Group key `Ivpk`, address    | unchanged                                         |
| Config version               | `+1`, which voids every approval collected before |

Larger exits run as several rotations, each reshared, or move funds to a new vault.

## What a removed member can and cannot read

| A removed member can                                                                         | A removed member cannot                                                    |
| -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Keep everything it decrypted while it was a member                                           | Get partials: keepers answer only members of the current epoch             |
| Keep the protocol secrets fixed by the address (nullifier hiding, outgoing viewing, tagging) | Decrypt new notes: every decryption needs `address_secret · P`             |
| Use the nullifier hiding key to see when a note it already knew is spent                     | Combine its old share with new shares (they lie on a different polynomial) |
| Combine old shares, if `t_v` old-epoch shares still existed                                  | Find the vault's unconstrained self-sends (new epoch tagging secret)       |
|                                                                                              | Approve: its key is no longer in the signer set                            |

The guarantee needs fewer than `t_v` removed members per reshare and staying members that erase their old shares
(keepers destroy them on activation), so that `t_v` old-epoch shares never exist again. Constrained self-sends (config
notes, `transfer_in_private` change) are tagged with handshakes, which need the address secret, so a removed member
cannot find those either.

## Guarantees and limits

- Rotation and committee change are one transaction: there is no state where the chain names a committee whose shares
  do not exist, as long as the reshare completed before the proposal was approved.
- A keeper activates or destroys shares only on its own engine's report of the chain, never on a peer's word. A keeper
  whose engine never reports keeps serving the superseded committee; those shares no longer combine with the new
  committee's and the contract no longer names them.
- **Pending shares are not cleaned up.** If the rotation transaction never lands, the reshare's `pending-<session>`
  share stays sealed in custody; nothing discards it on expiry, and activating a later reshare does not remove it.
- **Secrets to distribute.** The nullifier hiding, outgoing viewing and tagging secrets are shared at founding inside
  the lobby plan, and every staying engine needs the new epoch tagging secret. There is no flow yet that hands either
  to members after founding; the end-to-end suite passes them in process.
- **Not exposed in `claved` or the CLI.** The contract, the keeper (`ceremony.reshare`, `vault.observeConfig`) and the
  vault wallet (`setViewing`, `generateEpochTaggingSecret`) implement rotation, and the local-network suite runs it
  end to end, but `claved` neither starts reshares nor reports configs to its keeper.

## Code and tests

| Path                                                 | Contents                                                 |
| ---------------------------------------------------- | -------------------------------------------------------- |
| `contracts/vault_contract/src/main.nr`               | `rotate_membership`, signer note rewrite                 |
| `contracts/vault_contract/src/governance.nr`         | `rotate`, `removed_members`                              |
| `crates/crypto/src/reshare.rs`                       | Resharing                                                |
| `crates/keeper/src/keeper/ceremonies.rs`             | `ReshareRequest`, `ReshareResult`, pending shares        |
| `crates/keeper/src/keeper/vaults.rs`                 | `observe_config`, activation, removal                    |
| `packages/vault-wallet/src/wallet/vault-wallet.ts`   | `setViewing`, epoch tagging strategy                     |
| `packages/vault-wallet/src/vault/keys.ts`            | `VaultProtocolSecrets`, `generateEpochTaggingSecret`     |
| `contracts/vault_test/src/rotation.nr`               | 14 TXE rotation tests                                    |
| `crates/keeper/tests/keeper.rs`, `tests/ceremony.rs` | Reshare activation and removal, old/new share separation |
| `packages/vault-wallet/test/e2e/vault.test.ts`       | Rotation on a local network with real keepers (below)    |

The end-to-end test removes member 3 and adds member 4 in a 2-of-3 vault with `t_v = 2`: keepers 1 and 2 deal, keepers
1, 2 and 4 receive (keeper 4 gets the current manifest as `current`), the `rotate_membership` transaction is approved
and mined, the four keepers observe `activated, activated, removed, activated`, stale approvals are rejected, the new
member's approval counts, and both engines keep reading the vault at the new epoch.
