# md (terminal Markdown renderer) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `md`, a small fast Rust binary that renders a Markdown file as styled text in a terminal.

**Architecture:** Three-stage pipeline over a hand-written subset parser: lines → `Block` tree (`block.rs`), block text → styled `Span` list (`inline.rs`), spans → width-wrapped styled lines (`wrap.rs`) painted with 16-colour SGR by `render.rs`. `main.rs` only resolves CLI, input, width and colour, then drives the pipeline.

**Tech Stack:** Rust 1.99 (edition 2024), crates `unicode-width 0.2` and `libc 0.2` only.

**Spec:** `docs/superpowers/specs/2026-11-05-rust-md-terminal-renderer-design.md`

## Global Constraints

- Dependencies exactly `unicode-width = "0.2"` and `libc = "0.2"`. Adding any other crate is a spec violation.
- Release profile verbatim: `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.
- Colour is SGR 30–37/90–97 only; each span uses one combined sequence `\x1b[<params>m…\x1b[0m`.
- Zero escape bytes in output when stdout is not a tty, or `NO_COLOR` is set, or `--no-color` is passed.
- Render width: `--width N`, else `TIOCGWINSZ` on fd 1, else 80.
- Exit codes: 0 success, 1 read error, 2 usage error.
- Version control is **jj**, not git: end each task with `jj describe -m "…"` then `jj new`. Never `git add`/`git commit`. `.gitignore` already contains `/target`.
- Gate before each commit: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`.
- Between Tasks 2 and 5 the new modules are not yet called from `main`, so the crate root carries `#![allow(dead_code)]`. Task 5 removes it; from Task 5 on, clippy's dead-code lint is live.

## Review Focus

1. Output redirected to a file or a pipe must contain no escape byte — a user doing `md x.md > out.txt` gets clean text.
2. CJK and mixed-width text must never exceed the requested width; wide chars cost two columns.
3. Malformed input (unclosed `**bold`, unclosed ``` fence at EOF, `|` with no separator row) must not panic or swallow the rest of the document.
4. Empty file, whitespace-only file, and a bare `md` with stdin from `/dev/null` must exit 0 with no output.
5. `--width 1`, `--width 0`, and a width smaller than an indent must not underflow or panic; the program clamps and keeps rendering.

---

### Task 1: Crate skeleton, CLI, and input plumbing

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `tests/cli.rs`

**Interfaces:**
- Produces: `struct Cli { files: Vec<String>, width: Option<usize>, color: bool }`, `fn parse_args(args: &[String]) -> Result<Cli, String>`, `fn term_width() -> Option<usize>`, `fn usage() -> &'static str`.
- Produces binary name `md`.

- [ ] **Step 1: Write the failing tests**

`tests/cli.rs`:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_md"))
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("md [file...]"));
}

#[test]
fn missing_file_exits_one() {
    let out = bin().arg("/nonexistent/x.md").output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!out.stderr.is_empty());
}

#[test]
fn paged_stdin_has_no_escape_bytes() {
    let mut child = bin()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"# Title\n\nhello\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stdout.contains(&0x1b), "piped output must be plain");
}

#[test]
fn empty_stdin_exits_zero() {
    let out = bin()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn unknown_flag_exits_two() {
    let out = bin().arg("--nope").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli`
Expected: build error, `no bin target named md`.

- [ ] **Step 3: Create `Cargo.toml`**

```toml
[package]
name = "md"
version = "0.1.0"
edition = "2024"

[dependencies]
unicode-width = "0.2"
libc = "0.2"

[profile.release]
opt-level = "z"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

- [ ] **Step 4: Write `src/main.rs`**

Parse args by hand (no clap): `-h`/`--help` → print `usage()`, exit 0; `--no-color` → `cli.color = false`; `--width N` and `--width=N` → parse `usize`, unparseable → `Err`; `-` → stdin; anything else → file. Unknown `-…` → `Err`. `Err` prints the message plus `usage()` to stderr, exit 2.

```rust
#[repr(C)]
struct Winsize { ws_row: u16, ws_col: u16, ws_xpixel: u16, ws_ypixel: u16 }

/// Terminal width of fd 1, or None when it is not a tty.
pub fn term_width() -> Option<usize> {
    let mut ws = Winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    let r = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) };
    (r == 0 && ws.ws_col > 0).then_some(ws.ws_col as usize)
}

pub fn usage() -> &'static str {
    "usage: md [file...]\n\n  --width N    render width (default: terminal width, else 80)\n  --no-color   disable escapes (also honours NO_COLOR)\n"
}
```

Colour resolution: `color = cli.color && stdout.is_terminal() && env::var_os("NO_COLOR").is_none()`, using `std::io::IsTerminal`. Width: `cli.width.unwrap_or_else(|| term_width().unwrap_or(80)).max(1)`.

Input: no file args → read stdin. Read all into `String`; on error print to stderr and exit 1. For this task, write the input through unchanged (`print!("{input}")`); the pipeline arrives in Task 5.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test cli`
Expected: 5 passed.

- [ ] **Step 6: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: md binary skeleton with CLI, width and colour resolution"
jj new
```

---

### Task 2: Styles and palette

**Files:**
- Create: `src/style.rs` (add `mod style;` to `src/main.rs`)

**Interfaces:**
- Produces: `struct Style { bold: bool, italic: bool, underline: bool, strike: bool, dim: bool, fg: Option<u8> }` deriving `Clone, Copy, Debug, Default, PartialEq, Eq`.
- Produces: `fn merge(self, other: Style) -> Style` (OR the flags, `other.fg.or(self.fg)`), `fn is_empty(&self) -> bool`, `fn apply(&self, text: &str, color: bool) -> String`.
- Produces constants: `BOLD, ITALIC, STRIKE, CODE, LINK, H1, H2, H3, H4, QUOTE, RULE, CODE_BLOCK, TABLE_HEAD`.
- Palette values: `CODE` fg 93; `LINK` underline + fg 94; `H1` bold + underline + fg 94; `H2` bold + fg 96; `H3` bold + fg 95; `H4` bold; `QUOTE` dim; `RULE` fg 90; `CODE_BLOCK` dim; `TABLE_HEAD` bold.

- [ ] **Step 1: Write the failing tests** (in `src/style.rs`)

```rust
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
        let s = Style { bold: true, dim: true, italic: true, underline: true, strike: true, fg: Some(31) };
        assert_eq!(s.apply("x", true), "\x1b[1;2;3;4;9;31mx\x1b[0m");
    }

    #[test]
    fn merge_ors_flags_and_takes_new_color() {
        let m = BOLD.merge(Style { fg: Some(94), ..Default::default() });
        assert!(m.bold);
        assert_eq!(m.fg, Some(94));
    }

    #[test]
    fn is_empty() {
        assert!(Style::default().is_empty());
        assert!(!BOLD.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test style`
Expected: FAIL, `cannot find` items.

- [ ] **Step 3: Implement `src/style.rs`**

Declare `mod style;` in `src/main.rs` and add `#![allow(dead_code)]` as the first line of `src/main.rs` (Task 5 removes it).

`apply` returns `text.to_string()` when `!color || self.is_empty() || text.is_empty()`; otherwise builds params in the fixed order bold(1), dim(2), italic(3), underline(4), strike(9), then `fg` appended last, joined with `;`, wrapped as `\x1b[{params}m{text}\x1b[0m`. Constants are `pub const` struct literals.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test style`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat(style): 16-colour SGR styles and palette"
jj new
```

---

### Task 3: Inline parsing

**Files:**
- Create: `src/inline.rs` (add `mod inline;` to `src/main.rs`)

**Interfaces:**
- Consumes: `Style`, `style::{BOLD, ITALIC, STRIKE, CODE, LINK}`.
- Produces: `pub struct Span { pub text: String, pub style: Style }` deriving `Clone, Debug, PartialEq`, and `pub fn parse_inline(src: &str) -> Vec<Span>`.
- Produces: `pub fn plain(text: &str) -> Span`.

- [ ] **Step 1: Write the failing tests** (in `src/inline.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{BOLD, CODE, ITALIC, LINK, STRIKE};

    fn s(src: &str) -> Vec<(String, Style)> {
        parse_inline(src).into_iter().map(|sp| (sp.text, sp.style)).collect()
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
        assert_eq!(s("[https://x.io](https://x.io)"), vec![("https://x.io".into(), LINK)]);
    }

    #[test]
    fn image_renders_label_and_url() {
        assert_eq!(s("![alt](u.png)"), vec![t("[image: alt] u.png")]);
    }

    #[test]
    fn empty_input_is_no_spans() {
        assert!(parse_inline("").is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test inline`
Expected: FAIL, `cannot find function parse_inline`.

- [ ] **Step 3: Implement `src/inline.rs`**

Single left-to-right scan. At each position try, in order: `\` escape (emit next char literally), `` ` `` run (to next matching backtick; unmatched → literal), `**`/`__` (to matching closer → `BOLD`), `*`/`_` (to matching closer → `ITALIC`), `~~` (→ `STRIKE`), `![` (label + `(url)` → plain `[image: {label}] {url}`), `[` (label + `(url)` → `LINK` span for the label, then plain ` ({url})` unless `url == label`). Delimiter runs that find no closer are emitted literally. `_` is literal when the preceding and following chars are both alphanumeric. Merge adjacent spans with equal styles before returning.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test inline`
Expected: 10 passed.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat(inline): span parser for emphasis, code, links, images"
jj new
```

---

### Task 4: Wrapping

**Files:**
- Create: `src/wrap.rs` (add `mod wrap;` to `src/main.rs`)

**Interfaces:**
- Consumes: `inline::Span`.
- Produces: `pub fn display_width(s: &str) -> usize` (sum of `UnicodeWidthChar::width`, `None` counted as 0).
- Produces: `pub fn wrap_spans(spans: &[Span], width: usize, first: &str, rest: &str) -> Vec<Vec<Span>>` — one inner `Vec<Span>` per output line, already prefixed with `first` on line 1 and `rest` on later lines as plain spans; `width` is clamped to at least 1 internally.

- [ ] **Step 1: Write the failing tests** (in `src/wrap.rs`)

```rust
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
        assert_eq!(joined("alpha beta gamma delta", 12, "", ""), "alpha beta\ngamma delta\n");
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
        assert_eq!(joined("one two three four", 12, "- ", "  "), "- one two\n  three four\n");
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test wrap`
Expected: FAIL, `cannot find function wrap_spans`.

- [ ] **Step 3: Implement `src/wrap.rs`**

Tokenise the span list into `(text, style)` pieces where whitespace runs are separate tokens (styles carry across the split). Greedy fill: track the current column starting at `first`'s display width; a whitespace token at a line break is dropped; a word token wider than the available columns is split by chars at column boundaries. Emit the prefix span, then the tokens.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test wrap`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat(wrap): width-aware wrapping with hanging indent"
jj new
```

---

### Task 5: Blocks (paragraphs) and the render pipeline

**Files:**
- Create: `src/block.rs`, `src/render.rs`
- Modify: `src/main.rs` (use the pipeline instead of echoing input)

**Interfaces:**
- Produces from `block.rs`: `pub enum Block { Heading { level: u8, text: String }, Para(String), Code { lines: Vec<String> }, List(Vec<ListItem>), Quote(Vec<Block>), Rule, Table { header: Vec<String>, rows: Vec<Vec<String>> } }` deriving `Debug, PartialEq`; `pub struct ListItem { pub marker: String, pub indent: usize, pub checked: Option<bool>, pub text: String }`; `pub fn parse_blocks(src: &str) -> Vec<Block>`.
- Produces from `render.rs`: `pub struct Ctx { pub width: usize, pub color: bool }` and `pub fn render_blocks(blocks: &[Block], ctx: &Ctx) -> String` (every emitted line ends with `\n`; one blank line separates top-level blocks, never trailing).
- This task implements `Block::Para` and `Block::Heading { level: 4..=6 }` handling only; `render_blocks` must not panic on the still-unhandled variants — render them as their text content, and later tasks replace those arms.

- [ ] **Step 1: Write the failing tests** (in `src/block.rs` and `src/render.rs`)

`src/block.rs`:

```rust
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
```

`src/render.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::parse_blocks;

    fn r(src: &str, width: usize) -> String {
        render_blocks(&parse_blocks(src), &Ctx { width, color: false })
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL, `cannot find function parse_blocks` / `render_blocks`.

- [ ] **Step 3: Implement `src/block.rs` and `src/render.rs`**

`parse_blocks`: index loop over `src.lines()`. Blank line ends a paragraph. Paragraph lines accumulate and join with a single space. Recognise nothing else in this task.

`render_blocks`: for `Para`, `wrap_spans(&parse_inline(text), ctx.width, "", "")` then paint each span with `style.apply(&span.text, ctx.color)`. For `Heading` with `level >= 4`, same but with `style::H4`. Join block outputs with a single blank line.

`main.rs`: replace the echo with `print!("{}", render::render_blocks(&block::parse_blocks(&input), &ctx))`, where `ctx = Ctx { width, color }`. Remove the `#![allow(dead_code)]` line added in Task 2.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all pass, including the Task 1 CLI tests.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: block parser and paragraph/heading renderer"
jj new
```

---

### Task 6: Headings and horizontal rules

**Files:**
- Modify: `src/block.rs`, `src/render.rs`

**Interfaces:**
- Consumes: `Block::Heading`, `Block::Rule`, `style::{H1, H2, H3, H4, RULE}`.

- [ ] **Step 1: Write the failing tests**

`src/block.rs`:

```rust
#[test]
fn atx_headings() {
    assert_eq!(
        parse_blocks("# A\n## B\n###### C"),
        vec![
            Block::Heading { level: 1, text: "A".into() },
            Block::Heading { level: 2, text: "B".into() },
            Block::Heading { level: 6, text: "C".into() },
        ]
    );
}

#[test]
fn heading_closing_hashes_are_stripped() {
    assert_eq!(
        parse_blocks("## Sub ##"),
        vec![Block::Heading { level: 2, text: "Sub".into() }]
    );
}

#[test]
fn thematic_breaks() {
    assert_eq!(parse_blocks("---\n\n***\n\n___"), vec![Block::Rule, Block::Rule, Block::Rule]);
}

#[test]
fn seven_hashes_is_not_a_heading() {
    assert_eq!(parse_blocks("####### x"), vec![Block::Para("####### x".into())]);
}
```

`src/render.rs`:

```rust
#[test]
fn headings_have_no_marker_text() {
    assert_eq!(r("# Title", 40), "Title\n");
    assert_eq!(r("### Deep", 40), "Deep\n");
}

#[test]
fn h1_is_styled_when_colour_is_on() {
    let out = render_blocks(&parse_blocks("# T"), &Ctx { width: 40, color: true });
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

`block.rs`: match `^ {0,3}(#{1,6})\s+(.*)$`; strip trailing `\s#+$` from the text; a line of 3+ `-`, `*`, or `_` (spaces allowed between) is `Block::Rule`, but only when the line contains no other character.

`render.rs`: `Heading` picks `style` by level (1→H1, 2→H2, 3→H3, 4..=6→H4), wraps the inline spans at `ctx.width` and applies that style on top of each span's own style via `merge`. `Rule` emits `"─".repeat(ctx.width)` with `RULE`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: headings and horizontal rules"
jj new
```

---

### Task 7: Lists, nesting, and task items

**Files:**
- Modify: `src/block.rs`, `src/render.rs`

**Interfaces:**
- Consumes: `Block::List`, `ListItem`.

- [ ] **Step 1: Write the failing tests**

`src/block.rs`:

```rust
#[test]
fn unordered_items() {
    assert_eq!(
        parse_blocks("- one\n- two"),
        vec![Block::List(vec![
            ListItem { marker: "-".into(), indent: 0, checked: None, text: "one".into() },
            ListItem { marker: "-".into(), indent: 0, checked: None, text: "two".into() },
        ])]
    );
}

#[test]
fn ordered_markers_are_kept_as_written() {
    assert_eq!(
        parse_blocks("1. one\n2. two"),
        vec![Block::List(vec![
            ListItem { marker: "1.".into(), indent: 0, checked: None, text: "one".into() },
            ListItem { marker: "2.".into(), indent: 0, checked: None, text: "two".into() },
        ])]
    );
}

#[test]
fn nesting_is_recorded_as_indent() {
    assert_eq!(
        parse_blocks("- one\n  - two"),
        vec![Block::List(vec![
            ListItem { marker: "-".into(), indent: 0, checked: None, text: "one".into() },
            ListItem { marker: "-".into(), indent: 2, checked: None, text: "two".into() },
        ])]
    );
}

#[test]
fn task_items_carry_their_state() {
    assert_eq!(
        parse_blocks("- [x] done\n- [ ] todo"),
        vec![Block::List(vec![
            ListItem { marker: "-".into(), indent: 0, checked: Some(true), text: "done".into() },
            ListItem { marker: "-".into(), indent: 0, checked: Some(false), text: "todo".into() },
        ])]
    );
}

#[test]
fn continuation_lines_belong_to_the_item() {
    assert_eq!(
        parse_blocks("- one\n  more"),
        vec![Block::List(vec![
            ListItem { marker: "-".into(), indent: 0, checked: None, text: "one more".into() },
        ])]
    );
}
```

`src/render.rs`:

```rust
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
    assert_eq!(r("- alpha beta gamma delta", 12), "- alpha beta\n  gamma\n  delta\n");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

`block.rs`: item start matches `^(\s*)([-*+]|\d{1,9}[.)])\s+(.*)$`; indent is the leading whitespace's display width; after the marker, a leading `[ ]`/`[x]`/`[X]` sets `checked`. Consecutive item lines and indented continuation lines (indent > marker indent, no blank line between) join with a space. A blank line between items keeps the same `List` block. Nested items become items with a larger `indent`; the flat `Vec<ListItem>` is the nesting model.

`render.rs`: for each item, marker = `" ".repeat(indent) + &marker + " "`, plus `"[x] "`/`"[ ] "` when `checked.is_some()`. Wrap the item text with `first = marker`, `rest = " ".repeat(marker display width)`. Items are consecutive lines; no blank line between them.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: lists, indentation-based nesting, task items"
jj new
```

---

### Task 8: Fenced code blocks

**Files:**
- Modify: `src/block.rs`, `src/render.rs`

- [ ] **Step 1: Write the failing tests**

`src/block.rs`:

```rust
#[test]
fn fenced_code_is_verbatim() {
    assert_eq!(
        parse_blocks("```rust\nlet x = 1;\n\n  y();\n```"),
        vec![Block::Code { lines: vec!["let x = 1;".into(), "".into(), "  y();".into()] }]
    );
}

#[test]
fn tilde_fences_and_eof_closing() {
    assert_eq!(
        parse_blocks("~~~\na\n~~~"),
        vec![Block::Code { lines: vec!["a".into()] }]
    );
    assert_eq!(
        parse_blocks("```\nunclosed\n"),
        vec![Block::Code { lines: vec!["unclosed".into()] }]
    );
}

#[test]
fn markdown_inside_a_fence_is_not_parsed() {
    assert_eq!(
        parse_blocks("```\n# not a heading\n```"),
        vec![Block::Code { lines: vec!["# not a heading".into()] }]
    );
}
```

`src/render.rs`:

```rust
#[test]
fn code_lines_are_indented_and_unwrapped() {
    assert_eq!(r("```\naaaa bbbb cccc dddd\n```", 8), "  aaaa bbbb cccc dddd\n");
}

#[test]
fn code_block_is_dim_when_colour_is_on() {
    let out = render_blocks(&parse_blocks("```\nx\n```"), &Ctx { width: 40, color: true });
    assert_eq!(out, "\x1b[2m  x\x1b[0m\n");
}

#[test]
fn empty_code_block_renders_nothing() {
    assert_eq!(r("```\n```", 40), "");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

`block.rs`: on a line matching `^ {0,3}(```|~~~)`, collect lines until a line starting with the same fence run (same char, length ≥ opener) or EOF; the info string is discarded.

`render.rs`: each code line becomes `"  " + line` with `style::CODE_BLOCK` applied to the whole line, never wrapped. Empty `lines` emits nothing.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: fenced code blocks"
jj new
```

---

### Task 9: Block quotes

**Files:**
- Modify: `src/block.rs`, `src/render.rs`

- [ ] **Step 1: Write the failing tests**

`src/block.rs`:

```rust
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
            Block::Heading { level: 1, text: "T".into() },
            Block::List(vec![ListItem { marker: "-".into(), indent: 0, checked: None, text: "a".into() }]),
        ])]
    );
}
```

`src/render.rs`:

```rust
#[test]
fn quote_gets_a_gutter() {
    assert_eq!(r("> one\n> two", 20), "│ one\n│ two\n");
}

#[test]
fn quote_wraps_inside_the_gutter() {
    assert_eq!(r("> alpha beta gamma", 10), "│ alpha\n│ beta\n│ gamma\n");
}

#[test]
fn nested_quote_doubles_the_gutter() {
    assert_eq!(r("> > deep", 20), "│ │ deep\n");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

`block.rs`: consecutive lines matching `^ {0,3}> ?` are stripped of that prefix and re-parsed with `parse_blocks`, producing `Block::Quote(inner)`. A `>` line with nothing after it contributes an empty line.

`render.rs`: render inner blocks into `inner` with a `Ctx` whose `width` is `ctx.width.saturating_sub(2).max(1)`; then prefix every line (including blank ones, which become `"│"`) with `"│ "` styled `style::QUOTE`, and append the newline.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: block quotes with nested rendering"
jj new
```

---

### Task 10: Tables

**Files:**
- Modify: `src/block.rs`, `src/render.rs`

- [ ] **Step 1: Write the failing tests**

`src/block.rs`:

```rust
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
```

`src/render.rs`:

```rust
#[test]
fn table_pads_columns_and_underlines_the_header() {
    assert_eq!(
        r("| a | b |\n|---|---|\n| 1 | 22 |", 40),
        "a │ b\n───┼────\n1 │ 22\n"
    );
}

#[test]
fn table_width_counts_wide_chars() {
    assert_eq!(
        r("| 中 | b |\n|---|---|\n| x | y |", 40),
        "中 │ b\n────┼───\nx  │ y\n"
    );
}

#[test]
fn table_is_clamped_to_the_render_width() {
    let out = r("| aaaa | bbbb |\n|---|---|\n| cccc | dddd |", 8);
    assert!(out.lines().all(|l| crate::wrap::display_width(l) <= 8), "got {out:?}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

`block.rs`: a line containing `|` followed by a line matching `^\s*\|?[\s:|-]+\|[\s:|-]*$` containing at least one `-` starts a `Table`; run through the remaining lines while they contain `|`. Cells split on `|`, trimmed, and outer empty cells from leading/trailing pipes dropped. Alignment markers are ignored.

`render.rs`: column width = max `display_width` of the cells *as rendered* (each cell goes through `parse_inline`, measured by concatenated text width). Row line = cells padded to the column width, joined with `" │ "`, right-trimmed. Header separator = per column `"─".repeat(width + 2)` joined with `"┼"`. If the total line width exceeds `ctx.width`, shrink the widest column until it fits or until every column is 3 wide, then truncate cell text at that width by display columns (append `…` when truncated). If a line still exceeds `ctx.width` after every column is at its minimum of 3, truncate the finished line to `ctx.width` display columns — the width invariant must hold for pathological input. Header cells use `style::TABLE_HEAD`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "feat: pipe tables with width clamping"
jj new
```

---

### Task 11: Fixture smoke test, size gate, README

**Files:**
- Create: `tests/fixtures/sample.md`, `tests/smoke.rs`, `README.md`

- [ ] **Step 1: Write the failing test**

`tests/smoke.rs`:

```rust
use std::process::Command;

#[test]
fn renders_the_fixture_without_escapes_and_within_width() {
    let out = Command::new(env!("CARGO_BIN_EXE_md"))
        .arg("--width")
        .arg("60")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.md"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains('\u{1b}'), "piped output must be plain");
    assert!(text.contains("Fixture"), "headings render");
    assert!(text.len() > 200);
}

#[test]
fn no_panic_on_hostile_widths() {
    for w in ["0", "1", "2"] {
        let out = Command::new(env!("CARGO_BIN_EXE_md"))
            .arg("--width").arg(w)
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sample.md"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "width {w} must not fail");
    }
}

#[test]
fn hostile_markdown_does_not_panic() {
    let src = "**unclosed\n\n```\nunclosed fence\n\n| a |\n--- x\n\n> > > deep\n\n####### seven\n";
    let mut child = Command::new(env!("CARGO_BIN_EXE_md"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(src.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
}
```

- [ ] **Step 2: Write `tests/fixtures/sample.md`**

One file exercising every supported construct: an h1 and h2, a paragraph with **bold**, *italic*, ~~strike~~, `code`, a [link to example](https://example.com), an `![img](x.png)`; a nested list with a task item; a fenced code block; a two-line block quote containing a list; a pipe table; a rule; and a CJK paragraph (`终端渲染测试，宽度必须对齐。`).

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --test smoke`
Expected: FAIL (fixture missing, or escapes present).

- [ ] **Step 4: Fix whatever the smoke test exposes, then write `README.md`**

README states what `md` is, how to build (`cargo build --release`, binary at `target/release/md`), the CLI flags, the supported-construct table copied from the spec, and the known limits (no syntax highlighting, no footnotes, no reference links, no setext headings, code blocks inside list items flatten to text). It must not argue for any design decision.

- [ ] **Step 5: Verify the size and speed gates**

```bash
cargo build --release
ls -l target/release/md
/usr/bin/time -f "%e s" ./target/release/md tests/fixtures/sample.md > /dev/null
```

Expected: size under 1,000,000 bytes; wall time under 0.02 s. Record both numbers in the final report.

- [ ] **Step 6: Run the full gate and commit**

```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
jj describe -m "test: fixture smoke tests, README, size gate"
jj new
```
