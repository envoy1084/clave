//! Command implementations, one module per area.

mod assets;
mod config;
mod contacts;
mod daemon;
mod lobby;
mod proposals;
mod propose;
mod setup;
mod signer;
mod vault;

use crate::{Command, context::Ctx, error::Result};

pub(crate) fn run(ctx: &Ctx, command: Command) -> Result<()> {
    match command {
        Command::Init { name, touch_id } => setup::init(ctx, name, touch_id),
        Command::Doctor => setup::doctor(ctx),
        Command::Status => setup::status(ctx),
        Command::Profile(cmd) => setup::profile(ctx, cmd),
        Command::Completions { shell } => {
            setup::completions(shell);
            Ok(())
        }
        Command::Daemon(cmd) => daemon::run(ctx, &cmd),
        Command::Unlock { duration } => signer::unlock(ctx, &duration),
        Command::Lock => signer::lock(ctx),
        Command::Signer(cmd) => signer::run(ctx, &cmd),
        Command::Config(cmd) => config::config(ctx, cmd),
        Command::Relay(cmd) => config::relay(ctx, cmd),
        Command::Lobby(cmd) => lobby::run(ctx, cmd),
        Command::Vault(cmd) => vault::run(ctx, cmd),
        Command::Balance { token } => assets::balance(ctx, token.as_deref()),
        Command::Token(cmd) => assets::token(ctx, cmd),
        Command::Receive { qr } => assets::receive(ctx, qr),
        Command::Propose(cmd) => propose::run(ctx, cmd, false),
        Command::Simulate(cmd) => propose::run(ctx, cmd, true),
        Command::Proposals { status, limit } => proposals::list(ctx, &status, limit),
        Command::Proposal(crate::ProposalCmd::Show { id, raw }) => proposals::show(ctx, &id, raw),
        Command::Approve { id } => proposals::vote(ctx, &id, &proposals::Vote::Approve),
        Command::Reject { id, reason } => {
            proposals::vote(ctx, &id, &proposals::Vote::Reject(reason))
        }
        Command::Execute { id } => proposals::execute(ctx, &id),
        Command::Cancel { id } => proposals::cancel(ctx, &id),
        Command::Watch => proposals::watch(ctx),
        Command::Activity { limit } => proposals::activity(ctx, limit),
        Command::Tx { hash } => proposals::tx(ctx, &hash),
        Command::Contacts(cmd) => contacts::run(ctx, cmd),
    }
}
