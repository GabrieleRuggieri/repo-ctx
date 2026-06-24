//! Deterministic analysis core for RepoCtx.
//!
//! Walks the repository, hashes files for incremental builds, extracts symbols
//! with language heuristics (tree-sitter integration planned), and orchestrates
//! the `repoctx build` pipeline.

pub mod build;
pub mod error;
pub mod extract;
pub mod language;
pub mod walker;

pub use build::{BuildOptions, BuildPipeline, BuildReport};
pub use error::CoreError;
