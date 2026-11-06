//! Relay counters exported as Prometheus text.

use std::{
    fmt::Write as _,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::store::Usage;

/// Relay counters, exported in Prometheus text format.
#[derive(Debug, Default)]
pub struct Metrics {
    /// Envelopes stored.
    pub deposits: AtomicU64,
    /// Deposits refused (quota, expiry, size, allowlist, rate).
    pub rejected: AtomicU64,
    /// Fetch requests.
    pub fetches: AtomicU64,
    /// Envelopes returned by fetches.
    pub delivered: AtomicU64,
    /// Envelopes deleted by their recipients.
    pub deleted: AtomicU64,
    /// Circuit reservations accepted.
    pub reservations: AtomicU64,
    /// Circuits relayed.
    pub circuits: AtomicU64,
}

impl Metrics {
    pub(crate) fn add(counter: &AtomicU64, n: u64) {
        counter.fetch_add(n, Ordering::Relaxed);
    }

    /// Prometheus text exposition.
    #[must_use]
    pub fn render(&self, usage: Usage) -> String {
        let mut out = String::new();
        let counters = [
            ("deposits_total", &self.deposits),
            ("deposits_rejected_total", &self.rejected),
            ("fetches_total", &self.fetches),
            ("envelopes_delivered_total", &self.delivered),
            ("envelopes_deleted_total", &self.deleted),
            ("reservations_total", &self.reservations),
            ("circuits_total", &self.circuits),
        ];
        for (name, value) in counters {
            let _ = writeln!(
                out,
                "# TYPE tecpan_relay_{name} counter\ntecpan_relay_{name} {}",
                value.load(Ordering::Relaxed)
            );
        }
        let _ = writeln!(
            out,
            "# TYPE tecpan_relay_envelopes gauge\ntecpan_relay_envelopes {}",
            usage.envelopes
        );
        let _ = writeln!(
            out,
            "# TYPE tecpan_relay_bytes gauge\ntecpan_relay_bytes {}",
            usage.bytes
        );
        out
    }
}
