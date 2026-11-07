//! The clear-signing screen (`clave approve` / `clave reject`), built from the member's own rebuild.

use std::fmt::Write as _;

use super::{Style, utc};
use crate::{refs::short_hex, types::Review, units::group_thousands};

/// What the member is about to sign.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
    Approve,
    Reject,
}

/// The full screen, ending before the confirmation prompt.
pub(crate) fn screen(review: &Review, verb: Verb, style: Style) -> String {
    let p = &review.proposal;
    let title = match verb {
        Verb::Approve => "Approve",
        Verb::Reject => "Reject",
    };
    let mut out = format!(
        "{} proposal {}… on {} ({})\n\n",
        style.bold(title),
        p.id.strip_prefix("0x")
            .unwrap_or(&p.id)
            .chars()
            .take(6)
            .collect::<String>(),
        review.vault_name,
        short_hex(&p.vault)
    );
    let mut field = |label: &str, value: &str| {
        let _ = writeln!(out, "  {label:<14}{value}");
    };
    field("Action", &review.action);
    if let Some(r) = &review.recipient {
        let value = match (&r.label, r.source.as_deref()) {
            (Some(label), Some(source)) => {
                format!("{label}  {}  ({source})", short_hex(&r.address))
            }
            (Some(label), None) => format!("{label}  {}", short_hex(&r.address)),
            (None, _) => format!("{}  (not in address book)", r.address),
        };
        field("To", &value);
    }
    if let Some(token) = &p.token {
        field(
            "Token",
            &format!(
                "{}  {}  ({}, {} decimals)",
                token.symbol,
                short_hex(&token.address),
                token.name,
                token.decimals
            ),
        );
    }
    for (i, call) in p.calls.iter().enumerate() {
        let to = call.to_label.clone().unwrap_or_else(|| short_hex(&call.to));
        let label = if i == 0 { "Calls" } else { "" };
        field(
            label,
            &format!("{i}  {to}.{}({})", call.function, call.args.join(", ")),
        );
    }
    field("Fee", &p.fee_method);
    let mut proposer = format!("{} (member {})", p.proposer.name, p.proposer.index);
    if let Some(memo) = p.memo() {
        let _ = write!(proposer, "    Memo  \"{memo}\"  (proposer's text)");
    }
    field("Proposed by", &proposer);
    field(
        "Deadline",
        &format!("{} (block timestamp)", utc(p.expires_at)),
    );
    let config = if p.config_version == p.current_config_version {
        format!("v{} (current)", p.config_version)
    } else {
        format!(
            "v{} (vault is at v{})",
            p.config_version, p.current_config_version
        )
    };
    field("Config", &config);
    let simulation = match (
        &review.simulation.ok,
        &review.simulation.gas,
        &review.simulation.error,
    ) {
        (true, Some(gas), _) => format!(
            "ok · gas l2 {} · da {}",
            group_thousands(&gas.l2.to_string()),
            group_thousands(&gas.da.to_string())
        ),
        (true, None, _) => "ok".to_owned(),
        (false, _, error) => format!("failed: {}", error.as_deref().unwrap_or("unknown error")),
    };
    field("Simulation", &simulation);
    out.push('\n');
    checks(&mut out, review, verb, style);
    out
}

/// The checks the daemon ran and the exact message the signature covers.
fn checks(out: &mut String, review: &Review, verb: Verb, style: Style) {
    let c = &review.checks;
    let mark = |ok: bool| if ok { style.ok() } else { style.fail() };
    let _ = writeln!(
        out,
        "  {:<14}{} payload rebuilt, outer hash matches   {} config version current   {} not expired",
        "Checks",
        mark(c.rebuilt),
        mark(c.config_current),
        mark(c.not_expired)
    );
    if let Some(failure) = &c.failure {
        let _ = writeln!(out, "  {:<14}{}", "", style.red(failure));
    }
    let signs = match verb {
        Verb::Approve => &review.signs,
        Verb::Reject => &review.rejects,
    };
    let _ = writeln!(out, "  {:<14}{}", "Signs", signs.formula);
    let _ = writeln!(out, "  {:<14}= {}", "", signs.message);
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
Approve proposal 7f3a1c… on Ops (0x2874…2874)

  Action        transfer 25 TTT (25000000000000000000 base units)
  To            bea  0x3029…3029  (address book)
  Token         TTT  0x0f9d…0f9d  (Clave Test Token, 18 decimals)
  Calls         0  SponsoredFPC.sponsor_unconditionally()
                1  Token.transfer(to, amount)
  Fee           Sponsored FPC
  Proposed by   ana (member 1)    Memo  \"Q3 retainer\"  (proposer's text)
  Deadline      2026-11-14 01:16 UTC (block timestamp)
  Config        v0 (current)
  Simulation    ok · gas l2 41,203 · da 1,536

  Checks        ✓ payload rebuilt, outer hash matches   ✓ config version current   ✓ not expired
  Signs         Poseidon2(clave.approval.v0, [vault, outer hash, v0, expires_at])
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
        assert!(screen.contains("(not in address book)"));
        // A rejection shows the rejection message it signs, never the approval message.
        assert!(screen.contains("clave.rejection.v0"));
        assert!(!screen.contains("clave.approval.v0"));
    }
}
