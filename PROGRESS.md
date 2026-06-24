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
