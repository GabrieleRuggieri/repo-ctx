//! Versioned JSON artifact schemas and shared domain types for RepoCtx.
//!
//! This crate defines the public contract for `.repoctx/*.json` artifacts and
//! shared enums used across the deterministic core, store, query engine, and CLI.

pub mod artifacts;
pub mod edge;
pub mod error;
pub mod symbol;
pub mod version;

pub use artifacts::{
    ArchitectureArtifact, DependenciesArtifact, EntrypointsArtifact, FlowsArtifact,
    SymbolsArtifact, VersionedArtifact,
};
pub use edge::{BoundaryKind, EdgeType};
pub use error::SchemaError;
pub use symbol::{EntrypointKind, SymbolKind, Visibility};
pub use version::SCHEMA_VERSION;
