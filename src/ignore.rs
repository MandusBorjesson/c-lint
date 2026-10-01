use std::collections::{HashMap, HashSet};

use crate::config::Target;
use crate::diag::{Diagnostic, Severity};

#[derive(Debug, Default, Clone)]
struct IgnoreSet {
    all: bool,
    targets: HashSet<Target>,
}

impl IgnoreSet {
    fn contains(&self, target: Target) -> bool {
        self.all || self.targets.contains(&target)
    }

    fn is_empty(&self) -> bool {
        !self.all && self.targets.is_empty()
    }

    fn add_all(&mut self) {
        self.all = true;
        self.targets.clear();
    }

    fn add(&mut self, target: Target) {
        if !self.all {
            self.targets.insert(target);
        }
    }

    fn remove_all(&mut self) {
        self.all = false;
        self.targets.clear();
    }

    fn remove(&mut self, target: Target) {
        self.targets.remove(&target);
    }
}

#[derive(Debug, Default)]
pub struct IgnoreMap {
    line: HashMap<usize, IgnoreSet>,
    block_at: HashMap<usize, IgnoreSet>,
}

impl IgnoreMap {
    pub fn is_ignored(&self, line: usize, target: Target) -> bool {
        self.line.get(&line).is_some_and(|s| s.contains(target))
            || self.block_at.get(&line).is_some_and(|s| s.contains(target))
    }
}

pub fn parse_ignores(path: &str, source: &str) -> (IgnoreMap, Vec<Diagnostic>) {
    let mut map = IgnoreMap::default();
    let mut diags = Vec::new();
    let mut active = IgnoreSet::default();
    let mut block_starts: HashMap<String, usize> = HashMap::new();
    let comments = collect_line_comments(source);
    let comment_at: HashMap<usize, &str> = comments.iter().map(|(l, t)| (*l, t.as_str())).collect();
    let total_lines = count_lines(source);

    for line in 1..=total_lines {
        let mut starts = IgnoreSet::default();
        let mut ends: Option<IgnoreSet> = None;
        let mut line_only = IgnoreSet::default();

        if let Some(text) = comment_at.get(&line) {
            match parse_directive(text) {
                Ok(Some(Directive::Ignore(set))) => line_only = set,
                Ok(Some(Directive::BlockStart(set))) => starts = set,
                Ok(Some(Directive::BlockEnd(set))) => ends = Some(set),
                Ok(None) => {}
                Err(msg) => {
                    diags.push(Diagnostic {
                        path: path.to_string(),
                        line,
                        column: 1,
                        severity: Severity::Error,
                        message: msg,
                    });
                }
            }
        }

        if !starts.is_empty() {
            merge_into(&mut active, &starts, line, &mut block_starts);
        }
        if !active.is_empty() {
            map.block_at.insert(line, active.clone());
        }
        if !line_only.is_empty() {
            map.line.insert(line, line_only);
        }
        if let Some(end) = ends {
            apply_end(&mut active, &end, &mut block_starts);
        }
    }

    if active.all {
        if let Some(start) = block_starts.get("all") {
            diags.push(unclosed(path, *start, "all"));
        }
    }
    for target in &active.targets {
        if let Some(start) = block_starts.get(target.as_str()) {
            diags.push(unclosed(path, *start, target.as_str()));
        }
    }

    (map, diags)
}

fn unclosed(path: &str, line: usize, what: &str) -> Diagnostic {
    Diagnostic {
        path: path.to_string(),
        line,
        column: 1,
        severity: Severity::Warning,
        message: format!("unclosed ignore-block for '{what}'"),
    }
}

fn merge_into(
    active: &mut IgnoreSet,
    starts: &IgnoreSet,
    line: usize,
    block_starts: &mut HashMap<String, usize>,
) {
    if starts.all {
        active.add_all();
        block_starts.entry("all".to_string()).or_insert(line);
        return;
    }
    for t in &starts.targets {
        if !active.contains(*t) {
            block_starts.insert(t.as_str().to_string(), line);
        }
        active.add(*t);
    }
}

fn apply_end(active: &mut IgnoreSet, end: &IgnoreSet, block_starts: &mut HashMap<String, usize>) {
    if end.all || listed_as_bare_end(end) {
        active.remove_all();
        block_starts.clear();
        return;
    }
    for t in &end.targets {
        active.remove(*t);
        block_starts.remove(t.as_str());
    }
}

fn listed_as_bare_end(end: &IgnoreSet) -> bool {
    !end.all && end.targets.is_empty()
}

enum Directive {
    Ignore(IgnoreSet),
    BlockStart(IgnoreSet),
    BlockEnd(IgnoreSet),
}

fn parse_directive(comment: &str) -> Result<Option<Directive>, String> {
    let text = comment.trim();
    if let Some(rest) = strip_keyword(text, "ignore-block-end") {
        return Ok(Some(Directive::BlockEnd(parse_target_list(rest)?)));
    }
    if let Some(rest) = strip_keyword(text, "ignore-block") {
        let set = parse_target_list(rest)?;
        if set.is_empty() && !set.all {
            return Err("ignore-block requires a target or 'all'".to_string());
        }
        return Ok(Some(Directive::BlockStart(set)));
    }
    if let Some(rest) = strip_keyword(text, "ignore") {
        let set = parse_target_list(rest)?;
        if set.is_empty() && !set.all {
            return Err("ignore requires a target or 'all'".to_string());
        }
        return Ok(Some(Directive::Ignore(set)));
    }
    Ok(None)
}

fn strip_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let t = text.trim_start();
    if let Some(rest) = t.strip_prefix(keyword) {
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return Some(rest);
        }
    }
    None
}

fn parse_target_list(rest: &str) -> Result<IgnoreSet, String> {
    let mut set = IgnoreSet::default();
    for word in rest.split_whitespace() {
        if word == "all" {
            set.add_all();
            continue;
        }
        match Target::parse(word) {
            Some(t) => set.add(t),
            None => return Err(format!("unknown ignore target '{word}'")),
        }
    }
    Ok(set)
}

fn count_lines(source: &str) -> usize {
    if source.is_empty() {
        return 1;
    }
    let mut n = source.bytes().filter(|&b| b == b'\n').count();
    if !source.ends_with('\n') {
        n += 1;
    }
    n.max(1)
}

fn collect_line_comments(source: &str) -> Vec<(usize, String)> {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut line = 1;
    let mut in_block = false;
    let mut in_string = false;
    let mut in_char = false;
    let mut out = Vec::new();

    while i < bytes.len() {
        let b = bytes[i];
        if in_block {
            if b == b'\n' {
                line += 1;
                i += 1;
                continue;
            }
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_block = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_string {
            if b == b'\\' && i + 1 < bytes.len() {
                if bytes[i + 1] == b'\n' {
                    line += 1;
                }
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            if b == b'\n' {
                line += 1;
                in_string = false;
            }
            i += 1;
            continue;
        }
        if in_char {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'\'' {
                in_char = false;
            }
            if b == b'\n' {
                line += 1;
                in_char = false;
            }
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            in_block = true;
            i += 2;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            i += 2;
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            let text = String::from_utf8_lossy(&bytes[start..i]).into_owned();
            out.push((line, text));
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_char = true;
            i += 1;
            continue;
        }
        if b == b'\n' {
            line += 1;
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_ignore(src: &str) -> IgnoreMap {
        parse_ignores("t.c", src).0
    }

    #[test]
    fn line_ignore() {
        let src = "int bad_local; // ignore variable.local\n";
        let map = cfg_ignore(src);
        assert!(map.is_ignored(1, Target::VariableLocal));
        assert!(!map.is_ignored(1, Target::FunctionName));
    }

    #[test]
    fn block_ignore() {
        let src = "// ignore-block variable.local\nint a;\nint b;\n// ignore-block-end\nint c;\n";
        let map = cfg_ignore(src);
        assert!(map.is_ignored(2, Target::VariableLocal));
        assert!(map.is_ignored(3, Target::VariableLocal));
        assert!(map.is_ignored(4, Target::VariableLocal));
        assert!(!map.is_ignored(5, Target::VariableLocal));
    }

    #[test]
    fn unknown_target_is_error() {
        let (_, diags) = parse_ignores("t.c", "int x; // ignore not.a.thing\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("unknown ignore target"));
    }

    #[test]
    fn ignore_all() {
        let src = "int x; // ignore all\n";
        let map = cfg_ignore(src);
        assert!(map.is_ignored(1, Target::VariableLocal));
        assert!(map.is_ignored(1, Target::FunctionName));
    }

    #[test]
    fn unclosed_block_warns() {
        let (_, diags) = parse_ignores("t.c", "// ignore-block variable.local\nint a;\n");
        assert!(diags
            .iter()
            .any(|d| { !d.is_error() && d.message.contains("unclosed ignore-block") }));
    }
}
