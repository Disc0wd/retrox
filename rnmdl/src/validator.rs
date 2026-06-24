// ============================================================
// RetroX RNMDL Validator (GN-Z11)
// Strict validation. Invalid = rejected, not degraded.
// Rust 1.95.0 | Edition 2021 | FROZEN
// ============================================================

use crate::ast::{Node, DocumentHeader, RnmdlVersion};
use std::collections::HashSet;

#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
    pub line: usize,
}

impl ValidationError {
    pub fn new(message: impl Into<String>, line: usize) -> Self {
        ValidationError { message: message.into(), line }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Validation Error] Line {}: {}", self.line, self.message)
    }
}

pub struct Validator;

impl Validator {
    pub fn new() -> Self { Validator }

    pub fn validate(&self, document: &Node) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        match document {
            Node::Document { header, children } => {
                self.validate_header(header, &mut errors);
                let mut ids: HashSet<String> = HashSet::new();
                self.validate_children(children, &mut errors, &mut ids);
            }
            _ => errors.push(ValidationError::new("Root node must be a Document", 0)),
        }
        errors
    }

    fn validate_header(&self, header: &DocumentHeader, errors: &mut Vec<ValidationError>) {
        if header.declarations.is_empty() {
            errors.push(ValidationError::new(
                "Document must have at least one module declaration. Example: 'from gn-z11 declare text, images'",
                1,
            ));
            return;
        }

        let mut declared_modules: HashSet<String> = HashSet::new();

        for decl in &header.declarations {
            let version = RnmdlVersion::from_str(&decl.version);

            if matches!(version, RnmdlVersion::Unknown(_)) {
                errors.push(ValidationError::new(
                    format!(
                        "Unknown version '{}'. Known versions: gn-z11, milky-way, sol, luna, andromeda, pallas, halley, ceres, chiron",
                        decl.version
                    ),
                    decl.line,
                ));
            }

            let available = version.available_modules();

            for module in &decl.modules {
                if !available.is_empty() && !available.contains(&module.as_str()) {
                    errors.push(ValidationError::new(
                        format!(
                            "Module '{}' does not exist in {}. Available: {}",
                            module,
                            version.display_name(),
                            available.join(", ")
                        ),
                        decl.line,
                    ));
                }

                if declared_modules.contains(module) {
                    errors.push(ValidationError::new(
                        format!(
                            "Module '{}' is declared more than once. Each module may only be declared from one version.",
                            module
                        ),
                        decl.line,
                    ));
                } else {
                    declared_modules.insert(module.clone());
                }
            }
        }
    }

    fn validate_children(
        &self,
        children: &[Node],
        errors: &mut Vec<ValidationError>,
        ids: &mut HashSet<String>,
    ) {
        for child in children {
            self.validate_node(child, errors, ids);
        }
    }

    fn validate_node(
        &self,
        node: &Node,
        errors: &mut Vec<ValidationError>,
        ids: &mut HashSet<String>,
    ) {
        match node {
            Node::Container { id, children, line } => {
                self.validate_id(id, *line, errors, ids);
                self.validate_children(children, errors, ids);
            }

            Node::Section { id, children, line } => {
                self.validate_id(id, *line, errors, ids);
                self.validate_children(children, errors, ids);
            }

            Node::Heading { level, text, line } => {
                if *level < 1 || *level > 3 {
                    errors.push(ValidationError::new(
                        format!("Heading level must be 1, 2, or 3. Got '{}'", level),
                        *line,
                    ));
                }
                if text.trim().is_empty() {
                    errors.push(ValidationError::new("Heading must not be empty", *line));
                }
            }

            Node::Paragraph { text, line } => {
                if text.trim().is_empty() {
                    errors.push(ValidationError::new("Paragraph must not be empty", *line));
                }
            }

            Node::Image { path, alt, line } => {
                if path.trim().is_empty() {
                    errors.push(ValidationError::new(
                        "Image 'path' attribute is required and must not be empty",
                        *line,
                    ));
                }

                let alt_trimmed = alt.trim();
                if alt_trimmed.is_empty() {
                    errors.push(ValidationError::new(
                        "Image 'alt' attribute is required. Provide a meaningful description of the image.",
                        *line,
                    ));
                } else if alt_trimmed.len() < 5 {
                    errors.push(ValidationError::new(
                        format!(
                            "Image alt text '{}' is too short (minimum 5 characters). Describe what the image shows.",
                            alt_trimmed
                        ),
                        *line,
                    ));
                } else if self.is_generic_alt(alt_trimmed) {
                    errors.push(ValidationError::new(
                        format!(
                            "Image alt text '{}' is not meaningful. Describe the actual image content.",
                            alt_trimmed
                        ),
                        *line,
                    ));
                }

                let lower = path.to_lowercase();
                let valid_ext = lower.ends_with(".jpg")
                    || lower.ends_with(".jpeg")
                    || lower.ends_with(".png")
                    || lower.ends_with(".gif")
                    || lower.ends_with(".webp");

                if !path.trim().is_empty() && !valid_ext {
                    errors.push(ValidationError::new(
                        format!(
                            "Image '{}' has unsupported format. Supported formats: jpg, jpeg, png, gif, webp",
                            path
                        ),
                        *line,
                    ));
                }
            }

            Node::Comment { .. } => {}

            Node::Document { .. } => {
                errors.push(ValidationError::new("Nested document nodes are not allowed", 0));
            }
        }
    }

    fn validate_id(
        &self,
        id: &Option<String>,
        line: usize,
        errors: &mut Vec<ValidationError>,
        ids: &mut HashSet<String>,
    ) {
        if let Some(id_str) = id {
            if !id_str.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
                errors.push(ValidationError::new(
                    format!("ID '{}' must start with a letter", id_str),
                    line,
                ));
            }

            if !id_str.chars().all(|c| c.is_alphanumeric() || c == '-') {
                errors.push(ValidationError::new(
                    format!(
                        "ID '{}' contains invalid characters. Only letters, numbers, and hyphens allowed.",
                        id_str
                    ),
                    line,
                ));
            }

            if id_str.len() > 64 {
                errors.push(ValidationError::new(
                    format!("ID '{}' exceeds maximum length of 64 characters", id_str),
                    line,
                ));
            }

            if ids.contains(id_str) {
                errors.push(ValidationError::new(
                    format!("Duplicate ID '{}'. All IDs must be unique within the document.", id_str),
                    line,
                ));
            } else {
                ids.insert(id_str.clone());
            }
        }
    }

    fn is_generic_alt(&self, alt: &str) -> bool {
        let lower = alt.to_lowercase();
        matches!(
            lower.as_str(),
            "image" | "photo" | "picture" | "pic" | "img"
            | "photo here" | "image here" | "picture here"
            | "screenshot" | "diagram" | "figure"
            | "an image" | "a photo" | "a picture"
        )
    }
}
