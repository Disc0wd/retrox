// ============================================================
// RNMDL Version Dispatcher
// Routes validation to the correct version implementation.
// Rust 1.95.0 | Edition 2021
// ============================================================

pub mod gn_z11;
pub mod milky_way;

use crate::ast::{Node, RnmdlVersion};

// ─── Shared Error Type ─────────────────────────────────────

#[derive(Debug)]
pub struct ValidationError {
    pub message: String,
    pub line:    usize,
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

// ─── Version Dispatcher ────────────────────────────────────

/// Validate a document using the rules for its declared version.
pub fn validate(document: &Node) -> Vec<ValidationError> {
    // Determine which version is declared
    let version = match document {
        Node::Document { header, .. } => {
            header.declarations.first()
                .map(|d| RnmdlVersion::from_str(&d.version))
                .unwrap_or(RnmdlVersion::Unknown("none".into()))
        }
        _ => return vec![ValidationError::new("Root node must be a Document", 0)],
    };

    match version {
        RnmdlVersion::GnZ11
        | RnmdlVersion::Pallas
        | RnmdlVersion::Halley
        | RnmdlVersion::Ceres
        | RnmdlVersion::Chiron => gn_z11::validator::validate(document),

        RnmdlVersion::MilkyWay => {
            // MilkyWay not yet implemented — fall back to GN-Z11 for now
            gn_z11::validator::validate(document)
        }

        RnmdlVersion::Unknown(v) => vec![ValidationError::new(
            format!(
                "Unknown version '{}'. Known versions: \
                 gn-z11, milky-way, sol, luna, andromeda, \
                 pallas, halley, ceres, chiron",
                v
            ),
            1,
        )],

        _ => vec![ValidationError::new(
            format!(
                "Version '{}' is not yet implemented in this build.",
                version.display_name()
            ),
            1,
        )],
    }
}