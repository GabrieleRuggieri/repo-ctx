//! Auto-discovers business flows from folder names and call-graph traversal.

use std::collections::{HashMap, HashSet, VecDeque};

use repoctx_schema::artifacts::{FlowRecord, FlowStepRecord, SymbolRecord};
use uuid::Uuid;

/// Edge endpoints for call-graph walks.
#[derive(Debug, Clone)]
pub struct CallEdge {
    /// Source symbol id.
    pub src: String,
    /// Target symbol id.
    pub dst: String,
}

/// Reconstructs flows from path-derived domain names and call edges.
pub struct FlowReconstructor;

impl FlowReconstructor {
    /// Builds flow records from symbols and call edges.
    ///
    /// Domains are inferred from path segments (e.g. `src/payment/...` → `payment`).
    /// Steps follow downstream call edges starting at an entrypoint or root symbol.
    pub fn reconstruct(symbols: &[SymbolRecord], edges: &[CallEdge]) -> Vec<FlowRecord> {
        let domains = discover_domains(symbols);
        let mut flows = Vec::new();

        for domain in domains {
            if let Some(flow) = Self::build_flow(&domain, symbols, edges) {
                flows.push(flow);
            }
        }

        flows
    }

    fn build_flow(
        domain: &str,
        symbols: &[SymbolRecord],
        edges: &[CallEdge],
    ) -> Option<FlowRecord> {
        let domain_symbols: Vec<&SymbolRecord> = symbols
            .iter()
            .filter(|s| path_matches_domain(&s.file_path, domain))
            .collect();

        if domain_symbols.is_empty() {
            return None;
        }

        let domain_ids: HashSet<&str> = domain_symbols.iter().map(|s| s.id.as_str()).collect();

        let root = domain_symbols
            .iter()
            .find(|s| s.name == "main")
            .or_else(|| {
                domain_symbols
                    .iter()
                    .find(|s| matches!(s.kind, repoctx_schema::symbol::SymbolKind::Function))
            })?;

        let adjacency = build_adjacency(edges);
        let ordered_ids = bfs_order(root.id.as_str(), &adjacency, &domain_ids);

        let steps: Vec<FlowStepRecord> = ordered_ids
            .into_iter()
            .enumerate()
            .map(|(order, symbol_id)| FlowStepRecord {
                order: order as u32,
                symbol_id,
                external_system: None,
            })
            .collect();

        if steps.is_empty() {
            return None;
        }

        Some(FlowRecord {
            id: Uuid::new_v4().to_string(),
            name: domain.to_string(),
            description: Some(format!("Auto-discovered flow from path segment '{domain}'")),
            steps,
        })
    }
}

fn discover_domains(symbols: &[SymbolRecord]) -> Vec<String> {
    const SKIP: &[&str] = &[
        "src", "lib", "bin", "app", "tests", "test", "spec", "crates", "pkg", "internal", "cmd",
        "main",
    ];

    let mut domains = HashSet::new();
    for symbol in symbols {
        for segment in symbol.file_path.split('/') {
            let lower = segment.to_lowercase();
            if lower.len() >= 3 && !SKIP.contains(&lower.as_str()) {
                domains.insert(lower);
            }
        }
    }

    let mut list: Vec<String> = domains.into_iter().collect();
    list.sort();
    list
}

fn path_matches_domain(file_path: &str, domain: &str) -> bool {
    file_path
        .split('/')
        .any(|segment| segment.eq_ignore_ascii_case(domain))
}

fn build_adjacency(edges: &[CallEdge]) -> HashMap<&str, Vec<&str>> {
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        adjacency
            .entry(edge.src.as_str())
            .or_default()
            .push(edge.dst.as_str());
    }
    adjacency
}

fn bfs_order(
    root_id: &str,
    adjacency: &HashMap<&str, Vec<&str>>,
    allowed: &HashSet<&str>,
) -> Vec<String> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut order = Vec::new();

    if allowed.contains(root_id) {
        queue.push_back(root_id);
        visited.insert(root_id);
    }

    while let Some(current) = queue.pop_front() {
        order.push(current.to_string());
        if let Some(neighbors) = adjacency.get(current) {
            for next in neighbors {
                if allowed.contains(next) && visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use repoctx_schema::symbol::{SymbolKind, Visibility};

    fn sym(id: &str, name: &str, path: &str) -> SymbolRecord {
        SymbolRecord {
            id: id.into(),
            kind: SymbolKind::Function,
            name: name.into(),
            fqn: format!("{path}::{name}"),
            file_path: path.into(),
            start_line: 1,
            end_line: 1,
            visibility: Visibility::Public,
            module_id: None,
        }
    }

    #[test]
    fn discovers_payment_domain_from_path() {
        let symbols = vec![
            sym("1", "checkout", "src/payment/checkout.rs"),
            sym("2", "charge", "src/payment/gateway.rs"),
        ];
        let domains = discover_domains(&symbols);
        assert!(domains.contains(&"payment".to_string()));
    }

    #[test]
    fn builds_flow_steps_in_call_order() {
        let symbols = vec![
            sym("a", "checkout", "src/payment/checkout.rs"),
            sym("b", "charge", "src/payment/checkout.rs"),
            sym("c", "validate", "src/payment/checkout.rs"),
        ];
        let edges = vec![
            CallEdge {
                src: "a".into(),
                dst: "b".into(),
            },
            CallEdge {
                src: "b".into(),
                dst: "c".into(),
            },
        ];
        let flows = FlowReconstructor::reconstruct(&symbols, &edges);
        let payment = flows
            .iter()
            .find(|f| f.name == "payment")
            .expect("payment flow");
        assert_eq!(payment.steps.len(), 3);
        assert_eq!(payment.steps[0].symbol_id, "a");
    }
}
