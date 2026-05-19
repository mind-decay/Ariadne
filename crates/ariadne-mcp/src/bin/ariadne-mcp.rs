//! `ariadne-mcp` stdio MCP server binary. Tier-08 step 6 entrypoint.
//!
//! Usage:
//! ```text
//! ariadne-mcp serve --root <path>
//! ```
//! Defaults `--root` to the current working directory when omitted.

use std::path::PathBuf;
use std::process::ExitCode;

use ariadne_mcp::{ServeOpts, serve_stdio};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("serve") => match parse_serve_args(&args[1..]) {
            Ok(opts) => run(opts),
            Err(e) => {
                eprintln!("ariadne-mcp: {e}");
                ExitCode::FAILURE
            }
        },
        Some("--help" | "-h") | None => {
            println!("ariadne-mcp serve --root <path>");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("ariadne-mcp: unknown subcommand {other}");
            ExitCode::FAILURE
        }
    }
}

fn parse_serve_args(args: &[String]) -> Result<ServeOpts, String> {
    let mut root: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--root" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "--root requires a path".to_string())?;
                root = Some(PathBuf::from(v));
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let root = root.unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    Ok(ServeOpts::new(root))
}

fn run(opts: ServeOpts) -> ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build();
    let runtime = match runtime {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("ariadne-mcp: tokio init: {e}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve_stdio(opts)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ariadne-mcp: {e}");
            ExitCode::FAILURE
        }
    }
}
