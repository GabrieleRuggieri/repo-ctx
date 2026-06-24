//! Resolves parsed call edges to symbol ids in the index.

use std::collections::HashMap;

use repoctx_schema::artifacts::{DependencyEdgeRecord, SymbolRecord};
use repoctx_schema::edge::EdgeType;
use uuid::Uuid;

use crate::parse::ParsedCall;

/// Builds dependency edges from unresolved calls and the symbol catalog.
pub struct GraphResolver;

impl GraphResolver {
    /// Resolves call edges to [`DependencyEdgeRecord`] entries.
    ///
    /// Resolution order for a callee name: same file by name, then any file by name.
    pub fn resolve_calls(
        symbols: &[SymbolRecord],
        calls: &[ParsedCall],
    ) -> Vec<DependencyEdgeRecord> {
        let by_id: HashMap<&str, &SymbolRecord> =
            symbols.iter().map(|s| (s.id.as_str(), s)).collect();
        let mut by_name: HashMap<&str, Vec<&SymbolRecord>> = HashMap::new();
        for symbol in symbols {
            by_name
                .entry(symbol.name.as_str())
                .or_default()
                .push(symbol);
        }

        let mut edges = Vec::new();
        for call in calls {
            let Some(caller) = by_id.get(call.caller_symbol_id.as_str()) else {
                continue;
            };
            let Some(callee) = Self::resolve_callee(caller, &call.callee_name, &by_name) else {
                continue;
            };
            if caller.id == callee.id {
                continue;
            }
            edges.push(DependencyEdgeRecord {
                id: Uuid::new_v4().to_string(),
                src_symbol_id: caller.id.clone(),
                dst_symbol_id: callee.id.clone(),
                edge_type: EdgeType::Calls,
                boundary: None,
                confidence: 1.0,
            });
        }
        edges
    }

    fn resolve_callee<'a>(
        caller: &SymbolRecord,
        callee_name: &str,
        by_name: &HashMap<&str, Vec<&'a SymbolRecord>>,
    ) -> Option<&'a SymbolRecord> {
        let candidates = by_name.get(callee_name)?;
        candidates
            .iter()
            .find(|s| s.file_path == caller.file_path)
            .copied()
            .or_else(|| candidates.first().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use repoctx_schema::symbol::{SymbolKind, Visibility};

    fn sym(id: &str, name: &str, file: &str) -> SymbolRecord {
        SymbolRecord {
            id: id.into(),
            kind: SymbolKind::Function,
            name: name.into(),
            fqn: format!("{file}::{name}"),
            file_path: file.into(),
            start_line: 1,
            end_line: 1,
            visibility: Visibility::Public,
            module_id: None,
        }
    }

    #[test]
    fn resolves_call_chain() {
        let symbols = vec![
            sym("a", "func_a", "src/g.rs"),
            sym("b", "func_b", "src/g.rs"),
            sym("c", "func_c", "src/g.rs"),
        ];
        let calls = vec![
            ParsedCall {
                caller_symbol_id: "a".into(),
                callee_name: "func_b".into(),
            },
            ParsedCall {
                caller_symbol_id: "b".into(),
                callee_name: "func_c".into(),
            },
        ];
        let edges = GraphResolver::resolve_calls(&symbols, &calls);
        assert_eq!(edges.len(), 2);
    }
}
