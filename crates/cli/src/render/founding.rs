//! Founding steps (agree, dkg, derive, register, deploy, verify) and the lobby roster.

use std::fmt::Write as _;

use super::{Style, elapsed};
use crate::{
    refs::short_hex,
    types::{FoundingState, FoundingStep, LobbyView},
};

fn label(id: &str) -> &str {
    match id {
        "agree" => "Agree on the plan",
        "dkg" => "Distributed key generation",
        "derive" => "Derive the vault keys and address",
        "register" => "Register the vault in the PXE",
        "deploy" => "Deploy the vault contract",
        "verify" => "Verify the on-chain config",
        other => other,
    }
}

/// One step line, e.g. `✓ dkg       Distributed key generation          2.1s  round 3 of 3`.
pub(crate) fn step_line(step: &FoundingStep, now_ms: u64, style: Style) -> String {
    let mark = match step.status.as_str() {
        "done" => style.ok(),
        "failed" => style.fail(),
        "active" => style.yellow("…"),
        _ => style.dim("·"),
    };
    let took = match (step.started_at, step.finished_at) {
        (Some(start), Some(end)) => elapsed(end.saturating_sub(start)),
        (Some(start), None) if step.status == "active" => elapsed(now_ms.saturating_sub(start)),
        _ => String::new(),
    };
    let mut line = format!("{mark} {:<9}{:<36}{took:>7}", step.id, label(&step.id));
    if let Some(detail) = step.detail.as_deref().filter(|d| !d.is_empty()) {
        let _ = write!(line, "  {}", style.dim(detail));
    }
    line.trim_end().to_owned()
}

/// `… dkg      Distributed key generation`, printed when a step starts.
pub(crate) fn started_line(step: &FoundingStep, style: Style) -> String {
    format!("{} {:<9}{}", style.yellow("…"), step.id, label(&step.id))
}

/// Every step, then the outcome.
pub(crate) fn steps(state: &FoundingState, now_ms: u64, style: Style) -> String {
    let mut out = String::new();
    for step in &state.steps {
        out.push_str(&step_line(step, now_ms, style));
        out.push('\n');
    }
    out.push_str(&outcome(state, style));
    out
}

/// The closing line: founded vault, or the failure with blamed members.
pub(crate) fn outcome(state: &FoundingState, style: Style) -> String {
    if let Some(error) = &state.error {
        let mut out = format!(
            "{} Founding failed at {}: {}\n",
            style.fail(),
            error.step,
            error.message
        );
        if !error.blamed.is_empty() {
            let _ = writeln!(out, "  Blamed: {}", error.blamed.join(", "));
        }
        return out;
    }
    match (&state.vault, state.done) {
        (Some(vault), true) => format!(
            "{} Vault founded at {vault}\n  Next: tecpan receive · tecpan balance · tecpan token deploy-test\n",
            style.ok()
        ),
        _ => String::new(),
    }
}

/// The lobby header and member list.
pub(crate) fn roster(lobby: &LobbyView, style: Style) -> String {
    let mut out = format!(
        "Lobby {}  {}  spend {}-of-n  view {}-of-n  {}  {}\n",
        style.bold(&lobby.name),
        short_hex(&lobby.lobby),
        lobby.threshold,
        lobby.viewing_threshold,
        lobby.network,
        style.status(&lobby.status)
    );
    for member in &lobby.members {
        let you = if member.you { "  (you)" } else { "" };
        let _ = writeln!(out, "  {}  {}{you}", member.index, member.name);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FoundingError, LobbyMember};

    fn step(
        id: &str,
        status: &str,
        start: Option<u64>,
        end: Option<u64>,
        detail: Option<&str>,
    ) -> FoundingStep {
        FoundingStep {
            id: id.into(),
            status: status.into(),
            started_at: start,
            finished_at: end,
            detail: detail.map(Into::into),
        }
    }

    #[test]
    fn founding_steps() {
        let state = FoundingState {
            lobby: "0x01".into(),
            steps: vec![
                step("agree", "done", Some(0), Some(300), None),
                step("dkg", "done", Some(300), Some(2400), Some("round 3 of 3")),
                step("derive", "active", Some(2400), None, None),
                step("register", "pending", None, None, None),
            ],
            vault: None,
            error: None,
            done: false,
        };
        assert_eq!(
            steps(&state, 3500, Style::PLAIN),
            "\
✓ agree    Agree on the plan                     300ms
✓ dkg      Distributed key generation             2.1s  round 3 of 3
… derive   Derive the vault keys and address      1.1s
· register Register the vault in the PXE
"
        );
    }

    #[test]
    fn founding_outcomes() {
        let mut state = FoundingState {
            vault: Some("0x2874".into()),
            done: true,
            ..FoundingState::default()
        };
        assert!(outcome(&state, Style::PLAIN).starts_with("✓ Vault founded at 0x2874\n"));
        state.error = Some(FoundingError {
            step: "dkg".into(),
            message: "complaint against bea".into(),
            blamed: vec!["bea".into()],
        });
        assert_eq!(
            outcome(&state, Style::PLAIN),
            "✗ Founding failed at dkg: complaint against bea\n  Blamed: bea\n"
        );
    }

    #[test]
    fn lobby_roster() {
        let lobby = LobbyView {
            lobby: "0x0123456789abcdef0123456789abcdef".into(),
            status: "open".into(),
            name: "Ops".into(),
            threshold: 2,
            viewing_threshold: 2,
            network: "local".into(),
            members: vec![
                LobbyMember {
                    index: 1,
                    name: "ana".into(),
                    you: true,
                    ..LobbyMember::default()
                },
                LobbyMember {
                    index: 2,
                    name: "bea".into(),
                    ..LobbyMember::default()
                },
            ],
            ..LobbyView::default()
        };
        assert_eq!(
            roster(&lobby, Style::PLAIN),
            "Lobby Ops  0x0123…cdef  spend 2-of-n  view 2-of-n  local  open\n  1  ana  (you)\n  2  bea\n"
        );
    }
}
