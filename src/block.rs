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
}
