use std::io::Write;
use std::process::{Command, Stdio};

fn fixture() -> String {
    format!("{}/tests/fixtures/sample.md", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn renders_the_fixture_without_escapes_and_within_width() {
    let out = Command::new(env!("CARGO_BIN_EXE_mdpeek"))
        .arg("--width")
        .arg("60")
        .arg(fixture())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains('\u{1b}'), "piped output must be plain");
    assert!(text.contains("Fixture"), "headings render");
    assert!(text.len() > 200, "the whole document renders");
}

#[test]
fn no_panic_on_hostile_widths() {
    for w in ["0", "1", "2"] {
        let out = Command::new(env!("CARGO_BIN_EXE_mdpeek"))
            .arg("--width")
            .arg(w)
            .arg(fixture())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "width {w} must not fail");
    }
}

#[test]
fn hostile_markdown_does_not_panic() {
    let src = "**unclosed\n\n```\nunclosed fence\n\n| a |\n--- x\n\n> > > deep\n\n####### seven\n";
    let mut child = Command::new(env!("CARGO_BIN_EXE_mdpeek"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(src.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
}
