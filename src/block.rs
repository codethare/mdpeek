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
}
