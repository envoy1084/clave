//! ANSI styling, off under `--no-color`, `NO_COLOR`, `--json` and when stdout is not a terminal.

/// Whether to emit colors.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Style {
    pub(crate) color: bool,
}

impl Style {
    #[cfg(test)]
    pub(crate) const PLAIN: Self = Self { color: false };

    fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    pub(crate) fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }

    pub(crate) fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }

    pub(crate) fn green(self, text: &str) -> String {
        self.paint("32", text)
    }

    pub(crate) fn red(self, text: &str) -> String {
        self.paint("31", text)
    }

    pub(crate) fn yellow(self, text: &str) -> String {
        self.paint("33", text)
    }

    pub(crate) fn ok(self) -> String {
        self.green("✓")
    }

    pub(crate) fn fail(self) -> String {
        self.red("✗")
    }

    /// A status word in its color.
    pub(crate) fn status(self, word: &str) -> String {
        match word {
            "ready" | "executed" | "running" | "done" | "started" => self.green(word),
            "blocked" | "failed" | "stale" | "expired" => self.red(word),
            "collecting" | "starting" | "restarting" | "open" | "active" => self.yellow(word),
            _ => self.dim(word),
        }
    }
}

/// Display width of `text`, ignoring ANSI escapes (every other char counts as one column).
pub(crate) fn width(text: &str) -> usize {
    let mut width = 0;
    let mut escape = false;
    for c in text.chars() {
        if escape {
            escape = c != 'm';
        } else if c == '\x1b' {
            escape = true;
        } else {
            width += 1;
        }
    }
    width
}

/// `text` padded with spaces to `cols` display columns.
pub(crate) fn pad(text: &str, cols: usize) -> String {
    let w = width(text);
    format!("{text}{}", " ".repeat(cols.saturating_sub(w)))
}
