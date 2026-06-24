//! Error types for schema validation and serialization.

use thiserror::Error;

/// Errors raised when validating or serializing artifact schemas.
#[derive(Debug, Error)]
pub enum SchemaError {
    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// A required field was missing or invalid.
    #[error("invalid schema field: {0}")]
    InvalidField(String),

    /// The artifact schema version is unsupported.
    #[error("unsupported schema version: {0}")]
    UnsupportedVersion(String),
}
