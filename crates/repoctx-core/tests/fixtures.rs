//! Integration tests against synthetic fixtures under `tests/fixtures/`.

use std::path::PathBuf;

use repoctx_core::{BuildOptions, BuildPipeline};
use repoctx_query::QueryEngine;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn build_monorepo_edges_resolves_call_chain() {
    let root = fixture_path("monorepo-edges");
    let report = BuildPipeline::new(
        &root,
        BuildOptions {
            incremental: false,
            no_embeddings: true,
        },
    )
    .run()
    .expect("build should succeed");

    assert!(report.symbols_indexed >= 3);
    assert!(report.edges_indexed >= 2, "expected a->b->c edges");

    let engine = QueryEngine::new(&root);
    let impact = engine.impact("func_a", 3).expect("impact query");
    assert!(
        impact.affected_symbol_ids.len() >= 2,
        "func_a should reach func_b and func_c"
    );
}

#[test]
fn build_tiny_rust_indexes_symbols() {
    let root = fixture_path("tiny-rust");
    let report = BuildPipeline::new(
        &root,
        BuildOptions {
            incremental: false,
            no_embeddings: true,
        },
    )
    .run()
    .expect("build should succeed");

    assert!(report.symbols_indexed >= 2);

    let engine = QueryEngine::new(&root);
    let ctx = engine.context("Greeter", None).expect("context query");
    assert_eq!(ctx.symbol.name, "Greeter");
}

#[test]
fn build_tiny_python_detects_main_entrypoint() {
    let root = fixture_path("tiny-python");
    let report = BuildPipeline::new(
        &root,
        BuildOptions {
            incremental: false,
            no_embeddings: true,
        },
    )
    .run()
    .expect("build should succeed");

    assert!(report.entrypoints_indexed >= 1);
}
