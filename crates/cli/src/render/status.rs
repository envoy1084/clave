//! `tecpan status` and `tecpan signer status`.

use std::fmt::Write as _;

use super::{Style, relative};
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
    let mut keeper = match &k.peer_id {
        Some(peer) => short_peer(peer),
        None => style.status(&k.status),
    };
    if k.peer_id.is_some() && k.status != "running" {
        let _ = write!(keeper, " {}", style.status(&k.status));
    }
    let relays = if status.settings.relays.is_empty() {
        "none".to_owned()
    } else {
        status.settings.relays.len().to_string()
    };
    let _ = write!(
        keeper,
        "   {} addresses, relays: {relays}",
        k.addresses.len()
    );
    if let Some(error) = &k.error {
        let _ = write!(keeper, "   {}", style.red(error));
    }
    keeper
}

fn node_line(n: &NodeStatus, style: Style) -> String {
    let mut node = n.url.clone();
    if n.reachable {
        if let Some(version) = &n.node_version {
            let _ = write!(node, "   Aztec {version}");
        }
        if let Some(chain) = n.l1_chain_id {
            let local = if chain == 31_337 { " (local)" } else { "" };
            let _ = write!(node, "   L1 {chain}{local}");
        }
        if let Some(block) = n.block_number {
            let _ = write!(node, "   block {block}");
        }
    } else if n.checked_at == 0 && n.error.is_none() {
        let _ = write!(node, "   {}", style.dim("not checked yet"));
    } else {
        let _ = write!(
            node,
            "   {}",
            style.red(&format!(
                "unreachable{}",
                n.error
                    .as_ref()
                    .map(|e| format!(": {e}"))
                    .unwrap_or_default()
            ))
        );
    }
    node
}

/// The status screen; `open[i]` is the number of open proposals of `status.vaults[i]`, when known.
pub(crate) fn status(
    status: &Status,
    pid: Option<u32>,
    open: &[Option<usize>],
    now_ms: u64,
    style: Style,
) -> String {
    let mut out = String::new();
    let name = status
        .profile
        .as_ref()
        .map_or("(not created)", |p| p.name.as_str());
    let daemon = match pid {
        Some(pid) => format!("daemon {} (pid {pid})", style.green("running")),
        None => format!("daemon {}", style.green("running")),
    };
    let _ = writeln!(
        out,
        "{:<10}{name:<15}{daemon}      {}",
        "Profile",
        signer(&status.signer, now_ms, style)
    );
    let _ = writeln!(out, "{:<10}{}", "Keeper", keeper_line(status, style));
    let _ = writeln!(out, "{:<10}{}", "Node", node_line(&status.node, style));
    let pxe = status
        .pxe
        .synced_block
        .map_or_else(|| "not synced yet".to_owned(), |b| format!("synced to {b}"));
    let _ = writeln!(out, "{:<10}{pxe}", "PXE");
    if status.vaults.is_empty() {
        let _ = writeln!(
            out,
            "{:<10}none yet (tecpan lobby open, or tecpan lobby join <invite>)",
            "Vaults"
        );
    }
    for (i, v) in status.vaults.iter().enumerate() {
        let label = if i == 0 { "Vaults" } else { "" };
        let name = v.alias.clone().unwrap_or_else(|| v.name.clone());
        let default = if v.is_default && status.vaults.len() > 1 {
            " *"
        } else {
            ""
        };
        let mut line = format!(
            "{label:<10}{name}{default}  {}   {}/{} spend, {}/{} view",
            short_hex(&v.address),
            v.threshold,
            v.members,
            v.viewing_threshold,
            v.members
        );
        match open.get(i).copied().flatten() {
            Some(1) => line.push_str("   1 open proposal"),
            Some(n) => {
                let _ = write!(line, "   {n} open proposals");
            }
            None => {}
        }
        out.push_str(&line);
        out.push('\n');
    }
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
Profile   cai            daemon running (pid 48121)      signer locked
Keeper    12D3KooWAt4u…MJt   2 addresses, relays: none
Node      http://localhost:8080   Aztec 6.0.0   L1 31337 (local)   block 413
PXE       synced to 413
Vaults    ops  0x2874…5e69   2/3 spend, 2/3 view   1 open proposal
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
