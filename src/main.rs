#![allow(dead_code)]

mod block;
mod inline;
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
}

fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli {
        files: Vec::new(),
        width: None,
        color: true,
    };
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--no-color" => cli.color = false,
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
    "usage: md [file...]\n\n  --width N    render width (default: terminal width, else 80)\n  --no-color   disable escapes (also honours NO_COLOR)\n"
}

fn read_input(files: &[String]) -> Result<String, String> {
    if files.is_empty() || files.iter().any(|f| f == "-") {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("stdin: {e}"))?;
        return Ok(buf);
    }
    let mut out = String::new();
    for path in files {
        let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        out.push_str(&text);
        if !text.ends_with('\n') {
            out.push('\n');
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
            eprintln!("md: {msg}\n\n{}", usage());
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
            eprintln!("md: {msg}");
            exit(1);
        }
    };
    let ctx = render::Ctx { width, color };
    print!(
        "{}",
        render::render_blocks(&block::parse_blocks(&input), &ctx)
    );
}
