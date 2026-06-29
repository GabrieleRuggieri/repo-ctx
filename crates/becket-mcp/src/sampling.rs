//! Lazy LLM enrichment via MCP sampling (host-delegated).

use std::path::Path;

use becket_core::redact_secrets;
use becket_core::wiki::{
    extract_prose_content, replace_prose_slot, sanitize_for_context, wiki_adds_context, WikiStore,
    PROSE_SLOT,
};
use becket_query::assemble::refresh_context_markdown;
use becket_query::types::{ContextResult, FlowResult, SummarySource};
use becket_schema::artifacts::FlowRecord;
use becket_store::{BecketPaths, EnrichmentRecord, IndexStore};
use rmcp::{
    model::{CreateMessageRequestParams, CreateMessageResult, SamplingMessage},
    Peer, RoleServer,
};
use tracing::debug;

const ENTITY_FLOW: &str = "flow";
const ENTITY_WIKI: &str = "wiki";
const SOURCE_SAMPLING: &str = "mcp_sampling";

/// Returns true when the connected MCP client can handle `sampling/createMessage`.
pub fn client_supports_sampling(peer: &Peer<RoleServer>) -> bool {
    let Some(info) = peer.peer_info() else {
        return false;
    };
    let caps = &info.capabilities;
    if caps
        .tasks
        .as_ref()
        .map(|tasks| tasks.supports_sampling_create_message())
        .unwrap_or(false)
    {
        return true;
    }
    caps.sampling.is_some()
}

/// Loads or generates an enriched flow description.
pub async fn enrich_flow_description(
    peer: &Peer<RoleServer>,
    repo_root: &Path,
    flow: &FlowRecord,
) -> Option<String> {
    if !client_supports_sampling(peer) {
        return None;
    }

    let paths = BecketPaths::new(repo_root);
    let flow_id = flow.id.clone();

    if let Some(cached) = load_cached(ENTITY_FLOW, &flow_id, &paths.index_db) {
        return Some(cached);
    }

    let prompt = build_flow_prompt(flow);
    let summary = request_summary(peer, &prompt).await?;
    cache_enrichment(ENTITY_FLOW, &flow_id, &summary, &paths.index_db);
    Some(summary)
}

/// Applies cached or freshly sampled wiki prose enrichment to a context bundle.
pub async fn apply_context_enrichment(
    peer: &Peer<RoleServer>,
    repo_root: &Path,
    mut context: ContextResult,
) -> ContextResult {
    let paths = BecketPaths::new(repo_root);
    let wiki_store = WikiStore::new(&paths);

    if let Some(page_id) = context.wiki_page_id.clone() {
        if let Ok(Some(page)) = wiki_store.load_page(&page_id) {
            let enriched = enrich_wiki_prose(peer, repo_root, page).await;
            let sanitized = sanitize_for_context(&enriched.body);
            if wiki_adds_context(&sanitized) {
                context.wiki_body = Some(sanitized);
                context.enriched_summary = extract_prose_content(&enriched.body);
                context.summary_source = SummarySource::McpSampling;
            }
        }
    }

    if matches!(context.task, becket_query::ContextTask::Onboard) {
        if let Some(flow_id) = context.flow_wiki_page_id.clone() {
            if let Ok(Some(page)) = wiki_store.load_page(&flow_id) {
                let enriched = enrich_wiki_prose(peer, repo_root, page).await;
                let sanitized = sanitize_for_context(&enriched.body);
                if wiki_adds_context(&sanitized) {
                    context.flow_wiki_body = Some(sanitized);
                }
            }
        }
    }

    context.markdown = refresh_context_markdown(&context);
    context
}

/// Applies cached or freshly sampled enrichment to a flow result.
pub async fn apply_flow_enrichment(
    peer: &Peer<RoleServer>,
    repo_root: &Path,
    mut result: FlowResult,
) -> FlowResult {
    let Some(flow) = result.flow.as_ref() else {
        return result;
    };
    if let Some(description) = enrich_flow_description(peer, repo_root, flow).await {
        result.enriched_description = Some(description);
        result.description_source = SummarySource::McpSampling;
    }
    result
}

/// Fills the prose slot on a wiki page via MCP sampling when the host supports it.
pub async fn enrich_wiki_prose(
    peer: &Peer<RoleServer>,
    repo_root: &Path,
    mut page: becket_schema::wiki::WikiPage,
) -> becket_schema::wiki::WikiPage {
    if !client_supports_sampling(peer) || !page.body.contains(PROSE_SLOT) {
        return page;
    }

    let paths = BecketPaths::new(repo_root);
    let page_id = page.meta.id.clone();

    if let Some(cached) = load_cached(ENTITY_WIKI, &page_id, &paths.index_db) {
        page.body = replace_prose_slot(&page.body, &cached);
        page.meta.source = becket_schema::wiki::WikiPageSource::McpSampling;
        let _ = WikiStore::new(&paths).write_page(&page.meta, &page.body);
        return page;
    }

    let prompt = build_wiki_prompt(&page);
    let Some(prose) = request_summary(peer, &prompt).await else {
        return page;
    };
    cache_enrichment(ENTITY_WIKI, &page_id, &prose, &paths.index_db);
    page.body = replace_prose_slot(&page.body, &prose);
    page.meta.source = becket_schema::wiki::WikiPageSource::McpSampling;
    let _ = WikiStore::new(&paths).write_page(&page.meta, &page.body);
    page
}

fn build_wiki_prompt(page: &becket_schema::wiki::WikiPage) -> String {
    let facts = format!(
        "title: {}\nkind: {:?}\nanchored_symbols: {}\nbody_excerpt:\n{}",
        page.meta.title,
        page.meta.kind,
        page.meta.symbol_ids.join(", "),
        page.body.chars().take(1200).collect::<String>(),
    );
    let redacted = redact_secrets(&facts);
    format!(
        "Write 2-4 sentences of intent and gotchas for this grounded wiki page. \
Use ONLY the facts below. Do not invent APIs or behavior.\n\n{redacted}"
    )
}

fn load_cached(entity_kind: &str, entity_id: &str, db_path: &Path) -> Option<String> {
    let store = IndexStore::open(db_path).ok()?;
    store
        .get_enrichment(entity_kind, entity_id)
        .ok()
        .flatten()
        .map(|record| record.summary)
}

fn cache_enrichment(entity_kind: &str, entity_id: &str, summary: &str, db_path: &Path) {
    if let Ok(store) = IndexStore::open(db_path) {
        let _ = store.upsert_enrichment(&EnrichmentRecord {
            entity_kind: entity_kind.to_string(),
            entity_id: entity_id.to_string(),
            summary: summary.to_string(),
            source: SOURCE_SAMPLING.to_string(),
        });
    }
}

fn build_flow_prompt(flow: &FlowRecord) -> String {
    let steps: Vec<String> = flow
        .steps
        .iter()
        .map(|step| format!("{} ({})", step.symbol_id, step.order))
        .collect();
    let facts = format!(
        "flow: {}\ndescription: {}\nsteps: {}",
        flow.name,
        flow.description.as_deref().unwrap_or("(none)"),
        steps.join(" -> "),
    );
    let redacted = redact_secrets(&facts);
    format!(
        "Summarize this business flow in 1-2 concise sentences for an AI coding agent. \
Use ONLY the facts below.\n\n{redacted}"
    )
}

async fn request_summary(peer: &Peer<RoleServer>, prompt: &str) -> Option<String> {
    let params = CreateMessageRequestParams::new(vec![SamplingMessage::user_text(prompt)], 256)
        .with_system_prompt(
            "You write concise, factual summaries of code symbols and flows for developers.",
        );

    #[allow(deprecated)]
    let result = peer.create_message(params).await.ok()?;
    let text = extract_sampling_text(&result)?;
    debug!(chars = text.len(), "received MCP sampling enrichment");
    Some(text)
}

fn extract_sampling_text(result: &CreateMessageResult) -> Option<String> {
    result
        .message
        .content
        .iter()
        .find_map(|content| content.as_text().map(|text| text.text.trim().to_string()))
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use becket_schema::artifacts::{FlowRecord, FlowStepRecord};

    #[test]
    fn build_flow_prompt_lists_steps() {
        let flow = FlowRecord {
            id: "flow1".into(),
            name: "payment".into(),
            description: Some("auto".into()),
            steps: vec![
                FlowStepRecord {
                    order: 1,
                    symbol_id: "a".into(),
                    external_system: None,
                },
                FlowStepRecord {
                    order: 2,
                    symbol_id: "b".into(),
                    external_system: None,
                },
            ],
        };
        let prompt = build_flow_prompt(&flow);
        assert!(prompt.contains("payment"));
        assert!(prompt.contains("a"));
    }
}
