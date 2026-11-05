# md — minimal terminal Markdown renderer (Rust)

## Intent

A small, fast command that prints a Markdown file styled for a terminal, so
reading a README is nicer than `cat`. Used as `md README.md` or
`cat x.md | md`.

## Goals

- One static binary, no runtime dependencies.
- Correct-looking output for the constructs that actually appear in READMEs.
- Fast enough that startup is not perceptible; small enough to drop in `~/bin`.

## Non-goals

Syntax highlighting, images (sixel/kitty), footnotes, reference-style links,
setext headings, inline-HTML rendering, themes, config files, plugins, a
built-in pager, CommonMark conformance.

## Anti-goals

Binary over 1 MB, or a dependency tree deeper than two crates, or any of the
above non-goals arriving to "make it complete".

## Constraints

- Render width from the real terminal (`TIOCGWINSZ`), not from `$COLUMNS`
  (shells do not export it to children).
- Plain output with zero escape bytes when stdout is not a tty, or when
  `NO_COLOR` is set.
- 16-color SGR only, so output survives any terminal and `less -R`.
- Width counting must ignore escape bytes and treat East-Asian wide characters
  as two columns, or CJK text wraps wrong.

## Approaches considered

1. **Go + hand-written parser.** Fastest to write, ~2.5 MB binary,
   ~5 ms startup. Rejected: the user wants the smaller artifact.
2. **Rust + an existing Markdown crate** (comrak, pulldown-cmark) +
   glamour-style styling. Rejected: comrak's dependency tree is large and its
   full CommonMark surface is more than wanted; the styling stacks pull in
   more still, which fights both "small" and "fast".
3. **Rust + hand-written subset parser.** Chosen. ~400–500 KB stripped,
   ~1 ms startup, behaviour fully controlled, and the parser only knows the
   syntax we render.

## Design

### Dependencies

`unicode-width` (display width) and `libc` (one `ioctl` call). Both are
leaf crates with no transitive dependencies.

Release profile: `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`,
`panic = "abort"`, `strip = true`.

### CLI

```text
md [file...]        # no args: read stdin; "-" also means stdin
md --width N        # override render width
md --no-color       # same as NO_COLOR=1
md -h | --help
```

Files are concatenated in order. Exit 0 on success, 1 on read error,
2 on usage error.

### Pipeline (two phases)

1. **Blocks** — group lines into `Heading | Para | Code | List | Quote |
   Table | Rule`. Block nesting (a quote holding a list) is a tree, so an AST
   beats streaming line-by-line here.
2. **Inline** — split each text block into `Span { text, style }` over
   bold/italic/strike/code/link.
3. **Render** — spans to wrapped, styled lines; width accounting in `wrap.rs`.

### Rendering table

| Construct | Output |
|---|---|
| ATX heading `#`..`######` | bold, level-coloured; h1 underlined |
| emphasis `*`/`_`, `~~strike~~`, `` `code` `` | italic / strikethrough / reverse-video, nested OK |
| fenced code block | 2-space indent, dim, unwrapped, info string discarded |
| list `-`/`*`/`+`/`1.` | marker + hanging indent, nesting by indent |
| blockquote `>` | `│ ` gutter, dim, nestable |
| rule `---`/`***`/`___` | full-width `─` |
| `[text](url)` | text underlined, then ` (url)` unless url == text |
| `![alt](url)` | `[image: alt] url` |
| table | padded columns with `│`, `─` under the header |
| task `- [x]` | `[x]` / `[ ]` kept literal |
| literal `<tag>` | printed raw |
| escapes `\*` etc. | honoured |

Anything not listed renders as plain paragraph text.

### Wrapping

Width-aware: escape sequences excluded from counting, wide chars cost two
columns, break on spaces, hard-break terms longer than the width (long URLs).
Code blocks and tables are not wrapped; tables shrink by truncating the last
column when they exceed the width.

### Testing

`cargo test` with table-driven cases: one per construct (expected output with
escapes made visible), wrap cases at narrow widths, CJK width cases, and a
non-tty case asserting no escape bytes appear.

## Left to the builder

- Exact colour palette (I will pick 8-colour SGR values that read on both
  light and dark backgrounds).
- Whether nested quotes/ lists share a gutter style, decided once tests show
  what reads best.
- Module split inside `src/` — the boundaries above, but the line counts are
  guesses.

## Open items for the human

- Rust toolchain is absent on this machine (`rustup` has no installed
  toolchain). `rustup default stable` downloads ~300 MB; needs their go-ahead.
- The workspace is not a git repository. `git init` + the first commit is
  local setup and mine to do unless told otherwise.
