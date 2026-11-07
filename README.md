# Clave

**A private k-of-n multisig for [Aztec](https://aztec.network) with threshold viewing.** Spending a payment takes `k`
of the `n` members. Reading the vault takes `t` of the members' keepers, because the key that decrypts it never exists
in one place: not with a member, not on a server, not with me.

I built this for a hackathon. It is **not production-ready**: it runs on a local Aztec network, it has not been audited,
and a few pieces are unfinished (listed honestly [below](#whats-not-done-yet)). But the core is real and works end to
end: founding a vault together, threshold viewing, proposing, reviewing, approving, rejecting and executing private
payments, with no server in the middle.

## The problem

**A multisig on a public chain guards one thing.** A Safe keeps its owners and threshold in public storage, and
`execTransaction` checks `k` ECDSA signatures on-chain against that list. The balance, every transfer, every signer
and the threshold are public. The multisig only has to answer _who can spend_, because reading was never restricted.

**Aztec inverts that model.** Private state is a set of encrypted UTXO notes; the chain stores only commitments in the
note hash tree, and spending publishes nullifiers that can't be linked back to their notes. Transactions run and are
proven on the user's device (the PXE). Accounts are contracts, and the entrypoint decides what authorizes a
transaction. So a shared wallet on Aztec has a second question: _who can read it?_

**Spending is easy to share. Reading is not.** Authorization is code, so a k-of-n rule fits in the account contract.
Reading is a key. Every Aztec account has an incoming viewing secret `ivsk`, and every incoming note is opened with it:

```math
S = \text{address\_secret} \cdot P, \qquad \text{address\_secret} = \sigma \cdot (\text{ivsk} + \text{preaddress})
```

where `P` is the note's ephemeral public key, `S` becomes the AES key, and the sign `σ` and the preaddress are public.
And the address itself commits to the viewing public key:

```math
\text{address} = x\big(\text{preaddress} \cdot G + \text{Ivpk}\big), \qquad \text{Ivpk} = \text{ivsk} \cdot G
```

So the viewing key **can't be rotated**: a new key is a new address. A team sharing an Aztec account has to give
`ivsk` to every member's wallet, and a member you remove keeps it. Removing their signing rights is one transaction.
Removing their ability to read every future payment means migrating every fund to a new address.

DeFi Wonderland reached the same conclusion in
[_On an Aztec multisig_](https://forum.aztec.network/t/on-an-aztec-multisig/8280) (Aztec forum, January 2026). Their
ways out were protocol-level key rotation (not available), a TEE or MPC network holding the key (moves the trust), or
threshold-sharing the key, which without resharing survives at most `min(t, n − t)` removals in the vault's lifetime.
That post is research; Clave builds the third option and adds resharing.

## How Clave solves it

Two thresholds, enforced in two different places.

### Spending: k of n, inside the proof

`ClaveVault` (`contracts/vault_contract`, Aztec.nr v6) is an account contract. Each member signs, with Schnorr on
Grumpkin:

```math
m = \text{Poseidon2}\big(\texttt{"clave.approval.v0"},\ [\text{vault},\ \text{outer\_hash},\ \text{config\_version},\ \text{expires\_at}]\big)
```

The PXE passes a 49-field auth witness (8 signer keys, 8 signatures as 4 limbs each, a deadline). The private
entrypoint checks that the keys hash to the signer-set commitment in a private config note, that the deadline hasn't
passed, and that at least `k` signatures over `m` are valid. All 8 slots are always checked, so every vault size proves
the same circuit. The config note is read without being nullified, so the same check works in static
`verify_private_authwit` calls. Any config change bumps the version and voids pending approvals.

We use explicit signatures rather than FROST: they're cheap to verify inside a proof, need no interactive signing
rounds, and membership changes by ordinary consensus. Cost: **27,176 gates**, against 15,002 for Aztec's single-key
Schnorr account.

### Viewing: t of n, a key nobody ever holds

**Distributed key generation** (`crates/crypto`). At founding the keepers run a Feldman DKG on Grumpkin (the shape of
FROST's KeyGen). Each member `j` samples a polynomial of degree `t − 1`, publishes commitments to its coefficients with a
proof of knowledge, and sends each member `i` a sealed evaluation that the receiver checks against the commitments:

```math
f_j(x) = a_{j,0} + a_{j,1}x + \dots + a_{j,t-1}x^{t-1}, \qquad C_{j,k} = a_{j,k} \cdot G
```

```math
s_i = \sum_j f_j(i), \qquad \text{Ivpk} = \sum_j C_{j,0}, \qquad \text{ivsk} = \sum_j a_{j,0}\ \text{is never computed}
```

`Ivpk` goes into an ordinary Aztec address. Any `t` shares determine the secret; fewer reveal nothing.

**Shares of the address secret.** Lagrange coefficients over any qualified set `S` sum to 1, so shifting every share by
the public preaddress `h` shifts the secret by `h`. Each keeper derives its share of exactly the secret Aztec decrypts
with, and publishes the matching point:

```math
x_i = \sigma \cdot (s_i + h), \qquad A_i = x_i \cdot G
```

**Threshold ECDH.** To open a note, each keeper returns a partial with a Chaum-Pedersen DLEQ proof that it used the
share behind `A_i`. The proof's challenge binds the vault, epoch, member and `P`. The wallet verifies every proof,
drops bad partials and names their member, and combines any `t`:

```math
D_i = x_i \cdot P, \qquad \log_G A_i = \log_P D_i, \qquad \sum_{i \in S} \lambda_i D_i = \text{address\_secret} \cdot P
```

That is byte for byte what Aztec's own key store would compute. Golden tests check it against real Aztec 6.0.0 output:
point encoding, key derivation, the address secret for both signs, ECDH and app siloing. A minimal, marked patch to the
PXE (`patches/`) routes every read of the viewing secret to the keepers, batched and cached.

**Resharing when a member leaves.** `t` staying members deal their weighted shares onto fresh polynomials for the new
committee. Every receiver checks each dealer's constant term against that dealer's public share, so nobody can slip in
a different secret:

```math
g_i(0) = \lambda_i s_i, \qquad g_i(0) \cdot G \overset{?}{=} \lambda_i Y_i, \qquad s'_j = \sum_{i \in S} g_i(j), \qquad \sum_{i \in S} g_i(0) = \text{ivsk}
```

The secret, `Ivpk` and the address don't change. One `rotate_membership` transaction updates the signer set and the
viewing committee together. The removed member's share lies on the old polynomial and combines with nothing new. This
holds when fewer than `t` members leave per rotation and the staying members erase their old shares; rotations can
repeat as often as needed.

### What is split, and what isn't

| Secret                              | In Clave                       | Why                                                 |
| ----------------------------------- | ------------------------------ | --------------------------------------------------- |
| Incoming viewing (`ivsk`)           | split `t` of `n`, never formed | it reads every incoming note                        |
| Nullifier hiding, outgoing, tagging | shared among members           | fixed by the address; spending still needs `k` sigs |
| Signing keys                        | each member's device only      | sealed under the member's passphrase                |

Nobody can make a removed member forget notes they already decrypted. Clave protects what arrives after removal.

## What each member runs

There is no Clave server.

| Component                        | Where            | What it does                                                                               |
| -------------------------------- | ---------------- | ------------------------------------------------------------------------------------------ |
| `clave` (`crates/cli`)           | your terminal    | propose, review, approve, reject, execute                                                  |
| `claved` (`packages/engine`)     | your device      | your PXE with the threshold key store, your sealed signer, the founding and payment flows  |
| `clave-keeper` (`crates/keeper`) | always on, yours | your viewing share, DKG and resharing, DLEQ answers, libp2p sync (QUIC, or TCP with Noise) |
| `clave-relay` (`crates/relay`)   | optional, anyone | circuit relay and HPKE-sealed mailboxes; can drop messages, can't read them                |
| Aztec node                       | any node         | sync and proofs; never sees members, thresholds, amounts or signers                        |

A payment: Alice proposes and her daemon builds, simulates and pins a nonce and expiry. The keepers replicate the
proposal. Bob's daemon rebuilds the transaction from its calls, refuses unless the outer hash matches, and shows him
exactly what he signs. Approvals travel keeper to keeper, so members don't need to be online together. Once `k`
approvals are in, any member executes; fees go through Aztec's sponsored fee contract, so the vault never pays in
public.

**Measured on a laptop:** DKG across three keepers ≈ 0.4 s · threshold decryption across networks via a relay 7–12 ms
· 2-of-3 private payment with real client-side proving 6.9 s.

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
two people on two machines. In a 2-of-2 vault, stop Bob's daemon and Alice can't read the balance: not because of a
permission check, but because no single key exists that could.

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
  or who signed. Signer identities, thresholds and the committee are never written to public storage or public
  arguments.
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
- Every decryption partial carries a DLEQ proof and every dealt share is checked against public commitments, so a
  dishonest member can't corrupt a decryption or a ceremony without being caught and named.
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
<summary><b>Repository layout and development</b></summary>

| Path                     | What it is                                                      |
| ------------------------ | --------------------------------------------------------------- |
| `contracts/`             | The vault account contract and its tests (Noir, Aztec.nr 6.0.0) |
| `crates/`                | Rust: threshold crypto, wire format, keeper, relay, CLI, demo   |
| `packages/vault-wallet/` | TypeScript host for the Aztec PXE with the threshold key store  |
| `packages/engine/`       | `claved`, each member's daemon behind the CLI                   |
| `patches/`               | The minimal PXE patch                                           |
| `apps/web/`              | The project website                                             |
| `architecture/`          | Design documents, with the decisions behind them                |

```sh
pnpm check       # formatting, lint, typecheck, build, dependency policy
pnpm test        # contract, Rust and TypeScript tests
pnpm test:cli    # three members through the CLI against a local network
pnpm test:e2e    # wallet end to end against a local network
```

</details>

## License

Copyright 2026 Vedant Chainani. Licensed under the [Apache License, Version 2.0](LICENSE).
