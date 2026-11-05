use crate::block::Block;
use crate::inline::parse_inline;
use crate::style::{self, Style};
use crate::wrap::wrap_spans;

pub struct Ctx {
    pub width: usize,
    pub color: bool,
}

/// Render blocks, one blank line between top-level blocks and none at the end.
pub fn render_blocks(blocks: &[Block], ctx: &Ctx) -> String {
    let mut out = String::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&render_block(block, ctx));
    }
    out
}

fn render_block(block: &Block, ctx: &Ctx) -> String {
    match block {
        Block::Para(text) => render_text(text, Style::default(), ctx, "", ""),
        Block::Heading { level, text } => {
            let base = if *level >= 4 {
                style::H4
            } else {
                Style::default()
            };
            render_text(text, base, ctx, "", "")
        }
        Block::Code { lines } => {
            let mut out = String::new();
            for line in lines {
                out.push_str(line);
                out.push('\n');
            }
            out
        }
        Block::List(items) => {
            let mut out = String::new();
            for item in items {
                out.push_str(&item.marker);
                out.push(' ');
                out.push_str(&item.text);
                out.push('\n');
            }
            out
        }
        Block::Quote(inner) => render_blocks(inner, ctx),
        Block::Rule => String::new(),
        Block::Table { header, rows } => {
            let mut out = String::new();
            let all = std::iter::once(header).chain(rows.iter());
            for row in all {
                out.push_str(&row.join(" | "));
                out.push('\n');
            }
            out
        }
    }
}

/// Wrap `text` and paint it. `base` styles the whole block; a span's own style
/// wins over it, so inline code stays code-coloured inside a heading.
fn render_text(text: &str, base: Style, ctx: &Ctx, first: &str, rest: &str) -> String {
    let mut out = String::new();
    for line in wrap_spans(&parse_inline(text), ctx.width, first, rest) {
        for span in line {
            let style = base.merge(span.style);
            out.push_str(&style.apply(&span.text, ctx.color));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::parse_blocks;

    fn r(src: &str, width: usize) -> String {
        render_blocks(
            &parse_blocks(src),
            &Ctx {
                width,
                color: false,
            },
        )
    }

    #[test]
    fn paragraph_wraps_at_width() {
        assert_eq!(r("alpha beta gamma delta", 12), "alpha beta\ngamma delta\n");
    }

    #[test]
    fn blocks_separated_by_one_blank_line() {
        assert_eq!(r("a\n\nb", 20), "a\n\nb\n");
    }

    #[test]
    fn empty_document_renders_nothing() {
        assert_eq!(r("", 80), "");
    }
}
