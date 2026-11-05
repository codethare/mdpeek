# md

Renders a Markdown file as styled text in a terminal, so reading a README is
nicer than `cat`. One static binary, two dependencies at build time, no runtime
dependencies.

```text
md README.md
cat README.md | md
md --width 100 README.md | less -R
```

## Build

```text
cargo build --release
install -m755 target/release/md ~/.local/bin/md
```

Requires a Rust toolchain. The binary lands at `target/release/md` and is under
1 MB.

## Usage

```text
md [file...]

  --width N    render width (default: terminal width, else 80)
  --no-color   disable escapes (also honours NO_COLOR)
```

No file argument reads stdin; `-` also means stdin. Several files are rendered
in order. Exit status: 0 success, 1 read error, 2 usage error.

Colour is emitted only when stdout is a terminal and `NO_COLOR` is unset, so
piping or redirecting gives plain text.

## What it renders

| Construct | Output |
|---|---|
| `# .. ######` headings | bold, level-coloured; h1 underlined |
| `**bold**`, `*italic*`, `~~strike~~`, `` `code` `` | styled, nestable |
| fenced code blocks | indented two spaces, dim, not wrapped |
| `-`, `*`, `+`, `1.` lists | marker kept, text wraps under the text |
| `- [x]` tasks | box shown as `[x]` / `[ ]` |
| `>` block quotes | `│ ` gutter, nestable |
| `---`, `***`, `___` | full-width rule |
| `[text](url)` | underlined text, then the url |
| `![alt](url)` | `[image: alt] url` |
| pipe tables | padded columns, ruled header |

Everything else is plain text.

## Not implemented

Syntax highlighting, images, footnotes, reference-style links, setext
headings, inline-HTML rendering, themes, config files, a built-in pager,
CommonMark conformance. Line breaks inside a paragraph and inside a quote
soft-wrap into one paragraph; a code block inside a list item flattens to
indented text; a table wider than the render width shrinks its widest columns
and truncates cells with `…`, and only a table that cannot fit three columns
per cell falls back to wrapped rows.

## Layout

- `src/main.rs` — CLI, input, terminal width, colour decision
- `src/block.rs` — lines to blocks
- `src/inline.rs` — text to styled spans
- `src/wrap.rs` — width-aware wrapping
- `src/style.rs` — SGR styles and palette
- `src/render.rs` — blocks and spans to output lines
- `tests/` — CLI and whole-binary tests, plus a fixture that covers every
  construct above

Design and plan live in `docs/superpowers/`.
