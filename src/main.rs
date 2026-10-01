use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use c_lint::{collect_c_files, find_config_path, lint_file, Config};

const HELP: &str = "\
c-lint — naming-convention linter for C source files

Usage:
  c-lint [OPTIONS] [PATH ...]

Options:
  -c, --config PATH   Config file (default: C_LINT_CONFIG or .c-lint.config)
  -h, --help          Show this help
      --version       Show version

PATH may be a .c/.h file or a directory (scanned recursively).
With no PATH, .c/.h files under the current directory are linted.

Exit status is 1 if any error is reported, 0 otherwise.
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("c-lint: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut config_path: Option<PathBuf> = None;
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            "--version" => {
                println!("c-lint {}", env!("CARGO_PKG_VERSION"));
                return Ok(ExitCode::SUCCESS);
            }
            "-c" | "--config" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing argument for --config".to_string())?;
                config_path = Some(PathBuf::from(value));
            }
            "--" => {
                paths.extend(args.map(PathBuf::from));
                break;
            }
            s if s.starts_with('-') => {
                return Err(format!("unknown option '{s}'"));
            }
            s => paths.push(PathBuf::from(s)),
        }
    }

    let resolved = find_config_path(config_path.as_deref()).map_err(|e| e.to_string())?;
    if !resolved.is_file() {
        return Err(format!("config not found: {}", resolved.display()));
    }
    let config = Config::from_path(&resolved)?;

    let files = collect_c_files(&paths).map_err(|e| e.to_string())?;
    let mut failed = false;
    for file in files {
        let diags =
            lint_file(Path::new(&file), &config).map_err(|e| format!("{}: {e}", file.display()))?;
        for diag in &diags {
            println!("{diag}");
            if diag.is_error() {
                failed = true;
            }
        }
    }

    Ok(if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
