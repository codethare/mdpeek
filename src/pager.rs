use std::io::Write;
use std::process::{Command, Stdio};

/// The pager command to run: `$PAGER` when it names something, else `less -R`.
pub fn resolve_pager(from_env: Option<String>) -> String {
    from_env
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "less -R".to_string())
}

/// Run `cmd` with `text` on its stdin and its own stdout, returning its exit
/// code. `Err` only when the process cannot be started at all — a pager that
/// quits early is normal, so a broken pipe is ignored deliberately.
pub fn run(cmd: &str, text: &str) -> Result<i32, String> {
    let mut parts = cmd.split_whitespace();
    let program = parts.next().ok_or("empty pager command")?;
    let mut child = Command::new(program)
        .args(parts)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let status = child.wait().map_err(|e| format!("{program}: {e}"))?;
    Ok(status.code().unwrap_or(0))
}

/// Paging needs both the flag and a terminal to page on.
pub fn wants_pager(flag: bool, is_tty: bool) -> bool {
    flag && is_tty
}

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
