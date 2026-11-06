//! Token amounts (decimal ↔ base units) and durations (`15m`, `24h`, `7d`).

use crate::error::{Error, Result};

/// Parses a decimal amount (`25`, `0.5`, `1_000.25`) into base units with `decimals` places.
pub(crate) fn parse_amount(input: &str, decimals: u32) -> Result<u128> {
    let bad = || Error::usage(format!("\"{input}\" is not an amount"));
    let cleaned: String = input.chars().filter(|c| *c != '_' && *c != ',').collect();
    let (whole, fraction) = cleaned.split_once('.').unwrap_or((&cleaned, ""));
    if whole.is_empty() && fraction.is_empty() {
        return Err(bad());
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return Err(bad());
    }
    let fraction = fraction.trim_end_matches('0');
    if fraction.len() > decimals as usize {
        return Err(Error::usage(format!(
            "\"{input}\" has more than {decimals} decimal places, the token's precision"
        )));
    }
    let scale = 10u128
        .checked_pow(decimals)
        .ok_or_else(|| Error::usage("token decimals out of range"))?;
    let too_big = || Error::usage(format!("\"{input}\" is too large"));
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| too_big())?
    };
    let mut frac: u128 = 0;
    if !fraction.is_empty() {
        frac = fraction.parse().map_err(|_| bad())?;
        frac *= 10u128.pow(decimals - u32::try_from(fraction.len()).map_err(|_| bad())?);
    }
    whole
        .checked_mul(scale)
        .and_then(|w| w.checked_add(frac))
        .ok_or_else(too_big)
}

/// Formats base units (a decimal string) with `decimals` places, trimming trailing zeros and grouping thousands.
pub(crate) fn format_amount(base_units: &str, decimals: u32) -> String {
    let digits = base_units.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return base_units.to_owned();
    }
    let decimals = decimals as usize;
    let padded = format!("{digits:0>width$}", width = decimals + 1);
    let (whole, fraction) = padded.split_at(padded.len() - decimals);
    let fraction = fraction.trim_end_matches('0');
    let whole = group_thousands(whole);
    if fraction.is_empty() {
        whole
    } else {
        format!("{whole}.{fraction}")
    }
}

/// `41203` → `41,203`.
pub(crate) fn group_thousands(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `90s`, `15m`, `24h`, `7d`, `2w` → seconds.
pub(crate) fn parse_duration(input: &str) -> Result<u64> {
    let bad = || {
        Error::usage(format!(
            "\"{input}\" is not a duration; use e.g. 15m, 24h, 7d"
        ))
    };
    let split = input.find(|c: char| !c.is_ascii_digit()).ok_or_else(bad)?;
    let (number, unit) = input.split_at(split);
    let number: u64 = number.parse().map_err(|_| bad())?;
    let unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        "w" => 604_800,
        _ => return Err(bad()),
    };
    match number.checked_mul(unit) {
        Some(0) | None => Err(bad()),
        Some(secs) => Ok(secs),
    }
}

/// A proposal lifetime in whole hours (`--expires`).
pub(crate) fn parse_expiry_hours(input: &str) -> Result<u64> {
    let secs = parse_duration(input)?;
    if !secs.is_multiple_of(3600) {
        return Err(Error::usage(format!(
            "--expires \"{input}\": use whole hours or days (24h, 3d, 7d, 30d)"
        )));
    }
    Ok(secs / 3600)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_to_base_units() {
        assert_eq!(parse_amount("25", 18).unwrap(), 25_000_000_000_000_000_000);
        assert_eq!(parse_amount("0.5", 2).unwrap(), 50);
        assert_eq!(parse_amount(".5", 1).unwrap(), 5);
        assert_eq!(parse_amount("1_000.25", 2).unwrap(), 100_025);
        assert_eq!(parse_amount("1.10", 1).unwrap(), 11);
        assert_eq!(parse_amount("7", 0).unwrap(), 7);
        assert_eq!(parse_amount("3.", 6).unwrap(), 3_000_000);
    }

    #[test]
    fn amount_errors() {
        let excess = parse_amount("0.001", 2).err().unwrap();
        assert_eq!(excess.exit_code(), 2);
        assert!(excess.message.contains("more than 2 decimal places"));
        for bad in ["", ".", "-1", "1e3", "abc", "1.2.3"] {
            assert!(parse_amount(bad, 18).is_err(), "{bad}");
        }
        assert!(parse_amount("1000000000000000000000000000000", 18).is_err());
    }

    #[test]
    fn amounts_formatted() {
        assert_eq!(format_amount("25000000000000000000", 18), "25");
        assert_eq!(format_amount("1500", 3), "1.5");
        assert_eq!(format_amount("10000000", 2), "100,000");
        assert_eq!(format_amount("5", 2), "0.05");
        assert_eq!(format_amount("0", 18), "0");
        assert_eq!(format_amount("123", 0), "123");
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("15m").unwrap(), 900);
        assert_eq!(parse_duration("7d").unwrap(), 604_800);
        assert!(parse_duration("0m").is_err());
        assert!(parse_duration("15").is_err());
        assert!(parse_duration("m").is_err());
        assert!(parse_duration("1y").is_err());
        assert_eq!(parse_expiry_hours("3d").unwrap(), 72);
        assert!(parse_expiry_hours("90m").is_err());
    }
}
