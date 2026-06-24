//! Graph-grounded wiki page compiler (template slots + claim blocks).

use std::collections::{HashMap, HashSet};

use repoctx_schema::artifacts::{FlowRecord, ModuleRecord, SymbolRecord};
use repoctx_schema::wiki::{WikiPageKind, WikiPageMeta, WikiPageSource};
use repoctx_store::{IndexStore, RepoCtxPaths};

use crate::error::CoreError;
use crate::wiki::fingerprint::subgraph_fingerprint;
use crate::wiki::store::WikiStore;

/// Compiles wiki pages from the deterministic graph.
pub struct WikiCompiler {
    paths: RepoCtxPaths,
}

impl WikiCompiler {
    /// Creates a compiler for the repository at `paths`.
    pub fn new(paths: RepoCtxPaths) -> Self {
        Self { paths }
    }

    /// Compiles all wiki pages and `index.md`. Returns number of pages written.
    pub fn compile_all(&self, store: &IndexStore) -> Result<usize, CoreError> {
        let symbols = store.load_symbols()?;
        let (_, _, flows_art, entrypoints_art, architecture) = store.export_artifacts()?;
        let flows = &flows_art.flows;
        let entrypoints = &entrypoints_art.entrypoints;
        let call_edges = store.load_call_edges()?;

        let id_to_symbol: HashMap<&str, &SymbolRecord> =
            symbols.iter().map(|s| (s.id.as_str(), s)).collect();
        let _ = id_to_symbol;
        let entrypoint_symbols: HashSet<&str> =
            entrypoints.iter().map(|e| e.symbol_id.as_str()).collect();

        let wiki_store = WikiStore::new(&self.paths);
        wiki_store.ensure_dir()?;

        let mut pages: Vec<(WikiPageMeta, String)> = Vec::new();

        for flow in flows {
            let symbol_ids: Vec<String> = flow.steps.iter().map(|s| s.symbol_id.clone()).collect();
            let meta = WikiPageMeta {
                id: format!("wiki_flow_{}", slugify(&flow.name)),
                kind: WikiPageKind::Flow,
                symbol_ids: symbol_ids.clone(),
                source: WikiPageSource::Deterministic,
                graph_fingerprint: subgraph_fingerprint(store, &symbol_ids)?,
                see_also: Vec::new(),
                title: format!("Flow: {}", flow.name),
            };
            let body = render_flow_body(flow, &symbols, &call_edges);
            pages.push((meta, body));
        }

        for module in &architecture.modules {
            let meta = WikiPageMeta {
                id: format!("wiki_module_{}", slugify(&module.id)),
                kind: WikiPageKind::Module,
                symbol_ids: module.symbol_ids.clone(),
                source: WikiPageSource::Deterministic,
                graph_fingerprint: subgraph_fingerprint(store, &module.symbol_ids)?,
                see_also: Vec::new(),
                title: format!("Module: {}", module.name),
            };
            let body = render_module_body(module, &symbols, store)?;
            pages.push((meta, body));
        }

        for symbol in &symbols {
            if !entrypoint_symbols.contains(symbol.id.as_str()) {
                continue;
            }
            let symbol_ids = vec![symbol.id.clone()];
            let meta = WikiPageMeta {
                id: format!("wiki_service_{}", slugify(&symbol.name)),
                kind: WikiPageKind::Service,
                symbol_ids: symbol_ids.clone(),
                source: WikiPageSource::Deterministic,
                graph_fingerprint: subgraph_fingerprint(store, &symbol_ids)?,
                see_also: Vec::new(),
                title: format!("Service: {}", symbol.name),
            };
            let body = render_service_body(symbol, store, &call_edges)?;
            pages.push((meta, body));
        }

        let index_body = render_index(&pages);
        let index_meta = WikiPageMeta {
            id: "wiki_index".into(),
            kind: WikiPageKind::Overview,
            symbol_ids: Vec::new(),
            source: WikiPageSource::Deterministic,
            graph_fingerprint: "index".into(),
            see_also: pages.iter().map(|(m, _)| m.id.clone()).collect(),
            title: "Repo Wiki Index".into(),
        };
        wiki_store.write_index(&index_meta, &index_body)?;

        let mut count = 1usize;
        for (meta, body) in &pages {
            wiki_store.write_page(meta, body)?;
            count += 1;
        }

        Ok(count)
    }

    /// Recompiles pages whose ids are listed in `page_ids` (or all if empty).
    pub fn sync_pages(&self, store: &IndexStore, page_ids: &[String]) -> Result<usize, CoreError> {
        if page_ids.is_empty() {
            return self.compile_all(store);
        }
        self.compile_all(store)?;
        Ok(page_ids.len())
    }
}

fn slugify(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

fn render_index(pages: &[(WikiPageMeta, String)]) -> String {
    let mut md = String::from("# Repo Wiki\n\n");
    let mut by_kind: HashMap<WikiPageKind, Vec<&WikiPageMeta>> = HashMap::new();
    for (meta, _) in pages {
        by_kind.entry(meta.kind).or_default().push(meta);
    }
    for kind in [
        WikiPageKind::Flow,
        WikiPageKind::Service,
        WikiPageKind::Module,
    ] {
        let Some(items) = by_kind.get(&kind) else {
            continue;
        };
        md.push_str(&format!("## {:?}\n\n", kind));
        for meta in items {
            let stem = meta.id.strip_prefix("wiki_").unwrap_or(&meta.id);
            md.push_str(&format!("- [{title}]({stem}.md)\n", title = meta.title));
        }
        md.push('\n');
    }
    md
}

fn render_flow_body(
    flow: &FlowRecord,
    symbols: &[SymbolRecord],
    call_edges: &[(String, String)],
) -> String {
    let id_to_name: HashMap<&str, &str> = symbols
        .iter()
        .map(|s| (s.id.as_str(), s.name.as_str()))
        .collect();
    let mut md = format!("# {}\n\n## Execution path\n\n", flow.name);
    for step in &flow.steps {
        let name = id_to_name
            .get(step.symbol_id.as_str())
            .copied()
            .unwrap_or(&step.symbol_id);
        md.push_str(&format!(
            "{}. **{}** (`{}`)\n",
            step.order, name, step.symbol_id
        ));
        if let Some(ext) = &step.external_system {
            md.push_str(&format!("   - external: {ext}\n"));
        }
    }
    md.push_str("\n## Call edges (flow subgraph)\n\n");
    let flow_ids: HashSet<&str> = flow.steps.iter().map(|s| s.symbol_id.as_str()).collect();
    for (src, dst) in call_edges {
        if flow_ids.contains(src.as_str()) && flow_ids.contains(dst.as_str()) {
            md.push_str(&format!(
                "<!-- repoctx:claim calls {dst} source=graph -->\n- `{src}` → `{dst}`\n"
            ));
        }
    }
    md.push_str(prose_slot());
    md
}

fn render_module_body(
    module: &ModuleRecord,
    symbols: &[SymbolRecord],
    store: &IndexStore,
) -> Result<String, CoreError> {
    let mut md = format!("# {}\n\n## Symbols\n\n", module.name);
    for sym_id in &module.symbol_ids {
        let Some(sym) = symbols.iter().find(|s| &s.id == sym_id) else {
            continue;
        };
        md.push_str(&format!(
            "- **{}** — `{}:{}-{}`\n",
            sym.name, sym.file_path, sym.start_line, sym.end_line
        ));
    }
    md.push_str("\n## Impact summary\n\n");
    for sym_id in module.symbol_ids.iter().take(5) {
        let downstream = store.downstream_symbols(sym_id, 1)?;
        if !downstream.is_empty() {
            md.push_str(&format!(
                "- `{sym_id}` affects {} symbols (depth 1)\n",
                downstream.len()
            ));
        }
    }
    md.push_str(prose_slot());
    Ok(md)
}

fn render_service_body(
    symbol: &SymbolRecord,
    store: &IndexStore,
    call_edges: &[(String, String)],
) -> Result<String, CoreError> {
    let mut md = format!(
        "# {}\n\n## Structure\n\n**Location:** `{}:{}-{}`\n\n",
        symbol.name, symbol.file_path, symbol.start_line, symbol.end_line
    );

    md.push_str("### Callers\n\n");
    let callers: Vec<_> = call_edges
        .iter()
        .filter(|(_, dst)| dst == &symbol.id)
        .collect();
    if callers.is_empty() {
        md.push_str("_None detected._\n\n");
    } else {
        for (src, dst) in callers {
            md.push_str(&format!(
                "<!-- repoctx:claim calls {src} source=graph -->\n- `{src}` → `{dst}`\n"
            ));
        }
        md.push('\n');
    }

    md.push_str("### Callees\n\n");
    let callees: Vec<_> = call_edges
        .iter()
        .filter(|(src, _)| src == &symbol.id)
        .collect();
    if callees.is_empty() {
        md.push_str("_None detected._\n\n");
    } else {
        for (src, dst) in callees {
            md.push_str(&format!(
                "<!-- repoctx:claim calls {dst} source=graph -->\n- `{src}` → `{dst}`\n"
            ));
        }
        md.push('\n');
    }

    let affected = store.downstream_symbols(&symbol.id, 2)?;
    md.push_str("## Impact\n\n");
    if affected.is_empty() {
        md.push_str("_No downstream symbols within depth 2._\n\n");
    } else {
        for id in affected.iter().take(15) {
            md.push_str(&format!("- `{id}`\n"));
        }
        if affected.len() > 15 {
            md.push_str(&format!("\n_…and {} more_\n", affected.len() - 15));
        }
        md.push('\n');
    }

    md.push_str(prose_slot());
    Ok(md)
}

fn prose_slot() -> &'static str {
    "\n## Intent & gotchas\n\n<!-- repoctx:slot prose -->\n_Awaiting enrichment via `repoctx wiki sync` or MCP sampling._\n"
}
