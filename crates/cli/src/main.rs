//! `tecpan`: the Tecpan command line. A short-lived client of the profile's `tecpand` daemon
//! (`architecture/cli.md`): it parses arguments, resolves references, renders, prompts and reads passphrases.

mod commands;
mod context;
mod daemon;
mod error;
mod http;
mod profile;
mod refs;
mod render;
mod rpc;
mod secret;
mod types;
mod units;

use std::{io::Write, path::PathBuf, process::ExitCode};

use clap::{ArgAction, Args, Parser, Subcommand};

use crate::{context::Ctx, error::Error};

#[derive(Parser, Debug)]
#[command(
    name = "tecpan",
    version,
    about = "Tecpan: a private k-of-n multisig vault on Aztec with threshold viewing",
    // `-V` is `--vault`; the version flag stays as `--version`.
    disable_version_flag = true,
    propagate_version = true
)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub(crate) global: Global,
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Options every command accepts.
#[derive(Args, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent command-line flags"
)]
pub(crate) struct Global {
    /// Member profile.
    #[arg(
        short = 'P',
        long,
        env = "TECPAN_PROFILE",
        default_value = "default",
        global = true
    )]
    pub(crate) profile: String,
    /// Target vault (alias, name or address); default is the profile's selected or only vault.
    #[arg(short = 'V', long, env = "TECPAN_VAULT", global = true)]
    pub(crate) vault: Option<String>,
    /// Aztec node URL (used by `init` and `doctor`; the daemon keeps its own in `config`).
    #[arg(short = 'n', long, env = "AZTEC_NODE_URL", global = true)]
    pub(crate) node_url: Option<String>,
    /// Machine output on stdout; errors as JSON on stderr; never prompts.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Skip confirmations (never authentication).
    #[arg(short = 'y', long, global = true)]
    pub(crate) yes: bool,
    /// Less output.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub(crate) quiet: bool,
    /// More output (error details).
    #[arg(short, long, global = true, action = ArgAction::Count)]
    pub(crate) verbose: u8,
    /// No colors (also `NO_COLOR`).
    #[arg(long, global = true)]
    pub(crate) no_color: bool,
    /// Print the version.
    #[arg(long, action = ArgAction::Version)]
    version: Option<bool>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Create the profile: passphrase, signing key, keeper identity, account; starts the daemon.
    Init {
        /// Display name shown to other members (default: the profile name).
        #[arg(long)]
        name: Option<String>,
        /// Also unlock signatures with Touch ID (macOS).
        #[arg(long)]
        touch_id: bool,
    },
    /// Check Node, the keeper and engine builds, the node, and profile permissions.
    Doctor,
    /// Daemon, keeper, node, PXE, signer and vaults.
    Status,
    /// Manage profiles.
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// Start, stop and inspect the profile's daemon.
    #[command(subcommand)]
    Daemon(DaemonCmd),
    /// Keep the signer unlocked in the daemon for a while.
    Unlock {
        /// How long (e.g. 15m, 1h).
        #[arg(long = "for", default_value = "15m")]
        duration: String,
    },
    /// Lock the signer now.
    Lock,
    /// Signer status and Touch ID.
    #[command(subcommand)]
    Signer(SignerCmd),
    /// Daemon settings.
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Keeper relays (changing them restarts the keeper).
    #[command(subcommand)]
    Relay(RelayCmd),
    /// Print shell completions.
    Completions { shell: clap_complete::Shell },
    /// Found a vault: lobbies, DKG and deployment.
    #[command(subcommand)]
    Lobby(LobbyCmd),
    /// Vaults on this profile.
    #[command(subcommand)]
    Vault(VaultCmd),
    /// Private balances of the vault.
    Balance {
        /// Only this token.
        #[arg(long)]
        token: Option<String>,
    },
    /// Tracked tokens.
    #[command(subcommand)]
    Token(TokenCmd),
    /// The vault's address, to receive funds.
    Receive {
        /// Also as a terminal QR code.
        #[arg(long)]
        qr: bool,
    },
    /// Propose a transaction.
    #[command(subcommand)]
    Propose(ProposeCmd),
    /// Simulate a proposal without posting it (same as `propose … --dry-run`).
    #[command(subcommand)]
    Simulate(ProposeCmd),
    /// List proposals (default: open ones).
    Proposals {
        /// collecting, ready, blocked, executed, cancelled, expired, stale, open or all.
        #[arg(long, default_value = "open")]
        status: String,
        /// At most this many.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// One proposal.
    #[command(subcommand)]
    Proposal(ProposalCmd),
    /// Review, then sign an approval.
    Approve {
        /// Proposal id or unique prefix.
        id: String,
    },
    /// Review, then sign a rejection vote.
    Reject {
        /// Proposal id or unique prefix.
        id: String,
        /// Why (shown to the other members).
        #[arg(long)]
        reason: Option<String>,
    },
    /// Prove and send a ready proposal.
    Execute {
        /// Proposal id or unique prefix.
        id: String,
    },
    /// Propose burning a proposal's nonce (needs k approvals itself).
    Cancel {
        /// Proposal id or unique prefix.
        id: String,
    },
    /// Stream events.
    Watch,
    /// Finished proposals and incoming transfers.
    Activity {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Transaction status.
    Tx { hash: String },
    /// Local address book.
    #[command(subcommand)]
    Contacts(ContactsCmd),
}

#[derive(Subcommand, Debug)]
pub(crate) enum ProfileCmd {
    /// Profiles on this machine.
    List,
    /// This profile.
    Show,
    /// Stop the daemon and delete a profile directory.
    Remove { name: String },
}

#[derive(Subcommand, Debug)]
pub(crate) enum DaemonCmd {
    /// Start the daemon (detached unless --foreground) and wait until it answers.
    Start {
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the daemon and its keeper.
    Stop,
    /// Stop, then start.
    Restart,
    /// `daemon.info` and status.
    Status,
    /// Print the daemon or keeper log.
    Logs {
        /// Keep following.
        #[arg(short, long)]
        follow: bool,
        /// The keeper's log instead.
        #[arg(long)]
        keeper: bool,
        /// Lines to show.
        #[arg(long, default_value_t = 50)]
        lines: usize,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum SignerCmd {
    /// Locked or unlocked, and Touch ID.
    Status,
    /// Unlock signatures with Touch ID (macOS); the passphrase keeps working.
    EnableTouchId,
    /// Remove the Touch ID keychain item.
    DisableTouchId,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCmd {
    /// All settings, or one: node-url, prover, unlock-timeout, relays, default-vault.
    Get { key: Option<String> },
    /// Change a setting: node-url <url>, prover <on|off>, unlock-timeout <15m>, default-vault <ref>.
    Set { key: String, value: String },
}

#[derive(Subcommand, Debug)]
pub(crate) enum RelayCmd {
    /// Add a relay multiaddr ending in /p2p/<peer id>.
    Add { multiaddr: String },
    /// Configured relays.
    List,
    /// Remove a relay.
    Remove { multiaddr: String },
}

/// `tecpan lobby open`.
#[derive(Args, Debug)]
pub(crate) struct LobbyOpen {
    /// Vault name.
    #[arg(long)]
    pub(crate) name: String,
    /// Spending threshold k.
    #[arg(short = 'k', long = "threshold")]
    pub(crate) threshold: u32,
    /// Viewing threshold `t_v`.
    #[arg(long = "tv")]
    pub(crate) viewing_threshold: u32,
    /// Start once this many members (including you) joined; otherwise press Enter.
    #[arg(long)]
    pub(crate) members: Option<usize>,
    /// Alias for the founded vault.
    #[arg(long)]
    pub(crate) alias: Option<String>,
    /// Print the invite and return; start later with `tecpan lobby start`.
    #[arg(long)]
    pub(crate) no_wait: bool,
}

#[derive(Subcommand, Debug)]
pub(crate) enum LobbyCmd {
    /// Open a lobby, print the invite, wait for members, then found the vault.
    Open(LobbyOpen),
    /// Decode an invite (name, host, k, `t_v`, network).
    Inspect { invite: String },
    /// Join with an invite, then found the vault with the others.
    Join {
        invite: String,
        /// Alias for the founded vault.
        #[arg(long)]
        alias: Option<String>,
    },
    /// Host: start founding.
    Start {
        lobby: Option<String>,
        #[arg(short = 'k', long = "threshold")]
        threshold: Option<u32>,
        #[arg(long = "tv")]
        viewing_threshold: Option<u32>,
    },
    /// Roster and state.
    Status {
        lobby: Option<String>,
        /// Keep showing changes.
        #[arg(long)]
        watch: bool,
    },
    /// Lobbies of this profile.
    List,
    /// Leave (or, as host, close) a lobby.
    Leave { lobby: Option<String> },
}

#[derive(Subcommand, Debug)]
pub(crate) enum VaultCmd {
    /// Vaults with k, `t_v`, members and your slot.
    List,
    /// Select the default vault.
    Use { reference: String },
    /// Name a vault.
    Alias { reference: String, alias: String },
    /// Address, thresholds, config version, members, health.
    Show,
    /// Members with keys and keeper identities.
    Members,
    /// Reachability of each member's keeper.
    Peers {
        #[arg(long)]
        watch: bool,
    },
    /// Contract instance, public keys and commitments.
    Inspect,
    /// The keeper's ECDH audit log.
    Audit {
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum TokenCmd {
    /// Track a Token contract.
    Add { address: String },
    /// Tracked tokens.
    List,
    /// Local network only: deploy a test token and mint to the vault.
    DeployTest,
}

/// Options shared by every proposal kind.
#[derive(Args, Debug)]
pub(crate) struct ProposeOpts {
    /// Text for the other members (stored with the proposal).
    #[arg(long)]
    pub(crate) memo: Option<String>,
    /// Lifetime: 24h, 3d, 7d, 30d.
    #[arg(long, default_value = "7d")]
    pub(crate) expires: String,
    /// Simulate only; post nothing.
    #[arg(long)]
    pub(crate) dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ProposeCmd {
    /// Private Token.transfer from the vault.
    Transfer {
        /// Amount in token units, e.g. 25 or 0.5.
        amount: String,
        /// Token symbol, tokens:<symbol> or address.
        token: String,
        /// Recipient: contact label, member name or address.
        #[arg(long)]
        to: String,
        #[command(flatten)]
        opts: ProposeOpts,
    },
    /// Any function of a registered contract.
    Call {
        /// Contract address (or a tracked token).
        contract: String,
        /// Function name.
        function: String,
        /// Arguments, coerced from the ABI.
        #[arg(long, num_args = 0.., allow_hyphen_values = true)]
        args: Vec<String>,
        #[command(flatten)]
        opts: ProposeOpts,
    },
    /// Ordered calls in one transaction from a JSON file: [{ "to", "fn", "args" }] (at most 8).
    Batch {
        file: PathBuf,
        #[command(flatten)]
        opts: ProposeOpts,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum ProposalCmd {
    /// Everything about one proposal, including votes with reasons.
    Show {
        id: String,
        /// The daemon's JSON.
        #[arg(long)]
        raw: bool,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum ContactsCmd {
    /// Label an address (local only).
    Add { address: String, label: String },
    /// Labels.
    List,
    /// Remove a label.
    Remove { reference: String },
}

fn report(error: &Error, json: bool, verbose: bool) {
    let mut stderr = std::io::stderr().lock();
    if json {
        let _ = writeln!(stderr, "{}", error.to_json());
        return;
    }
    let _ = writeln!(stderr, "✗ {}", error.message);
    if let Some(hint) = &error.hint {
        let _ = writeln!(stderr, "  {hint}");
    }
    if verbose {
        let _ = writeln!(stderr, "  ({error:?})");
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.global.json;
    let verbose = cli.global.verbose > 0;
    let result = Ctx::new(&cli.global).and_then(|ctx| commands::run(&ctx, cli.command));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error, json, verbose);
            ExitCode::from(error.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("tecpan").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn clap_tree_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_flags_anywhere() {
        let cli = parse(&[
            "approve",
            "7f3a",
            "-P",
            "bea",
            "-V",
            "vaults:ops",
            "--json",
            "-y",
        ]);
        assert_eq!(cli.global.profile, "bea");
        assert_eq!(cli.global.vault.as_deref(), Some("vaults:ops"));
        assert!(cli.global.json && cli.global.yes);
        assert!(matches!(cli.command, Command::Approve { ref id } if id == "7f3a"));
        let cli = parse(&[
            "--profile",
            "cai",
            "-n",
            "http://node:8080",
            "--no-color",
            "-q",
            "status",
        ]);
        assert_eq!(cli.global.node_url.as_deref(), Some("http://node:8080"));
        assert!(cli.global.no_color && cli.global.quiet);
    }

    #[test]
    fn quiet_and_verbose_conflict() {
        assert!(Cli::try_parse_from(["tecpan", "-q", "-v", "status"]).is_err());
    }

    #[test]
    fn propose_transfer() {
        let cli = parse(&[
            "propose",
            "transfer",
            "25",
            "ttt",
            "--to",
            "bea",
            "--memo",
            "Q3",
            "--expires",
            "3d",
            "--dry-run",
        ]);
        let Command::Propose(ProposeCmd::Transfer {
            amount,
            token,
            to,
            opts,
        }) = cli.command
        else {
            panic!("not a transfer");
        };
        assert_eq!(
            (amount.as_str(), token.as_str(), to.as_str()),
            ("25", "ttt", "bea")
        );
        assert_eq!(opts.memo.as_deref(), Some("Q3"));
        assert_eq!(opts.expires, "3d");
        assert!(opts.dry_run);
    }

    #[test]
    fn propose_call_args() {
        let cli = parse(&[
            "simulate",
            "call",
            "0x01",
            "mint_to_public",
            "--args",
            "0x02",
            "-5",
            "true",
        ]);
        let Command::Simulate(ProposeCmd::Call { args, opts, .. }) = cli.command else {
            panic!("not a call");
        };
        assert_eq!(args, ["0x02", "-5", "true"]);
        assert_eq!(opts.expires, "7d");
    }

    #[test]
    fn lobby_open_flags() {
        let cli = parse(&[
            "lobby",
            "open",
            "--name",
            "Ops",
            "-k",
            "2",
            "--tv",
            "2",
            "--members",
            "3",
        ]);
        let Command::Lobby(LobbyCmd::Open(LobbyOpen {
            threshold,
            viewing_threshold,
            members,
            no_wait,
            ..
        })) = cli.command
        else {
            panic!("not lobby open");
        };
        assert_eq!(
            (threshold, viewing_threshold, members, no_wait),
            (2, 2, Some(3), false)
        );
    }

    #[test]
    fn misc_commands() {
        assert!(
            matches!(parse(&["unlock", "--for", "1h"]).command, Command::Unlock { ref duration } if duration == "1h")
        );
        assert!(matches!(
            parse(&["daemon", "logs", "-f", "--keeper"]).command,
            Command::Daemon(DaemonCmd::Logs {
                follow: true,
                keeper: true,
                ..
            })
        ));
        assert!(matches!(
            parse(&["completions", "zsh"]).command,
            Command::Completions { .. }
        ));
        assert!(matches!(
            parse(&["reject", "7f", "--reason", "too much"]).command,
            Command::Reject {
                reason: Some(_),
                ..
            }
        ));
        assert!(
            matches!(parse(&["proposals"]).command, Command::Proposals { ref status, limit: 20 } if status == "open")
        );
        assert!(Cli::try_parse_from(["tecpan", "approve"]).is_err());
    }
}
