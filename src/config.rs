use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    StructName,
    StructField,
    UnionName,
    UnionField,
    EnumName,
    EnumMember,
    TypedefName,
    VariableLocal,
    VariableGlobal,
    VariableStatic,
    MacroName,
    FunctionName,
    FunctionStatic,
    FunctionArguments,
}

pub const ALL_TARGETS: &[Target] = &[
    Target::StructName,
    Target::StructField,
    Target::UnionName,
    Target::UnionField,
    Target::EnumName,
    Target::EnumMember,
    Target::TypedefName,
    Target::VariableLocal,
    Target::VariableGlobal,
    Target::VariableStatic,
    Target::MacroName,
    Target::FunctionName,
    Target::FunctionStatic,
    Target::FunctionArguments,
];

impl Target {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StructName => "struct.name",
            Self::StructField => "struct.field",
            Self::UnionName => "union.name",
            Self::UnionField => "union.field",
            Self::EnumName => "enum.name",
            Self::EnumMember => "enum.member",
            Self::TypedefName => "typedef.name",
            Self::VariableLocal => "variable.local",
            Self::VariableGlobal => "variable.global",
            Self::VariableStatic => "variable.static",
            Self::MacroName => "macro.name",
            Self::FunctionName => "function.name",
            Self::FunctionStatic => "function.static",
            Self::FunctionArguments => "function.arguments",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        ALL_TARGETS.iter().copied().find(|t| t.as_str() == s)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Pattern {
    pub prefix: String,
    pub style: Style,
    pub suffix: String,
    pub display: String,
}

impl Pattern {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let display = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let parts: Vec<&str> = raw
            .split('+')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if parts.is_empty() {
            return Err("empty pattern".to_string());
        }

        let mut style_index = None;
        for (i, part) in parts.iter().enumerate() {
            if Style::parse(part).is_some() {
                if style_index.is_some() {
                    return Err(format!("multiple styles in pattern '{raw}'"));
                }
                style_index = Some(i);
            }
        }
        let style_index =
            style_index.ok_or_else(|| format!("no naming style in pattern '{raw}'"))?;
        let style = Style::parse(parts[style_index]).unwrap();
        let prefix = parts[..style_index].join("");
        let suffix = parts[style_index + 1..].join("");
        Ok(Self {
            prefix,
            style,
            suffix,
            display,
        })
    }

    pub fn matches(&self, name: &str) -> bool {
        if !name.starts_with(&self.prefix) || !name.ends_with(&self.suffix) {
            return false;
        }
        let rest_len = name
            .len()
            .saturating_sub(self.prefix.len() + self.suffix.len());
        if name.len() < self.prefix.len() + self.suffix.len() {
            return false;
        }
        let rest = &name[self.prefix.len()..self.prefix.len() + rest_len];
        self.style.matches(rest)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    rules: HashMap<Target, Pattern>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut rules = HashMap::new();
        for (i, raw_line) in text.lines().enumerate() {
            let line_no = i + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            let (target_str, pattern_str) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| format!("line {line_no}: expected '<target> <pattern>'"))?;
            let target = Target::parse(target_str)
                .ok_or_else(|| format!("line {line_no}: unknown target '{target_str}'"))?;
            let pattern =
                Pattern::parse(pattern_str.trim()).map_err(|e| format!("line {line_no}: {e}"))?;
            rules.insert(target, pattern);
        }
        Ok(Self { rules })
    }

    pub fn from_path(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn get(&self, target: Target) -> Option<&Pattern> {
        self.rules.get(&target)
    }

    pub fn rule_for(&self, target: Target) -> Option<(Target, &Pattern)> {
        if target == Target::FunctionStatic {
            if let Some(pat) = self.rules.get(&Target::FunctionStatic) {
                return Some((Target::FunctionStatic, pat));
            }
            if let Some(pat) = self.rules.get(&Target::FunctionName) {
                return Some((Target::FunctionName, pat));
            }
            return None;
        }
        self.rules.get(&target).map(|pat| (target, pat))
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

pub fn find_config_path(explicit: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Ok(from_env) = std::env::var("C_LINT_CONFIG") {
        if !from_env.is_empty() {
            return Ok(PathBuf::from(from_env));
        }
    }
    let mut dir = std::env::current_dir()?;
    loop {
        let candidate = dir.join(".c-lint.config");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !dir.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no .c-lint.config found (use --config PATH)",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_styles_with_affixes() {
        let cfg = Config::parse(
            "
struct.name PascalCase
variable.global g_ + snake_case
enum.name snake_case + _t
macro.name SCREAMING_SNAKE_CASE
",
        )
        .unwrap();
        let g = cfg.get(Target::VariableGlobal).unwrap();
        assert!(g.matches("g_foo"));
        assert!(g.matches("g_foo_bar"));
        assert!(!g.matches("foo"));
        assert!(!g.matches("g_Foo"));
        let e = cfg.get(Target::EnumName).unwrap();
        assert!(e.matches("color_t"));
        assert!(!e.matches("color"));
        let s = cfg.get(Target::StructName).unwrap();
        assert!(s.matches("Point2D"));
        assert!(!s.matches("point"));
    }

    #[test]
    fn compact_plus() {
        let p = Pattern::parse("g_+snake_case").unwrap();
        assert_eq!(p.prefix, "g_");
        assert!(p.matches("g_count"));
    }

    #[test]
    fn unknown_style() {
        let err = Pattern::parse("kebab-case").unwrap_err();
        assert!(err.contains("no naming style"));
    }

    #[test]
    fn unknown_target() {
        let err = Config::parse("not.a.target snake_case").unwrap_err();
        assert!(err.contains("unknown target"));
    }

    #[test]
    fn function_static_falls_back() {
        let cfg = Config::parse("function.name snake_case\n").unwrap();
        let (t, p) = cfg.rule_for(Target::FunctionStatic).unwrap();
        assert_eq!(t, Target::FunctionName);
        assert!(p.matches("foo_bar"));
    }
}
