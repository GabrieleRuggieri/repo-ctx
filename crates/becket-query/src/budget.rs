//! Token budget estimation and packing for context assembly.

use crate::types::{BudgetAdvice, CodeSnippet, ContextTask};

/// Default token budget when none is specified.
pub const DEFAULT_BUDGET: u32 = 6000;
/// Rough chars-per-token ratio for packing heuristics.
pub const CHARS_PER_TOKEN: u32 = 4;
/// Fixed reserve for headers, symbol section, and call-graph labels.
pub const OVERHEAD_TOKENS: u32 = 150;
/// Headroom added to recommended budget so minor edits do not immediately truncate.
const RECOMMEND_MARGIN: f32 = 1.15;

/// Estimates token count for a text block (fence overhead included).
#[must_use]
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(CHARS_PER_TOKEN) + 20
}

/// Estimates tokens for a code snippet including markdown fence headers.
#[must_use]
pub fn estimate_snippet_tokens(snippet: &CodeSnippet) -> u32 {
    let header = format!(
        "### {} (`{}:{}-{}`)\n\n```{}\n",
        snippet.symbol_name,
        snippet.file_path,
        snippet.start_line,
        snippet.end_line,
        snippet.language
    );
    estimate_tokens(&format!("{header}{}\n```\n\n", snippet.content))
}

/// Estimates tokens for one impact list entry.
#[must_use]
pub fn estimate_impact_line_tokens(name: &str, file_path: &str, start: u32, end: u32) -> u32 {
    estimate_tokens(&format!("- **{name}** — `{file_path}:{start}-{end}`\n"))
}

/// Computes a recommended budget that fits the full desired bundle for this task.
#[must_use]
pub fn recommend_budget(
    task: ContextTask,
    wiki_tokens: u32,
    flow_wiki_tokens: u32,
    impact_line_tokens: &[u32],
    snippet_tokens: &[u32],
    max_snippets: usize,
) -> u32 {
    let mut total = OVERHEAD_TOKENS + wiki_tokens + flow_wiki_tokens;
    let impact_cap = impact_display_cap(task);
    for est in impact_line_tokens.iter().take(impact_cap) {
        total += est;
    }
    if !impact_line_tokens.is_empty() {
        total += estimate_tokens("## Impact\n\nSymbols affected if this changes:\n\n");
    }
    let snippet_cap = if max_snippets == usize::MAX {
        snippet_tokens.len()
    } else {
        max_snippets.min(snippet_tokens.len())
    };
    for est in snippet_tokens.iter().take(snippet_cap) {
        total += est;
    }
    if snippet_cap > 0 {
        total += estimate_tokens("## Code\n\n");
    }
    ((total as f32) * RECOMMEND_MARGIN).ceil() as u32
}

/// Max impact rows shown in the markdown bundle for a task.
#[must_use]
pub fn impact_display_cap(task: ContextTask) -> usize {
    match task {
        ContextTask::Onboard => 10,
        ContextTask::Fix => 20,
        ContextTask::Refactor => 30,
    }
}

/// Packs impact symbol ids to fit within `remaining` tokens (returns shown count).
pub fn pack_impact_count(line_tokens: &[u32], remaining: u32, cap: usize) -> usize {
    let mut used = 0u32;
    let header = estimate_tokens("## Impact\n\nSymbols affected if this changes:\n\n");
    if header > remaining {
        return 0;
    }
    used += header;
    let mut count = 0usize;
    for est in line_tokens.iter().take(cap) {
        if used + est > remaining && count > 0 {
            break;
        }
        used += est;
        count += 1;
    }
    count
}

/// Snippet/impact counts after packing (input to budget advice).
#[derive(Debug, Clone, Copy)]
pub struct PackingStats {
    pub snippets_included: usize,
    pub snippets_total: usize,
    pub impact_shown: usize,
    pub impact_total: usize,
}

/// Builds budget advice after packing.
#[must_use]
pub fn build_advice(
    task: ContextTask,
    requested_budget: u32,
    recommended_tokens: u32,
    estimated_tokens: u32,
    packing: PackingStats,
) -> BudgetAdvice {
    let impact_cap = impact_display_cap(task);
    let truncated = packing.snippets_included < packing.snippets_total
        || packing.impact_shown < packing.impact_total.min(impact_cap);
    BudgetAdvice {
        requested_budget,
        recommended_tokens,
        estimated_tokens,
        snippets_included: packing.snippets_included,
        snippets_omitted: packing
            .snippets_total
            .saturating_sub(packing.snippets_included),
        impact_entries_shown: packing.impact_shown,
        impact_entries_total: packing.impact_total,
        within_budget: !truncated && requested_budget >= recommended_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommend_budget_scales_with_snippets() {
        let small = recommend_budget(ContextTask::Fix, 100, 0, &[50, 50], &[200, 200], usize::MAX);
        let large = recommend_budget(
            ContextTask::Fix,
            100,
            0,
            &[50, 50],
            &[200, 200, 200, 200],
            usize::MAX,
        );
        assert!(large > small);
    }
}
