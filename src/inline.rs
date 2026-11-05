use crate::style::{self, Style};

/// One run of text with a single style. Consecutive spans never share a style.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

pub fn plain(text: &str) -> Span {
    Span {
        text: text.to_string(),
        style: Style::default(),
    }
}

/// Split `src` into styled spans. Markdown that is not recognised stays
/// literal text, so malformed input degrades instead of disappearing.
pub fn parse_inline(src: &str) -> Vec<Span> {
    let chars: Vec<char> = src.chars().collect();
    let mut out: Vec<Span> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = find_char(&chars, i + 1, '`') {
                flush(&mut out, &mut buf);
                let text: String = chars[i + 1..end].iter().collect();
                push(
                    &mut out,
                    Span {
                        text,
                        style: style::CODE,
                    },
                );
                i = end + 1;
                continue;
            }
            buf.push(c);
            i += 1;
            continue;
        }
        if c == '*' || c == '_' {
            if chars.get(i + 1) == Some(&c)
                && chars.get(i + 2) == Some(&c)
                && let Some(end) = find_triple(&chars, i + 3, c)
            {
                flush(&mut out, &mut buf);
                let inner: String = chars[i + 3..end].iter().collect();
                push_styled(&mut out, &inner, style::BOLD.merge(style::ITALIC));
                i = end + 3;
                continue;
            }
            if chars.get(i + 1) == Some(&c) {
                if let Some(end) = find_double(&chars, i + 2, c) {
                    flush(&mut out, &mut buf);
                    let inner: String = chars[i + 2..end].iter().collect();
                    push_styled(&mut out, &inner, style::BOLD);
                    i = end + 2;
                    continue;
                }
                buf.push(c);
                buf.push(c);
                i += 2;
                continue;
            }
            // An opener cannot start on whitespace: `2 * 3 * 4` is arithmetic,
            // not emphasis.
            let opens = chars.get(i + 1).is_some_and(|n| !n.is_whitespace());
            let intra_word = c == '_'
                && i > 0
                && chars[i - 1].is_alphanumeric()
                && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
            if opens
                && !intra_word
                && let Some(end) = find_char(&chars, i + 1, c)
            {
                flush(&mut out, &mut buf);
                let inner: String = chars[i + 1..end].iter().collect();
                push_styled(&mut out, &inner, style::ITALIC);
                i = end + 1;
                continue;
            }
            buf.push(c);
            i += 1;
            continue;
        }
        if c == '~' && chars.get(i + 1) == Some(&'~') {
            if let Some(end) = find_double(&chars, i + 2, '~') {
                flush(&mut out, &mut buf);
                let inner: String = chars[i + 2..end].iter().collect();
                push_styled(&mut out, &inner, style::STRIKE);
                i = end + 2;
                continue;
            }
            buf.push(c);
            i += 1;
            continue;
        }
        if c == '!' && chars.get(i + 1) == Some(&'[') {
            if let Some((label, url, next)) = parse_link(&chars, i + 1) {
                flush(&mut out, &mut buf);
                push(&mut out, plain(&format!("[image: {label}] {url}")));
                i = next;
                continue;
            }
            buf.push(c);
            i += 1;
            continue;
        }
        if c == '[' {
            if let Some((label, url, next)) = parse_link(&chars, i) {
                flush(&mut out, &mut buf);
                push_styled(&mut out, &label, style::LINK);
                if label != url {
                    push(&mut out, plain(&format!(" ({url})")));
                }
                i = next;
                continue;
            }
            buf.push(c);
            i += 1;
            continue;
        }
        buf.push(c);
        i += 1;
    }
    flush(&mut out, &mut buf);
    out
}

fn flush(out: &mut Vec<Span>, buf: &mut String) {
    if buf.is_empty() {
        return;
    }
    let text = std::mem::take(buf);
    push(
        out,
        Span {
            text,
            style: Style::default(),
        },
    );
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

/// Parse `inner` and stamp `style` over every span it yields, so markup inside
/// a label keeps working.
fn push_styled(out: &mut Vec<Span>, inner: &str, style: Style) {
    for span in parse_inline(inner) {
        push(
            out,
            Span {
                text: span.text,
                style: span.style.merge(style),
            },
        );
    }
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == target {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `***bold italic***` closer: three of `c` in a row.
fn find_triple(chars: &[char], from: usize, c: char) -> Option<usize> {
    let mut i = from;
    while i + 2 < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == c && chars[i + 1] == c && chars[i + 2] == c {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_double(chars: &[char], from: usize, c: char) -> Option<usize> {
    let mut i = from;
    while i + 1 < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == c && chars[i + 1] == c {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `[label](url)` starting at `open`. Returns label, url and the index after
/// the closing parenthesis.
fn parse_link(chars: &[char], open: usize) -> Option<(String, String, usize)> {
    if chars.get(open) != Some(&'[') {
        return None;
    }
    let close_bracket = find_char(chars, open + 1, ']')?;
    if chars.get(close_bracket + 1) != Some(&'(') {
        return None;
    }
    let close_paren = find_char(chars, close_bracket + 2, ')')?;
    let label: String = chars[open + 1..close_bracket].iter().collect();
    let url: String = chars[close_bracket + 2..close_paren].iter().collect();
    Some((label, url, close_paren + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{BOLD, CODE, ITALIC, LINK, STRIKE};

    fn s(src: &str) -> Vec<(String, Style)> {
        parse_inline(src)
            .into_iter()
            .map(|sp| (sp.text, sp.style))
            .collect()
    }
    fn t(text: &str) -> (String, Style) {
        (text.to_string(), Style::default())
    }

    #[test]
    fn plain_text_is_one_span() {
        assert_eq!(s("hello"), vec![t("hello")]);
    }

    #[test]
    fn bold_italic_strike() {
        assert_eq!(s("a **b** c"), vec![t("a "), ("b".into(), BOLD), t(" c")]);
        assert_eq!(s("*i*"), vec![("i".into(), ITALIC)]);
        assert_eq!(s("~~x~~"), vec![("x".into(), STRIKE)]);
    }

    #[test]
    fn code_span_wins_over_markup() {
        assert_eq!(s("`**x**`"), vec![("**x**".into(), CODE)]);
    }

    #[test]
    fn underscores_inside_words_are_literal() {
        assert_eq!(s("snake_case_name"), vec![t("snake_case_name")]);
    }

    #[test]
    fn escapes_are_honoured() {
        assert_eq!(s("\\*not italic\\*"), vec![t("*not italic*")]);
    }

    #[test]
    fn unmatched_delimiters_stay_literal() {
        assert_eq!(s("**unclosed"), vec![t("**unclosed")]);
    }

    #[test]
    fn link_renders_text_then_url() {
        assert_eq!(
            s("[git](https://git-scm.com)"),
            vec![("git".into(), LINK), t(" (https://git-scm.com)")]
        );
    }

    #[test]
    fn link_with_same_text_and_url_shows_once() {
        assert_eq!(
            s("[https://x.io](https://x.io)"),
            vec![("https://x.io".into(), LINK)]
        );
    }

    #[test]
    fn image_renders_label_and_url() {
        assert_eq!(s("![alt](u.png)"), vec![t("[image: alt] u.png")]);
    }

    #[test]
    fn empty_input_is_no_spans() {
        assert!(parse_inline("").is_empty());
    }

    #[test]
    fn triple_star_is_bold_italic() {
        assert_eq!(s("***x***"), vec![("x".into(), BOLD.merge(ITALIC))]);
    }

    #[test]
    fn spaced_asterisks_stay_literal() {
        assert_eq!(s("2 * 3 * 4"), vec![t("2 * 3 * 4")]);
    }
}
