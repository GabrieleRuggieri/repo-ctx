//! Grounded knowledge wiki — graph-compiled markdown pages.

pub mod compiler;
pub mod fingerprint;
pub mod index;
pub mod lint;
pub mod store;
pub mod util;

pub use compiler::WikiCompiler;
pub use index::WikiPageIndex;
pub use lint::{claim_calls_valid, WikiLinter};
pub use store::{find_flow_page_for_symbol, find_page_for_symbol, load_page_from_path, WikiStore};
pub use util::{
    extract_prose_content, needs_prose_enrichment, replace_prose_slot, sanitize_for_context,
    wiki_adds_context, PROSE_PLACEHOLDER, PROSE_SLOT,
};
