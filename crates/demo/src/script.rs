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
                "Her signing key is created on her machine and locked with a passphrase only she knows.",
                "→ Type a passphrase twice in Alice's terminal.",
            ],
            acts: vec![run(Alice, "clave init --name Alice{yes}")],
        },
        Step {
            clear: false,
            title: "Bob creates his wallet",
            lines: &[
                "Same for Bob: his own key, his own keeper. No server sits between them.",
                "→ Type Bob's passphrase twice.",
            ],
            acts: vec![run(Bob, "clave init --name Bob{yes}")],
        },
        Step {
            clear: true,
            title: "Alice opens a 2-of-2 vault",
            lines: &[
                "Both must sign every payment, and both are needed to see the balance.",
                "Alice gets an invite to send to Bob.",
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
                "→ Bob checks the plan and types y. Their keepers then create the vault's viewing key together,",
                "split in two shares, so neither of them ever holds the whole key.",
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
                "Each machine checks the deployed contract on its own: same address, same members, same rules.",
            ],
            acts: vec![run(Alice, "clave vault show"), run(Bob, "clave vault show")],
        },
        Step {
            clear: false,
            title: "Fund the vault",
            lines: &[
                "Mint 10,000 JADE, a test token, privately into the vault.",
                "Alice can read the balance only because Bob's keeper helps decrypt it.",
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
                "Bob sees the same balance, with Alice's keeper helping. Neither can read the vault alone.",
            ],
            acts: vec![
                run(Bob, "clave token add {token}"),
                run(Bob, "clave balance"),
            ],
        },
        Step {
            clear: true,
            title: "Alice proposes paying Bob 100 JADE",
            lines: &[
                "The payment is prepared and shared with Bob's keeper. Nothing is on-chain yet.",
            ],
            acts: vec![
                Act::Capture(Capture::BobAccount),
                run(Alice, "clave contacts add {bob} bob"),
                run(
                    Alice,
                    "clave propose transfer 100 jade --to bob --memo 'Invoice 42'",
                ),
                Act::Capture(Capture::Proposal),
            ],
        },
        Step {
            clear: false,
            title: "Alice approves",
            lines: &[
                "Her machine rebuilds the transaction and shows exactly what she signs.",
                "→ y, then her passphrase.",
            ],
            acts: vec![run(Alice, "clave approve {id}{yes}")],
        },
        Step {
            clear: false,
            title: "Bob reviews and approves",
            lines: &[
                "Bob's machine rebuilds it independently and would refuse if anything differed.",
                "→ y, then his passphrase.",
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
                "Two of two signatures: Bob proves the transaction on his machine and sends it.",
                "The vault contract checks both signatures; who signed stays private.",
            ],
            acts: vec![run(Bob, "clave execute {id}")],
        },
        Step {
            clear: false,
            title: "Both verify",
            lines: &[
                "Each machine confirms the payment on its own: executed, and 100 JADE less in the vault.",
            ],
            acts: vec![
                run(Alice, "clave proposal show {id}"),
                run(Alice, "clave balance"),
                run(Bob, "clave balance"),
            ],
        },
    ]
}
