//! `clave status` and `clave signer status`.

use super::{Style, layout, relative};
use crate::{
    refs::short_hex,
    types::{NodeStatus, SignerStatus, Status},
};

/// `signer locked` / `signer unlocked (in 14m)`; `unlockedUntil` is unix milliseconds.
pub(crate) fn signer(signer: &SignerStatus, now_ms: u64, style: Style) -> String {
    match (signer.locked, signer.unlocked_until) {
        (true, _) => format!("signer {}", style.dim("locked")),
        (false, Some(until)) => format!(
            "signer {} ({})",
            style.yellow("unlocked"),
            relative(until / 1000, now_ms / 1000)
        ),
        (false, None) => format!("signer {}", style.yellow("unlocked")),
    }
}

fn short_peer(peer: &str) -> String {
    if peer.len() <= 16 {
        return peer.to_owned();
    }
    format!("{}…{}", &peer[..12], &peer[peer.len() - 3..])
}

fn keeper_line(status: &Status, style: Style) -> String {
    let k = &status.keeper;
    let mut parts = Vec::new();
    if let Some(peer) = &k.peer_id {
        parts.push(style.cyan(&short_peer(peer)));
    }
    parts.push(style.status(&k.status));
    parts.push(style.dim(&format!("{} addresses", k.addresses.len())));
    let relays = status.settings.relays.len();
    parts.push(style.dim(&if relays == 0 {
        "no relays".to_owned()
    } else {
        format!("{relays} relays")
    }));
    if let Some(error) = &k.error {
        parts.push(style.red(error));
    }
    parts.join(&style.dim(" · "))
}

fn node_line(n: &NodeStatus, style: Style) -> String {
    let mut parts = vec![n.url.clone()];
    if n.reachable {
        if let Some(version) = &n.node_version {
            parts.push(format!("Aztec {version}"));
        }
        if let Some(chain) = n.l1_chain_id {
            let local = if chain == 31_337 { " (local)" } else { "" };
            parts.push(style.dim(&format!("L1 {chain}{local}")));
        }
        if let Some(block) = n.block_number {
            parts.push(style.dim(&format!("block {block}")));
        }
    } else if n.checked_at == 0 && n.error.is_none() {
        parts.push(style.dim("not checked yet"));
    } else {
        parts.push(style.red(&format!(
            "unreachable{}",
            n.error.as_ref().map(|e| format!(": {e}")).unwrap_or_default()
        )));
    }
    parts.join(&style.dim(" · "))
}

/// The status screen; `open[i]` is the number of open proposals of `status.vaults[i]`, when known.
pub(crate) fn status(
    status: &Status,
    pid: Option<u32>,
    open: &[Option<usize>],
    now_ms: u64,
    style: Style,
) -> String {
    let name = status
        .profile
        .as_ref()
        .map_or("profile not created", |p| p.name.as_str());
    let daemon = format!("daemon {}", style.green("running"));
    let pid = pid.map(|p| format!("pid {p}")).unwrap_or_default();
    let mut out = layout::title(style, name, &[&daemon, &pid]);
    out.push('\n');
    let pxe = status.pxe.synced_block.map_or_else(
        || style.dim("not synced yet"),
        |b| format!("synced to block {b}"),
    );
    let mut rows = vec![
        (
            "Signer",
            signer(&status.signer, now_ms, style)
                .trim_start_matches("signer ")
                .to_owned(),
        ),
        ("Keeper", keeper_line(status, style)),
        ("Node", node_line(&status.node, style)),
        ("PXE", pxe),
    ];
    let vaults = if status.vaults.is_empty() {
        style.dim("none yet · clave lobby open, or clave lobby join <invite>")
    } else {
        status
            .vaults
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let name = v.alias.clone().unwrap_or_else(|| v.name.clone());
                let default = if v.is_default && status.vaults.len() > 1 {
                    " *"
                } else {
                    ""
                };
                let open = match open.get(i).copied().flatten() {
                    Some(1) => " · 1 open proposal".to_owned(),
                    Some(n) => format!(" · {n} open proposals"),
                    None => String::new(),
                };
                format!(
                    "{}{default}  {}  {}",
                    style.bold(&name),
                    style.cyan(&short_hex(&v.address)),
                    style.dim(&format!(
                        "{}-of-{} · view {}{open}",
                        v.threshold, v.members, v.viewing_threshold
                    ))
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    rows.push(("Vaults", vaults));
    out.push_str(&layout::fields(style, &rows));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{KeeperState, NodeStatus, Profile, PxeStatus, VaultSummary};

    #[test]
    fn status_screen() {
        let status = Status {
            profile: Some(Profile {
                name: "cai".into(),
                ..Profile::default()
            }),
            keeper: KeeperState {
                status: "running".into(),
                peer_id: Some("12D3KooWAt4uXyZabcdefMJt".into()),
                addresses: vec![
                    "/ip4/127.0.0.1/udp/1/quic-v1".into(),
                    "/ip4/127.0.0.1/tcp/2".into(),
                ],
                ..KeeperState::default()
            },
            node: NodeStatus {
                url: "http://localhost:8080".into(),
                reachable: true,
                node_version: Some("6.0.0".into()),
                l1_chain_id: Some(31_337),
                block_number: Some(413),
                ..NodeStatus::default()
            },
            pxe: PxeStatus {
                synced_block: Some(413),
            },
            signer: SignerStatus {
                locked: true,
                unlocked_until: None,
            },
            vaults: vec![VaultSummary {
                address: format!("0x2874{}5e69", "0".repeat(56)),
                name: "Ops".into(),
                alias: Some("ops".into()),
                threshold: 2,
                viewing_threshold: 2,
                members: 3,
                is_default: true,
                ..VaultSummary::default()
            }],
            ..Status::default()
        };
        assert_eq!(
            super::status(&status, Some(48121), &[Some(1)], 0, Style::PLAIN),
            "\
cai  daemon running · pid 48121

  Signer  locked
  Keeper  12D3KooWAt4u…MJt · running · 2 addresses · no relays
  Node    http://localhost:8080 · Aztec 6.0.0 · L1 31337 (local) · block 413
  PXE     synced to block 413
  Vaults  ops  0x2874…5e69  2-of-3 · view 2 · 1 open proposal
"
        );
    }

    #[test]
    fn signer_windows() {
        let unlocked = SignerStatus {
            locked: false,
            unlocked_until: Some(900_000),
        };
        assert_eq!(
            signer(&unlocked, 0, Style::PLAIN),
            "signer unlocked (in 15m)"
        );
    }
}
