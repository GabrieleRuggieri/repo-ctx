//! Context assembly: code snippets + impact + markdown bundle.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use becket_core::wiki::{
    find_flow_page_for_symbol, find_page_for_symbol, sanitize_for_context, wiki_adds_context,
    WikiStore,
};
use becket_embed::{embed_with_model, symbol_embedding_text};
use becket_schema::artifacts::SymbolRecord;
use becket_store::{BecketPaths, IndexStore};

use crate::budget::{
    build_advice, estimate_impact_line_tokens, estimate_snippet_tokens, estimate_tokens,
    impact_display_cap, pack_impact_count, recommend_budget, DEFAULT_BUDGET, OVERHEAD_TOKENS,
};
use crate::types::{BudgetAdvice, CodeSnippet, ContextResult, ContextTask, SummarySource};

/// Options for context assembly.
#[derive(Debug, Clone, Copy)]
pub struct AssembleOptions {
    /// Token budget; `None` uses the recommended budget for this symbol/task.
    pub budget: Option<u32>,
    pub task: ContextTask,
}

impl Default for AssembleOptions {
    fn default() -> Self {
        Self {
            budget: Some(DEFAULT_BUDGET),
            task: ContextTask::Fix,
        }
    }
}

/// Assembles a rich context bundle for a symbol within a token budget.
pub fn assemble_context(
    store: &IndexStore,
    repo_root: &Path,
    root: SymbolRecord,
    budget: Option<u32>,
    task: ContextTask,
) -> Result<ContextResult, crate::error::QueryError> {
    assemble_context_with_options(store, repo_root, root, AssembleOptions { budget, task })
}

/// Assembles context with explicit options (supports auto budget via `budget: None`).
pub fn assemble_context_with_options(
    store: &IndexStore,
    repo_root: &Path,
    root: SymbolRecord,
    options: AssembleOptions,
) -> Result<ContextResult, crate::error::QueryError> {
    let task = options.task;
    let impact_depth = task.impact_depth();

    let all_symbols = store.load_symbols()?;
    let id_to_symbol: HashMap<_, _> = all_symbols.iter().map(|s| (s.id.as_str(), s)).collect();

    let callers = direct_neighbors(store, &root.id, true)?;
    let callees = direct_neighbors(store, &root.id, false)?;
    let affected_ids = store.downstream_symbols(&root.id, impact_depth)?;

    let related_components: Vec<String> = all_symbols
        .iter()
        .filter(|s| s.file_path == root.file_path && s.id != root.id)
        .map(|s| s.name.clone())
        .take(10)
        .collect();

    let responsibility = format!(
        "{} ({:?}) defined in {} at lines {}-{}",
        root.name, root.kind, root.file_path, root.start_line, root.end_line
    );

    let external_dependencies = root
        .file_path
        .split('/')
        .take(2)
        .map(str::to_string)
        .collect();

    let invariants = vec![format!("visibility: {:?}", root.visibility)];

    let mut ranked = rank_symbols(
        &root,
        &callers,
        &callees,
        &affected_ids,
        &semantic_neighbor_ids(store, &root, 5)?,
        &related_components,
    );
    ranked.sort_by_key(|(_, priority)| *priority);

    let max_snippets = match task {
        ContextTask::Onboard => 3,
        _ => usize::MAX,
    };

    let paths = BecketPaths::new(repo_root);
    let wiki_store = WikiStore::new(&paths);
    let wiki_page = find_page_for_symbol(&wiki_store, &root.id).ok().flatten();
    let flow_wiki_page = if matches!(task, ContextTask::Onboard) {
        find_flow_page_for_symbol(&wiki_store, &root.id)
            .ok()
            .flatten()
    } else {
        None
    };

    let wiki_sanitized = wiki_page
        .as_ref()
        .map(|p| sanitize_for_context(&p.body))
        .filter(|body| wiki_adds_context(body));
    let flow_wiki_sanitized = flow_wiki_page
        .as_ref()
        .map(|p| sanitize_for_context(&p.body))
        .filter(|body| wiki_adds_context(body));

    let caller_names: Vec<String> = callers
        .iter()
        .filter_map(|id| id_to_symbol.get(id.as_str()).map(|s| s.name.clone()))
        .collect();
    let callee_names: Vec<String> = callees
        .iter()
        .filter_map(|id| id_to_symbol.get(id.as_str()).map(|s| s.name.clone()))
        .collect();

    let impact_line_tokens: Vec<u32> = affected_ids
        .iter()
        .filter_map(|id| id_to_symbol.get(id.as_str()))
        .map(|sym| {
            estimate_impact_line_tokens(&sym.name, &sym.file_path, sym.start_line, sym.end_line)
        })
        .collect();

    let mut candidate_snippets = Vec::new();
    let mut seen = HashSet::new();
    for (sym_id, _) in &ranked {
        if candidate_snippets.len() >= max_snippets && max_snippets != usize::MAX {
            break;
        }
        if seen.contains(sym_id) {
            continue;
        }
        seen.insert(sym_id.clone());
        let Some(sym) = id_to_symbol.get(sym_id.as_str()) else {
            continue;
        };
        let Some(snippet) = slice_symbol(repo_root, sym) else {
            continue;
        };
        candidate_snippets.push(snippet);
    }

    let snippet_token_estimates: Vec<u32> = candidate_snippets
        .iter()
        .map(estimate_snippet_tokens)
        .collect();

    let wiki_tokens = wiki_sanitized
        .as_ref()
        .map(|b| estimate_tokens(b) + estimate_tokens("## Knowledge\n\n"))
        .unwrap_or(0);
    let flow_wiki_tokens = flow_wiki_sanitized
        .as_ref()
        .map(|b| estimate_tokens(b) + estimate_tokens("## Flow overview\n\n"))
        .unwrap_or(0);

    let recommended = recommend_budget(
        task,
        wiki_tokens,
        flow_wiki_tokens,
        &impact_line_tokens,
        &snippet_token_estimates,
        max_snippets,
    );

    let requested_budget = options.budget.unwrap_or(recommended);
    let mut remaining = requested_budget.saturating_sub(OVERHEAD_TOKENS);

    if wiki_tokens > 0 && wiki_tokens <= remaining {
        remaining = remaining.saturating_sub(wiki_tokens);
    } else if wiki_tokens > remaining && wiki_tokens <= requested_budget / 2 {
        remaining = 0;
    }

    if flow_wiki_tokens > 0 && flow_wiki_tokens <= remaining {
        remaining = remaining.saturating_sub(flow_wiki_tokens);
    }

    let impact_cap = impact_display_cap(task);
    let impact_shown = pack_impact_count(&impact_line_tokens, remaining, impact_cap);
    let impact_section_tokens = if impact_shown > 0 {
        let header = estimate_tokens("## Impact\n\nSymbols affected if this changes:\n\n");
        let lines: u32 = impact_line_tokens.iter().take(impact_shown).sum();
        header + lines
    } else {
        0
    };
    remaining = remaining.saturating_sub(impact_section_tokens);

    let mut snippets = Vec::new();
    let mut snippet_tokens_used = 0u32;
    for (snippet, est) in candidate_snippets
        .into_iter()
        .zip(snippet_token_estimates.iter())
    {
        if snippets.is_empty() || snippet_tokens_used + est <= remaining {
            snippet_tokens_used += est;
            snippets.push(snippet);
        } else {
            break;
        }
    }

    let snippets_total = snippet_token_estimates.len();
    let snippets_included = snippets.len();

    let render_input = RenderInput {
        root: &root,
        responsibility: &responsibility,
        snippets: &snippets,
        callers: &caller_names,
        callees: &callee_names,
        affected_ids: &affected_ids,
        impact_shown,
        id_to_symbol: &id_to_symbol,
        related: &related_components,
        wiki_page_id: wiki_page.as_ref().map(|p| p.meta.id.as_str()),
        wiki_body: wiki_sanitized.as_deref(),
        flow_wiki_page_id: flow_wiki_page.as_ref().map(|p| p.meta.id.as_str()),
        flow_wiki_body: flow_wiki_sanitized.as_deref(),
        task,
        budget_advice: None,
    };

    let markdown = render_markdown(&render_input);
    let estimated_tokens = estimate_tokens(&markdown);

    let budget_advice = build_advice(
        task,
        requested_budget,
        recommended,
        estimated_tokens,
        snippets_included,
        snippets_total,
        impact_shown,
        affected_ids.len(),
    );

    let mut render_input = render_input;
    render_input.budget_advice = Some(&budget_advice);
    let markdown = render_markdown(&render_input);

    let semantic_neighbors = semantic_neighbor_names(store, &root, 5)?;

    Ok(ContextResult {
        symbol: root,
        responsibility,
        enriched_summary: None,
        summary_source: SummarySource::Deterministic,
        related_components,
        external_dependencies,
        invariants,
        semantic_neighbors,
        snippets,
        callers: caller_names,
        callees: callee_names,
        affected_symbol_ids: affected_ids,
        wiki_page_id: wiki_page.as_ref().map(|p| p.meta.id.clone()),
        wiki_body: wiki_sanitized,
        flow_wiki_page_id: flow_wiki_page.as_ref().map(|p| p.meta.id.clone()),
        flow_wiki_body: flow_wiki_sanitized,
        budget_advice,
        markdown,
        task,
        budget_tokens: requested_budget,
    })
}

/// Rebuilds the markdown bundle from an assembled context (e.g. after wiki enrichment).
pub fn refresh_context_markdown(context: &ContextResult) -> String {
    let id_to_symbol: HashMap<&str, &SymbolRecord> =
        std::iter::once((context.symbol.id.as_str(), &context.symbol)).collect();
    let impact_shown = context.budget_advice.impact_entries_shown;
    render_markdown(&RenderInput {
        root: &context.symbol,
        responsibility: &context.responsibility,
        snippets: &context.snippets,
        callers: &context.callers,
        callees: &context.callees,
        affected_ids: &context.affected_symbol_ids,
        impact_shown,
        id_to_symbol: &id_to_symbol,
        related: &context.related_components,
        wiki_page_id: context.wiki_page_id.as_deref(),
        wiki_body: context.wiki_body.as_deref(),
        flow_wiki_page_id: context.flow_wiki_page_id.as_deref(),
        flow_wiki_body: context.flow_wiki_body.as_deref(),
        task: context.task,
        budget_advice: Some(&context.budget_advice),
    })
}

fn direct_neighbors(
    store: &IndexStore,
    symbol_id: &str,
    upstream: bool,
) -> Result<Vec<String>, crate::error::QueryError> {
    let edges = store.load_call_edges()?;
    Ok(edges
        .into_iter()
        .filter_map(|(src, dst)| {
            if upstream && dst == symbol_id {
                Some(src)
            } else if !upstream && src == symbol_id {
                Some(dst)
            } else {
                None
            }
        })
        .collect())
}

fn rank_symbols(
    root: &SymbolRecord,
    callers: &[String],
    callees: &[String],
    affected: &[String],
    semantic: &[String],
    related: &[String],
) -> Vec<(String, u8)> {
    let mut out = Vec::new();
    out.push((root.id.clone(), 0));
    for id in callers {
        out.push((id.clone(), 1));
    }
    for id in callees {
        out.push((id.clone(), 2));
    }
    for id in semantic {
        if id != &root.id {
            out.push((id.clone(), 3));
        }
    }
    for id in affected {
        if id != &root.id {
            out.push((id.clone(), 4));
        }
    }
    let _ = related;
    out
}

fn semantic_neighbor_ids(
    store: &IndexStore,
    symbol: &SymbolRecord,
    limit: usize,
) -> Result<Vec<String>, crate::error::QueryError> {
    if store.count_symbol_embeddings()? == 0 {
        return Ok(Vec::new());
    }
    let query = embed_with_model(&symbol_embedding_text(symbol));
    let hits = store.nearest_symbol_ids(&query, limit + 1)?;
    Ok(hits
        .into_iter()
        .filter(|(id, _)| id != &symbol.id)
        .take(limit)
        .map(|(id, _)| id)
        .collect())
}

pub(crate) fn semantic_neighbor_names(
    store: &IndexStore,
    symbol: &SymbolRecord,
    limit: usize,
) -> Result<Vec<String>, crate::error::QueryError> {
    let ids = semantic_neighbor_ids(store, symbol, limit)?;
    let id_to_name: HashMap<_, _> = store
        .load_symbols()?
        .into_iter()
        .map(|s| (s.id, s.name))
        .collect();
    Ok(ids
        .into_iter()
        .filter_map(|id| id_to_name.get(&id).cloned())
        .collect())
}

fn slice_symbol(repo_root: &Path, symbol: &SymbolRecord) -> Option<CodeSnippet> {
    let path = repo_root.join(&symbol.file_path);
    let content = fs::read_to_string(&path).ok()?;
    let lines: Vec<&str> = content.lines().collect();
    let start = symbol.start_line.saturating_sub(1) as usize;
    let end = (symbol.end_line as usize).min(lines.len());
    if start >= lines.len() || start >= end {
        return None;
    }
    let slice = lines[start..end].join("\n");
    let lang = extension_to_lang(path.extension()?.to_str()?);
    Some(CodeSnippet {
        symbol_id: symbol.id.clone(),
        symbol_name: symbol.name.clone(),
        file_path: symbol.file_path.clone(),
        start_line: symbol.start_line,
        end_line: symbol.end_line,
        language: lang.to_string(),
        content: slice,
    })
}

fn extension_to_lang(ext: &str) -> &'static str {
    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        "java" => "java",
        _ => "text",
    }
}

struct RenderInput<'a> {
    root: &'a SymbolRecord,
    responsibility: &'a str,
    snippets: &'a [CodeSnippet],
    callers: &'a [String],
    callees: &'a [String],
    affected_ids: &'a [String],
    impact_shown: usize,
    id_to_symbol: &'a HashMap<&'a str, &'a SymbolRecord>,
    related: &'a [String],
    wiki_page_id: Option<&'a str>,
    wiki_body: Option<&'a str>,
    flow_wiki_page_id: Option<&'a str>,
    flow_wiki_body: Option<&'a str>,
    task: ContextTask,
    budget_advice: Option<&'a BudgetAdvice>,
}

fn render_markdown(input: &RenderInput<'_>) -> String {
    let RenderInput {
        root,
        responsibility,
        snippets,
        callers,
        callees,
        affected_ids,
        impact_shown,
        id_to_symbol,
        related,
        wiki_page_id,
        wiki_body,
        flow_wiki_page_id,
        flow_wiki_body,
        task,
        budget_advice,
    } = input;
    let mut md = String::new();
    md.push_str(&format!("# Context: {}\n\n", root.name));
    md.push_str(&format!("**Task:** {:?}\n\n", task));

    if let Some(advice) = budget_advice {
        if !advice.within_budget {
            md.push_str("> **Budget notice:** The requested budget may be too small for this ");
            md.push_str(&format!(
                "symbol and task. Used ~{} tokens; recommended ~{} tokens",
                advice.estimated_tokens, advice.recommended_tokens
            ));
            if advice.snippets_omitted > 0 {
                md.push_str(&format!(" ({} snippet(s) omitted", advice.snippets_omitted));
                if advice.impact_entries_shown < advice.impact_entries_total {
                    md.push_str(&format!(
                        ", impact truncated to {}",
                        advice.impact_entries_shown
                    ));
                }
                md.push(')');
            } else if advice.impact_entries_shown < advice.impact_entries_total {
                md.push_str(&format!(
                    " (impact truncated to {} of {})",
                    advice.impact_entries_shown, advice.impact_entries_total
                ));
            }
            md.push_str(". Re-run with `--auto-budget` or `--budget ");
            md.push_str(&advice.recommended_tokens.to_string());
            md.push_str("`.\n\n");
        }
    }

    md.push_str("## Symbol\n\n");
    md.push_str(responsibility);
    md.push('\n');

    if let (Some(page_id), Some(body)) = (flow_wiki_page_id, flow_wiki_body) {
        if !body.is_empty() {
            md.push_str("\n## Flow overview\n\n");
            md.push_str(&format!("_Grounded markdown: `{page_id}`_\n\n"));
            md.push_str(body);
            if !body.ends_with('\n') {
                md.push('\n');
            }
            md.push('\n');
        }
    }

    if let (Some(page_id), Some(body)) = (wiki_page_id, wiki_body) {
        if !body.is_empty() {
            md.push_str("\n## Knowledge\n\n");
            md.push_str(&format!("_Grounded markdown: `{page_id}`_\n\n"));
            md.push_str(body);
            if !body.ends_with('\n') {
                md.push('\n');
            }
            md.push('\n');
        }
    }

    if !snippets.is_empty() {
        md.push_str("\n## Code\n\n");
        for snip in *snippets {
            md.push_str(&format!(
                "### {} (`{}:{}-{}`)\n\n",
                snip.symbol_name, snip.file_path, snip.start_line, snip.end_line
            ));
            md.push_str(&format!("```{}\n", snip.language));
            md.push_str(&snip.content);
            if !snip.content.ends_with('\n') {
                md.push('\n');
            }
            md.push_str("```\n\n");
        }
    }

    if !callers.is_empty() || !callees.is_empty() {
        md.push_str("## Call graph\n\n");
        if !callers.is_empty() {
            md.push_str(&format!("**Callers:** {}\n\n", callers.join(", ")));
        }
        if !callees.is_empty() {
            md.push_str(&format!("**Callees:** {}\n\n", callees.join(", ")));
        }
    }

    if *impact_shown > 0 {
        md.push_str("## Impact\n\n");
        md.push_str("Symbols affected if this changes:\n\n");
        for id in affected_ids.iter().take(*impact_shown) {
            if let Some(sym) = id_to_symbol.get(id.as_str()) {
                md.push_str(&format!(
                    "- **{}** — `{}:{}-{}`\n",
                    sym.name, sym.file_path, sym.start_line, sym.end_line
                ));
            }
        }
        if affected_ids.len() > *impact_shown {
            md.push_str(&format!(
                "\n_…and {} more (increase `--budget` or use `--auto-budget`)_\n",
                affected_ids.len() - impact_shown
            ));
        }
        md.push('\n');
    }

    if !related.is_empty() {
        md.push_str(&format!("## Related in module\n\n{}\n", related.join(", ")));
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use becket_schema::symbol::{SymbolKind, Visibility};

    #[test]
    fn refresh_markdown_includes_budget_notice_when_truncated() {
        let symbol = SymbolRecord {
            id: "sym1".into(),
            kind: SymbolKind::Function,
            name: "pay".into(),
            fqn: "pay".into(),
            file_path: "src/pay.rs".into(),
            start_line: 1,
            end_line: 3,
            visibility: Visibility::Public,
            module_id: None,
        };
        let advice = BudgetAdvice {
            requested_budget: 1000,
            recommended_tokens: 5000,
            estimated_tokens: 1200,
            snippets_included: 1,
            snippets_omitted: 2,
            impact_entries_shown: 5,
            impact_entries_total: 20,
            within_budget: false,
        };
        let ctx = ContextResult {
            symbol,
            responsibility: "pay".into(),
            enriched_summary: None,
            summary_source: SummarySource::Deterministic,
            related_components: vec![],
            external_dependencies: vec![],
            invariants: vec![],
            semantic_neighbors: vec![],
            snippets: vec![],
            callers: vec![],
            callees: vec![],
            affected_symbol_ids: (0..20).map(|i| format!("sym{i}")).collect(),
            wiki_page_id: None,
            wiki_body: None,
            flow_wiki_page_id: None,
            flow_wiki_body: None,
            budget_advice: advice,
            markdown: String::new(),
            task: ContextTask::Fix,
            budget_tokens: 1000,
        };
        let md = refresh_context_markdown(&ctx);
        assert!(md.contains("Budget notice"));
        assert!(md.contains("recommended ~5000"));
    }
}
