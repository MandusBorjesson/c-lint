use crate::config::Target;
use crate::lexer::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub target: Target,
    pub name: String,
    pub line: usize,
    pub column: usize,
}

struct Parser<'a> {
    tokens: &'a [Token],
    i: usize,
    out: Vec<Extracted>,
}

struct Decl {
    name: Option<Token>,
    is_function: bool,
    has_pointer: bool,
    params: Vec<Param>,
}

struct Param {
    name: Token,
}

struct Specs {
    is_typedef: bool,
    is_static: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    File,
    Block,
}

pub fn extract(tokens: &[Token]) -> Vec<Extracted> {
    let mut p = Parser {
        tokens,
        i: 0,
        out: Vec::new(),
    };
    p.parse_translation_unit();
    p.out
}

impl<'a> Parser<'a> {
    fn parse_translation_unit(&mut self) {
        while !self.eof() {
            if self.take_define() {
                continue;
            }
            if !self.parse_declaration(Scope::File) {
                self.i += 1;
            }
        }
    }

    fn parse_declaration(&mut self, scope: Scope) -> bool {
        if self.eof() {
            return false;
        }
        if !self.is_decl_start(scope) {
            return false;
        }

        let specs = match self.parse_specifiers() {
            Some(s) => s,
            None => return false,
        };

        self.skip_attributes();

        if self.at(";") {
            self.bump();
            return true;
        }

        let decl = self.parse_declarator();
        self.skip_attributes();

        if decl.is_function && self.at("{") {
            self.emit_function(&decl, specs.is_static);
            self.parse_compound();
            return true;
        }

        self.emit_declarator(&decl, &specs, scope);

        if self.at("=") {
            self.skip_initializer();
        }

        while self.at(",") {
            self.bump();
            let d = self.parse_declarator();
            self.skip_attributes();
            self.emit_declarator(&d, &specs, scope);
            if self.at("=") {
                self.skip_initializer();
            }
        }

        if self.at(";") {
            self.bump();
        }
        true
    }

    fn parse_specifiers(&mut self) -> Option<Specs> {
        let mut specs = Specs {
            is_typedef: false,
            is_static: false,
        };
        let mut saw_type = false;
        let mut any = false;

        loop {
            self.skip_attributes();
            if self.eof() {
                break;
            }
            if self.at_ident_text("typedef") {
                specs.is_typedef = true;
                any = true;
                self.bump();
                continue;
            }
            if self.at_ident_text("static") {
                specs.is_static = true;
                any = true;
                self.bump();
                continue;
            }
            if self.at_ident_text("extern")
                || self.at_ident_text("auto")
                || self.at_ident_text("register")
                || self.at_ident_text("_Thread_local")
                || self.at_ident_text("__thread")
                || self.at_ident_text("inline")
                || self.at_ident_text("_Noreturn")
                || self.at_ident_text("__inline__")
                || self.at_ident_text("__inline")
                || self.at_ident_text("__extension__")
            {
                any = true;
                self.bump();
                continue;
            }
            if self.at_ident_text("const")
                || self.at_ident_text("volatile")
                || self.at_ident_text("restrict")
                || self.at_ident_text("_Atomic")
                || self.at_ident_text("__const")
                || self.at_ident_text("__restrict")
                || self.at_ident_text("__restrict__")
            {
                any = true;
                self.bump();
                continue;
            }
            if self.at_ident_text("_Alignas") || self.at_ident_text("alignas") {
                any = true;
                self.bump();
                self.skip_parens();
                continue;
            }
            if self.at_ident_text("struct") {
                any = true;
                saw_type = true;
                self.parse_record(Target::StructName, Target::StructField);
                continue;
            }
            if self.at_ident_text("union") {
                any = true;
                saw_type = true;
                self.parse_record(Target::UnionName, Target::UnionField);
                continue;
            }
            if self.at_ident_text("enum") {
                any = true;
                saw_type = true;
                self.parse_enum();
                continue;
            }
            if self.at_builtin_type() {
                any = true;
                saw_type = true;
                self.bump();
                continue;
            }
            if self.at_ident() && !saw_type && !self.at_statement_keyword() {
                any = true;
                saw_type = true;
                self.bump();
                continue;
            }
            break;
        }

        if any {
            Some(specs)
        } else {
            None
        }
    }

    fn parse_record(&mut self, name_target: Target, field_target: Target) {
        self.bump();
        if self.at_ident() && !self.is_keyword(self.peek_text()) {
            self.emit(name_target);
            self.bump();
        }
        if self.at("{") {
            self.bump();
            while !self.eof() && !self.at("}") {
                if !self.parse_struct_field(field_target) {
                    if self.at("}") {
                        break;
                    }
                    self.i += 1;
                }
            }
            if self.at("}") {
                self.bump();
            }
        }
    }

    fn parse_struct_field(&mut self, field_target: Target) -> bool {
        if self.at("}") {
            return false;
        }
        if self.parse_specifiers().is_none() {
            if self.at(";") {
                self.bump();
                return true;
            }
            return false;
        }
        if self.at(":") {
            self.bump();
            self.skip_expression_until(&[";", "}"]);
            if self.at(";") {
                self.bump();
            }
            return true;
        }
        if !self.at(";") {
            let decl = self.parse_declarator();
            if let Some(tok) = decl.name {
                self.push_tok(field_target, &tok);
            }
            if self.at(":") {
                self.bump();
                self.skip_expression_until(&[",", ";"]);
            }
            while self.at(",") {
                self.bump();
                let d = self.parse_declarator();
                if let Some(tok) = d.name {
                    self.push_tok(field_target, &tok);
                }
                if self.at(":") {
                    self.bump();
                    self.skip_expression_until(&[",", ";"]);
                }
            }
        }
        if self.at(";") {
            self.bump();
        }
        true
    }

    fn parse_enum(&mut self) {
        self.bump();
        if self.at_ident() && !self.is_keyword(self.peek_text()) {
            self.emit(Target::EnumName);
            self.bump();
        }
        if !self.at("{") {
            return;
        }
        self.bump();
        while !self.eof() && !self.at("}") {
            if self.at_ident() {
                self.emit(Target::EnumMember);
                self.bump();
            }
            if self.at("=") {
                self.bump();
                self.skip_expression_until(&[",", "}"]);
            }
            if self.at(",") {
                self.bump();
                continue;
            }
            break;
        }
        if self.at("}") {
            self.bump();
        }
    }

    fn parse_declarator(&mut self) -> Decl {
        let mut has_pointer = false;
        while self.at("*") {
            has_pointer = true;
            self.bump();
            self.skip_qualifiers();
        }

        let mut decl = self.parse_direct_declarator();
        if has_pointer && !decl.is_function {
            decl.has_pointer = true;
        }

        decl
    }

    fn parse_direct_declarator(&mut self) -> Decl {
        let mut decl = if self.at("(") && self.looks_like_grouped_declarator() {
            self.bump();
            let inner = self.parse_declarator();
            if self.at(")") {
                self.bump();
            }
            inner
        } else if self.at_ident() && !self.is_type_keyword(self.peek_text()) {
            let name = self.peek().cloned();
            self.bump();
            Decl {
                name,
                is_function: false,
                has_pointer: false,
                params: Vec::new(),
            }
        } else {
            Decl {
                name: None,
                is_function: false,
                has_pointer: false,
                params: Vec::new(),
            }
        };

        loop {
            if self.at("[") {
                self.skip_brackets();
                continue;
            }
            if self.at("(") {
                self.bump();
                let params = self.parse_parameter_list();
                if self.at(")") {
                    self.bump();
                }
                if decl.has_pointer {
                    decl.is_function = false;
                } else {
                    decl.is_function = true;
                    decl.params = params;
                }
                continue;
            }
            break;
        }
        decl
    }

    fn looks_like_grouped_declarator(&self) -> bool {
        let inner = match self.peek_nth(1) {
            Some(t) => t,
            None => return false,
        };
        if inner.kind == TokenKind::Punct && (inner.text == "*" || inner.text == "(") {
            return true;
        }
        if inner.kind != TokenKind::Ident {
            return false;
        }
        if self.is_type_keyword(&inner.text) {
            return false;
        }
        match self.peek_nth(2) {
            Some(t) if t.kind == TokenKind::Punct && (t.text == ")" || t.text == "[") => true,
            _ => false,
        }
    }

    fn parse_parameter_list(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if self.at(")") {
            return params;
        }
        if self.at_ident_text("void") {
            let next = self.peek_nth(1);
            if next.is_some_and(|t| t.kind == TokenKind::Punct && t.text == ")") {
                self.bump();
                return params;
            }
        }
        loop {
            if self.eof() || self.at(")") {
                break;
            }
            if self.at("...") {
                self.bump();
                break;
            }
            if self.at_ident()
                && self
                    .peek_nth(1)
                    .is_some_and(|t| t.kind == TokenKind::Punct && (t.text == "," || t.text == ")"))
            {
                // K&R identifier list: skip names without treating as typed params.
                self.bump();
                if self.at(",") {
                    self.bump();
                    continue;
                }
                break;
            }
            if self.parse_specifiers().is_none() {
                self.skip_expression_until(&[",", ")"]);
            } else {
                let d = self.parse_declarator();
                if let Some(name) = d.name {
                    params.push(Param { name });
                }
            }
            if self.at(",") {
                self.bump();
                continue;
            }
            break;
        }
        params
    }

    fn parse_compound(&mut self) {
        if !self.at("{") {
            return;
        }
        self.bump();
        while !self.eof() && !self.at("}") {
            if self.take_define() {
                continue;
            }
            if self.at("{") {
                self.parse_compound();
                continue;
            }
            if self.parse_declaration(Scope::Block) {
                continue;
            }
            self.skip_statement();
        }
        if self.at("}") {
            self.bump();
        }
    }

    fn skip_statement(&mut self) {
        if self.eof() {
            return;
        }
        if self.at("{") {
            self.parse_compound();
            return;
        }
        if self.at_ident_text("if") || self.at_ident_text("switch") || self.at_ident_text("while") {
            self.bump();
            self.skip_parens();
            self.skip_statement();
            if self.at_ident_text("else") {
                self.bump();
                self.skip_statement();
            }
            return;
        }
        if self.at_ident_text("for") {
            self.bump();
            if self.at("(") {
                self.bump();
                if !self.parse_declaration(Scope::Block) {
                    self.skip_expression_until(&[";", ")"]);
                    if self.at(";") {
                        self.bump();
                    }
                }
                self.skip_expression_until(&[";", ")"]);
                if self.at(";") {
                    self.bump();
                }
                self.skip_expression_until(&[")"]);
                if self.at(")") {
                    self.bump();
                }
            }
            self.skip_statement();
            return;
        }
        if self.at_ident_text("do") {
            self.bump();
            self.skip_statement();
            if self.at_ident_text("while") {
                self.bump();
                self.skip_parens();
            }
            if self.at(";") {
                self.bump();
            }
            return;
        }
        if self.at_ident()
            && self
                .peek_nth(1)
                .is_some_and(|t| t.kind == TokenKind::Punct && t.text == ":")
        {
            self.bump();
            self.bump();
            return;
        }
        self.skip_expression_until(&[";", "}"]);
        if self.at(";") {
            self.bump();
        }
    }

    fn skip_initializer(&mut self) {
        if !self.at("=") {
            return;
        }
        self.bump();
        if self.at("{") {
            self.skip_braces();
        } else {
            self.skip_expression_until(&[",", ";"]);
        }
    }

    fn emit_function(&mut self, decl: &Decl, is_static: bool) {
        if let Some(tok) = &decl.name {
            let target = if is_static {
                Target::FunctionStatic
            } else {
                Target::FunctionName
            };
            self.push_tok(target, tok);
        }
        for p in &decl.params {
            self.push_tok(Target::FunctionArguments, &p.name);
        }
    }

    fn emit_declarator(&mut self, decl: &Decl, specs: &Specs, scope: Scope) {
        if specs.is_typedef {
            if let Some(tok) = &decl.name {
                self.push_tok(Target::TypedefName, tok);
            }
            return;
        }
        if decl.is_function {
            self.emit_function(decl, specs.is_static);
            return;
        }
        if let Some(tok) = &decl.name {
            let target = if specs.is_static {
                Target::VariableStatic
            } else if scope == Scope::File {
                Target::VariableGlobal
            } else {
                Target::VariableLocal
            };
            self.push_tok(target, tok);
        }
    }

    fn emit(&mut self, target: Target) {
        if let Some(tok) = self.peek().cloned() {
            self.push_tok(target, &tok);
        }
    }

    fn push_tok(&mut self, target: Target, tok: &Token) {
        self.out.push(Extracted {
            target,
            name: tok.text.clone(),
            line: tok.line,
            column: tok.column,
        });
    }

    fn take_define(&mut self) -> bool {
        if self.peek().is_some_and(|t| t.kind == TokenKind::Define) {
            let tok = self.peek().cloned().unwrap();
            self.push_tok(Target::MacroName, &tok);
            self.bump();
            true
        } else {
            false
        }
    }

    fn is_decl_start(&self, scope: Scope) -> bool {
        if self.at_specifier_keyword() {
            return true;
        }
        if !self.at_ident() {
            return false;
        }
        if self.at_statement_keyword() {
            return false;
        }
        if scope == Scope::File {
            return true;
        }
        match self.peek_nth(1) {
            Some(t) if t.kind == TokenKind::Ident => true,
            Some(t) if t.kind == TokenKind::Punct && t.text == "*" => true,
            Some(t) if t.kind == TokenKind::Punct && t.text == "(" => self
                .peek_nth(2)
                .is_some_and(|n| n.kind == TokenKind::Punct && n.text == "*"),
            _ => false,
        }
    }

    fn skip_qualifiers(&mut self) {
        while self.at_ident_text("const")
            || self.at_ident_text("volatile")
            || self.at_ident_text("restrict")
            || self.at_ident_text("_Atomic")
            || self.at_ident_text("__restrict")
            || self.at_ident_text("__restrict__")
        {
            self.bump();
        }
    }

    fn skip_attributes(&mut self) {
        while self.at_ident_text("__attribute__")
            || self.at_ident_text("__attribute")
            || self.at_ident_text("__declspec")
        {
            self.bump();
            if self.at("(") {
                self.skip_parens();
            }
        }
    }

    fn skip_parens(&mut self) {
        self.skip_balanced("(", ")");
    }

    fn skip_brackets(&mut self) {
        self.skip_balanced("[", "]");
    }

    fn skip_braces(&mut self) {
        self.skip_balanced("{", "}");
    }

    fn skip_balanced(&mut self, open: &str, close: &str) {
        if !self.at(open) {
            return;
        }
        let mut depth = 0;
        while !self.eof() {
            if self.at(open) {
                depth += 1;
                self.bump();
            } else if self.at(close) {
                self.bump();
                depth -= 1;
                if depth == 0 {
                    return;
                }
            } else {
                self.bump();
            }
        }
    }

    fn skip_expression_until(&mut self, stops: &[&str]) {
        let mut paren = 0;
        let mut bracket = 0;
        let mut brace = 0;
        while !self.eof() {
            if paren == 0 && bracket == 0 && brace == 0 && self.at_any(stops) {
                return;
            }
            if self.at("(") {
                paren += 1;
                self.bump();
            } else if self.at(")") {
                if paren == 0 {
                    return;
                }
                paren -= 1;
                self.bump();
            } else if self.at("[") {
                bracket += 1;
                self.bump();
            } else if self.at("]") {
                if bracket == 0 {
                    return;
                }
                bracket -= 1;
                self.bump();
            } else if self.at("{") {
                brace += 1;
                self.bump();
            } else if self.at("}") {
                if brace == 0 {
                    return;
                }
                brace -= 1;
                self.bump();
            } else {
                self.bump();
            }
        }
    }

    fn eof(&self) -> bool {
        self.i >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.i)
    }

    fn peek_nth(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.i + n)
    }

    fn peek_text(&self) -> &str {
        self.peek().map(|t| t.text.as_str()).unwrap_or("")
    }

    fn at(&self, punct: &str) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Punct && t.text == punct)
    }

    fn at_any(&self, puncts: &[&str]) -> bool {
        puncts.iter().any(|p| self.at(p))
    }

    fn at_ident(&self) -> bool {
        self.peek().is_some_and(|t| t.kind == TokenKind::Ident)
    }

    fn at_ident_text(&self, s: &str) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Ident && t.text == s)
    }

    fn bump(&mut self) {
        if self.i < self.tokens.len() {
            self.i += 1;
        }
    }

    fn at_specifier_keyword(&self) -> bool {
        let t = match self.peek() {
            Some(t) if t.kind == TokenKind::Ident => t.text.as_str(),
            _ => return false,
        };
        self.is_type_keyword(t)
            || matches!(
                t,
                "typedef"
                    | "extern"
                    | "static"
                    | "auto"
                    | "register"
                    | "_Thread_local"
                    | "__thread"
                    | "inline"
                    | "_Noreturn"
                    | "const"
                    | "volatile"
                    | "restrict"
                    | "_Atomic"
                    | "struct"
                    | "union"
                    | "enum"
                    | "_Alignas"
                    | "alignas"
                    | "__attribute__"
                    | "__attribute"
                    | "__declspec"
                    | "__extension__"
                    | "__inline__"
                    | "__inline"
            )
    }

    fn at_builtin_type(&self) -> bool {
        self.peek()
            .is_some_and(|t| t.kind == TokenKind::Ident && self.is_builtin_type(&t.text))
    }

    fn at_statement_keyword(&self) -> bool {
        self.peek().is_some_and(|t| {
            t.kind == TokenKind::Ident
                && matches!(
                    t.text.as_str(),
                    "if" | "else"
                        | "for"
                        | "while"
                        | "do"
                        | "switch"
                        | "case"
                        | "default"
                        | "return"
                        | "break"
                        | "continue"
                        | "goto"
                        | "sizeof"
                        | "_Static_assert"
                        | "static_assert"
                        | "asm"
                        | "__asm__"
                )
        })
    }

    fn is_keyword(&self, s: &str) -> bool {
        self.is_type_keyword(s)
            || matches!(
                s,
                "if" | "else"
                    | "for"
                    | "while"
                    | "do"
                    | "switch"
                    | "case"
                    | "default"
                    | "return"
                    | "break"
                    | "continue"
                    | "goto"
                    | "sizeof"
                    | "typedef"
                    | "extern"
                    | "static"
                    | "auto"
                    | "register"
                    | "const"
                    | "volatile"
                    | "restrict"
                    | "inline"
            )
    }

    fn is_type_keyword(&self, s: &str) -> bool {
        self.is_builtin_type(s) || matches!(s, "struct" | "union" | "enum")
    }

    fn is_builtin_type(&self, s: &str) -> bool {
        matches!(
            s,
            "void"
                | "char"
                | "short"
                | "int"
                | "long"
                | "float"
                | "double"
                | "signed"
                | "unsigned"
                | "_Bool"
                | "bool"
                | "_Complex"
                | "_Imaginary"
                | "__builtin_va_list"
                | "size_t"
                | "ssize_t"
                | "ptrdiff_t"
                | "intptr_t"
                | "uintptr_t"
                | "int8_t"
                | "int16_t"
                | "int32_t"
                | "int64_t"
                | "uint8_t"
                | "uint16_t"
                | "uint32_t"
                | "uint64_t"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn names(src: &str) -> Vec<(Target, String)> {
        extract(&lex(src))
            .into_iter()
            .map(|e| (e.target, e.name))
            .collect()
    }

    #[test]
    fn extracts_core_constructs() {
        let src = r#"
#define MAX_SIZE 8
typedef struct Foo {
    int nextHop;
} foo_t;
enum color_t { RED, BLUE = 1 };
static int s_count;
int g_total;
int compute(int argc) {
    int local_n;
    static int s_inner;
    return local_n;
}
static void helper(void) {}
"#;
        let n = names(src);
        assert!(n.contains(&(Target::MacroName, "MAX_SIZE".into())));
        assert!(n.contains(&(Target::StructName, "Foo".into())));
        assert!(n.contains(&(Target::StructField, "nextHop".into())));
        assert!(n.contains(&(Target::TypedefName, "foo_t".into())));
        assert!(n.contains(&(Target::EnumName, "color_t".into())));
        assert!(n.contains(&(Target::EnumMember, "RED".into())));
        assert!(n.contains(&(Target::VariableStatic, "s_count".into())));
        assert!(n.contains(&(Target::VariableGlobal, "g_total".into())));
        assert!(n.contains(&(Target::FunctionName, "compute".into())));
        assert!(n.contains(&(Target::FunctionArguments, "argc".into())));
        assert!(n.contains(&(Target::VariableLocal, "local_n".into())));
        assert!(n.contains(&(Target::VariableStatic, "s_inner".into())));
        assert!(n.contains(&(Target::FunctionStatic, "helper".into())));
    }

    #[test]
    fn typedef_vs_struct_tag() {
        let n = names("typedef struct Foo { int x; } foo_t;\n");
        assert!(n.contains(&(Target::StructName, "Foo".into())));
        assert!(n.contains(&(Target::StructField, "x".into())));
        assert!(n.contains(&(Target::TypedefName, "foo_t".into())));
    }

    #[test]
    fn function_pointer_is_variable() {
        let n = names("int (*g_handler)(int x);\n");
        assert!(n.contains(&(Target::VariableGlobal, "g_handler".into())));
        assert!(!n.iter().any(|(t, _)| *t == Target::FunctionName));
    }
}
