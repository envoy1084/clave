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
            title: "Alice creates her wallet",
            lines: &[
                "A signing key is generated on her machine and sealed under her passphrase.",
                "Her daemon (PXE) and her keeper start. Type a passphrase twice in Alice's terminal.",
            ],
            acts: vec![run(Alice, "clave init --name Alice{yes}")],
        },
        Step {
            title: "Bob creates his wallet",
            lines: &[
                "Same on Bob's machine: his own keys, his own keeper. Type his passphrase twice.",
            ],
            acts: vec![run(Bob, "clave init --name Bob{yes}")],
        },
        Step {
            title: "Alice opens a 2-of-2 vault",
            lines: &[
                "k = 2: both must approve every spend. t_v = 2: both keepers are needed to read it.",
                "Her keeper opens a lobby and prints an invite for Bob.",
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
            title: "Bob joins with the invite",
            lines: &[
                "He checks the plan (name, host, thresholds, network) and confirms with y.",
                "Then both machines run the founding: DKG, the vault address, deployment, verification.",
            ],
            acts: vec![
                run(Bob, "clave lobby join {invite} --alias treasury{yes}"),
                Act::Wait(Alice),
            ],
        },
        Step {
            title: "Both see the same vault",
            lines: &[
                "Same address, same members, same on-chain config, each verified on its own machine.",
            ],
            acts: vec![run(Alice, "clave vault show"), run(Bob, "clave vault show")],
        },
        Step {
            title: "Fund the vault",
            lines: &[
                "On the local network: deploy a test token (TTT) and mint 10,000 privately to the vault.",
            ],
            acts: vec![
                run(Alice, "clave token deploy-test"),
                run(Alice, "clave balance"),
                Act::Capture(Capture::Token),
            ],
        },
        Step {
            title: "Bob reads the balance too",
            lines: &[
                "The vault's notes decrypt only with both keepers (threshold ECDH).",
                "Neither member can read the vault alone.",
            ],
            acts: vec![
                run(Bob, "clave token add {token}"),
                run(Bob, "clave balance"),
            ],
        },
        Step {
            title: "Alice proposes paying Bob 100 TTT",
            lines: &[
                "She labels Bob's account, then proposes. The proposal is replicated to Bob's keeper.",
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
            title: "Alice approves",
            lines: &[
                "Her daemon rebuilds the transaction itself and shows exactly what she signs.",
                "Confirm with y, then her passphrase.",
            ],
            acts: vec![run(Alice, "clave approve {id}{yes}")],
        },
        Step {
            title: "Bob reviews and approves",
            lines: &[
                "Bob's daemon rebuilds the same transaction independently; if anything differed it would refuse.",
                "Confirm with y, then his passphrase.",
            ],
            acts: vec![
                run(Bob, "clave proposals"),
                run(Bob, "clave approve {id}{yes}"),
            ],
        },
        Step {
            title: "Bob executes",
            lines: &[
                "Two of two approvals: Bob's daemon builds the witness, proves and sends the transaction.",
            ],
            acts: vec![run(Bob, "clave execute {id}")],
        },
        Step {
            title: "Both verify",
            lines: &[
                "Each machine reads the result on its own: executed, and the vault is down by 100.",
            ],
            acts: vec![
                run(Alice, "clave proposal show {id}"),
                run(Alice, "clave balance"),
                run(Bob, "clave balance"),
            ],
        },
    ]
}
