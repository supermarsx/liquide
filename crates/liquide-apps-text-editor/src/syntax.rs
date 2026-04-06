//! Syntax highlighting with language definitions.

/// A syntax token type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Type,
    Function,
    String,
    Number,
    Comment,
    Operator,
    Punctuation,
    Identifier,
    Whitespace,
    Unknown,
}

/// A highlighted token span.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub len: usize,
}

impl Token {
    #[must_use]
    pub fn new(kind: TokenKind, start: usize, len: usize) -> Self {
        Self { kind, start, len }
    }

    #[must_use]
    pub fn end(&self) -> usize { self.start + self.len }
}

/// A language definition for syntax highlighting.
#[derive(Debug, Clone)]
pub struct Language {
    pub name: String,
    pub extensions: Vec<String>,
    pub keywords: Vec<String>,
    pub types: Vec<String>,
    pub line_comment: String,
    pub block_comment_start: String,
    pub block_comment_end: String,
    pub string_delimiters: Vec<char>,
}

impl Language {
    /// Detect a language from a file extension.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Language> {
        match ext {
            "rs" => Some(Self::rust()),
            "py" => Some(Self::python()),
            "js" | "ts" | "jsx" | "tsx" => Some(Self::javascript()),
            "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" => Some(Self::c()),
            "toml" => Some(Self::toml()),
            // Return None for known text formats (no keyword highlighting needed).
            // This keeps them as Plain Text but still recognized.
            _ => None,
        }
    }

    fn rust() -> Self {
        Self {
            name: "Rust".into(),
            extensions: vec!["rs".into()],
            keywords: vec![
                "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl",
                "trait", "type", "const", "static", "if", "else", "match", "for",
                "while", "loop", "return", "break", "continue", "where", "async",
                "await", "move", "ref", "self", "super", "crate", "as", "in",
                "unsafe", "extern", "dyn",
            ].into_iter().map(String::from).collect(),
            types: vec![
                "bool", "u8", "u16", "u32", "u64", "u128", "usize",
                "i8", "i16", "i32", "i64", "i128", "isize",
                "f32", "f64", "char", "str", "String", "Vec", "Option",
                "Result", "Box", "Rc", "Arc", "Self",
            ].into_iter().map(String::from).collect(),
            line_comment: "//".into(),
            block_comment_start: "/*".into(),
            block_comment_end: "*/".into(),
            string_delimiters: vec!['"'],
        }
    }

    fn python() -> Self {
        Self {
            name: "Python".into(),
            extensions: vec!["py".into()],
            keywords: vec![
                "def", "class", "if", "elif", "else", "for", "while", "return",
                "import", "from", "as", "try", "except", "finally", "with",
                "yield", "lambda", "pass", "break", "continue", "raise", "in",
                "is", "not", "and", "or", "True", "False", "None", "async", "await",
            ].into_iter().map(String::from).collect(),
            types: vec![
                "int", "float", "str", "bool", "list", "dict", "tuple", "set",
                "bytes", "object", "type",
            ].into_iter().map(String::from).collect(),
            line_comment: "#".into(),
            block_comment_start: "\"\"\"".into(),
            block_comment_end: "\"\"\"".into(),
            string_delimiters: vec!['"', '\''],
        }
    }

    fn javascript() -> Self {
        Self {
            name: "JavaScript".into(),
            extensions: vec!["js".into(), "ts".into(), "jsx".into(), "tsx".into()],
            keywords: vec![
                "function", "var", "let", "const", "if", "else", "for", "while",
                "do", "switch", "case", "break", "continue", "return", "class",
                "extends", "new", "this", "super", "import", "export", "default",
                "from", "try", "catch", "finally", "throw", "async", "await",
                "yield", "typeof", "instanceof", "in", "of", "delete", "void",
            ].into_iter().map(String::from).collect(),
            types: vec![
                "string", "number", "boolean", "object", "undefined", "null",
                "symbol", "bigint", "any", "void", "never", "unknown",
            ].into_iter().map(String::from).collect(),
            line_comment: "//".into(),
            block_comment_start: "/*".into(),
            block_comment_end: "*/".into(),
            string_delimiters: vec!['"', '\'', '`'],
        }
    }

    fn c() -> Self {
        Self {
            name: "C".into(),
            extensions: vec!["c".into(), "h".into()],
            keywords: vec![
                "auto", "break", "case", "char", "const", "continue", "default",
                "do", "double", "else", "enum", "extern", "float", "for", "goto",
                "if", "int", "long", "register", "return", "short", "signed",
                "sizeof", "static", "struct", "switch", "typedef", "union",
                "unsigned", "void", "volatile", "while",
            ].into_iter().map(String::from).collect(),
            types: vec![
                "int", "char", "float", "double", "void", "long", "short",
                "unsigned", "signed", "size_t", "FILE",
            ].into_iter().map(String::from).collect(),
            line_comment: "//".into(),
            block_comment_start: "/*".into(),
            block_comment_end: "*/".into(),
            string_delimiters: vec!['"'],
        }
    }

    fn toml() -> Self {
        Self {
            name: "TOML".into(),
            extensions: vec!["toml".into()],
            keywords: vec!["true".into(), "false".into()],
            types: Vec::new(),
            line_comment: "#".into(),
            block_comment_start: String::new(),
            block_comment_end: String::new(),
            string_delimiters: vec!['"'],
        }
    }
}

/// Simple line-by-line tokenizer.
pub struct Highlighter {
    language: Option<Language>,
}

impl Highlighter {
    #[must_use]
    pub fn new(language: Option<Language>) -> Self {
        Self { language }
    }

    /// Detect the language from a file path and create a highlighter.
    #[must_use]
    pub fn detect(path: &std::path::Path) -> Self {
        let lang = path.extension()
            .and_then(|e| e.to_str())
            .and_then(|ext| Language::from_extension(ext));
        Self::new(lang)
    }

    /// Set the language.
    pub fn set_language(&mut self, language: Option<Language>) {
        self.language = language;
    }

    /// Get the language name.
    #[must_use]
    pub fn language_name(&self) -> &str {
        self.language.as_ref().map_or("Plain Text", |l| &l.name)
    }

    /// Tokenize a single line.
    #[must_use]
    pub fn tokenize_line(&self, line: &str) -> Vec<Token> {
        let Some(lang) = &self.language else {
            return vec![Token::new(TokenKind::Unknown, 0, line.len())];
        };

        let mut tokens = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        // Check for line comment.
        if !lang.line_comment.is_empty() && line.trim_start().starts_with(&lang.line_comment) {
            return vec![Token::new(TokenKind::Comment, 0, line.len())];
        }

        while i < len {
            let ch = chars[i];

            // Whitespace.
            if ch.is_whitespace() {
                let start = i;
                while i < len && chars[i].is_whitespace() { i += 1; }
                tokens.push(Token::new(TokenKind::Whitespace, start, i - start));
                continue;
            }

            // Strings.
            if lang.string_delimiters.contains(&ch) {
                let start = i;
                i += 1;
                while i < len && chars[i] != ch {
                    if chars[i] == '\\' { i += 1; } // skip escaped.
                    i += 1;
                }
                if i < len { i += 1; } // closing quote.
                tokens.push(Token::new(TokenKind::String, start, i - start));
                continue;
            }

            // Numbers.
            if ch.is_ascii_digit() {
                let start = i;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::new(TokenKind::Number, start, i - start));
                continue;
            }

            // Identifiers / keywords.
            if ch.is_alphabetic() || ch == '_' {
                let start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let kind = if lang.keywords.iter().any(|k| k == &word) {
                    TokenKind::Keyword
                } else if lang.types.iter().any(|t| t == &word) {
                    TokenKind::Type
                } else {
                    TokenKind::Identifier
                };
                tokens.push(Token::new(kind, start, i - start));
                continue;
            }

            // Operators and punctuation.
            let kind = match ch {
                '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' => TokenKind::Operator,
                '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '.' | '@' | '#' => TokenKind::Punctuation,
                _ => TokenKind::Unknown,
            };
            tokens.push(Token::new(kind, i, 1));
            i += 1;
        }

        tokens
    }
}
