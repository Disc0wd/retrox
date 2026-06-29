// ============================================================
// RNMDL GN-Z11 Validator
// Strict validation rules for GN-Z11 documents.
// FROZEN at GN-Z11. Never modify this file for new versions.
// Rust 1.95.0 | Edition 2021
// ============================================================

use std::collections::HashSet;
use crate::ast::{Node, DocumentHeader};
use crate::versions::ValidationError;
use super::tags::{MODULES, get_tag, module_introduced_in, tag_introduced_in};

pub fn validate(document: &Node) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    match document {
        Node::Document { header, children } => {
            let declared = validate_header(header, &mut errors);
            let mut ids  = HashSet::new();
            validate_children(children, &mut errors, &mut ids, &declared);
        }
        _ => errors.push(ValidationError::new("Root node must be a Document", 0)),
    }
    errors
}

// ─── Header Validation ─────────────────────────────────────

/// Returns the set of declared modules for use in content validation.
fn validate_header(
    header: &DocumentHeader,
    errors: &mut Vec<ValidationError>,
) -> HashSet<String> {
    let mut declared: HashSet<String> = HashSet::new();

    if header.declarations.is_empty() {
        errors.push(ValidationError::new(
            "Document must have at least one module declaration. \
             Example: 'from gn-z11 declare text, images'",
            1,
        ));
        return declared;
    }

    for decl in &header.declarations {
        for module in &decl.modules {
            // Check if module exists in GN-Z11
            if !MODULES.contains(&module.as_str()) {
                let introduced = module_introduced_in(module)
                    .map(|v| format!(" It was introduced in {}.", v))
                    .unwrap_or_else(|| " It does not exist in any known version.".into());

                errors.push(ValidationError::new(
                    format!(
                        "The declared module '{}' does not exist in GN-Z11 (v0.0.0).{}",
                        module, introduced
                    ),
                    decl.line,
                ));
                continue;
            }

            // Check for duplicate declarations
            if declared.contains(module.as_str()) {
                errors.push(ValidationError::new(
                    format!(
                        "The declared module '{}' is declared more than once. \
                         Each module may only be declared from one version.",
                        module
                    ),
                    decl.line,
                ));
            } else {
                declared.insert(module.clone());
            }
        }
    }

    declared
}

// ─── Content Validation ────────────────────────────────────

fn validate_children(
    children: &[Node],
    errors:   &mut Vec<ValidationError>,
    ids:      &mut HashSet<String>,
    declared: &HashSet<String>,
) {
    for child in children {
        validate_node(child, errors, ids, declared);
    }
}

fn validate_node(
    node:     &Node,
    errors:   &mut Vec<ValidationError>,
    ids:      &mut HashSet<String>,
    declared: &HashSet<String>,
) {
    match node {
        Node::Container { id, children, line } => {
            check_tag("container", declared, *line, errors);
            validate_id(id, *line, errors, ids);
            validate_children(children, errors, ids, declared);
        }

        Node::Section { id, children, line } => {
            check_tag("section", declared, *line, errors);
            validate_id(id, *line, errors, ids);
            validate_children(children, errors, ids, declared);
        }

        Node::Heading { level, text, line } => {
            let tag = format!("h{}", level);
            check_tag(&tag, declared, *line, errors);
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
            check_tag("paragraph", declared, *line, errors);
            if text.trim().is_empty() {
                errors.push(ValidationError::new("Paragraph must not be empty", *line));
            }
        }

        Node::Image { path, alt, line } => {
            check_tag("image", declared, *line, errors);

            if path.trim().is_empty() {
                errors.push(ValidationError::new(
                    "Image 'path' attribute is required and must not be empty",
                    *line,
                ));
            }

            let alt_trimmed = alt.trim();
            if alt_trimmed.is_empty() {
                errors.push(ValidationError::new(
                    "Image 'alt' attribute is required. \
                     Provide a meaningful description of the image.",
                    *line,
                ));
            } else if alt_trimmed.len() < 5 {
                errors.push(ValidationError::new(
                    format!(
                        "Image alt text '{}' is too short (minimum 5 characters). \
                         Describe what the image shows.",
                        alt_trimmed
                    ),
                    *line,
                ));
            } else if is_generic_alt(alt_trimmed) {
                errors.push(ValidationError::new(
                    format!(
                        "Image alt text '{}' is not meaningful. \
                         Describe the actual image content.",
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
                        "Image '{}' has unsupported format. \
                         Supported formats: jpg, jpeg, png, gif, webp",
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

// ─── Tag Check ─────────────────────────────────────────────

/// Check that a tag is valid for the declared modules in GN-Z11.
fn check_tag(
    tag_name: &str,
    declared: &HashSet<String>,
    line:     usize,
    errors:   &mut Vec<ValidationError>,
) {
    match get_tag(tag_name) {
        None => {
            // Tag doesn't exist in GN-Z11 at all
            let introduced = tag_introduced_in(tag_name)
                .map(|v| format!(" It was introduced in {}.", v))
                .unwrap_or_else(|| " It does not exist in any known version.".into());

            errors.push(ValidationError::new(
                format!(
                    "The declared version GN-Z11 (v0.0.0) does not support tag '[{}]'.{}",
                    tag_name, introduced
                ),
                line,
            ));
        }
        Some(tag_info) => {
            // Tag exists but module not declared
            if !declared.contains(tag_info.module) {
                errors.push(ValidationError::new(
                    format!(
                        "The declared modules do not include '{}', \
                         which is required by tag '[{}]'. \
                         Add 'from gn-z11 declare {}' to your document header.",
                        tag_info.module, tag_name, tag_info.module
                    ),
                    line,
                ));
            }
        }
    }
}

// ─── ID Validation ─────────────────────────────────────────

fn validate_id(
    id:     &Option<String>,
    line:   usize,
    errors: &mut Vec<ValidationError>,
    ids:    &mut HashSet<String>,
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
                    "ID '{}' contains invalid characters. \
                     Only letters, numbers, and hyphens allowed.",
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
                format!(
                    "Duplicate ID '{}'. All IDs must be unique within the document.",
                    id_str
                ),
                line,
            ));
        } else {
            ids.insert(id_str.clone());
        }
    }
}

// ─── Alt Text ──────────────────────────────────────────────

fn is_generic_alt(alt: &str) -> bool {
    let lower = alt.to_lowercase();
    matches!(
        lower.as_str(),
        "image" | "photo" | "picture" | "pic" | "img"
        | "photo here" | "image here" | "picture here"
        | "screenshot" | "diagram" | "figure"
        | "an image" | "a photo" | "a picture"
    )
}