mod block;
mod inline;
mod pager;
mod render;
mod style;
mod wrap;

use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process::exit;

struct Cli {
    files: Vec<String>,
    width: Option<usize>,
    color: bool,
    pager: bool,
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        files: Vec::new(),
        width: None,
        color: true,
        pager: false,
    };
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--no-color" => cli.color = false,
            "-p" | "--pager" => cli.pager = true,
            "--width" => {
                i += 1;
                let value = args.get(i).ok_or("--width needs a value")?;
                cli.width = Some(parse_width(value)?);
            }
            _ if arg.starts_with("--width=") => {
                cli.width = Some(parse_width(&arg["--width=".len()..])?);
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("unknown option `{arg}`"));
            }
            _ => cli.files.push(arg.clone()),
        }
        i += 1;
    }
    Ok(cli)
}

fn parse_width(value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid width `{value}`"))
}

// Field names mirror struct winsize; only ws_col is read.
#[allow(dead_code)]
#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

/// Terminal width of fd 1, or None when it is not a tty.
pub fn term_width() -> Option<usize> {
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let r = unsafe { libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) };
    (r == 0 && ws.ws_col > 0).then_some(ws.ws_col as usize)
}

pub fn usage() -> &'static str {
    "usage: mdpeek [file...]\n\n  --width N    render width (default: terminal width, else 80)\n  --no-color   disable escapes (also honours NO_COLOR)\n  -p, --pager  open the result in $PAGER (default: less -R)\n"
}

fn read_stdin() -> Result<String, String> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| format!("stdin: {e}"))?;
    Ok(buf)
}

/// Files are concatenated in the order given; `-` stands for stdin at that
/// position, so `mdpeek CHANGES.md -` appends the piped text instead of dropping
/// the file.
fn read_input(files: &[String]) -> Result<String, String> {
    if files.is_empty() {
        return read_stdin();
    }
    let mut out = String::new();
    let mut stdin_read = false;
    for path in files {
        let text = if path == "-" {
            if stdin_read {
                String::new()
            } else {
                stdin_read = true;
                read_stdin()?
            }
        } else {
            fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?
        };
        if !text.is_empty() && !text.ends_with('\n') {
            out.push_str(&text);
            out.push('\n');
        } else {
            out.push_str(&text);
        }
    }
    Ok(out)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", usage());
        return;
    }
    let cli = match parse_args(&args) {
        Ok(cli) => cli,
        Err(msg) => {
            eprintln!("mdpeek: {msg}\n\n{}", usage());
            exit(2);
        }
    };
    let width = cli
        .width
        .unwrap_or_else(|| term_width().unwrap_or(80))
        .max(1);
    let color = cli.color && io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none();
    let input = match read_input(&cli.files) {
        Ok(text) => text,
        Err(msg) => {
            eprintln!("mdpeek: {msg}");
            exit(1);
        }
    };
    let ctx = render::Ctx { width, color };
    let rendered = render::render_blocks(&block::parse_blocks(&input), &ctx);
    if pager::wants_pager(cli.pager, io::stdout().is_terminal()) {
        match pager::run(&pager::resolve_pager(env::var("PAGER").ok()), &rendered) {
            Ok(code) if code != 0 => exit(code),
            Ok(_) => {}
            Err(_) => print!("{rendered}"),
        }
    } else {
        print!("{rendered}");
    }
}
