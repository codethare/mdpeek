# mdpeek `-p` / `--pager` — short design

## Intent

`mdpeek -p README.md` opens the rendered document in the user's pager, so vim keys
(`j`/`k`/`d`/`u`/`g`/`G`, `/` search) come from `less` instead of from mdpeek's own
TUI code. Motivation: the current binary is 338 KB and starts in 2.6 ms, and a
hand-written or crossterm TUI would cost either ~300 lines of raw-mode code or a
binary above 1 MB.

## Goals

- `-p` / `--pager` flag; pager chosen from `$PAGER`, defaulting to `less -R`.
- Nothing lost when paging is impossible: missing pager, a pager that cannot be
  spawned, or non-tty stdout all fall back to writing to stdout as today.
- Exit status reflects the pager's failure, not silently swallowed.

## Non-goals

A built-in viewer of any kind: no alt screen, no footer, no scroll percentage, no
file picker, no search, no mouse, no `glow` keymap parity. No shell-quoting layer
of our own beyond handing `$PAGER` to `sh -c`.

## Anti-goal

Add a dependency, or special-case a pager by name beyond the default.

## Design

### CLI

`-p`, `--pager`. Existing flags unchanged. `mdpeek -p` with no file reads stdin
first, then pages the result.

### Behaviour

1. Render into a string, exactly as today.
2. If `-p` is absent, or `stdout` is not a tty, print and exit — the flag is a
   no-op when redirected, so `mdpeek -p x.md > out.txt` still produces a file.
3. Otherwise resolve the command: `$PAGER` when set and non-empty, else `less -R`.
4. Split the resolved command on whitespace and spawn it directly — no shell.
   The rendered bytes go to the child's stdin, stdout and stderr are inherited,
   and we wait.
5. If the spawn fails or the pager exits before reading (broken pipe), fall back
   to printing the document ourselves. A user who asked to read a file gets to
   read it.
6. Exit code: the pager's, when it is nonzero; otherwise 0.

### Colour

No change to the existing rule (`--no-color` off, `NO_COLOR` unset, stdout is a
tty). The pager inherits our stdout, so a tty there means a tty on the pager too,
and escapes are what `less -R` expects. A redirection disables colour by the same
rule that already exists.

A user-set `$PAGER` without `-R` will show escape codes; that is the user's
command, and README says so rather than mdpeek rewriting it.

#### Revision during planning

`$PAGER` is split on whitespace instead of being handed to `sh -c`. Under a
shell, a broken `$PAGER` exits 127 and is indistinguishable from a pager that
ran and returned nonzero, which would kill the fallback this design promises.
Cost: a `$PAGER` containing quoted arguments no longer works; `LESS` and friends
cover the usual cases, and README says so.

## Size

`src/pager.rs` about 25 lines of logic, `src/main.rs` about 8, tests about 70 —
the tests dominate because each one spawns the binary.

## Files

- `src/pager.rs` (new): resolve, spawn, feed, fall back.
- `src/main.rs`: the flag, the call, exit code.
- `src/main.rs` usage text, README, and the spec's non-goal line, which now reads
  "no built-in pager; `-p` delegates to `$PAGER`".

### Tests

- `PAGER=cat mdpeek -p fixture` prints the same bytes as `mdpeek fixture`.
- `PAGER="cat -A" mdpeek ...` is covered at the unit level: a command with
  arguments is split and the extra argument takes effect.
- Redirected stdout ignores the pager: `PAGER=/bin/false mdpeek -p fixture > file`
  exits 0 and writes the document.
- A pager that cannot be spawned (`PAGER=/nonexistent/pager`) still prints the
  document and exits 0.
- A pager that exits without reading (`PAGER=true`) on a large document does not
  panic on the broken pipe.
- A pager that exits nonzero (`PAGER=false`) makes mdpeek exit 1.
- No-escape guarantee still holds for the piped case already covered.

## Left to the builder

Exact function names and the wait/fallback loop shape; whether the fallback
repaints from the buffer or re-renders (buffer is obvious, but it is the
implementer's call).
