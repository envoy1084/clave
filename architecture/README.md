# Clave architecture

Clave is a private k-of-n multisig vault on Aztec 6.0.0 with threshold viewing.

- **Spending** is enforced on chain. The vault is an Aztec account contract, `ClaveVault`, whose `entrypoint` runs a
  payload only when the auth witness carries valid Schnorr signatures from at least `k` of its `n` registered signers.
- **Viewing** is enforced off chain. The vault's incoming viewing secret is created by distributed key generation and
  never exists in one place. Each member's keeper holds one share; the PXE obtains `address_secret · P` through
  threshold ECDH from any `t_v` keepers, each partial carrying a DLEQ proof. Shares are reshared on every membership
  change, so a removed member cannot read what arrives afterwards, and the vault keeps its address.

This folder describes the system as built. Each page states how a part works, its exact formats and parameters, what it
guarantees, and where it lives in the code.

## Pages

| Page                                     | Covers                                                                                 |
| ---------------------------------------- | -------------------------------------------------------------------------------------- |
| [contract.md](contract.md)               | `ClaveVault`: storage, commitments, approval message, auth witness, governance, gates  |
| [viewing-keys.md](viewing-keys.md)       | Aztec key compatibility, DKG, threshold ECDH with DLEQ, resharing, envelopes           |
| [membership.md](membership.md)           | Membership rotation end to end: reshare, `rotate_membership`, activation, removal      |
| [wire.md](wire.md)                       | `clave-wire` message catalogue, canonical encoding, signing, replay, Node bindings     |
| [pxe-integration.md](pxe-integration.md) | The PXE patch, `ThresholdKeyStore`, the vault wallet, transactions, upgrade procedure  |
| [keeper.md](keeper.md)                   | `clave-keeper`: custody, committees, ECDH, ceremonies, lobbies, proposal store, RPC    |
| [networking.md](networking.md)           | libp2p mesh, protocols, membership gate, sync, mailboxes, `clave-relay` and its limits |
| [cli.md](cli.md)                         | The `clave` CLI, the `claved` daemon contract, profiles and the sealed signer          |
| [brand.md](brand.md)                     | Mark, colour, type, imagery and figures of the site                                    |

## System overview

```text
member device
  clave ──JSON-RPC, unix socket──▶ claved ──JSON-RPC, unix socket──▶ clave-keeper
  (per command)                    PXE (patched), sealed signer,     viewing share, DKG / reshare,
                                   founding, proposals               ECDH, proposal store, p2p
                                     │                                 │
                                     │ Aztec node API                  │ libp2p (QUIC/TCP, Noise),
                                     ▼                                 │ committee members only
                            Aztec network                              ├──▶ other members' keepers
                            ClaveVault (private only)                  └──▶ clave-relay (optional, untrusted):
                                                                            circuits, sealed mailboxes
```

`claved` always supervises a local keeper. The keeper binary can also run standalone on a member's own server
(passphrase-derived master key, `clave-keeper rpc` for operators), but `claved` has no option to use a remote keeper.

| Component      | Runs where                                | Holds                                                       |
| -------------- | ----------------------------------------- | ----------------------------------------------------------- |
| `clave`        | Member's device, one process per command  | Nothing; arguments, references, rendering, prompts          |
| `claved`       | Member's device, one daemon per profile   | PXE store, sealed approval signing key, vault records       |
| `clave-keeper` | Member's device (child of `claved`)       | The member's viewing share, keeper identity, proposal store |
| `clave-relay`  | Anyone's server; a vault may name several | Circuit metadata and sealed mailbox envelopes               |
| `ClaveVault`   | Aztec network                             | Config note (thresholds, commitments) and signer notes      |

## Trust model and privacy invariants

These are product requirements. A change that weakens one needs an explicit decision recorded here.

| Invariant                                                                        | Enforced by                                                                |
| -------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| No single party, member or server ever holds the full incoming viewing secret    | DKG without a dealer; `t_v ≥ 2` on chain; shares never exported            |
| Signing keys never leave the signer's device                                     | Passphrase-sealed signer inside `claved`; only signatures leave            |
| Relays and nodes see ciphertext and public data only                             | Noise end to end, HPKE-sealed mailboxes, private-only contract             |
| Signer identities, thresholds and the committee are never in public storage/args | All config lives in private notes; the vault has no public functions       |
| Fees are not paid from the vault in public                                       | Vault transactions use an external fee payer; the vault holds no Fee Juice |

| Party             | Trusted for                                       | Not trusted for                                         |
| ----------------- | ------------------------------------------------- | ------------------------------------------------------- |
| Your own `claved` | Driving its keeper and PXE, signing approvals     | n/a: it acts for the member                             |
| Other members     | Holding their share; fewer than `t_v` may collude | Anything beyond their signatures and DLEQ proofs        |
| `clave-relay`     | Availability only                                 | Confidentiality, integrity, linking mailboxes to vaults |
| Aztec node        | Availability and chain data                       | Learning vault membership or thresholds                 |

Any current member can read the vault: that is what membership means. Data seen before a member's removal cannot be
revoked.

## End-to-end flows

### Found a vault

Steps are the `founding` states of `claved` (`agree`, `dkg`, `derive`, `register`, `deploy`, `verify`). Every member
runs them independently and reaches the same address.

```text
host claved/keeper                       co-founder claved/keeper                   Aztec
lobby.open ──▶ invite string ──(out of band)──▶ lobby.join (HMAC join proof)
           ◀── LobbyJoin ─────────────────────
LobbyState (each roster change) ─────────────▶
lobby.start: LobbyPlan (session, k, t_v, salt, sealed protocol secrets) ─▶     agree
ceremony.dkg ◀────────── Feldman rounds, echo, transcript confirm ───────▶     dkg
derive: preaddress, Ivpk, address = x(preaddress·G + Ivpk) (both sides)        derive
vault.register (manifest) on each keeper; vault registered in each PXE         register
host: private constructor ───────────────────────────────────────────────▶   deploy
others: wait until get_config answers                                          deploy
every member: verifyVaultConfig (commitments, k, t_v, n) before use            verify
```

Details: lobbies in [keeper.md](keeper.md#lobbies), the DKG in
[viewing-keys.md](viewing-keys.md#distributed-key-generation), derivation and deployment in
[pxe-integration.md](pxe-integration.md#vault-keys-and-deployment), the constructor in
[contract.md](contract.md#functions).

### Receive and read a note

```text
sender ── private transfer to vault ──▶ Aztec (tagged log, ephemeral key E)
member PXE sync: needs address_secret · E
  ThresholdKeyStore.addressSecretMul ─▶ own keeper (ecdh.batch)
     own keeper ── EcdhRequest ──▶ t_v - 1 other keepers (members of the current epoch only)
     each keeper: D_i = x_i · E with a DLEQ proof; gatherer verifies every proof
  claved verifies again, combines Σ λ_i · D_i = address_secret · E
PXE derives the message key from the point and decrypts as for any account
```

Tagging secrets for discovering notes use the same path with counterparty address points. Details:
[viewing-keys.md](viewing-keys.md#threshold-ecdh), [pxe-integration.md](pxe-integration.md#the-patch),
[keeper.md](keeper.md#threshold-ecdh).

### Propose, approve, execute

```text
proposer claved: build payload (+ sponsored fee call), pin tx_nonce, config version, expires_at
                 outer_hash from Aztec's DefaultAccountEntrypoint; Proposal stored and pushed via keepers
each approver:   review rebuilds the tx from payload, checks outer_hash, config version and deadline, simulates
                 signs approval = Poseidon2("clave.approval.v0", [vault, outer_hash, version, expires_at])
                 Approval replicated to every member's keeper
executor:        ≥ k valid approvals → 49-field auth witness → prove → send
ClaveVault:      witness keys hash to signer_set_commitment, deadline, ≥ k signatures valid
executor:        ExecutionReceipt replicated
```

- **Reject.** A member signs `Poseidon2("clave.rejection.v0", [vault, id_hi, id_lo])`, an off-chain vote. When more
  than `n - k` members reject, the proposal is `blocked`.
- **Cancel.** A new proposal with the same `tx_nonce` and no calls. Once it executes (with `k` approvals of its own),
  the nonce is spent and the original can never run.
- **Invalidate.** `invalidate_pending` bumps the config version, which voids every collected approval.

Details: [contract.md](contract.md#authorization), [pxe-integration.md](pxe-integration.md#vault-transactions),
[wire.md](wire.md#proposals-approvals-and-rejections), [keeper.md](keeper.md#proposal-store).

### Rotate membership

```text
members agree on the new ordered set (removing at most t_v - 1 current members)
exactly t_v current keepers deal λ_i·s_i to the new committee (ceremony.reshare); new shares held as pending
new verification shares → committee_commitment
rotate_membership proposal (current set, new set, k', t_v', new shares) → k approvals under the current config
transaction included: config version + 1
each keeper (vault.observeConfig): commitment matches its pending reshare → activate, erase old share
                                   commitment it is not part of         → destroy share, vault removed
engines switch the PXE to the new committee and a new epoch tagging secret
```

The contract, keeper and vault wallet implement this flow and the end-to-end suite runs it; `claved` and the CLI do
not offer it yet. Details: [membership.md](membership.md).

## Where to find it in the code

| Concern                                     | Path                                   |
| ------------------------------------------- | -------------------------------------- |
| Vault contract                              | `contracts/vault_contract/src/`        |
| Contract tests (TXE)                        | `contracts/vault_test/src/`            |
| Test-only signer helper (never deploy)      | `contracts/test_signer_contract/`      |
| Curve, Aztec derivation, DKG, ECDH, reshare | `crates/crypto/src/`                   |
| Wire messages, encoding, signing, p2p codec | `crates/wire/src/`                     |
| Keeper daemon                               | `crates/keeper/src/`                   |
| Relay                                       | `crates/relay/src/`                    |
| Node bindings                               | `crates/napi/src/`                     |
| CLI                                         | `crates/cli/src/`                      |
| Narrated two-member demo (dev tool)         | `crates/demo/src/`                     |
| Deployment files (Docker, systemd)          | `crates/deploy/`                       |
| PXE host, key store, vault transactions     | `packages/vault-wallet/src/`           |
| `claved` daemon                             | `packages/engine/src/`                 |
| PXE patch                                   | `patches/@aztec-labs__pxe@6.0.0.patch` |
| Aztec golden vectors generator              | `tools/aztec-vectors/`                 |
| Site                                        | `apps/web/`                            |
