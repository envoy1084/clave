//! The clear-signing screen (`clave approve` / `clave reject`), built from the member's own rebuild.

use std::fmt::Write as _;

use super::{Style, layout, proposals::call_line, utc};
use crate::{
    refs::{short_hex, short_id},
    types::Review,
    units::group_thousands,
};

/// What the member is about to sign.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Approve,
    Reject,
}

/// Label column of the screen, shared by every group.
const LABEL: usize = 13;

/// The full screen, ending before the confirmation prompt.
pub(crate) fn screen(review: &Review, verb: Verb, style: Style) -> String {
    let p = &review.proposal;
    let title = match verb {
        Verb::Approve => "Approve",
        Verb::Reject => "Reject",
    };
    let mut out = layout::title(
        style,
        &format!("{title} proposal {}", short_id(&p.id)),
        &[&review.vault_name, &short_hex(&p.vault)],
    );

    out.push('\n');
    out.push_str(&layout::heading(style, "What"));
    out.push_str(&layout::fields_at(style, LABEL, &what(review, style)));

    out.push('\n');
    out.push_str(&layout::heading(
        style,
        "Transaction, rebuilt on this device",
    ));
    let calls = p
        .calls
        .iter()
        .enumerate()
        .map(|(i, call)| format!("{}  {}", style.dim(&i.to_string()), call_line(call, style)))
        .collect::<Vec<_>>()
        .join("\n");
    let simulation = match (
        &review.simulation.ok,
        &review.simulation.gas,
        &review.simulation.error,
    ) {
        (true, Some(gas), _) => format!(
            "{} ok  {}",
            style.ok(),
            style.dim(&format!(
                "gas l2 {} · da {}",
                group_thousands(&gas.l2.to_string()),
                group_thousands(&gas.da.to_string())
            ))
        ),
        (true, None, _) => format!("{} ok", style.ok()),
        (false, _, error) => format!(
            "{} {}",
            style.fail(),
            style.red(error.as_deref().unwrap_or("failed"))
        ),
    };
    let config = if p.config_version == p.current_config_version {
        format!("v{}  {}", p.config_version, style.dim("current"))
    } else {
        style.red(&format!(
            "v{}  vault is at v{}",
            p.config_version, p.current_config_version
        ))
    };
    out.push_str(&layout::fields_at(
        style,
        LABEL,
        &[
            ("Calls", calls),
            ("Fee", p.fee_method.clone()),
            ("Simulation", simulation),
            (
                "Deadline",
                format!("{}  {}", utc(p.expires_at), style.dim("block timestamp")),
            ),
            ("Config", config),
        ],
    ));

    out.push('\n');
    let c = &review.checks;
    let mark = |ok: bool| if ok { style.ok() } else { style.fail() };
    let mut checks = format!(
        "{} payload rebuilt, outer hash matches\n{} config version current\n{} not expired",
        mark(c.rebuilt),
        mark(c.config_current),
        mark(c.not_expired)
    );
    if let Some(failure) = &c.failure {
        let _ = write!(checks, "\n{}", style.red(failure));
    }
    let signs = match verb {
        Verb::Approve => &review.signs,
        Verb::Reject => &review.rejects,
    };
    out.push_str(&layout::fields_at(style, LABEL, &[("Checks", checks)]));
    out.push('\n');
    out.push_str(&layout::fields_at(
        style,
        LABEL,
        &[(
            "Signs",
            format!(
                "{}\n{}",
                signs.formula,
                style.dim(&format!("= {}", signs.message))
            ),
        )],
    ));
    out
}

/// The "What" group: action, recipient, token, memo, proposer.
fn what(review: &Review, style: Style) -> Vec<(&'static str, String)> {
    let p = &review.proposal;
    let mut rows = vec![("Action", style.bold(&review.action))];
    if let Some(r) = &review.recipient {
        let value = match (&r.label, r.source.as_deref()) {
            (Some(label), Some(source)) => format!(
                "{}  {}  {}",
                style.bold(label),
                style.cyan(&short_hex(&r.address)),
                style.dim(source)
            ),
            (Some(label), None) => format!(
                "{}  {}",
                style.bold(label),
                style.cyan(&short_hex(&r.address))
            ),
            (None, _) => format!(
                "{}  {}",
                style.cyan(&r.address),
                style.yellow("not in address book")
            ),
        };
        rows.push(("To", value));
    }
    if let Some(token) = &p.token {
        rows.push((
            "Token",
            format!(
                "{}  {}",
                style.bold(&token.symbol),
                style.dim(&format!(
                    "{} · {} decimals · {}",
                    token.name,
                    token.decimals,
                    short_hex(&token.address)
                ))
            ),
        ));
    }
    if let Some(memo) = p.memo() {
        rows.push((
            "Memo",
            format!("\"{memo}\"  {}", style.dim("proposer's text")),
        ));
    }
    rows.push((
        "Proposed by",
        format!(
            "{}  {}",
            p.proposer.name,
            style.dim(&format!("member {}", p.proposer.index))
        ),
    ));
    rows
}

/// `Approve as cai (member 3)? [y/N] `.
pub(crate) fn question(review: &Review, verb: Verb) -> String {
    let word = match verb {
        Verb::Approve => "Approve",
        Verb::Reject => "Reject",
    };
    format!(
        "{word} as {} (member {})?",
        review.my_name,
        review.proposal.my_slot + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        render::proposals::tests::{NOW, addr, transfer},
        types::{Checks, DecodedCall, Gas, Recipient, Signs, Simulation},
    };

    fn review() -> Review {
        let mut proposal = transfer();
        proposal.expires_at = 1_794_618_960;
        proposal.calls = vec![
            DecodedCall {
                to: addr("aa"),
                to_label: Some("SponsoredFPC".into()),
                function: "sponsor_unconditionally".into(),
                ..Default::default()
            },
            DecodedCall {
                to: addr("0f9d"),
                to_label: Some("Token".into()),
                function: "transfer".into(),
                args: vec!["to".into(), "amount".into()],
                ..Default::default()
            },
        ];
        proposal.my_slot = 2;
        Review {
            proposal,
            vault_name: "Ops".into(),
            my_name: "cai".into(),
            checks: Checks {
                rebuilt: true,
                outer_hash: "0x0857".into(),
                config_current: true,
                not_expired: true,
                failure: None,
            },
            action: "transfer 25 TTT (25000000000000000000 base units)".into(),
            recipient: Some(Recipient {
                address: addr("3029"),
                label: Some("bea".into()),
                source: Some("address book".into()),
            }),
            simulation: Simulation {
                ok: true,
                gas: Some(Gas {
                    da: 1536,
                    l2: 41_203,
                }),
                error: None,
            },
            signs: Signs {
                message: "0x00e28886".into(),
                formula: "Poseidon2(clave.approval.v0, [vault, outer hash, v0, expires_at])".into(),
            },
            rejects: Signs {
                message: "0x0bad5eed".into(),
                formula: "Poseidon2(clave.rejection.v0, [vault, id_hi, id_lo])".into(),
            },
        }
    }

    #[test]
    fn clear_signing_screen() {
        let _ = NOW;
        assert_eq!(
            screen(&review(), Verb::Approve, Style::PLAIN),
            "\
Approve proposal 7f3a1c  Ops · 0x2874…2874

What
Action       transfer 25 TTT (25000000000000000000 base units)
To           bea  0x3029…3029  address book
Token        TTT  Clave Test Token · 18 decimals · 0x0f9d…0f9d
Memo         \"Q3 retainer\"  proposer's text
Proposed by  ana  member 1

Transaction, rebuilt on this device
Calls        0  SponsoredFPC.sponsor_unconditionally()
             1  Token.transfer(to, amount)
Fee          Sponsored FPC
Simulation   ✓ ok  gas l2 41,203 · da 1,536
Deadline     2026-11-14 01:16 UTC  block timestamp
Config       v0  current

Checks       ✓ payload rebuilt, outer hash matches
             ✓ config version current
             ✓ not expired

Signs        Poseidon2(clave.approval.v0, [vault, outer hash, v0, expires_at])
             = 0x00e28886
"
        );
        assert_eq!(
            question(&review(), Verb::Approve),
            "Approve as cai (member 3)?"
        );
    }

    #[test]
    fn failed_checks_and_unknown_recipient() {
        let mut r = review();
        r.checks.config_current = false;
        r.checks.failure = Some("config version changed: proposed at v0, vault at v1".into());
        r.recipient = Some(Recipient {
            address: addr("3029"),
            label: None,
            source: None,
        });
        let screen = screen(&r, Verb::Reject, Style::PLAIN);
        assert!(screen.starts_with("Reject proposal"));
        assert!(screen.contains("✗ config version current"));
        assert!(screen.contains("config version changed"));
        assert!(screen.contains("not in address book"));
        // A rejection shows the rejection message it signs, never the approval message.
        assert!(screen.contains("clave.rejection.v0"));
        assert!(!screen.contains("clave.approval.v0"));
    }
}
