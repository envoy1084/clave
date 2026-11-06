//! Human output. Renderers return strings so tests can compare them with colors off.

pub(crate) mod founding;
pub(crate) mod proposals;
pub(crate) mod review;
pub(crate) mod status;
pub(crate) mod style;
pub(crate) mod table;

pub(crate) use style::Style;

/// Seconds since the epoch, now.
pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Unix seconds from a daemon time that may be milliseconds (the contract's default) or seconds (what some
/// fields carry in practice): anything past 1e11 is taken as milliseconds.
pub(crate) fn secs(time: u64) -> u64 {
    if time >= 100_000_000_000 {
        time / 1000
    } else {
        time
    }
}

/// `2026-11-14 01:16 UTC` for unix seconds.
pub(crate) fn utc(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// `in 6d 23h` / `2h 5m ago` for a time `at` relative to `now` (both unix seconds).
pub(crate) fn relative(at: u64, now: u64) -> String {
    let span = |secs: u64| {
        let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
        if d > 0 {
            format!("{d}d {h}h")
        } else if h > 0 {
            format!("{h}h {m}m")
        } else if m > 0 {
            format!("{m}m")
        } else {
            format!("{secs}s")
        }
    };
    if at >= now {
        format!("in {}", span(at - now))
    } else {
        format!("{} ago", span(now - at))
    }
}

/// `4.1s`, `1m 12s`, `350ms` for milliseconds.
pub(crate) fn elapsed(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{}.{}s", ms / 1000, ms % 1000 / 100)
    } else {
        format!("{}m {}s", ms / 60_000, ms % 60_000 / 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(1_794_618_960), "2026-11-14 01:16 UTC");
        assert_eq!(
            relative(100 + 6 * 86_400 + 23 * 3600 + 59, 100),
            "in 6d 23h"
        );
        assert_eq!(relative(100, 100 + 7500), "2h 5m ago");
        assert_eq!(relative(130, 100), "in 30s");
        assert_eq!(elapsed(4100), "4.1s");
        assert_eq!(elapsed(350), "350ms");
        assert_eq!(elapsed(72_000), "1m 12s");
        assert_eq!(secs(1_794_005_972_699), 1_794_005_972);
        assert_eq!(secs(1_794_005_972), 1_794_005_972);
    }
}
