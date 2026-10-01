//! Style names used in c-lint config patterns.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
}

impl Style {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "PascalCase" => Some(Self::PascalCase),
            "camelCase" => Some(Self::CamelCase),
            "snake_case" => Some(Self::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Some(Self::ScreamingSnakeCase),
            _ => None,
        }
    }

    pub fn matches(self, name: &str) -> bool {
        match self {
            Self::PascalCase => is_pascal_case(name),
            Self::CamelCase => is_camel_case(name),
            Self::SnakeCase => is_snake_case(name),
            Self::ScreamingSnakeCase => is_screaming_snake_case(name),
        }
    }
}

pub fn is_pascal_case(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => chars.all(|c| c.is_ascii_alphanumeric()),
        _ => false,
    }
}

pub fn is_camel_case(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => chars.all(|c| c.is_ascii_alphanumeric()),
        _ => false,
    }
}

pub fn is_snake_case(s: &str) -> bool {
    is_undecorated_snake(s, true)
}

pub fn is_screaming_snake_case(s: &str) -> bool {
    is_undecorated_snake(s, false)
}

fn is_undecorated_snake(s: &str, lower: bool) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let first = bytes[0];
    let ok_first = if lower {
        first.is_ascii_lowercase()
    } else {
        first.is_ascii_uppercase()
    };
    if !ok_first {
        return false;
    }
    let mut prev_underscore = false;
    for &b in &bytes[1..] {
        if b == b'_' {
            if prev_underscore {
                return false;
            }
            prev_underscore = true;
            continue;
        }
        let ok = if lower {
            b.is_ascii_lowercase() || b.is_ascii_digit()
        } else {
            b.is_ascii_uppercase() || b.is_ascii_digit()
        };
        if !ok {
            return false;
        }
        prev_underscore = false;
    }
    !prev_underscore
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case() {
        assert!(is_pascal_case("Point2D"));
        assert!(is_pascal_case("HttpResponse"));
        assert!(is_pascal_case("A"));
        assert!(!is_pascal_case("foo"));
        assert!(!is_pascal_case("Foo_Bar"));
        assert!(!is_pascal_case(""));
    }

    #[test]
    fn camel_case() {
        assert!(is_camel_case("nextHop"));
        assert!(is_camel_case("x1"));
        assert!(is_camel_case("foo"));
        assert!(!is_camel_case("Foo"));
        assert!(!is_camel_case("next_hop"));
    }

    #[test]
    fn snake_case() {
        assert!(is_snake_case("foo"));
        assert!(is_snake_case("foo_bar"));
        assert!(is_snake_case("foo2_bar"));
        assert!(!is_snake_case("Foo"));
        assert!(!is_snake_case("_foo"));
        assert!(!is_snake_case("foo_"));
        assert!(!is_snake_case("foo__bar"));
        assert!(!is_snake_case("fooBar"));
    }

    #[test]
    fn screaming_snake_case() {
        assert!(is_screaming_snake_case("FOO"));
        assert!(is_screaming_snake_case("MAX_SIZE"));
        assert!(!is_screaming_snake_case("Max_SIZE"));
        assert!(!is_screaming_snake_case("MAX_"));
        assert!(!is_screaming_snake_case("_MAX"));
    }
}
