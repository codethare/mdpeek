/// One top-level Markdown block. Nesting only happens through `Quote`.
#[derive(Debug, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        text: String,
    },
    Para(String),
    Code {
        lines: Vec<String>,
    },
    List(Vec<ListItem>),
    Quote(Vec<Block>),
    Rule,
    Table {
        header: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

/// A list item. `indent` is the marker's column, which is how nesting is
/// represented: the renderer indents by it instead of walking a tree.
#[derive(Debug, PartialEq)]
pub struct ListItem {
    pub marker: String,
    pub indent: usize,
    pub checked: Option<bool>,
    pub text: String,
}

pub fn parse_blocks(src: &str) -> Vec<Block> {
    let lines: Vec<&str> = src.lines().collect();
    let mut blocks: Vec<Block> = Vec::new();
    let mut para = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() {
            flush(&mut blocks, &mut para);
            continue;
        }
        if let Some((level, text)) = heading(line) {
            flush(&mut blocks, &mut para);
            blocks.push(Block::Heading { level, text });
            continue;
        }
        if is_rule(line) {
            flush(&mut blocks, &mut para);
            blocks.push(Block::Rule);
            continue;
        }
        if let Some(stripped) = strip_quote(line) {
            flush(&mut blocks, &mut para);
            let mut inner: Vec<String> = vec![stripped];
            while i < lines.len() {
                match strip_quote(lines[i]) {
                    Some(next) => {
                        inner.push(next);
                        i += 1;
                    }
                    None => break,
                }
            }
            blocks.push(Block::Quote(parse_blocks(&inner.join("\n"))));
            continue;
        }
        if let Some((fence_char, fence_len)) = fence(line) {
            flush(&mut blocks, &mut para);
            let mut code: Vec<String> = Vec::new();
            while i < lines.len() {
                let inner = lines[i];
                i += 1;
                if is_closing_fence(inner, fence_char, fence_len) {
                    break;
                }
                code.push(inner.to_string());
            }
            blocks.push(Block::Code { lines: code });
            continue;
        }
        if is_table_header(&lines, i, line) {
            flush(&mut blocks, &mut para);
            let header = split_row(line);
            i += 1;
            let mut rows: Vec<Vec<String>> = Vec::new();
            while i < lines.len() && !lines[i].trim().is_empty() && lines[i].contains('|') {
                rows.push(split_row(lines[i]));
                i += 1;
            }
            blocks.push(Block::Table { header, rows });
            continue;
        }
        if let Some(first) = item_start(line) {
            flush(&mut blocks, &mut para);
            let mut items = vec![first];
            loop {
                let indent = items.last().map_or(0, |item| item.indent);
                while i < lines.len() && is_continuation(&lines, i, indent) {
                    if let Some(item) = items.last_mut()
                        && !item.text.is_empty()
                    {
                        item.text.push(' ');
                        item.text.push_str(lines[i].trim());
                    }
                    i += 1;
                }
                // A blank line between items keeps the list going.
                let mut next = i;
                while next < lines.len() && lines[next].trim().is_empty() {
                    next += 1;
                }
                if next < lines.len()
                    && let Some(item) = item_start(lines[next])
                {
                    items.push(item);
                    i = next + 1;
                    continue;
                }
                break;
            }
            blocks.push(Block::List(items));
            continue;
        }
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(line.trim());
    }
    flush(&mut blocks, &mut para);
    blocks
}

/// `# Heading` — 1 to 6 hashes followed by a space. Closing hashes are dropped.
fn heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim_end();
    Some((level as u8, text.to_string()))
}

/// `---`, `***`, `___` and spaced variants: one repeated character, three or
/// more times, and nothing else on the line.
fn is_rule(line: &str) -> bool {
    let compact: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && matches!(compact[0], '-' | '*' | '_')
        && compact.iter().all(|c| *c == compact[0])
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// `> quoted`, up to three leading spaces, with the marker and one following
/// space removed. `None` when the line is not part of a quote.
fn strip_quote(line: &str) -> Option<String> {
    if indent_of(line) > 3 {
        return None;
    }
    let rest = line.trim_start().strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest).to_string())
}

/// An opening fence: three or more backticks or tildes. The info string is
/// discarded, since there is no syntax highlighting.
fn fence(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    let c = trimmed.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let run = trimmed.chars().take_while(|ch| *ch == c).count();
    if run < 3 {
        return None;
    }
    Some((c, run))
}

fn is_closing_fence(line: &str, c: char, open_len: usize) -> bool {
    let trimmed = line.trim();
    let run = trimmed.chars().take_while(|ch| *ch == c).count();
    run >= open_len && run == trimmed.chars().count()
}

/// `header` is a table row when the line after it is a `---`-style separator
/// row containing at least one pipe.
fn is_table_header(lines: &[&str], separator: usize, header: &str) -> bool {
    if !header.contains('|') {
        return false;
    }
    let Some(sep) = lines.get(separator) else {
        return false;
    };
    sep.contains('|')
        && sep.contains('-')
        && sep.chars().all(|c| matches!(c, ' ' | '|' | ':' | '-'))
}

fn split_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    trimmed
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// `- item`, `* item`, `+ item`, `1. item` or `1) item`, optionally with a
/// `[ ]`/`[x]` task box after the marker.
fn item_start(line: &str) -> Option<ListItem> {
    let indent = indent_of(line);
    let rest = &line[indent..];
    let (marker, after) = bullet(rest).or_else(|| ordered(rest))?;
    let mut text = after.trim_start();
    let mut checked = None;
    match text.get(..3).map(str::to_ascii_lowercase).as_deref() {
        Some("[ ]") => {
            checked = Some(false);
            text = text[3..].trim_start();
        }
        Some("[x]") => {
            checked = Some(true);
            text = text[3..].trim_start();
        }
        _ => {}
    }
    Some(ListItem {
        marker,
        indent,
        checked,
        text: text.to_string(),
    })
}

fn bullet(rest: &str) -> Option<(String, &str)> {
    let c = rest.chars().next()?;
    if !matches!(c, '-' | '*' | '+') {
        return None;
    }
    let after = &rest[c.len_utf8()..];
    let text = after.strip_prefix(' ')?;
    Some((c.to_string(), text))
}

fn ordered(rest: &str) -> Option<(String, &str)> {
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 9 {
        return None;
    }
    let after = &rest[digits.len()..];
    let sep = after.chars().next()?;
    if !matches!(sep, '.' | ')') {
        return None;
    }
    let text = after[1..].strip_prefix(' ')?;
    Some((format!("{digits}{sep}"), text))
}

/// A line belonging to the item above it: indented past the marker and not the
/// start of another block.
fn is_continuation(lines: &[&str], i: usize, item_indent: usize) -> bool {
    let line = lines[i];
    if line.trim().is_empty()
        || item_start(line).is_some()
        || heading(line).is_some()
        || is_rule(line)
    {
        return false;
    }
    indent_of(line) > item_indent
}

fn flush(blocks: &mut Vec<Block>, para: &mut String) {
    if para.is_empty() {
        return;
    }
    blocks.push(Block::Para(std::mem::take(para)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_line_separates_paragraphs() {
        assert_eq!(
            parse_blocks("a\n\nb"),
            vec![Block::Para("a".into()), Block::Para("b".into())]
        );
    }

    #[test]
    fn soft_wrapped_lines_join_with_a_space() {
        assert_eq!(parse_blocks("a\nb"), vec![Block::Para("a b".into())]);
    }

    #[test]
    fn empty_input_is_no_blocks() {
        assert!(parse_blocks("").is_empty());
        assert!(parse_blocks("  \n\n").is_empty());
    }

    #[test]
    fn atx_headings() {
        assert_eq!(
            parse_blocks("# A\n## B\n###### C"),
            vec![
                Block::Heading {
                    level: 1,
                    text: "A".into()
                },
                Block::Heading {
                    level: 2,
                    text: "B".into()
                },
                Block::Heading {
                    level: 6,
                    text: "C".into()
                },
            ]
        );
    }

    #[test]
    fn heading_closing_hashes_are_stripped() {
        assert_eq!(
            parse_blocks("## Sub ##"),
            vec![Block::Heading {
                level: 2,
                text: "Sub".into()
            }]
        );
    }

    #[test]
    fn thematic_breaks() {
        assert_eq!(
            parse_blocks("---\n\n***\n\n___"),
            vec![Block::Rule, Block::Rule, Block::Rule]
        );
    }

    #[test]
    fn seven_hashes_is_not_a_heading() {
        assert_eq!(
            parse_blocks("####### x"),
            vec![Block::Para("####### x".into())]
        );
    }

    #[test]
    fn unordered_items() {
        assert_eq!(
            parse_blocks("- one\n- two"),
            vec![Block::List(vec![
                ListItem {
                    marker: "-".into(),
                    indent: 0,
                    checked: None,
                    text: "one".into()
                },
                ListItem {
                    marker: "-".into(),
                    indent: 0,
                    checked: None,
                    text: "two".into()
                },
            ])]
        );
    }

    #[test]
    fn ordered_markers_are_kept_as_written() {
        assert_eq!(
            parse_blocks("1. one\n2. two"),
            vec![Block::List(vec![
                ListItem {
                    marker: "1.".into(),
                    indent: 0,
                    checked: None,
                    text: "one".into()
                },
                ListItem {
                    marker: "2.".into(),
                    indent: 0,
                    checked: None,
                    text: "two".into()
                },
            ])]
        );
    }

    #[test]
    fn nesting_is_recorded_as_indent() {
        assert_eq!(
            parse_blocks("- one\n  - two"),
            vec![Block::List(vec![
                ListItem {
                    marker: "-".into(),
                    indent: 0,
                    checked: None,
                    text: "one".into()
                },
                ListItem {
                    marker: "-".into(),
                    indent: 2,
                    checked: None,
                    text: "two".into()
                },
            ])]
        );
    }

    #[test]
    fn task_items_carry_their_state() {
        assert_eq!(
            parse_blocks("- [x] done\n- [ ] todo"),
            vec![Block::List(vec![
                ListItem {
                    marker: "-".into(),
                    indent: 0,
                    checked: Some(true),
                    text: "done".into()
                },
                ListItem {
                    marker: "-".into(),
                    indent: 0,
                    checked: Some(false),
                    text: "todo".into()
                },
            ])]
        );
    }

    #[test]
    fn continuation_lines_belong_to_the_item() {
        assert_eq!(
            parse_blocks("- one\n  more"),
            vec![Block::List(vec![ListItem {
                marker: "-".into(),
                indent: 0,
                checked: None,
                text: "one more".into()
            }])]
        );
    }

    #[test]
    fn fenced_code_is_verbatim() {
        assert_eq!(
            parse_blocks("```rust\nlet x = 1;\n\n  y();\n```"),
            vec![Block::Code {
                lines: vec!["let x = 1;".into(), "".into(), "  y();".into()]
            }]
        );
    }

    #[test]
    fn tilde_fences_and_eof_closing() {
        assert_eq!(
            parse_blocks("~~~\na\n~~~"),
            vec![Block::Code {
                lines: vec!["a".into()]
            }]
        );
        assert_eq!(
            parse_blocks("```\nunclosed\n"),
            vec![Block::Code {
                lines: vec!["unclosed".into()]
            }]
        );
    }

    #[test]
    fn markdown_inside_a_fence_is_not_parsed() {
        assert_eq!(
            parse_blocks("```\n# not a heading\n```"),
            vec![Block::Code {
                lines: vec!["# not a heading".into()]
            }]
        );
    }

    #[test]
    fn quote_lines_become_a_nested_block_list() {
        assert_eq!(
            parse_blocks("> one\n> two"),
            vec![Block::Quote(vec![Block::Para("one two".into())])]
        );
    }

    #[test]
    fn quote_can_hold_other_blocks() {
        assert_eq!(
            parse_blocks("> # T\n> - a"),
            vec![Block::Quote(vec![
                Block::Heading {
                    level: 1,
                    text: "T".into()
                },
                Block::List(vec![ListItem {
                    marker: "-".into(),
                    indent: 0,
                    checked: None,
                    text: "a".into()
                }]),
            ])]
        );
    }

    #[test]
    fn pipe_table_with_separator_row() {
        assert_eq!(
            parse_blocks("| a | b |\n|---|---|\n| 1 | 22 |"),
            vec![Block::Table {
                header: vec!["a".into(), "b".into()],
                rows: vec![vec!["1".into(), "22".into()]],
            }]
        );
    }

    #[test]
    fn pipes_without_a_separator_row_stay_a_paragraph() {
        assert_eq!(parse_blocks("a | b"), vec![Block::Para("a | b".into())]);
    }
}
