use crate::block::Block;
use crate::inline::{Span, parse_inline};
use crate::style::{self, Style};
use crate::wrap::{display_width, wrap_spans};

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
            let base = match level {
                1 => style::H1,
                2 => style::H2,
                3 => style::H3,
                _ => style::H4,
            };
            render_text(text, base, ctx, "", "")
        }
        Block::Code { lines } => {
            let mut out = String::new();
            for line in lines {
                let text = format!("  {line}");
                out.push_str(&style::CODE_BLOCK.apply(&text, ctx.color));
                out.push('\n');
            }
            out
        }
        Block::List(items) => {
            let mut out = String::new();
            for item in items {
                let mut marker = " ".repeat(item.indent);
                marker.push_str(&item.marker);
                marker.push(' ');
                match item.checked {
                    Some(true) => marker.push_str("[x] "),
                    Some(false) => marker.push_str("[ ] "),
                    None => {}
                }
                let pad = " ".repeat(display_width(&marker));
                out.push_str(&render_text(
                    &item.text,
                    Style::default(),
                    ctx,
                    &marker,
                    &pad,
                ));
            }
            out
        }
        Block::Quote(inner) => {
            let inner_ctx = Ctx {
                width: ctx.width.saturating_sub(2).max(1),
                color: ctx.color,
            };
            let rendered = render_blocks(inner, &inner_ctx);
            let mut out = String::new();
            for line in rendered.lines() {
                let text = if line.is_empty() {
                    "│".to_string()
                } else {
                    format!("│ {line}")
                };
                out.push_str(&style::QUOTE.apply(&text, ctx.color));
                out.push('\n');
            }
            out
        }
        Block::Rule => {
            let rule = "─".repeat(ctx.width);
            let mut out = style::RULE.apply(&rule, ctx.color);
            out.push('\n');
            out
        }
        Block::Table { header, rows } => render_table(header, rows, ctx),
    }
}

fn render_table(header: &[String], rows: &[Vec<String>], ctx: &Ctx) -> String {
    let cols = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    if cols == 0 {
        return String::new();
    }
    let mut widths = vec![0usize; cols];
    for (idx, cell) in header.iter().enumerate() {
        widths[idx] = widths[idx].max(cell_width(cell));
    }
    for row in rows {
        for (idx, cell) in row.iter().enumerate().take(cols) {
            widths[idx] = widths[idx].max(cell_width(cell));
        }
    }
    let gaps = 3 * (cols - 1);
    loop {
        let total: usize = widths.iter().sum::<usize>() + gaps;
        if total <= ctx.width {
            break;
        }
        let Some(widest) = (0..cols).max_by_key(|idx| widths[*idx]) else {
            break;
        };
        if widths[widest] <= 3 {
            break;
        }
        widths[widest] -= 1;
    }
    let fits = widths.iter().sum::<usize>() + gaps <= ctx.width;
    let mut out = String::new();
    if !fits {
        // Too narrow to lay out as columns: wrap the rows as plain text, which
        // keeps the width invariant true for any input.
        for (n, row) in std::iter::once(header)
            .chain(rows.iter().map(Vec::as_slice))
            .enumerate()
        {
            if n == 1 {
                out.push('\n');
            }
            out.push_str(&render_text(
                &row.join(" │ "),
                Style::default(),
                ctx,
                "",
                "",
            ));
        }
        return out;
    }
    out.push_str(&render_row(header, &widths, style::TABLE_HEAD, ctx));
    out.push('\n');
    // Each segment takes its column width plus the space on either side of the
    // `│` it meets, so the `┼` sits exactly under that `│` and the rule is as
    // wide as the row it underlines.
    let separator: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(i, w)| "─".repeat(w + usize::from(i > 0) + usize::from(i + 1 < cols)))
        .collect();
    out.push_str(&separator.join("┼"));
    out.push('\n');
    for row in rows {
        out.push_str(&render_row(row, &widths, Style::default(), ctx));
        out.push('\n');
    }
    out
}

fn cell_width(cell: &str) -> usize {
    parse_inline(cell)
        .iter()
        .map(|span| display_width(&span.text))
        .sum()
}

fn render_row(cells: &[String], widths: &[usize], base: Style, ctx: &Ctx) -> String {
    let last = widths.len().saturating_sub(1);
    let mut parts: Vec<String> = Vec::new();
    for (idx, width) in widths.iter().enumerate() {
        let cell = cells.get(idx).map_or("", String::as_str);
        let (runs, truncated, used) = clamp_spans(&parse_inline(cell), *width);
        let mut text = String::new();
        for run in runs {
            text.push_str(&base.merge(run.style).apply(&run.text, ctx.color));
        }
        if truncated {
            text.push('…');
        }
        if idx != last {
            text.push_str(&" ".repeat(width.saturating_sub(used)));
        }
        parts.push(text);
    }
    parts.join(" │ ")
}

/// Cut spans down to `width` columns, reserving one column for the ellipsis
/// when the text has to be shortened.
fn clamp_spans(spans: &[Span], width: usize) -> (Vec<Span>, bool, usize) {
    let full: usize = spans.iter().map(|span| display_width(&span.text)).sum();
    let truncated = full > width;
    let limit = if truncated {
        width.saturating_sub(1)
    } else {
        width
    };
    let mut out: Vec<Span> = Vec::new();
    let mut used = 0;
    let mut cut = false;
    for span in spans {
        if cut {
            break;
        }
        let mut text = String::new();
        for ch in span.text.chars() {
            let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + w > limit {
                cut = true;
                break;
            }
            used += w;
            text.push(ch);
        }
        out.push(Span {
            text,
            style: span.style,
        });
    }
    if truncated {
        used += 1;
    }
    (out, truncated, used)
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

    #[test]
    fn headings_have_no_marker_text() {
        assert_eq!(r("# Title", 40), "Title\n");
        assert_eq!(r("### Deep", 40), "Deep\n");
    }

    #[test]
    fn h1_is_styled_when_colour_is_on() {
        let out = render_blocks(
            &parse_blocks("# T"),
            &Ctx {
                width: 40,
                color: true,
            },
        );
        assert_eq!(out, "\x1b[1;4;94mT\x1b[0m\n");
    }

    #[test]
    fn rule_spans_the_width() {
        assert_eq!(r("---", 10), "──────────\n");
    }

    #[test]
    fn heading_text_still_wraps() {
        assert_eq!(r("# alpha beta", 6), "alpha\nbeta\n");
    }

    #[test]
    fn items_keep_their_markers() {
        assert_eq!(r("- one\n- two", 20), "- one\n- two\n");
        assert_eq!(r("1. one\n2. two", 20), "1. one\n2. two\n");
    }

    #[test]
    fn nested_items_indent() {
        assert_eq!(r("- one\n  - two", 20), "- one\n  - two\n");
    }

    #[test]
    fn task_items_render_checkboxes() {
        assert_eq!(r("- [x] done\n- [ ] todo", 20), "- [x] done\n- [ ] todo\n");
    }

    #[test]
    fn item_text_wraps_under_the_marker() {
        assert_eq!(
            r("- alpha beta gamma delta", 12),
            "- alpha beta\n  gamma\n  delta\n"
        );
    }

    #[test]
    fn code_lines_are_indented_and_unwrapped() {
        assert_eq!(
            r("```\naaaa bbbb cccc dddd\n```", 8),
            "  aaaa bbbb cccc dddd\n"
        );
    }

    #[test]
    fn code_block_is_dim_when_colour_is_on() {
        let out = render_blocks(
            &parse_blocks("```\nx\n```"),
            &Ctx {
                width: 40,
                color: true,
            },
        );
        assert_eq!(out, "\x1b[2m  x\x1b[0m\n");
    }

    #[test]
    fn empty_code_block_renders_nothing() {
        assert_eq!(r("```\n```", 40), "");
    }

    #[test]
    fn quote_gets_a_gutter() {
        assert_eq!(r("> one\n> two", 20), "\u{2502} one two\n");
    }

    #[test]
    fn quote_wraps_inside_the_gutter() {
        assert_eq!(
            r("> alpha beta gamma", 10),
            "\u{2502} alpha\n\u{2502} beta\n\u{2502} gamma\n"
        );
    }

    #[test]
    fn nested_quote_doubles_the_gutter() {
        assert_eq!(r("> > deep", 20), "\u{2502} \u{2502} deep\n");
    }

    #[test]
    fn table_pads_columns_and_underlines_the_header() {
        assert_eq!(
            r("| a | b |\n|---|---|\n| 1 | 22 |", 40),
            "a │ b\n──┼───\n1 │ 22\n"
        );
    }

    #[test]
    fn table_width_counts_wide_chars() {
        assert_eq!(
            r("| 中 | b |\n|---|---|\n| x | y |", 40),
            "中 │ b\n───┼──\nx  │ y\n"
        );
    }

    #[test]
    fn table_is_clamped_to_the_render_width() {
        let out = r("| aaaa | bbbb |\n|---|---|\n| cccc | dddd |", 8);
        assert!(
            out.lines().all(|l| crate::wrap::display_width(l) <= 8),
            "got {out:?}"
        );
    }

    #[test]
    fn table_rule_aligns_with_the_columns() {
        let out = r("| a | b |\n|---|---|\n| 1 | 22 |", 40);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "a │ b");
        assert_eq!(lines[1], "──┼───");
    }

    #[test]
    fn table_rule_never_exceeds_the_width_after_shrinking() {
        let out = r(
            "| aaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbb |\n|---|---|\n| c | d |",
            40,
        );
        let widths: Vec<usize> = out.lines().map(crate::wrap::display_width).collect();
        assert!(widths.iter().all(|w| *w <= 40), "got {widths:?}");
        assert_eq!(widths[0], widths[1], "rule must match the header row");
    }

    #[test]
    fn truncated_cell_keeps_its_text_and_adds_an_ellipsis() {
        let out = r("| aaaaaaaaaaaaaaaaaaaaaa | b |\n|---|---|\n| c | d |", 20);
        let header = out.lines().next().unwrap();
        assert!(header.starts_with("aaa"), "cell text kept: {header:?}");
        assert!(header.contains('…'), "ellipsis marks the cut: {header:?}");
    }
}
