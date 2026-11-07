//! The demo's story: a 2-of-2 vault between Alice and Bob, from empty profiles to an executed payment.

/// Which terminal an action runs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Who {
    Alice,
    Bob,
}

impl Who {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Alice => 0,
            Self::Bob => 1,
        }
    }

    pub(crate) fn profile(self) -> &'static str {
        match self {
            Self::Alice => "alice",
            Self::Bob => "bob",
        }
    }
}

/// A value read from a daemon while the story runs, pasted into later commands as `{name}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Capture {
    /// Alice's open lobby invite.
    Invite,
    /// Bob's own Aztec account.
    BobAccount,
    /// The newest proposal id (first six hex digits).
    Proposal,
    /// The test token Alice deployed.
    Token,
}

impl Capture {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Invite => "invite",
            Self::BobAccount => "bob",
            Self::Proposal => "id",
            Self::Token => "token",
        }
    }
}

/// One thing the runner does.
#[derive(Clone, Debug)]
pub(crate) enum Act {
    /// Type `command` in `who`'s terminal. `{name}` placeholders are pasted at once; `{yes}` becomes ` -y` in headless
    /// runs. With `wait`, the step continues only after the command finishes.
    Run {
        who: Who,
        command: &'static str,
        wait: bool,
    },
    /// Wait for `who`'s running command to finish.
    Wait(Who),
    /// Read a value from a daemon (retrying until it appears).
    Capture(Capture),
}

/// One narrated step.
pub(crate) struct Step {
    /// Clear both terminals before this step: a new chapter of the story.
    pub(crate) clear: bool,
    pub(crate) title: &'static str,
    pub(crate) lines: &'static [&'static str],
    pub(crate) acts: Vec<Act>,
}

fn run(who: Who, command: &'static str) -> Act {
    Act::Run {
        who,
        command,
        wait: true,
    }
}

/// The story.
#[allow(
    clippy::too_many_lines,
    reason = "the story is data: one block per step"
)]
pub(crate) fn story() -> Vec<Step> {
    use Who::{Alice, Bob};
    vec![
        Step {
            clear: false,
            title: "Alice creates her wallet",
            lines: &[
                "Generates a Grumpkin signing key, sealed at rest under Argon2id(passphrase) + ChaCha20-Poly1305.",
                "Also creates her keeper's libp2p identity and an Aztec Schnorr account; starts her PXE and keeper.",
                "→ Type a passphrase twice in Alice's terminal. It never leaves her machine.",
            ],
            acts: vec![run(Alice, "clave init --name Alice{yes}")],
        },
        Step {
            clear: false,
            title: "Bob creates his wallet",
            lines: &[
                "Same on Bob's side: his own signing key, keeper identity, account and PXE.",
                "No shared server: the two machines only ever talk keeper to keeper over libp2p.",
                "→ Type Bob's passphrase twice.",
            ],
            acts: vec![run(Bob, "clave init --name Bob{yes}")],
        },
        Step {
            clear: true,
            title: "Alice opens a 2-of-2 vault",
            lines: &[
                "k = 2: the contract accepts a transaction only with both members' signatures.",
                "t_v = 2: the vault's viewing secret will be Shamir-shared, so both keepers are needed to read it.",
                "Her keeper opens a lobby; the invite carries its peer id, addresses and the network.",
            ],
            acts: vec![
                Act::Run {
                    who: Alice,
                    command: "clave lobby open --name Treasury -k 2 --tv 2 --members 2 --alias treasury",
                    wait: false,
                },
                Act::Capture(Capture::Invite),
            ],
        },
        Step {
            clear: false,
            title: "Bob joins with the invite",
            lines: &[
                "Bob checks the plan (thresholds, host, network) and confirms. → y in Bob's terminal.",
                "Founding: a Feldman DKG gives each keeper a share of the viewing secret; nobody ever holds the whole.",
                "Then both derive the same vault address, Alice deploys it, and both verify the on-chain config.",
            ],
            acts: vec![
                run(Bob, "clave lobby join {invite} --alias treasury{yes}"),
                Act::Wait(Alice),
            ],
        },
        Step {
            clear: true,
            title: "Both see the same vault",
            lines: &[
                "Each machine reads the contract's config itself: version, k, t_v, and the Poseidon2",
                "commitments to the signer set and the viewing committee, checked against the founding plan.",
            ],
            acts: vec![run(Alice, "clave vault show"), run(Bob, "clave vault show")],
        },
        Step {
            clear: false,
            title: "Fund the vault",
            lines: &[
                "Local network only: deploy a test token (TTT) and mint 10,000 to the vault as private notes.",
                "Reading the balance needs t_v keepers: each returns s_i · P with a Chaum-Pedersen DLEQ proof,",
                "and Alice's PXE combines them by Lagrange interpolation into address_secret · P to decrypt.",
            ],
            acts: vec![
                run(Alice, "clave token deploy-test"),
                run(Alice, "clave balance"),
                Act::Capture(Capture::Token),
            ],
        },
        Step {
            clear: false,
            title: "Bob reads the balance too",
            lines: &[
                "Bob's PXE runs the same threshold decryption, with Alice's keeper answering his request.",
                "Neither member can decrypt alone, and every partial is proof-checked before it is used.",
            ],
            acts: vec![
                run(Bob, "clave token add {token}"),
                run(Bob, "clave balance"),
            ],
        },
        Step {
            clear: true,
            title: "Alice proposes paying Bob 100 TTT",
            lines: &[
                "Her PXE builds a private Token.transfer, simulates it and pins a tx nonce and expiry.",
                "The proposal (payload, outer hash, config version) is signed and replicated to Bob's keeper.",
                "Nothing touches the chain yet, and the proposal is visible to members only.",
            ],
            acts: vec![
                Act::Capture(Capture::BobAccount),
                run(Alice, "clave contacts add {bob} bob"),
                run(
                    Alice,
                    "clave propose transfer 100 ttt --to bob --memo 'Invoice 42'",
                ),
                Act::Capture(Capture::Proposal),
            ],
        },
        Step {
            clear: false,
            title: "Alice approves",
            lines: &[
                "Her daemon rebuilds the transaction from the payload and checks the outer hash, config and expiry.",
                "She signs Poseidon2(clave.approval.v0, [vault, outer hash, config version, expires_at]).",
                "→ y, then her passphrase: the key is unsealed in memory for this one signature.",
            ],
            acts: vec![run(Alice, "clave approve {id}{yes}")],
        },
        Step {
            clear: false,
            title: "Bob reviews and approves",
            lines: &[
                "Bob's daemon rebuilds the same transaction independently. If anything differed, it would refuse.",
                "What he sees is decoded from his own rebuild, not from Alice's description.",
                "→ y, then Bob's passphrase.",
            ],
            acts: vec![
                run(Bob, "clave proposals"),
                run(Bob, "clave approve {id}{yes}"),
            ],
        },
        Step {
            clear: true,
            title: "Bob executes",
            lines: &[
                "With 2 of 2 approvals, Bob's PXE packs the signatures into the 49-field auth witness,",
                "proves the transaction on his machine and sends it. The vault's entrypoint verifies both",
                "signatures on-chain; signer identities and thresholds stay private.",
            ],
            acts: vec![run(Bob, "clave execute {id}")],
        },
        Step {
            clear: false,
            title: "Both verify",
            lines: &[
                "Each member reads the receipt and the new balance on their own machine:",
                "executed, and 100 TTT less in the vault, decrypted once more by both keepers together.",
            ],
            acts: vec![
                run(Alice, "clave proposal show {id}"),
                run(Alice, "clave balance"),
                run(Bob, "clave balance"),
            ],
        },
    ]
}
