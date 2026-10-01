#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Number,
    String,
    Char,
    Punct,
    Define,
}

pub fn lex(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let mut col = 1;
    let mut line_start = true;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            i += 1;
            line += 1;
            col = 1;
            line_start = true;
            continue;
        }
        if b == b'\r' {
            i += 1;
            continue;
        }
        if b == b' ' || b == b'\t' || b == b'\x0b' || b == b'\x0c' {
            i += 1;
            col += 1;
            continue;
        }

        if line_start && b == b'#' {
            lex_directive(bytes, &mut i, &mut line, &mut col, &mut tokens);
            line_start = true;
            continue;
        }
        line_start = false;

        if b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                skip_line_comment(bytes, &mut i, &mut line, &mut col);
                continue;
            }
            if bytes[i + 1] == b'*' {
                skip_block_comment(bytes, &mut i, &mut line, &mut col);
                continue;
            }
        }

        let start_line = line;
        let start_col = col;

        if b == b'"' {
            let text = lex_quoted(bytes, &mut i, &mut line, &mut col, b'"');
            tokens.push(Token {
                kind: TokenKind::String,
                text,
                line: start_line,
                column: start_col,
            });
            continue;
        }
        if b == b'\'' {
            let text = lex_quoted(bytes, &mut i, &mut line, &mut col, b'\'');
            tokens.push(Token {
                kind: TokenKind::Char,
                text,
                line: start_line,
                column: start_col,
            });
            continue;
        }

        if is_ident_start(b) {
            let start = i;
            i += 1;
            col += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
                col += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Ident,
                text: source[start..i].to_string(),
                line: start_line,
                column: start_col,
            });
            continue;
        }

        if b.is_ascii_digit() || (b == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            let text = lex_number(bytes, &mut i, &mut col);
            tokens.push(Token {
                kind: TokenKind::Number,
                text,
                line: start_line,
                column: start_col,
            });
            continue;
        }

        let punct = lex_punct(bytes, &mut i, &mut col);
        tokens.push(Token {
            kind: TokenKind::Punct,
            text: punct,
            line: start_line,
            column: start_col,
        });
    }

    tokens
}

fn lex_directive(
    bytes: &[u8],
    i: &mut usize,
    line: &mut usize,
    col: &mut usize,
    tokens: &mut Vec<Token>,
) {
    // Skip '#'
    *i += 1;
    *col += 1;
    skip_horizontal_ws(bytes, i, col);

    let dir_start = *i;
    while *i < bytes.len() && is_ident_continue(bytes[*i]) {
        *i += 1;
        *col += 1;
    }
    let directive = std::str::from_utf8(&bytes[dir_start..*i]).unwrap_or("");

    if directive == "define" {
        skip_horizontal_ws(bytes, i, col);
        if *i < bytes.len() && is_ident_start(bytes[*i]) {
            let name_col = *col;
            let name_line = *line;
            let name_start = *i;
            *i += 1;
            *col += 1;
            while *i < bytes.len() && is_ident_continue(bytes[*i]) {
                *i += 1;
                *col += 1;
            }
            let name = std::str::from_utf8(&bytes[name_start..*i])
                .unwrap_or("")
                .to_string();
            tokens.push(Token {
                kind: TokenKind::Define,
                text: name,
                line: name_line,
                column: name_col,
            });
        }
    }

    skip_preprocessor_tail(bytes, i, line, col);
}

fn skip_preprocessor_tail(bytes: &[u8], i: &mut usize, line: &mut usize, col: &mut usize) {
    while *i < bytes.len() {
        if bytes[*i] == b'\\' && *i + 1 < bytes.len() && bytes[*i + 1] == b'\n' {
            *i += 2;
            *line += 1;
            *col = 1;
            continue;
        }
        if bytes[*i] == b'\\'
            && *i + 2 < bytes.len()
            && bytes[*i + 1] == b'\r'
            && bytes[*i + 2] == b'\n'
        {
            *i += 3;
            *line += 1;
            *col = 1;
            continue;
        }
        if bytes[*i] == b'\n' {
            *i += 1;
            *line += 1;
            *col = 1;
            return;
        }
        *i += 1;
        *col += 1;
    }
}

fn skip_line_comment(bytes: &[u8], i: &mut usize, line: &mut usize, col: &mut usize) {
    *i += 2;
    *col += 2;
    while *i < bytes.len() && bytes[*i] != b'\n' {
        *i += 1;
        *col += 1;
    }
    let _ = (line, col);
}

fn skip_block_comment(bytes: &[u8], i: &mut usize, line: &mut usize, col: &mut usize) {
    *i += 2;
    *col += 2;
    while *i + 1 < bytes.len() {
        if bytes[*i] == b'\n' {
            *i += 1;
            *line += 1;
            *col = 1;
            continue;
        }
        if bytes[*i] == b'*' && bytes[*i + 1] == b'/' {
            *i += 2;
            *col += 2;
            return;
        }
        *i += 1;
        *col += 1;
    }
    *i = bytes.len();
}

fn lex_quoted(bytes: &[u8], i: &mut usize, line: &mut usize, col: &mut usize, quote: u8) -> String {
    let start = *i;
    *i += 1;
    *col += 1;
    while *i < bytes.len() {
        let b = bytes[*i];
        if b == b'\\' && *i + 1 < bytes.len() {
            if bytes[*i + 1] == b'\n' {
                *i += 2;
                *line += 1;
                *col = 1;
                continue;
            }
            *i += 2;
            *col += 2;
            continue;
        }
        if b == quote {
            *i += 1;
            *col += 1;
            break;
        }
        if b == b'\n' {
            break;
        }
        *i += 1;
        *col += 1;
    }
    String::from_utf8_lossy(&bytes[start..*i]).into_owned()
}

fn lex_number(bytes: &[u8], i: &mut usize, col: &mut usize) -> String {
    let start = *i;
    if bytes[*i] == b'.' {
        *i += 1;
        *col += 1;
    }
    while *i < bytes.len() {
        let b = bytes[*i];
        if b.is_ascii_alphanumeric() || b == b'.' || b == b'_' {
            *i += 1;
            *col += 1;
            continue;
        }
        if (b == b'+' || b == b'-')
            && *i > start
            && (bytes[*i - 1] == b'e'
                || bytes[*i - 1] == b'E'
                || bytes[*i - 1] == b'p'
                || bytes[*i - 1] == b'P')
        {
            *i += 1;
            *col += 1;
            continue;
        }
        break;
    }
    String::from_utf8_lossy(&bytes[start..*i]).into_owned()
}

fn lex_punct(bytes: &[u8], i: &mut usize, col: &mut usize) -> String {
    const MULTI: &[&[u8]] = &[
        b"...", b"<<=", b">>=", b"<<", b">>", b"->", b"++", b"--", b"+=", b"-=", b"*=", b"/=",
        b"%=", b"&=", b"|=", b"^=", b"==", b"!=", b"<=", b">=", b"&&", b"||", b"##",
    ];
    for op in MULTI {
        if bytes[*i..].starts_with(op) {
            *i += op.len();
            *col += op.len();
            return String::from_utf8_lossy(op).into_owned();
        }
    }
    let ch = bytes[*i];
    *i += 1;
    *col += 1;
    (ch as char).to_string()
}

fn skip_horizontal_ws(bytes: &[u8], i: &mut usize, col: &mut usize) {
    while *i < bytes.len() && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
        *i += 1;
        *col += 1;
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_comments_and_strings() {
        let src = r#"
int x; // comment
/* block
   comment */
char *s = "hello // not";
"#;
        let toks = lex(src);
        let idents: Vec<_> = toks
            .iter()
            .filter(|t| t.kind == TokenKind::Ident)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(idents, ["int", "x", "char", "s"]);
    }

    #[test]
    fn define_name() {
        let toks = lex("#define MAX_SIZE 4\n#define FOO(x) (x)\n");
        let defs: Vec<_> = toks
            .iter()
            .filter(|t| t.kind == TokenKind::Define)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(defs, ["MAX_SIZE", "FOO"]);
    }
}
