//! Building blocks shared by every screen, in the Aztec CLI's manner: a title line, then aligned `label  value`
//! blocks separated by blank lines.

use std::fmt::Write as _;

use super::{Style, style::pad};

/// `Title  detail · detail` (details dimmed).
pub(crate) fn title(style: Style, text: &str, details: &[&str]) -> String {
    let details: Vec<&str> = details.iter().copied().filter(|d| !d.is_empty()).collect();
    let mut line = style.bold(text);
    if !details.is_empty() {
        let _ = write!(line, "  {}", style.dim(&details.join(" · ")));
    }
    line.push('\n');
    line
}

/// Aligned `label  value` rows; labels dimmed.
pub(crate) fn fields(style: Style, rows: &[(&str, String)]) -> String {
    let width = rows
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0)
        + 2;
    fields_at(style, width, rows)
}

/// [`fields`] with a fixed label column, so several groups line up.
pub(crate) fn fields_at(style: Style, width: usize, rows: &[(&str, String)]) -> String {
    rows.iter().fold(String::new(), |mut out, (label, value)| {
        let mut lines = value.lines();
        let first = lines.next().unwrap_or_default();
        let _ = writeln!(out, "{}{first}", pad(&style.dim(label), width));
        for more in lines {
            let _ = writeln!(out, "{}{more}", " ".repeat(width));
        }
        out
    })
}

/// A bold group heading.
pub(crate) fn heading(style: Style, text: &str) -> String {
    format!("{}\n", style.bold(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks() {
        let s = Style::PLAIN;
        assert_eq!(
            title(s, "Treasury", &["0x12", "2-of-2", ""]),
            "Treasury  0x12 · 2-of-2\n"
        );
        assert_eq!(
            fields(
                s,
                &[
                    ("Address", "0x12".into()),
                    ("Spend", "2 of 2\nview 2".into())
                ]
            ),
            "Address  0x12\nSpend    2 of 2\n         view 2\n"
        );
    }
}
