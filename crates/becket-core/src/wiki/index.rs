//! In-memory wiki page index (single directory scan).

use std::collections::HashMap;

use becket_schema::wiki::{WikiPage, WikiPageKind};

use crate::error::CoreError;
use crate::wiki::store::WikiStore;
use crate::wiki::util::page_kind_priority;

/// All wiki pages loaded once for fast symbol lookups.
#[derive(Debug, Clone, Default)]
pub struct WikiPageIndex {
    pages: Vec<WikiPage>,
    by_id: HashMap<String, usize>,
}

impl WikiPageIndex {
    /// Loads every page under `.becket/wiki/` in one pass.
    pub fn load(store: &WikiStore) -> Result<Self, CoreError> {
        let mut pages = Vec::new();
        let mut by_id = HashMap::new();
        if !store.wiki_dir().exists() {
            return Ok(Self { pages, by_id });
        }
        for entry in std::fs::read_dir(store.wiki_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem == "index" {
                continue;
            }
            let page = crate::wiki::store::load_page_from_path(&path)?;
            let idx = pages.len();
            by_id.insert(page.meta.id.clone(), idx);
            pages.push(page);
        }
        Ok(Self { pages, by_id })
    }

    /// Returns a page by stable id.
    #[must_use]
    pub fn get(&self, page_id: &str) -> Option<&WikiPage> {
        self.by_id.get(page_id).map(|&i| &self.pages[i])
    }

    /// Best anchored page for a symbol (service > module > flow).
    #[must_use]
    pub fn best_for_symbol(&self, symbol_id: &str) -> Option<&WikiPage> {
        let mut best: Option<(u8, &WikiPage)> = None;
        for page in &self.pages {
            if !page.meta.symbol_ids.iter().any(|s| s == symbol_id) {
                continue;
            }
            let priority = page_kind_priority(page.meta.kind);
            if best.as_ref().map_or(true, |(p, _)| priority < *p) {
                best = Some((priority, page));
            }
        }
        best.map(|(_, p)| p)
    }

    /// Shortest flow page that includes `symbol_id`.
    #[must_use]
    pub fn best_flow_for_symbol(&self, symbol_id: &str) -> Option<&WikiPage> {
        let mut best: Option<(usize, &WikiPage)> = None;
        for page in &self.pages {
            if page.meta.kind != WikiPageKind::Flow {
                continue;
            }
            if !page.meta.symbol_ids.iter().any(|s| s == symbol_id) {
                continue;
            }
            let steps = page.meta.symbol_ids.len();
            if best.as_ref().map_or(true, |(n, _)| steps < *n) {
                best = Some((steps, page));
            }
        }
        best.map(|(_, p)| p)
    }
}
