# PROGRESS.md — RepoCtx development log

> Cronologia di sviluppo (work in progress e milestone completate).
> Complementare a [BACKLOG.md](./BACKLOG.md) (cosa manca).

---

## 2026-06-23 — Sessione 1: scaffolding v0.1.0

### Completato

- **Workspace Rust** multi-crate allineato ad [ARCHITECTURE.md](./ARCHITECTURE.md):
  - `repoctx-schema` — tipi artifact versionati (`schemaVersion` 1.0.0)
  - `repoctx-store` — SQLite (`index.db`) + writer JSON `.repoctx/`
  - `repoctx-core` — file walker, hash incrementale, estrazione euristica simboli
  - `repoctx-query` — motori `impact`, `flow`, `context`, `dependencies`
  - `repoctx-cli` — CLI `build | impact | flow | context | domain`
  - `repoctx-mcp` — stub (stdio MCP da integrare con `rmcp`)
- **Pipeline `repoctx build`**: walk → hash → parse euristico → index → emit 5 JSON
- **CI GitHub Actions**: fmt, clippy, test, build release (Ubuntu + macOS)
- **LICENSE** Apache-2.0
- **`.repoctxignore`** di esempio

### Verificato

- `cargo build` e `cargo test` (4 unit test) passano
- `repoctx build` sul repo stesso genera `.repoctx/*.json`

### Note

- L'estrazione simboli è **euristica per linea** — tree-sitter è il prossimo step P0
- Grafo dipendenze / flow / entrypoint non ancora popolati (tabelle pronte)
- Embeddings ONNX e MCP sampling non implementati

---

## 2026-06-23 — Sessione 2: tree-sitter + grafo call (branch `feature/tree-sitter-parser`)

### Completato

- **tree-sitter** integrato per Rust, Python, JS, TS, Go, Java
- **`GraphResolver`**: risoluzione call edges (`func_a → func_b → func_c`)
- **Entrypoint detector** v0: simboli `main` → `entrypoints.json`
- **Fix incrementale**: purge simboli per file prima del re-index
- **Fixture sintetiche** in `tests/fixtures/` + 3 integration test
- `repoctx impact` funziona su catene di chiamate reali

### Verificato

- 8 unit test + 3 integration test passano
- `cargo clippy` e `cargo fmt` puliti

### Note

- `HeuristicExtractor` deprecato (non più usato da `build`)
- Import/extends e flow reconstructor ancora da fare (P0-4)
- MCP server ancora stub (branch `feature/mcp-server` da aprire)
