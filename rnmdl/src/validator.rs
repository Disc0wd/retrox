// ============================================================
// RNMDL Validator — Version Dispatcher
// Routes to the correct version-specific validator.
// Rust 1.95.0 | Edition 2021
// ============================================================

pub use crate::versions::ValidationError;
use crate::ast::Node;

pub struct Validator;

impl Validator {
    pub fn new() -> Self { Validator }

    pub fn validate(&self, document: &Node) -> Vec<ValidationError> {
        crate::versions::validate(document)
    }
}