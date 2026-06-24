//! Query engine for impact, flow, context, and dependency lookups.
//!
//! Shared by the CLI and MCP server — no duplicated query logic.

pub mod engine;
pub mod error;
pub mod types;

pub use engine::QueryEngine;
pub use error::QueryError;
pub use types::{ContextResult, DependenciesResult, FlowResult, ImpactResult, SummarySource};
