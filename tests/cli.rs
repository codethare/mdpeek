use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mdpeek"))
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("mdpeek [file...]"));
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

#[test]
fn dash_keeps_the_named_files_too() {
    let dir = std::env::temp_dir().join(format!("mdpeek-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.md");
    std::fs::write(&file, "from file\n").unwrap();
    let mut child = bin()
        .arg(&file)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"from stdin\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("from file"), "named file kept: {text:?}");
    assert!(text.contains("from stdin"), "stdin kept: {text:?}");
}
