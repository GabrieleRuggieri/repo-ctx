//! `repoctx build` orchestration: walk, hash, extract, persist, emit artifacts.

use std::path::Path;

use repoctx_store::{ArtifactWriter, IndexStore, RepoCtxPaths};
use tracing::info;
use uuid::Uuid;

use crate::error::CoreError;
use crate::extract::HeuristicExtractor;
use crate::walker::FileWalker;

/// Options controlling a build run.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// When true, skip files whose content hash is unchanged.
    pub incremental: bool,
    /// When true, skip embedding generation (not yet implemented).
    pub no_embeddings: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            incremental: true,
            no_embeddings: false,
        }
    }
}

/// Summary counters emitted after a successful build.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildReport {
    /// Total source files discovered.
    pub files_discovered: usize,
    /// Files parsed in this run.
    pub files_parsed: usize,
    /// Files skipped due to incremental cache hit.
    pub files_skipped: usize,
    /// Symbols indexed.
    pub symbols_indexed: usize,
    /// Path to `.repoctx/` output directory.
    pub output_dir: String,
}

/// End-to-end deterministic build pipeline.
pub struct BuildPipeline {
    paths: RepoCtxPaths,
    options: BuildOptions,
}

impl BuildPipeline {
    /// Creates a pipeline for the repository at `root`.
    ///
    /// # Arguments
    ///
    /// * `root` - Repository root directory.
    /// * `options` - Build configuration.
    pub fn new(root: impl AsRef<Path>, options: BuildOptions) -> Self {
        Self {
            paths: RepoCtxPaths::new(root),
            options,
        }
    }

    /// Runs the full build: walk → parse → index → emit JSON artifacts.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] on I/O, store, or validation failures.
    pub fn run(&self) -> Result<BuildReport, CoreError> {
        let walker = FileWalker::new(&self.paths.root);
        let discovered = walker.discover()?;

        let store = IndexStore::open(&self.paths.index_db)?;
        if !self.options.incremental {
            store.clear_all()?;
        }

        let mut files_parsed = 0usize;
        let mut files_skipped = 0usize;
        let mut symbols_indexed = 0usize;

        for file in &discovered {
            if self.options.incremental {
                if let Some(existing_hash) = store.file_hash(&file.relative_path)? {
                    if existing_hash == file.content_hash {
                        files_skipped += 1;
                        continue;
                    }
                }
            }

            let file_id = Uuid::new_v4().to_string();
            store.upsert_file(
                &file_id,
                &file.relative_path,
                file.language.id(),
                &file.content_hash,
                file.mtime_secs,
            )?;

            let symbols = HeuristicExtractor::extract(
                &file.relative_path,
                file.language,
                &file.absolute_path,
            )?;

            for symbol in &symbols {
                store.insert_symbol(symbol, &file_id)?;
                symbols_indexed += 1;
            }

            files_parsed += 1;
            info!(path = %file.relative_path, symbols = symbols.len(), "parsed file");
        }

        let writer = ArtifactWriter::new(self.paths.clone());
        let (symbols, dependencies, flows, entrypoints, architecture) = store.export_artifacts()?;

        writer.write_artifact("symbols", &symbols)?;
        writer.write_artifact("dependencies", &dependencies)?;
        writer.write_artifact("flows", &flows)?;
        writer.write_artifact("entrypoints", &entrypoints)?;
        writer.write_artifact("architecture", &architecture)?;

        Ok(BuildReport {
            files_discovered: discovered.len(),
            files_parsed,
            files_skipped,
            symbols_indexed,
            output_dir: writer.output_dir().display().to_string(),
        })
    }
}
