use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::diag::{Diagnostic, Severity};
use crate::extract::extract;
use crate::ignore::parse_ignores;
use crate::lexer::lex;

pub fn lint_source(path: &str, source: &str, config: &Config) -> Vec<Diagnostic> {
    let (ignores, mut diags) = parse_ignores(path, source);
    let tokens = lex(source);
    for ident in extract(&tokens) {
        if ignores.is_ignored(ident.line, ident.target) {
            continue;
        }
        let Some((rule_target, pattern)) = config.rule_for(ident.target) else {
            continue;
        };
        if pattern.matches(&ident.name) {
            continue;
        }
        diags.push(Diagnostic {
            path: path.to_string(),
            line: ident.line,
            column: ident.column,
            severity: Severity::Error,
            message: format!(
                "{rule_target} '{}' does not match '{}'",
                ident.name, pattern.display
            ),
        });
    }
    diags.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then(a.column.cmp(&b.column))
            .then(a.message.cmp(&b.message))
    });
    diags
}

pub fn lint_file(path: &Path, config: &Config) -> io::Result<Vec<Diagnostic>> {
    let source = fs::read_to_string(path)?;
    let display = path.to_string_lossy().into_owned();
    Ok(lint_source(&display, &source, config))
}

pub fn collect_c_files(paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut visited = Vec::new();
    if paths.is_empty() {
        walk_dir(&std::env::current_dir()?, &mut out, &mut visited)?;
    } else {
        for path in paths {
            if path.is_dir() {
                walk_dir(path, &mut out, &mut visited)?;
            } else if path.is_file() {
                if is_c_file(path) {
                    out.push(path.clone());
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no such file or directory: {}", path.display()),
                ));
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn is_c_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase),
        Some(ext) if ext == "c" || ext == "h"
    )
}

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>, visited: &mut Vec<PathBuf>) -> io::Result<()> {
    let canonical = match dir.canonicalize() {
        Ok(p) => p,
        Err(e) => return Err(e),
    };
    if visited.iter().any(|p| p == &canonical) {
        return Ok(());
    }
    visited.push(canonical);

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk_dir(&path, out, visited)?;
        } else if ft.is_file() && is_c_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg() -> Config {
        Config::parse(
            "
struct.name PascalCase
struct.field camelCase
enum.name snake_case + _t
enum.member SCREAMING_SNAKE_CASE
typedef.name snake_case + _t
variable.local snake_case
variable.global g_ + snake_case
variable.static s_ + snake_case
macro.name SCREAMING_SNAKE_CASE
function.name snake_case
function.arguments snake_case
",
        )
        .unwrap()
    }

    #[test]
    fn reports_mismatch() {
        let diags = lint_source("t.c", "int foo;\n", &cfg());
        assert!(diags.iter().any(|d| d.message.contains("variable.global")));
    }

    #[test]
    fn accepts_matching_names() {
        let src = r#"
#define MAX_SIZE 1
typedef struct Foo { int nextHop; } foo_t;
int g_total;
int compute(int argc) { int local_n; return local_n; }
"#;
        let diags: Vec<_> = lint_source("t.c", src, &cfg())
            .into_iter()
            .filter(|d| d.is_error())
            .collect();
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn line_ignore_suppresses() {
        let diags: Vec<_> = lint_source("t.c", "int foo; // ignore variable.global\n", &cfg())
            .into_iter()
            .filter(|d| d.is_error())
            .collect();
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn block_ignore_suppresses() {
        let src = "// ignore-block variable.global\nint foo;\nint bar;\n// ignore-block-end\n";
        let diags: Vec<_> = lint_source("t.c", src, &cfg())
            .into_iter()
            .filter(|d| d.is_error())
            .collect();
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn typedef_alias_and_struct_tag() {
        let src = "typedef struct Foo { int nextHop; } Foo_t;\n";
        let diags: Vec<_> = lint_source("t.c", src, &cfg())
            .into_iter()
            .filter(|d| d.is_error())
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("typedef.name 'Foo_t'"));
    }
}
