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
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"# Title\n\nhello\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stdout.contains(&0x1b), "piped output must be plain");
}

#[test]
fn empty_stdin_exits_zero() {
    let out = bin().stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn unknown_flag_exits_two() {
    let out = bin().arg("--nope").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
