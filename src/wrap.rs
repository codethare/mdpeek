use unicode_width::UnicodeWidthChar;

use crate::inline::{Span, plain};
use crate::style::Style;

/// Columns a string occupies on screen. Wide characters cost two columns;
/// escape sequences never reach this function.
pub fn display_width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
}

/// Greedy width-aware wrap. `first` prefixes line 1 and `rest` the others, so
/// a list item's continuation lines line up under its text, not its marker.
pub fn wrap_spans(spans: &[Span], width: usize, first: &str, rest: &str) -> Vec<Vec<Span>> {
    let width = width.max(1);
    let tokens = tokenize(spans);
    if tokens.is_empty() {
        return if first.is_empty() {
            Vec::new()
        } else {
            vec![vec![plain(first)]]
        };
    }
    let mut lines: Vec<Vec<Span>> = Vec::new();
    let mut line: Vec<Span> = Vec::new();
    let mut indent = new_line(&mut line, first);
    let mut col = indent;
    let mut pending_space = false;
    for (text, style, is_space) in tokens {
        if is_space {
            pending_space = true;
            continue;
        }
        let has_content = col > indent;
        if pending_space && has_content && col + 1 + display_width(&text) <= width {
            push(&mut line, spaced(" ", style));
            col += 1;
        } else if pending_space && has_content {
            lines.push(std::mem::take(&mut line));
            indent = new_line(&mut line, rest);
            col = indent;
        }
        pending_space = false;
        let mut chars: Vec<char> = text.chars().collect();
        while !chars.is_empty() {
            let (take, used) = fit(&chars, width.saturating_sub(col));
            let piece: String = chars.drain(..take).collect();
            push(&mut line, spaced(piece, style));
            col += used;
            if !chars.is_empty() {
                lines.push(std::mem::take(&mut line));
                indent = new_line(&mut line, rest);
                col = indent;
            }
        }
    }
    lines.push(line);
    lines
}

/// How many leading chars of `chars` fit in `room` columns, and the columns
/// they use. A single char wider than the room is taken anyway: overflowing by
/// one column beats losing text or looping forever.
fn fit(chars: &[char], room: usize) -> (usize, usize) {
    let mut used = 0;
    let mut take = 0;
    while take < chars.len() {
        let w = chars[take].width().unwrap_or(0);
        if used + w > room {
            break;
        }
        used += w;
        take += 1;
    }
    if take == 0 {
        (1, chars[0].width().unwrap_or(0))
    } else {
        (take, used)
    }
}

fn new_line(line: &mut Vec<Span>, prefix: &str) -> usize {
    if !prefix.is_empty() {
        push(line, plain(prefix));
    }
    display_width(prefix)
}

fn spaced(text: impl Into<String>, style: Style) -> Span {
    Span {
        text: text.into(),
        style,
    }
}

fn push(out: &mut Vec<Span>, span: Span) {
    if span.text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.style == span.style
    {
        last.text.push_str(&span.text);
        return;
    }
    out.push(span);
}

/// Split spans into word and whitespace tokens, keeping each token's style.
/// Whitespace runs are later collapsed to one space.
fn tokenize(spans: &[Span]) -> Vec<(String, Style, bool)> {
    let mut out: Vec<(String, Style, bool)> = Vec::new();
    for span in spans {
        let mut cur = String::new();
        let mut kind: Option<bool> = None;
        for ch in span.text.chars() {
            let is_space = ch.is_whitespace();
            match kind {
                Some(k) if k == is_space => cur.push(ch),
                Some(k) => {
                    out.push((std::mem::take(&mut cur), span.style, k));
                    cur.push(ch);
                    kind = Some(is_space);
                }
                None => {
                    cur.push(ch);
                    kind = Some(is_space);
                }
            }
        }
        if !cur.is_empty() {
            out.push((cur, span.style, kind.unwrap_or(false)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline::parse_inline;

    fn joined(src: &str, width: usize, first: &str, rest: &str) -> String {
        let lines = wrap_spans(&parse_inline(src), width, first, rest);
        let mut out = String::new();
        for line in lines {
            for sp in line {
                out.push_str(&sp.text);
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn breaks_on_spaces() {
        assert_eq!(
            joined("alpha beta gamma delta", 12, "", ""),
            "alpha beta\ngamma delta\n"
        );
    }

    #[test]
    fn counts_wide_chars_as_two_columns() {
        assert_eq!(display_width("中文"), 4);
        assert_eq!(joined("中文中文中文", 6, "", ""), "中文中\n文中文\n");
    }

    #[test]
    fn hard_breaks_a_word_longer_than_the_width() {
        assert_eq!(
            joined("https://example.com/very/long/path", 10, "", ""),
            "https://ex\nample.com/\nvery/long/\npath\n"
        );
    }

    #[test]
    fn hanging_indent_applies_to_later_lines() {
        assert_eq!(
            joined("one two three four", 12, "- ", "  "),
            "- one two\n  three four\n"
        );
    }

    #[test]
    fn no_underflow_at_width_one() {
        assert_eq!(joined("ab cd", 1, "", ""), "a\nb\nc\nd\n");
    }

    #[test]
    fn single_space_keeps_words_glued() {
        let lines = wrap_spans(&parse_inline("a b"), 80, "", "");
        assert_eq!(lines.len(), 1);
    }
}
