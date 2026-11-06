# mdpeek `-p` / `--pager` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `mdpeek -p file.md` opens the rendered document in `$PAGER` (default `less -R`), falling back to plain output whenever paging cannot happen.

**Architecture:** One new module wraps the pager decision and the child process; `main.rs` gains a flag, a tty-gated call, and exit-code propagation. No new dependency.

**Tech Stack:** Rust 2024, existing `libc` + `unicode-width` only.

**Spec:** `docs/superpowers/specs/2026-11-05-pager-flag-design.md`

## Global Constraints

- No new crate. `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` all clean before the commit.
- Never lose output: a missing pager, a spawn failure, or a broken pipe all end with the document printed by mdpeek.
- Raw-mode/TUI behaviour is a non-goal; nothing may open the alternate screen or read keys.
- VCS is jj: `jj describe -m "…"` then `jj new`. Never `git commit`.
- Colour rule is unchanged: `--no-color` off, `NO_COLOR` unset, stdout is a tty.

## Review Focus

1. `mdpeek -p x.md > out.txt` must still produce the document (pager is a no-op when stdout is not a tty).
2. A `$PAGER` that does not exist must not swallow the document — the user sees their file.
3. A pager that exits without reading stdin (e.g. `true`) must not panic on EPIPE, even for a large document.
4. A nonzero pager exit must reach the shell as mdpeek's exit code.
5. `$PAGER` with arguments (`cat -A`) must run the intended command, not a file named `cat -A`.

---

### Task 1: `-p` / `--pager`

**Files:**
- Create: `src/pager.rs`, modify `src/main.rs`, `tests/cli.rs`, `README.md`, `docs/superpowers/specs/2026-11-05-rust-md-terminal-renderer-design.md`

**Interfaces:**
- Produces: `pub fn resolve_pager(from_env: Option<String>) -> String` — trims, falls back to `less -R` when empty/absent.
- Produces: `pub fn run(cmd: &str, text: &str) -> Result<i32, String>` — splits on whitespace, spawns, writes, waits; `Err` only when the process cannot be started.
- Produces: `pub fn wants_pager(flag: bool, is_tty: bool) -> bool` — `flag && is_tty`.
- Produces on `Cli`: `pager: bool`; usage text gains `-p, --pager`.

- [ ] **Step 1: Write the failing unit tests** in `src/pager.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_pager_falls_back_to_less_r() {
        assert_eq!(resolve_pager(None), "less -R");
        assert_eq!(resolve_pager(Some("   ".to_string())), "less -R");
    }

    #[test]
    fn user_pager_is_used_verbatim_but_trimmed() {
        assert_eq!(resolve_pager(Some("  more -x  ".to_string())), "more -x");
    }

    #[test]
    fn runs_a_pager_that_succeeds() {
        assert_eq!(run("cat", "hello\n"), Ok(0));
    }

    #[test]
    fn extra_arguments_reach_the_pager() {
        assert_eq!(run("head -c 0", "ignored\n"), Ok(0));
    }

    #[test]
    fn nonzero_pager_exit_is_reported() {
        assert_eq!(run("false", "x\n"), Ok(1));
    }

    #[test]
    fn a_pager_that_cannot_be_spawned_is_an_error() {
        assert!(run("/nonexistent/pager", "x\n").is_err());
    }

    #[test]
    fn a_pager_that_exits_without_reading_does_not_panic() {
        let big = "x\n".repeat(200_000);
        assert_eq!(run("true", &big), Ok(0));
    }

    #[test]
    fn pager_needs_both_the_flag_and_a_tty() {
        assert!(wants_pager(true, true));
        assert!(!wants_pager(true, false));
        assert!(!wants_pager(false, true));
        assert!(!wants_pager(false, false));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test pager`
Expected: FAIL — `cannot find function resolve_pager` (and the module is linked from `main.rs` in the same step, so the failure is a real compile error, not a silently skipped file).

- [ ] **Step 3: Implement `src/pager.rs`**

`resolve_pager` trims and falls back. `run` uses `split_whitespace`, `Command::new(first).args(rest).stdin(Stdio::piped())`, ignores the error from `write_all` (EPIPE means the pager quit early, which is normal), takes `child.stdin` before waiting, and returns `child.wait()`'s code or `Err` when `spawn` fails. `wants_pager` is `flag && is_tty`.

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test pager`
Expected: 8 passed.

- [ ] **Step 5: Write the failing CLI tests** in `tests/cli.rs`

```rust
#[test]
fn pager_flag_is_ignored_when_stdout_is_not_a_tty() {
    // PAGER would delete the output if it ran; the document must survive.
    let out = bin()
        .args(["-p"])
        .env("PAGER", "/bin/false")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(b"# Title\n").unwrap();
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("Title"));
}

#[test]
fn pager_flag_is_accepted_with_a_file() {
    let out = bin()
        .args(["--pager", "/nonexistent/x.md"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "read error still wins");
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `cargo test --test cli`
Expected: FAIL — `unknown option --pager`, exit 2 where 0 and 1 are asserted.

- [ ] **Step 7: Wire it into `src/main.rs`**

`Cli` gains `pager: bool` (default false); `parse_args` matches `-p` and `--pager`. After rendering into `let rendered = render::render_blocks(...)`:

```rust
let wants_pager = pager::wants_pager(cli.pager, io::stdout().is_terminal());
if wants_pager {
    match pager::run(&pager::resolve_pager(env::var("PAGER").ok()), &rendered) {
        Ok(code) if code != 0 => exit(code),
        Ok(_) => {}
        Err(_) => print!("{rendered}"),
    }
} else {
    print!("{rendered}");
}
```

Usage text gains `  -p, --pager  open the result in $PAGER (default: less -R)`.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test`
Expected: all pass (78 existing + 8 unit + 2 CLI).

- [ ] **Step 9: Update the docs**

README usage block gains the flag and one line: `-p` is ignored when stdout is redirected, `$PAGER` is split on whitespace so no shell quoting, a pager without `-R` will show escape codes. The main spec's non-goal line becomes `no built-in pager; -p delegates to $PAGER`.

- [ ] **Step 10: Commit**

```bash
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check
jj describe -m "feat: -p/--pager opens the result in \$PAGER, falling back to stdout"
jj new
```
