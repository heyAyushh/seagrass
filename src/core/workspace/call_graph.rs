//! Name-keyed workspace call graph.
//!
//! Edges are syntactic and deliberately incomplete. Defense queries may use any
//! edge because the failure mode is silence; offense queries must require
//! unambiguous callees before making a new claim.

use {
    super::indexing::IndexedFunctionEntry,
    std::collections::{HashMap, HashSet, VecDeque},
};

pub(crate) const MAX_REACHABILITY_DEPTH: usize = 8;

#[derive(Debug, Default, Clone)]
pub(crate) struct CallGraph {
    edges: HashMap<String, Vec<String>>,
    definition_counts: HashMap<String, usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Reachability {
    pub(crate) names: HashSet<String>,
    pub(crate) truncated: bool,
}

impl CallGraph {
    pub(crate) fn build<'a>(functions: impl IntoIterator<Item = &'a IndexedFunctionEntry>) -> Self {
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut definition_counts: HashMap<String, usize> = HashMap::new();

        for function in functions {
            *definition_counts.entry(function.name.clone()).or_default() += 1;
            let callees = edges.entry(function.name.clone()).or_default();
            for call in &function.calls {
                if !callees.iter().any(|existing| existing == call) {
                    callees.push(call.clone());
                }
            }
        }

        for callees in edges.values_mut() {
            callees.sort();
        }

        Self {
            edges,
            definition_counts,
        }
    }

    pub(crate) fn reachable_from(&self, from: &str) -> Reachability {
        self.reachable_from_with_policy(from, AmbiguityPolicy::AllowAmbiguous)
    }

    pub(crate) fn unambiguous_reachable_from(&self, from: &str) -> Reachability {
        self.reachable_from_with_policy(from, AmbiguityPolicy::RequireUnambiguous)
    }

    pub(crate) fn is_unambiguous(&self, name: &str) -> bool {
        self.definition_counts.get(name).copied() == Some(1)
    }

    fn reachable_from_with_policy(
        &self,
        from: &str,
        ambiguity_policy: AmbiguityPolicy,
    ) -> Reachability {
        let mut reachability = Reachability::default();
        let mut pending = self
            .edges
            .get(from)
            .into_iter()
            .flat_map(|callees| callees.iter().cloned())
            .map(|callee| (callee, 1_usize))
            .collect::<VecDeque<_>>();

        while let Some((name, depth)) = pending.pop_front() {
            if ambiguity_policy == AmbiguityPolicy::RequireUnambiguous
                && !self.is_unambiguous(&name)
            {
                continue;
            }
            if !reachability.names.insert(name.clone()) {
                continue;
            }

            let Some(callees) = self.edges.get(&name) else {
                continue;
            };
            if depth >= MAX_REACHABILITY_DEPTH {
                reachability.truncated |= !callees.is_empty();
                continue;
            }
            pending.extend(callees.iter().cloned().map(|callee| (callee, depth + 1)));
        }

        reachability
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmbiguityPolicy {
    AllowAmbiguous,
    RequireUnambiguous,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        tower_lsp::lsp_types::{Location, Range, Url},
    };

    fn entry(name: &str, calls: &[&str]) -> IndexedFunctionEntry {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        IndexedFunctionEntry {
            uri: uri.clone(),
            is_open: true,
            name: name.to_string(),
            context_name: None,
            return_type_display: None,
            location: Location {
                uri,
                range: Range::default(),
            },
            is_program_instruction: false,
            calls: calls.iter().map(|call| call.to_string()).collect(),
            cpi_program_usages: Vec::new(),
            signer_usages: Vec::new(),
            signer_checks: Vec::new(),
            account_key_comparisons: Vec::new(),
            arguments: Vec::new(),
        }
    }

    #[test]
    fn call_graph_reaches_linear_chain() {
        let graph = CallGraph::build([&entry("a", &["b"]), &entry("b", &["c"]), &entry("c", &[])]);

        let reachable = graph.reachable_from("a");

        assert_eq!(
            reachable.names,
            ["b".to_string(), "c".to_string()].into_iter().collect()
        );
        assert!(!reachable.truncated);
    }

    #[test]
    fn call_graph_reaches_diamond_once() {
        let graph = CallGraph::build([
            &entry("a", &["b", "c"]),
            &entry("b", &["d"]),
            &entry("c", &["d"]),
            &entry("d", &[]),
        ]);

        let reachable = graph.reachable_from("a");

        assert_eq!(reachable.names.len(), 3);
        assert!(reachable.names.contains("d"));
    }

    #[test]
    fn call_graph_cycle_terminates() {
        let graph = CallGraph::build([&entry("a", &["b"]), &entry("b", &["a"])]);

        let reachable = graph.reachable_from("a");

        assert!(reachable.names.contains("b"));
        assert!(reachable.names.contains("a"));
        assert!(!reachable.truncated);
    }

    #[test]
    fn call_graph_depth_cap_sets_truncated() {
        let entries = (0..=MAX_REACHABILITY_DEPTH + 1)
            .map(|index| {
                let name = format!("f{index}");
                let call = format!("f{}", index + 1);
                entry(&name, &[&call])
            })
            .collect::<Vec<_>>();
        let graph = CallGraph::build(entries.iter());

        let reachable = graph.reachable_from("f0");

        assert!(reachable.truncated);
        assert!(reachable
            .names
            .contains(&format!("f{MAX_REACHABILITY_DEPTH}")));
        assert!(!reachable
            .names
            .contains(&format!("f{}", MAX_REACHABILITY_DEPTH + 1)));
    }

    #[test]
    fn call_graph_counts_duplicate_definitions() {
        let first = entry("helper", &[]);
        let second = entry("helper", &[]);
        let graph = CallGraph::build([&entry("handler", &["helper"]), &first, &second]);

        assert!(!graph.is_unambiguous("helper"));
        assert!(!graph
            .unambiguous_reachable_from("handler")
            .names
            .contains("helper"));
        assert!(graph.reachable_from("handler").names.contains("helper"));
    }

    #[test]
    fn call_graph_unknown_name_is_empty() {
        let graph = CallGraph::build([&entry("handler", &[])]);

        assert!(graph.reachable_from("missing").names.is_empty());
    }
}
