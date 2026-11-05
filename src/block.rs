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
    let mut blocks: Vec<Block> = Vec::new();
    let mut para = String::new();
    for line in src.lines() {
        if line.trim().is_empty() {
            flush(&mut blocks, &mut para);
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
}
