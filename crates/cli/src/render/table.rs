//! Aligned tables: columns separated by two spaces, the last column unpadded.

use super::style::{pad, width};

/// A table with a header row.
pub(crate) struct Table {
    rows: Vec<Vec<String>>,
}

impl Table {
    pub(crate) fn new(header: &[&str]) -> Self {
        Self {
            rows: vec![header.iter().map(|h| (*h).to_owned()).collect()],
        }
    }

    pub(crate) fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    pub(crate) fn render(&self) -> String {
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> = (0..columns)
            .map(|i| {
                self.rows
                    .iter()
                    .filter_map(|r| r.get(i))
                    .map(|c| width(c))
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let mut out = String::new();
        for row in &self.rows {
            let mut line = String::new();
            for (i, cell) in row.iter().enumerate() {
                if i + 1 == row.len() {
                    line.push_str(cell);
                } else {
                    line.push_str(&pad(cell, widths[i]));
                    line.push_str("  ");
                }
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_columns() {
        let mut table = Table::new(&["ID", "STATUS"]);
        table.row(vec!["7f3a1c".into(), "ready".into()]);
        table.row(vec!["a".into(), "\x1b[32mok\x1b[0m".into()]);
        assert_eq!(
            table.render(),
            "ID      STATUS\n7f3a1c  ready\na       \x1b[32mok\x1b[0m\n"
        );
    }
}
