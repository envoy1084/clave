# Clave

**A shared wallet on [Aztec](https://aztec.network) that no single member can open alone, or even look inside alone.**

Clave is a k-of-n multisig vault. Like any multisig, a payment needs `k` of the `n` members to sign it. Unlike other
multisigs, the vault is also private _from its own members one at a time_: reading the balance or the history takes
`t` of the members' keepers working together. Nobody holds the key that decrypts the vault: not a member, not a
server, not me.

I built this for a hackathon. It is **not production-ready**: it runs on a local Aztec network, it has not been audited,
and a few pieces are unfinished (listed honestly [below](#whats-not-done-yet)). But the core is real and works end to
end: founding a vault together, threshold viewing, proposing, reviewing, approving, rejecting and executing private
payments, all without a server in the middle.

## Why it exists

On a public chain, a multisig protects _who can spend_ and leaves everything else public. On Aztec, funds are encrypted
notes, so a second question appears: _who can read them?_ The usual answer is "give everyone the viewing key", which
means every member, including one you remove later, can watch the treasury forever.

Clave puts a threshold on both:

- **Spending: k of n.** The vault is an Aztec account contract. A payment only goes through if its proof carries valid
  signatures from `k` members. The check happens inside the zero-knowledge proof, so the chain never sees who the
  members are, how many there are, or who signed.
- **Viewing: t of n.** The vault's viewing key is created by the members' keepers together and split between them from
  the first moment. To read a note, a member's wallet asks the keepers; any `t` of them can help, fewer learn nothing.
- **No server.** Each member runs their own small daemon and keeper. Keepers talk to each other directly; an optional
  relay helps when members can't reach each other, and it only ever sees encrypted messages.

## Try it

You need Node 24, pnpm 12, Rust 1.96 and Aztec 6.0.0 (`aztec-up install 6.0.0`).

```sh
pnpm install
aztec start --local-network          # in its own terminal, leave it running
pnpm demo                            # Alice and Bob side by side: found a 2-of-2 vault, fund, propose, approve, execute
```

`pnpm demo` is narrated and runs everything for real. Press Enter to go step by step; `pnpm demo --headless` runs the
whole story in about 30 seconds.

To use the CLI yourself:

```sh
pnpm cli:install                     # installs clave, its daemon and the keeper into ~/.clave
export PATH="$HOME/.clave/bin:$PATH"
clave doctor                         # checks Node, the binaries and the node

clave -P alice init                  # one profile per member; asks for a passphrase
clave -P alice lobby open --name Ops -k 2 --tv 2 --members 2   # prints an invite
clave -P bob init
clave -P bob lobby join '<invite>'   # both run the founding steps and get the same vault address

clave -P alice token deploy-test     # local network only: mints 10,000 test JADE into the vault
clave -P alice contacts add <bob's account address> bob
clave -P alice propose transfer 25 jade --to bob
clave -P alice approve <id>          # shows exactly what you are signing, then asks for your passphrase
clave -P bob approve <id>
clave -P bob execute <id>
clave -P bob balance
```

Every member is a separate profile with its own daemon, keeper and keys, so two terminals on one laptop behave like
two people on two machines.

## CROPS

**C (censorship resistance).** I run nothing that members depend on: no server, no API, no relayer. Each member runs
their own daemon and keeper. The only outside dependencies are an Aztec node (any node works; switch with
`clave config set node-url <url>`) and, optionally, a relay (anyone can run one, members can list several). If a relay
disappears or refuses service, keepers that can reach each other directly keep working, and offline members catch up
when they come back. Getting transactions included is up to the Aztec network itself.

**O (open source).** Apache-2.0, see [LICENSE](LICENSE). Everything builds and runs from this repository alone:
`pnpm install`, then `pnpm cli:install` or `pnpm demo`. No hosted service, no closed component.

**P (privacy).** What each outside party learns:

- **The Aztec node** sees your IP address, the note tags your wallet asks about and the transactions you send. Tags are
  derived from secrets and transactions are proofs, so the node doesn't learn the members, the thresholds, the amounts
  or who signed. Fees go through Aztec's sponsored fee contract, so the vault never pays in public.
- **A relay** sees which keepers connect and when, and the size of the sealed messages it forwards. It cannot read
  them or tell which vault they belong to.
- **Other members' keepers** see your IP address (you are in a vault together).
- **Me, analytics, trackers:** nothing. There are none, in the CLI or on the website.

**S (security).**

- Your signing key is created on your device and never leaves it. It is sealed with your passphrase (Argon2id and
  ChaCha20-Poly1305), or unlocked with Touch ID on macOS, and opened only to sign.
- Your share of the viewing key lives only in your keeper and is never exported or backed up.
- Your account secrets and vault records are protected by file permissions on your machine, so the daemon can start
  without asking for a password; the signing key always needs it.
- Every answer a keeper gives comes with a proof, so a dishonest member can't corrupt a decryption without being
  caught.
- Trust assumptions: fewer than `t` members collude to read the vault, and fewer than `k` collude to spend. Aztec and
  its node are trusted for liveness, not for secrecy.

## What's not done yet

- **Local network only.** No testnet or mainnet release yet. On the local network proofs are not really generated (the
  network accepts them unproven); on a real network they are, and a 2-of-3 private payment took about 7 s on a laptop.
- **Removing a member end to end.** The resharing protocol and the contract's `rotate_membership` are built and tested
  on their own, but the full "remove a member from a live vault and prove they can't read the next payment" flow is the
  next milestone.
- **No recovery.** If a member loses their device and passphrase, the other members have to rotate them out (which
  depends on the point above). There is no backup on purpose: keys never leave the device.
- **Rough edges.** Paying another member by name needs them added as a contact first; Touch ID is new and lightly
  tested; the CLI is the only interface (the earlier desktop app was retired); Linux and macOS only.
- **No audit.** The cryptography follows standard constructions and is tested against real Aztec output, but nobody
  outside has reviewed it.

<details>
<summary><b>How it works, for the technically curious</b></summary>

**The vault contract** (`contracts/vault_contract`, Aztec.nr v6). `ClaveVault` is an account contract. Its private
entrypoint reads a private config note (threshold, a commitment to the signer set, a commitment to the viewing
committee, a version) without nullifying it, so the same check also works inside static `verify_private_authwit` calls.
It takes a 49-field auth witness (up to eight signer keys, eight Schnorr signatures, a deadline) and requires `k` valid
signatures over `Poseidon2("clave.approval.v0", [vault, outer hash, config version, expires_at])`. Every slot is
checked, so all vault sizes share one circuit. Config changes bump the version, which voids old approvals.

**Threshold viewing** (`crates/crypto`). A Feldman DKG on Grumpkin gives each keeper a share whose group key is the
vault's incoming viewing public key, so the vault has an ordinary Aztec address. To decrypt a note with ephemeral key
`P`, each keeper returns its share of the address secret times `P` with a Chaum-Pedersen DLEQ proof, and the wallet
combines `t` of them with Lagrange coefficients into `address_secret · P`, exactly what Aztec's own key store would
compute. Golden tests check point encoding, key derivation, the address secret and ECDH byte for byte against Aztec
6.0.0.

**The wallet** (`packages/vault-wallet`). A small, marked patch to Aztec's PXE swaps in a key store that never holds a
vault's viewing secret and asks the keepers instead. The Rust crypto reaches TypeScript through napi-rs.

**Keepers and the network** (`crates/keeper`, `crates/relay`). Keepers hold the sealed share, run the key generation
and resharing ceremonies, answer decryption requests and replicate signed proposals, approvals, rejections and receipts
over libp2p (QUIC, or TCP with Noise). Offline members catch up through sync and sealed relay mailboxes.

**The daemon and CLI** (`packages/engine`, `crates/cli`). `claved` hosts the member's PXE, signer and workflows behind
an owner-only unix socket; `clave` is a Rust CLI on top. Before you approve, your own machine rebuilds the transaction
from its calls, checks it matches what was proposed, simulates it and shows you the exact message you are signing.

The full design, with the decisions behind it, is in [`architecture/`](architecture/).

</details>

<details>
<summary><b>Repository layout and development</b></summary>

| Path                     | What it is                                                      |
| ------------------------ | --------------------------------------------------------------- |
| `contracts/`             | The vault account contract and its tests (Noir, Aztec.nr 6.0.0) |
| `crates/`                | Rust: threshold crypto, wire format, keeper, relay, CLI, demo   |
| `packages/vault-wallet/` | TypeScript host for the Aztec PXE with the threshold key store  |
| `packages/engine/`       | `claved`, each member's daemon behind the CLI                   |
| `patches/`               | The minimal PXE patch                                           |
| `apps/web/`              | The project website                                             |
| `architecture/`          | Design documents                                                |

```sh
pnpm check       # formatting, lint, typecheck, build, dependency policy
pnpm test        # contract, Rust and TypeScript tests
pnpm test:cli    # three members through the CLI against a local network
pnpm test:e2e    # wallet end to end against a local network
```

</details>

## License

Copyright 2026 Vedant Chainani. Licensed under the [Apache License, Version 2.0](LICENSE).
