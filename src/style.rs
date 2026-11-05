/// Terminal style for one span of text. `fg` is an SGR foreground parameter
/// (30–37, 90–97), never a 256-colour index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub dim: bool,
    pub fg: Option<u8>,
}

impl Style {
    pub fn merge(self, other: Style) -> Style {
        Style {
            bold: self.bold || other.bold,
            italic: self.italic || other.italic,
            underline: self.underline || other.underline,
            strike: self.strike || other.strike,
            dim: self.dim || other.dim,
            fg: other.fg.or(self.fg),
        }
    }

    pub fn is_empty(&self) -> bool {
        !(self.bold
            || self.italic
            || self.underline
            || self.strike
            || self.dim
            || self.fg.is_some())
    }

    /// Wrap `text` in one combined SGR sequence, or return it untouched when
    /// colour is off or the style says nothing.
    pub fn apply(&self, text: &str, color: bool) -> String {
        if !color || self.is_empty() || text.is_empty() {
            return text.to_string();
        }
        let mut params: Vec<String> = Vec::new();
        if self.bold {
            params.push("1".to_string());
        }
        if self.dim {
            params.push("2".to_string());
        }
        if self.italic {
            params.push("3".to_string());
        }
        if self.underline {
            params.push("4".to_string());
        }
        if self.strike {
            params.push("9".to_string());
        }
        if let Some(fg) = self.fg {
            params.push(fg.to_string());
        }
        format!("\x1b[{}m{text}\x1b[0m", params.join(";"))
    }
}

const fn st(
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    fg: Option<u8>,
) -> Style {
    Style {
        bold,
        italic,
        underline,
        strike,
        dim,
        fg,
    }
}

pub const BOLD: Style = st(true, false, false, false, false, None);
pub const ITALIC: Style = st(false, false, true, false, false, None);
pub const STRIKE: Style = st(false, false, false, false, true, None);
pub const CODE: Style = st(false, false, false, false, false, Some(93));
pub const LINK: Style = st(false, false, false, true, false, Some(94));
pub const H1: Style = st(true, false, false, true, false, Some(94));
pub const H2: Style = st(true, false, false, false, false, Some(96));
pub const H3: Style = st(true, false, false, false, false, Some(95));
pub const H4: Style = st(true, false, false, false, false, None);
pub const QUOTE: Style = st(false, true, false, false, false, None);
pub const RULE: Style = st(false, false, false, false, false, Some(90));
pub const CODE_BLOCK: Style = st(false, true, false, false, false, None);
pub const TABLE_HEAD: Style = st(true, false, false, false, false, None);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_when_color_disabled() {
        assert_eq!(BOLD.apply("x", false), "x");
    }

    #[test]
    fn empty_style_emits_no_escapes() {
        assert_eq!(Style::default().apply("x", true), "x");
    }

    #[test]
    fn empty_text_emits_no_escapes() {
        assert_eq!(BOLD.apply("", true), "");
    }

    #[test]
    fn single_combined_sequence() {
        assert_eq!(BOLD.apply("x", true), "\x1b[1mx\x1b[0m");
        assert_eq!(H1.apply("x", true), "\x1b[1;4;94mx\x1b[0m");
    }

    #[test]
    fn sgr_parameter_order_is_fixed() {
        let s = Style {
            bold: true,
            dim: true,
            italic: true,
            underline: true,
            strike: true,
            fg: Some(31),
        };
        assert_eq!(s.apply("x", true), "\x1b[1;2;3;4;9;31mx\x1b[0m");
    }

    #[test]
    fn merge_ors_flags_and_takes_new_color() {
        let m = BOLD.merge(Style {
            fg: Some(94),
            ..Default::default()
        });
        assert!(m.bold);
        assert_eq!(m.fg, Some(94));
    }

    #[test]
    fn is_empty() {
        assert!(Style::default().is_empty());
        assert!(!BOLD.is_empty());
    }
}
