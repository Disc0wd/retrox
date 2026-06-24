// ============================================================
// RetroX RNMDL Parser (GN-Z11)
// Converts token stream into an AST.
// Rust 1.95.0 | Edition 2021 | FROZEN
// ============================================================

use crate::lexer::{Token, TokenKind};
use crate::ast::{Node, DocumentHeader, ModuleDeclaration};

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl ParseError {
    pub fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        ParseError { message: message.into(), line, col }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Parse Error] Line {}, Col {}: {}", self.line, self.col, self.message)
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn get_attr(attrs: &[(String, String)], key: &str) -> Option<String> {
        attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    // Parse the full document
    pub fn parse(&mut self) -> Result<Node, ParseError> {
        // Expect <RNMDL> as first token
        let first = self.peek().clone();
        match &first.kind {
            TokenKind::RnmdlDecl => { self.advance(); }
            _ => return Err(ParseError::new(
                "Document must start with '<RNMDL>' on the first line",
                first.line, first.col,
            )),
        }

        // Parse header (declarations)
        let header = self.parse_header()?;

        // Parse content nodes
        let children = self.parse_children(None)?;

        Ok(Node::Document { header, children })
    }

    fn parse_header(&mut self) -> Result<DocumentHeader, ParseError> {
        let mut declarations = Vec::new();
        let mut implement_bugfixsets = false;

        loop {
            let tok = self.peek().clone();
            match &tok.kind {
                TokenKind::Declaration(version, modules) => {
                    declarations.push(ModuleDeclaration {
                        version: version.clone(),
                        modules: modules.clone(),
                        line: tok.line,
                    });
                    self.advance();
                }
                TokenKind::BugfixSets(enabled) => {
                    implement_bugfixsets = *enabled;
                    self.advance();
                }
                // Anything else ends the header
                _ => break,
            }
        }

        Ok(DocumentHeader { declarations, implement_bugfixsets })
    }

    // Parse children until we hit a matching close tag or EOF
    fn parse_children(&mut self, parent_tag: Option<&str>) -> Result<Vec<Node>, ParseError> {
        let mut children = Vec::new();

        loop {
            if self.is_eof() {
                if let Some(tag) = parent_tag {
                    let tok = self.peek();
                    return Err(ParseError::new(
                        format!("Unexpected end of file: expected '[/{}]'", tag),
                        tok.line, tok.col,
                    ));
                }
                break;
            }

            let tok = self.peek().clone();

            match &tok.kind {
                // Close tag - end of this children list
                TokenKind::CloseTag(name) => {
                    if let Some(parent) = parent_tag {
                        if name == parent {
                            self.advance(); // consume the close tag
                            break;
                        } else {
                            return Err(ParseError::new(
                                format!(
                                    "Mismatched tags: expected '[/{}]' but found '[/{}]'",
                                    parent, name
                                ),
                                tok.line, tok.col,
                            ));
                        }
                    } else {
                        return Err(ParseError::new(
                            format!("Unexpected closing tag '[/{}]' with no matching opening tag", name),
                            tok.line, tok.col,
                        ));
                    }
                }

                // Open tag
                TokenKind::OpenTag(name, attrs) => {
                    let name = name.clone();
                    let attrs = attrs.clone();
                    let line = tok.line;
                    self.advance();

                    let node = self.parse_open_tag(&name, &attrs, line)?;
                    children.push(node);
                }

                // Self-closing tag
                TokenKind::SelfCloseTag(name, attrs) => {
                    let name = name.clone();
                    let attrs = attrs.clone();
                    let line = tok.line;
                    self.advance();

                    let node = self.parse_self_closing_tag(&name, &attrs, line)?;
                    children.push(node);
                }

                // Text
                TokenKind::Text(text) => {
                    // Text at top level (inside a container/section parent) is accumulated
                    let t = text.clone();
                    let line = tok.line;
                    self.advance();
                    children.push(Node::Paragraph { text: t, line });
                }

                // Comment
                TokenKind::Comment(content) => {
                    let c = content.clone();
                    let line = tok.line;
                    self.advance();
                    children.push(Node::Comment { content: c, line });
                }

                // Declaration tokens in content = error
                TokenKind::Declaration(_, _) | TokenKind::BugfixSets(_) | TokenKind::RnmdlDecl => {
                    return Err(ParseError::new(
                        "Declaration statements must appear at the top of the document, before content",
                        tok.line, tok.col,
                    ));
                }

                TokenKind::Eof => break,
            }
        }

        Ok(children)
    }

    fn parse_open_tag(
        &mut self,
        name: &str,
        attrs: &[(String, String)],
        line: usize,
    ) -> Result<Node, ParseError> {
        match name {
            "container" => {
                let id = Self::get_attr(attrs, "id");
                let children = self.parse_children(Some("container"))?;
                Ok(Node::Container { id, children, line })
            }

            "section" => {
                let id = Self::get_attr(attrs, "id");
                let children = self.parse_children(Some("section"))?;
                Ok(Node::Section { id, children, line })
            }

            "h1" | "h2" | "h3" => {
                let level: u8 = name[1..].parse().unwrap_or(1);
                let text = self.parse_text_content(name)?;
                Ok(Node::Heading { level, text, line })
            }

            "heading" => {
                let level: u8 = Self::get_attr(attrs, "level")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                let text = self.parse_text_content("heading")?;
                Ok(Node::Heading { level, text, line })
            }

            "paragraph" | "p" => {
                let text = self.parse_text_content(name)?;
                Ok(Node::Paragraph { text, line })
            }

            unknown => {
                Err(ParseError::new(
                    format!(
                        "Unknown tag '[{}]'. Valid GN-Z11 tags: container, section, h1, h2, h3, heading, paragraph, p, image",
                        unknown
                    ),
                    line, 0,
                ))
            }
        }
    }

    fn parse_self_closing_tag(
        &mut self,
        name: &str,
        attrs: &[(String, String)],
        line: usize,
    ) -> Result<Node, ParseError> {
        match name {
            "image" => {
                let path = Self::get_attr(attrs, "path").unwrap_or_default();
                let alt = Self::get_attr(attrs, "alt").unwrap_or_default();
                Ok(Node::Image { path, alt, line })
            }
            unknown => Err(ParseError::new(
                format!("Unknown self-closing tag '[{}]'", unknown),
                line, 0,
            )),
        }
    }

    // Parses text content inside a tag like [h1]...[/h1]
    fn parse_text_content(&mut self, close_tag: &str) -> Result<String, ParseError> {
        let mut parts = Vec::new();

        loop {
            let tok = self.peek().clone();
            match &tok.kind {
                TokenKind::Text(t) => {
                    parts.push(t.clone());
                    self.advance();
                }
                TokenKind::CloseTag(name) if name == close_tag => {
                    self.advance();
                    break;
                }
                TokenKind::CloseTag(name) => {
                    return Err(ParseError::new(
                        format!("Expected '[/{}]' but found '[/{}]'", close_tag, name),
                        tok.line, tok.col,
                    ));
                }
                TokenKind::Eof => {
                    return Err(ParseError::new(
                        format!("Unexpected end of file inside '[{}]'", close_tag),
                        tok.line, tok.col,
                    ));
                }
                _ => {
                    return Err(ParseError::new(
                        format!(
                            "Tags like '[{}]' can only contain text, not nested tags",
                            close_tag
                        ),
                        tok.line, tok.col,
                    ));
                }
            }
        }

        Ok(parts.join(" ").trim().to_string())
    }
}
