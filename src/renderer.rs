// ============================================================
// RetroX RNMDL Terminal Renderer (GN-Z11)
// Renders an AST to formatted terminal output.
// Rust 1.95.0 | Edition 2021 | FROZEN
// ============================================================

use crate::ast::{Node, DocumentHeader};

// ANSI escape codes for terminal formatting
const RESET:     &str = "\x1b[0m";
const BOLD:      &str = "\x1b[1m";
const DIM:       &str = "\x1b[2m";
const UNDERLINE: &str = "\x1b[4m";
const CYAN:      &str = "\x1b[36m";
const YELLOW:    &str = "\x1b[33m";
const GREEN:     &str = "\x1b[32m";
const MAGENTA:   &str = "\x1b[35m";
const BLUE:      &str = "\x1b[34m";
const WHITE:     &str = "\x1b[97m";
const DARK_GRAY: &str = "\x1b[90m";

pub struct Renderer {
    indent_size: usize,
    show_comments: bool,
    show_meta: bool,
    width: usize,
}

impl Renderer {
    pub fn new() -> Self {
        Renderer {
            indent_size: 2,
            show_comments: false,
            show_meta: true,
            width: 80,
        }
    }

    pub fn with_comments(mut self, show: bool) -> Self {
        self.show_comments = show;
        self
    }

    pub fn with_meta(mut self, show: bool) -> Self {
        self.show_meta = show;
        self
    }

    fn horizontal_rule(&self, ch: char) -> String {
        ch.to_string().repeat(self.width)
    }

    fn wrap_text(&self, text: &str, indent: usize) -> String {
        let max_width = self.width.saturating_sub(indent);
        let indent_str = " ".repeat(indent);
        let mut result = String::new();
        let mut line_len = 0;
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            if line_len + word.len() + 1 > max_width && line_len > 0 {
                result.push_str(&indent_str);
                result.push_str(&current_line);
                result.push('\n');
                current_line = word.to_string();
                line_len = word.len();
            } else {
                if line_len > 0 {
                    current_line.push(' ');
                    line_len += 1;
                }
                current_line.push_str(word);
                line_len += word.len();
            }
        }

        if !current_line.is_empty() {
            result.push_str(&indent_str);
            result.push_str(&current_line);
        }

        result
    }

    pub fn render(&self, node: &Node) -> String {
        let mut output = String::new();

        match node {
            Node::Document { header, children } => {
                if self.show_meta {
                    output.push_str(&self.render_header(header));
                }
                for child in children {
                    output.push_str(&self.render_node(child, 0));
                }
                if self.show_meta {
                    output.push_str(&format!(
                        "\n{}{}{}{}\n",
                        DARK_GRAY, DIM,
                        self.horizontal_rule('─'),
                        RESET
                    ));
                    output.push_str(&format!(
                        "{}{}  RetroX RNMDL GN-Z11 | Rendered by RetroX v0.0.0{}\n",
                        DARK_GRAY, DIM, RESET
                    ));
                }
            }
            other => {
                output.push_str(&self.render_node(other, 0));
            }
        }

        output
    }

    fn render_header(&self, header: &DocumentHeader) -> String {
        let mut out = String::new();

        out.push_str(&format!("{}{}{}{}\n", CYAN, BOLD, self.horizontal_rule('═'), RESET));
        out.push_str(&format!(
            "{}{}  ◈ RetroX RNMDL Document  ◈  GN-Z11 (v0.0.0){}\n",
            CYAN, BOLD, RESET
        ));
        out.push_str(&format!("{}{}{}{}\n", CYAN, BOLD, self.horizontal_rule('═'), RESET));

        out.push_str(&format!("{}{}  Modules:{}\n", DARK_GRAY, DIM, RESET));
        for decl in &header.declarations {
            out.push_str(&format!(
                "{}{}    from {} declare {}{}\n",
                DARK_GRAY, DIM,
                decl.version,
                decl.modules.join(", "),
                RESET
            ));
        }

        if header.implement_bugfixsets {
            out.push_str(&format!(
                "{}{}    implement_bugfixsets = True{}\n",
                DARK_GRAY, DIM, RESET
            ));
        }

        out.push_str(&format!("{}{}{}{}\n\n", DARK_GRAY, DIM, self.horizontal_rule('─'), RESET));
        out
    }

    fn render_node(&self, node: &Node, depth: usize) -> String {
        match node {
            Node::Container { id, children, .. } => {
                self.render_container(id, children, depth)
            }
            Node::Section { id, children, .. } => {
                self.render_section(id, children, depth)
            }
            Node::Heading { level, text, .. } => {
                self.render_heading(*level, text, depth)
            }
            Node::Paragraph { text, .. } => {
                self.render_paragraph(text, depth)
            }
            Node::Image { path, alt, .. } => {
                self.render_image(path, alt, depth)
            }
            Node::Comment { content, .. } => {
                if self.show_comments {
                    self.render_comment(content, depth)
                } else {
                    String::new()
                }
            }
            Node::Document { .. } => String::new(),
        }
    }

    fn render_container(&self, id: &Option<String>, children: &[Node], depth: usize) -> String {
        let mut out = String::new();
        let indent = " ".repeat(depth * self.indent_size);

        if self.show_meta {
            let id_str = id.as_deref().unwrap_or("anonymous");
            out.push_str(&format!(
                "{}{}{}[container: {}]{}\n",
                indent, DARK_GRAY, DIM, id_str, RESET
            ));
        }

        for child in children {
            out.push_str(&self.render_node(child, depth + 1));
        }

        if self.show_meta {
            out.push('\n');
        }

        out
    }

    fn render_section(&self, id: &Option<String>, children: &[Node], depth: usize) -> String {
        let mut out = String::new();
        let indent = " ".repeat(depth * self.indent_size);

        if self.show_meta {
            let id_str = id.as_deref().unwrap_or("anonymous");
            out.push_str(&format!(
                "\n{}{}{}── section: {} ──{}\n",
                indent, DARK_GRAY, DIM, id_str, RESET
            ));
        } else {
            out.push('\n');
        }

        for child in children {
            out.push_str(&self.render_node(child, depth + 1));
        }

        out
    }

    fn render_heading(&self, level: u8, text: &str, depth: usize) -> String {
        let indent = " ".repeat(depth * self.indent_size);

        match level {
            1 => {
                let bar = self.horizontal_rule('═');
                format!(
                    "\n{}{}{}{}{}\n{}{}{}{}{}\n{}{}{}{}{}\n\n",
                    indent, YELLOW, BOLD, bar, RESET,
                    indent, YELLOW, BOLD, text.to_uppercase(), RESET,
                    indent, YELLOW, BOLD, bar, RESET,
                )
            }
            2 => {
                let bar = "─".repeat(text.len() + 4);
                format!(
                    "\n{}{}{}{}{}\n{}{}{}  {} {}\n{}{}{}{}{}\n\n",
                    indent, GREEN, BOLD, bar, RESET,
                    indent, GREEN, BOLD, text, RESET,
                    indent, GREEN, BOLD, bar, RESET,
                )
            }
            3 => {
                format!(
                    "\n{}{}{}▸ {}{}\n\n",
                    indent, WHITE, BOLD, text, RESET
                )
            }
            _ => format!("\n{}{}{}{}{}\n\n", indent, BOLD, WHITE, text, RESET),
        }
    }

    fn render_paragraph(&self, text: &str, depth: usize) -> String {
        let indent_size = depth * self.indent_size;
        let wrapped = self.wrap_text(text, indent_size);
        format!("{}\n\n", wrapped)
    }

    fn render_image(&self, path: &str, alt: &str, depth: usize) -> String {
        let indent = " ".repeat(depth * self.indent_size);
        format!(
            "{}{}{}┌─────────────────────────────────┐{}\n\
             {}{}{}│  🖼  IMAGE                        │{}\n\
             {}{}{}│  Path: {:<25} │{}\n\
             {}{}{}│  Alt:  {:<25} │{}\n\
             {}{}{}└─────────────────────────────────┘{}\n\n",
            indent, MAGENTA, BOLD, RESET,
            indent, MAGENTA, BOLD, RESET,
            indent, MAGENTA, BOLD,
            Self::truncate(path, 25), RESET,
            indent, MAGENTA, BOLD,
            Self::truncate(alt, 25), RESET,
            indent, MAGENTA, BOLD, RESET,
        )
    }

    fn render_comment(&self, content: &str, depth: usize) -> String {
        let indent = " ".repeat(depth * self.indent_size);
        format!(
            "{}{}{}<!-- {} -->{}\n",
            indent, DARK_GRAY, DIM, content, RESET
        )
    }

    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            format!("{}…", &s[..max - 1])
        }
    }
}
