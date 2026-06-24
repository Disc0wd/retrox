// ============================================================
// RetroX RNMDL Lexer (GN-Z11)
// Tokenizes raw .rnmdl source into a stream of tokens.
// Rust 1.95.0 | Edition 2021 | FROZEN
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    RnmdlDecl,                              // <RNMDL>
    Declaration(String, Vec<String>),       // from VERSION declare mod1, mod2
    BugfixSets(bool),                       // implement_bugfixsets = True/False
    OpenTag(String, Vec<(String, String)>), // [tagname attr="val"]
    CloseTag(String),                       // [/tagname]
    SelfCloseTag(String, Vec<(String, String)>), // [image path="x" alt="y"]
    Text(String),                           // raw text content
    Comment(String),                        // <!-- ... -->
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Token { kind, line, col }
    }
}

#[derive(Debug)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl LexError {
    pub fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        LexError { message: message.into(), line, col }
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Lex Error] Line {}, Col {}: {}", self.line, self.col, self.message)
    }
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            input: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    fn skip_whitespace_inline(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t') | Some('\r')) {
            self.advance();
        }
    }

    fn pos_info(&self) -> (usize, usize) {
        (self.line, self.col)
    }

    fn starts_with(&self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        if self.pos + chars.len() > self.input.len() {
            return false;
        }
        chars.iter().enumerate().all(|(i, &c)| {
            self.input[self.pos + i].to_ascii_lowercase() == c.to_ascii_lowercase()
        })
    }

    fn read_word(&mut self) -> String {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '-' || c == '.' || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        s
    }

    fn read_quoted_string(&mut self) -> Result<String, LexError> {
        let (line, col) = self.pos_info();
        match self.advance() {
            Some('"') => {}
            _ => return Err(LexError::new("Expected opening '\"' for attribute value", line, col)),
        }
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some(c) => s.push(c),
                None => return Err(LexError::new("Unterminated string literal", line, col)),
            }
        }
        Ok(s)
    }

    fn read_attributes(&mut self) -> Result<Vec<(String, String)>, LexError> {
        let mut attrs = Vec::new();
        loop {
            self.skip_whitespace_inline();
            if matches!(self.peek(), Some(']') | None) {
                break;
            }
            let (line, col) = self.pos_info();
            let key = self.read_word();
            if key.is_empty() {
                return Err(LexError::new("Expected attribute name", line, col));
            }
            self.skip_whitespace_inline();
            let (l2, c2) = self.pos_info();
            if self.advance() != Some('=') {
                return Err(LexError::new(
                    format!("Expected '=' after attribute '{}'", key), l2, c2,
                ));
            }
            self.skip_whitespace_inline();
            let val = self.read_quoted_string()?;
            attrs.push((key, val));
        }
        Ok(attrs)
    }

    fn read_tag(&mut self) -> Result<Token, LexError> {
        let (line, col) = self.pos_info();
        self.advance(); // consume '['

        if self.peek() == Some('/') {
            self.advance();
            let name = self.read_word().to_lowercase();
            if name.is_empty() {
                return Err(LexError::new("Expected tag name after '[/'", line, col));
            }
            self.skip_whitespace_inline();
            if self.advance() != Some(']') {
                return Err(LexError::new(
                    format!("Expected ']' to close '[/{}]'", name), line, col,
                ));
            }
            return Ok(Token::new(TokenKind::CloseTag(name), line, col));
        }

        let name = self.read_word().to_lowercase();
        if name.is_empty() {
            return Err(LexError::new("Expected tag name after '['", line, col));
        }

        let attrs = self.read_attributes()?;

        self.skip_whitespace_inline();
        if self.advance() != Some(']') {
            return Err(LexError::new(
                format!("Expected ']' to close '[{}]'", name), line, col,
            ));
        }

        let self_closing = matches!(name.as_str(), "image");

        if self_closing {
            Ok(Token::new(TokenKind::SelfCloseTag(name, attrs), line, col))
        } else {
            Ok(Token::new(TokenKind::OpenTag(name, attrs), line, col))
        }
    }

    fn read_comment(&mut self) -> Result<Token, LexError> {
        let (line, col) = self.pos_info();
        for _ in 0..4 { self.advance(); }
        let mut content = String::new();
        loop {
            if self.starts_with("-->") {
                for _ in 0..3 { self.advance(); }
                break;
            }
            match self.advance() {
                Some(c) => content.push(c),
                None => return Err(LexError::new("Unterminated comment", line, col)),
            }
        }
        Ok(Token::new(TokenKind::Comment(content.trim().to_string()), line, col))
    }

    fn read_declaration_line(&mut self) -> Result<Option<Token>, LexError> {
        let (line, col) = self.pos_info();
        self.skip_whitespace_inline();

        if self.starts_with("<rnmdl>") {
            for _ in 0..7 { self.advance(); }
            return Ok(Some(Token::new(TokenKind::RnmdlDecl, line, col)));
        }

        if self.starts_with("from ") {
            for _ in 0..5 { self.advance(); }
            self.skip_whitespace_inline();
            let version = self.read_word().to_lowercase();
            if version.is_empty() {
                return Err(LexError::new("Expected version name after 'from'", line, col));
            }
            self.skip_whitespace_inline();
            if !self.starts_with("declare") {
                return Err(LexError::new(
                    format!("Expected 'declare' after version '{}'", version),
                    line, self.col,
                ));
            }
            for _ in 0..7 { self.advance(); }
            self.skip_whitespace_inline();

            let mut modules = Vec::new();
            loop {
                self.skip_whitespace_inline();
                let m = self.read_word().to_lowercase();
                if m.is_empty() {
                    return Err(LexError::new("Expected module name in declaration", line, self.col));
                }
                modules.push(m);
                self.skip_whitespace_inline();
                if self.peek() == Some(',') {
                    self.advance();
                } else {
                    break;
                }
            }
            return Ok(Some(Token::new(TokenKind::Declaration(version, modules), line, col)));
        }

        if self.starts_with("implement_bugfixsets") {
            self.read_word();
            self.skip_whitespace_inline();
            if self.peek() == Some('=') { self.advance(); }
            self.skip_whitespace_inline();
            let val = self.read_word().to_lowercase();
            let enabled = val == "true";
            return Ok(Some(Token::new(TokenKind::BugfixSets(enabled), line, col)));
        }

        Ok(None)
    }

    fn consume_line(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' { self.advance(); break; }
            self.advance();
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        let mut in_header = true;

        while self.pos < self.input.len() {
            if matches!(self.peek(), Some('\n') | Some('\r')) {
                self.advance();
                continue;
            }

            if self.starts_with("<!--") {
                let tok = self.read_comment()?;
                tokens.push(tok);
                continue;
            }

            if in_header {
                self.skip_whitespace_inline();
                if self.peek() == Some('[') {
                    in_header = false;
                    continue;
                }
                if let Some(tok) = self.read_declaration_line()? {
                    tokens.push(tok);
                    self.consume_line();
                    continue;
                }
                self.consume_line();
                continue;
            }

            if self.peek() == Some('[') {
                let tok = self.read_tag()?;
                tokens.push(tok);
                continue;
            }

            let (line, col) = self.pos_info();
            let mut text = String::new();
            while let Some(c) = self.peek() {
                if c == '[' || c == '\n' { break; }
                if self.starts_with("<!--") { break; }
                text.push(c);
                self.advance();
            }
            if self.peek() == Some('\n') { self.advance(); }
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                tokens.push(Token::new(TokenKind::Text(trimmed), line, col));
            }
        }

        tokens.push(Token::new(TokenKind::Eof, self.line, self.col));
        Ok(tokens)
    }
}
