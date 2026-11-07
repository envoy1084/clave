//! `clave proposals` and `clave proposal show`.

use std::fmt::Write as _;

use serde_json::Value;

use super::{Style, relative, table::Table, utc};
use crate::{
    refs::{short_hex, short_id},
    types::{Contact, ProposalView, VaultSummary, Vote},
    units::format_amount,
};

/// Statuses `clave proposals` shows by default.
pub(crate) const OPEN: [&str; 3] = ["collecting", "ready", "blocked"];

/// `Ops  0x2874…5e69   spend 2/3  view 2/3  config v0   block 412`.
pub(crate) fn vault_header(
    vault: &VaultSummary,
    config_version: Option<u64>,
    block: Option<u64>,
    style: Style,
) -> String {
    let mut line = format!(
        "{}  {}   spend {}/{}  view {}/{}",
        style.bold(&vault.name),
        short_hex(&vault.address),
        vault.threshold,
        vault.members,
        vault.viewing_threshold,
        vault.members
    );
    if let Some(v) = config_version {
        let _ = write!(line, "  config v{v}");
    }
    if let Some(block) = block {
        let _ = write!(line, "   block {block}");
    }
    line
}

/// One line describing what a proposal does, from its intent.
pub(crate) fn describe(proposal: &ProposalView, contacts: &[Contact]) -> String {
    let intent = &proposal.intent;
    let text = |key: &str| intent.get(key).and_then(Value::as_str).unwrap_or_default();
    match text("kind") {
        "transfer" => {
            let (amount, symbol) = match &proposal.token {
                Some(token) => (
                    format_amount(text("amount"), token.decimals),
                    token.symbol.clone(),
                ),
                None => (
                    format!("{} base units of", text("amount")),
                    short_hex(text("token")),
                ),
            };
            format!(
                "transfer {amount} {symbol} → {}",
                recipient(text("to"), contacts)
            )
        }
        "calls" => {
            let calls = intent
                .get("calls")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            match calls {
                [call] => {
                    let to = call.get("to").and_then(Value::as_str).unwrap_or_default();
                    let function = call.get("fn").and_then(Value::as_str).unwrap_or_default();
                    format!("call {}.{function}(…)", contract_label(proposal, to))
                }
                many => format!("batch of {} calls", many.len()),
            }
        }
        "cancel" => format!("cancel {} (burn nonce)", short_id(text("cancels"))),
        other => other.to_owned(),
    }
}

/// `bea (0x3029…3d08)` for a contact, else the short address.
pub(crate) fn recipient(address: &str, contacts: &[Contact]) -> String {
    match contacts
        .iter()
        .find(|c| c.address.eq_ignore_ascii_case(address))
    {
        Some(contact) => format!("{} ({})", contact.label, short_hex(address)),
        None => short_hex(address),
    }
}

fn contract_label(proposal: &ProposalView, address: &str) -> String {
    proposal
        .calls
        .iter()
        .find(|c| c.to.eq_ignore_ascii_case(address))
        .and_then(|c| c.to_label.clone())
        .unwrap_or_else(|| short_hex(address))
}

/// `██ 2/2` (filled to the threshold).
fn approvals_bar(proposal: &ProposalView) -> String {
    let k = proposal.threshold as usize;
    let have = proposal.valid_approvals();
    format!(
        "{}{} {have}/{k}",
        "█".repeat(have.min(k)),
        "░".repeat(k.saturating_sub(have))
    )
}

fn names(votes: &[Vote]) -> String {
    votes
        .iter()
        .filter(|v| v.valid)
        .map(|v| v.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The proposals table with its header and footer.
pub(crate) fn table(
    header: &str,
    proposals: &[&ProposalView],
    contacts: &[Contact],
    now: u64,
    style: Style,
) -> String {
    let mut out = format!("{header}\n\n");
    if proposals.is_empty() {
        out.push_str(
            "No proposals.\n\nPropose one: clave propose transfer <amount> <token> --to <ref>\n",
        );
        return out;
    }
    let mut table = Table::new(&["ID", "PROPOSAL", "APPROVALS", "EXPIRES", "STATUS"]);
    for p in proposals {
        let mut status = style.status(&p.status);
        let rejected = names(&p.rejections);
        if !rejected.is_empty() {
            let _ = write!(status, "  rejected by {rejected}");
        }
        table.row(vec![
            short_id(&p.id),
            describe(p, contacts),
            approvals_bar(p),
            relative(p.expires_at, now),
            status,
        ]);
    }
    out.push_str(&table.render());
    let open = proposals
        .iter()
        .filter(|p| OPEN.contains(&p.status.as_str()))
        .count();
    let _ = write!(
        out,
        "\n{open} open · clave proposal show <id> · clave approve <id>\n"
    );
    out
}

/// Everything about one proposal.
pub(crate) fn show(
    proposal: &ProposalView,
    contacts: &[Contact],
    now: u64,
    style: Style,
) -> String {
    let p = proposal;
    let mut out = format!(
        "Proposal {}  {}\n\n",
        style.bold(&p.id),
        style.status(&p.status)
    );
    let mut field = |label: &str, value: String| {
        let _ = writeln!(out, "  {label:<13}{value}");
    };
    field("Action", describe(p, contacts));
    if let Some(memo) = p.memo() {
        field("Memo", format!("\"{memo}\"  (proposer's text)"));
    }
    field(
        "Proposed by",
        format!("{} (member {})", p.proposer.name, p.proposer.index),
    );
    field(
        "Approvals",
        format!(
            "{} of {} needed ({} members)",
            p.valid_approvals(),
            p.threshold,
            p.members
        ),
    );
    field(
        "Rejections",
        format!(
            "{} (blocked above {})",
            p.valid_rejections(),
            p.members.saturating_sub(p.threshold)
        ),
    );
    let config = if p.config_version == p.current_config_version {
        format!("v{} (current)", p.config_version)
    } else {
        format!(
            "v{} (vault is at v{})",
            p.config_version, p.current_config_version
        )
    };
    field("Config", config);
    field(
        "Expires",
        format!("{} ({})", utc(p.expires_at), relative(p.expires_at, now)),
    );
    field("Fee", p.fee_method.clone());
    field("Tx nonce", p.tx_nonce.clone());
    field("Outer hash", p.outer_hash.clone());
    field("Signs", p.approval_message.clone());
    if let Some(execution) = &p.execution {
        field(
            "Executed",
            format!(
                "{} at {}",
                execution.tx_hash,
                utc(super::secs(execution.at))
            ),
        );
    }
    if let Some(by) = &p.cancelled_by {
        field("Cancel", by.clone());
    }
    out.push_str("\nCalls\n");
    for (i, call) in p.calls.iter().enumerate() {
        let to = call.to_label.clone().unwrap_or_else(|| short_hex(&call.to));
        let _ = writeln!(
            out,
            "  {i}  {to}.{}({})",
            call.function,
            call.args.join(", ")
        );
    }
    out.push_str("\nVotes\n");
    if p.approvals.is_empty() && p.rejections.is_empty() {
        out.push_str("  none yet\n");
    }
    for (word, votes) in [("approved", &p.approvals), ("rejected", &p.rejections)] {
        for v in votes {
            let mark = if v.valid { style.ok() } else { style.fail() };
            let mut line = format!("  {mark} member {}  {:<10} {word}", v.index, v.name);
            if let Some(reason) = v.reason.as_deref().filter(|r| !r.is_empty()) {
                let _ = write!(line, "  \"{reason}\"");
            }
            if !v.valid {
                line.push_str("  (invalid signature)");
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use serde_json::json;

    use super::*;
    use crate::types::TokenInfo;

    pub(crate) const NOW: u64 = 1_794_000_000;

    pub(crate) fn addr(fill: &str) -> String {
        format!("0x{}", fill.repeat(64 / fill.len()))
    }

    pub(crate) fn transfer() -> ProposalView {
        ProposalView {
            id: format!("0x7f3a1c{}", "0".repeat(58)),
            vault: addr("2874"),
            intent: json!({ "kind": "transfer", "token": addr("0f9d"), "to": addr("3029"), "amount": "25000000000000000000", "memo": "Q3 retainer" }),
            proposer: crate::types::Proposer {
                index: 1,
                name: "ana".into(),
            },
            status: "ready".into(),
            threshold: 2,
            members: 3,
            approvals: vec![
                Vote {
                    slot: 1,
                    index: 1,
                    name: "ana".into(),
                    valid: true,
                    reason: None,
                },
                Vote {
                    slot: 2,
                    index: 2,
                    name: "bea".into(),
                    valid: true,
                    reason: None,
                },
            ],
            expires_at: NOW + 6 * 86_400 + 23 * 3600 + 120,
            token: Some(TokenInfo {
                address: addr("0f9d"),
                name: "Clave Test Token".into(),
                symbol: "TTT".into(),
                decimals: 18,
            }),
            fee_method: "Sponsored FPC".into(),
            ..ProposalView::default()
        }
    }

    fn call() -> ProposalView {
        ProposalView {
            id: format!("0xc09e44{}", "0".repeat(58)),
            intent: json!({ "kind": "calls", "calls": [{ "to": addr("0f9d"), "fn": "mint_to_public", "args": [] }] }),
            status: "collecting".into(),
            threshold: 2,
            members: 3,
            approvals: vec![Vote {
                slot: 1,
                index: 1,
                name: "ana".into(),
                valid: true,
                reason: None,
            }],
            rejections: vec![Vote {
                slot: 3,
                index: 3,
                name: "cai".into(),
                valid: true,
                reason: Some("not yet".into()),
            }],
            calls: vec![crate::types::DecodedCall {
                to: addr("0f9d"),
                to_label: Some("Token".into()),
                function: "mint_to_public".into(),
                ..Default::default()
            }],
            expires_at: NOW + 2 * 86_400 + 4 * 3600 + 30,
            ..ProposalView::default()
        }
    }

    fn cancel() -> ProposalView {
        ProposalView {
            id: format!("0x51bd02{}", "0".repeat(58)),
            intent: json!({ "kind": "cancel", "cancels": "0x9a10e2ff" }),
            status: "collecting".into(),
            threshold: 2,
            expires_at: NOW + 6 * 86_400 + 22 * 3600 + 30,
            ..ProposalView::default()
        }
    }

    pub(crate) fn contacts() -> Vec<Contact> {
        vec![Contact {
            address: addr("3029"),
            label: "bea".into(),
        }]
    }

    #[test]
    fn proposals_table() {
        let vault = VaultSummary {
            name: "Ops".into(),
            address: addr("2874"),
            threshold: 2,
            viewing_threshold: 2,
            members: 3,
            ..VaultSummary::default()
        };
        let header = vault_header(&vault, Some(0), Some(412), Style::PLAIN);
        let (a, b, c) = (transfer(), call(), cancel());
        let rendered = table(&header, &[&a, &b, &c], &contacts(), NOW, Style::PLAIN);
        assert_eq!(
            rendered,
            "\
Ops  0x2874…2874   spend 2/3  view 2/3  config v0   block 412

ID      PROPOSAL                             APPROVALS  EXPIRES    STATUS
7f3a1c  transfer 25 TTT → bea (0x3029…3029)  ██ 2/2     in 6d 23h  ready
c09e44  call Token.mint_to_public(…)         █░ 1/2     in 2d 4h   collecting  rejected by cai
51bd02  cancel 9a10e2 (burn nonce)           ░░ 0/2     in 6d 22h  collecting

3 open · clave proposal show <id> · clave approve <id>
"
        );
    }

    #[test]
    fn show_lists_votes_with_reasons() {
        let rendered = show(&call(), &[], NOW, Style::PLAIN);
        assert!(
            rendered.contains("  ✓ member 1  ana        approved\n"),
            "{rendered}"
        );
        assert!(
            rendered.contains("  ✓ member 3  cai        rejected  \"not yet\"\n"),
            "{rendered}"
        );
        assert!(
            rendered.contains("  0  Token.mint_to_public()\n"),
            "{rendered}"
        );
        assert!(
            rendered.contains("  Rejections   1 (blocked above 1)\n"),
            "{rendered}"
        );
    }
}
