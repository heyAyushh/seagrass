//! Generic trie over ordered symbols.
//!
//! Three downstream consumers drive the design:
//!  - Completion prefix matching — symbols are `char`, values are completion items.
//!  - PDA seed prefix-collision detection — symbols are `u8`, values are PDA descriptors.
//!  - Module-path longest-prefix resolution — symbols are path segments (`String`).
//!
//! `BTreeMap` children keep iteration order deterministic without any external
//! crates, which matters for stable diagnostic output and reproducible tests.

use std::collections::BTreeMap;

/// A node in the trie.
///
/// Each node may hold a terminal value and a sorted map from the next symbol to
/// the child sub-trie.
#[derive(Debug, Clone)]
struct TrieNode<S, V> {
    /// The value stored at this key, present only when a key ends here.
    value: Option<V>,
    children: BTreeMap<S, TrieNode<S, V>>,
}

/// Manual `Default` avoids propagating `S: Default, V: Default` bounds from
/// `#[derive(Default)]`, which would unnecessarily restrict callers.
impl<S, V> Default for TrieNode<S, V> {
    fn default() -> Self {
        Self {
            value: None,
            children: BTreeMap::new(),
        }
    }
}

/// A generic trie keyed on sequences of `S` with leaf values of type `V`.
///
/// # Type parameters
/// - `S` — the symbol type; must be `Ord` for deterministic child ordering and
///   `Clone` so keys can be reconstructed during traversal.
/// - `V` — the value stored at each inserted key.
#[derive(Debug, Clone)]
pub struct Trie<S, V> {
    root: TrieNode<S, V>,
}

/// Manual `Default` — see `TrieNode` for rationale.
impl<S, V> Default for Trie<S, V> {
    fn default() -> Self {
        Self {
            root: TrieNode::default(),
        }
    }
}

impl<S: Ord + Clone, V> Trie<S, V> {
    /// Create an empty trie.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `value` at `key`, overwriting any previous value at that exact key.
    pub fn insert(&mut self, key: impl IntoIterator<Item = S>, value: V) {
        let mut node = &mut self.root;
        for symbol in key {
            node = node.children.entry(symbol).or_default();
        }
        node.value = Some(value);
    }

    /// Exact-match lookup. Returns `None` when the key was never inserted.
    pub fn get(&self, key: impl IntoIterator<Item = S>) -> Option<&V> {
        let mut node = &self.root;
        for symbol in key {
            node = node.children.get(&symbol)?;
        }
        node.value.as_ref()
    }

    /// Collect all `(key, &value)` pairs whose key starts with `prefix`.
    ///
    /// An empty prefix collects every entry in the trie.
    /// Results are yielded in lexicographic key order (because children use
    /// `BTreeMap`).
    pub fn collect_with_prefix(
        &self,
        prefix: impl IntoIterator<Item = S>,
    ) -> Vec<(Vec<S>, &V)> {
        // Walk down to the node that represents the end of `prefix`, collecting
        // the consumed symbols so we can reconstruct full keys below.
        let mut prefix_key: Vec<S> = Vec::new();
        let mut node = &self.root;

        for symbol in prefix {
            let Some(child) = node.children.get(&symbol) else {
                // No entry shares this prefix.
                return Vec::new();
            };
            prefix_key.push(symbol);
            node = child;
        }

        // Depth-first collection of all values reachable from `node`.
        let mut results = Vec::new();
        collect_subtree(node, &mut prefix_key, &mut results);
        results
    }

    /// Return the longest key stored in the trie that is a prefix of `key`.
    ///
    /// Returns `Some((depth, &value))` where `depth` is the number of symbols
    /// consumed from `key` that matched the stored prefix, or `None` when no
    /// stored key is a prefix of `key`.
    ///
    /// When `key` itself is stored, that is the longest match and is returned.
    pub fn longest_prefix_of(
        &self,
        key: impl IntoIterator<Item = S>,
    ) -> Option<(usize, &V)> {
        let mut node = &self.root;
        let mut depth: usize = 0;
        // Track the deepest node that had a value, together with its depth.
        let mut best: Option<(usize, &V)> = self.root.value.as_ref().map(|v| (0, v));

        for symbol in key {
            let Some(child) = node.children.get(&symbol) else {
                break;
            };
            depth += 1;
            node = child;
            if let Some(v) = &node.value {
                best = Some((depth, v));
            }
        }

        best
    }

    /// Return all pairs `(a, b)` where the stored key `a` is a **proper** prefix
    /// of the stored key `b`.
    ///
    /// Equal-length keys are **not** reported as a collision with themselves.
    /// Results are in lexicographic order of the shorter key.
    pub fn prefix_collisions(&self) -> Vec<(Vec<S>, Vec<S>)> {
        let mut pairs = Vec::new();
        find_collisions(&self.root, &mut Vec::new(), &mut pairs);
        pairs
    }
}

// --- helpers ----------------------------------------------------------------

/// Recursively collect every `(full_key, &value)` pair reachable from `node`.
///
/// `current_key` is the prefix accumulated so far; the function pushes/pops
/// symbols as it descends so no extra allocation is needed per level.
fn collect_subtree<'a, S: Clone, V>(
    node: &'a TrieNode<S, V>,
    current_key: &mut Vec<S>,
    results: &mut Vec<(Vec<S>, &'a V)>,
) {
    if let Some(value) = &node.value {
        results.push((current_key.clone(), value));
    }

    for (symbol, child) in &node.children {
        current_key.push(symbol.clone());
        collect_subtree(child, current_key, results);
        current_key.pop();
    }
}

/// Recursively find `(ancestor_key, descendant_key)` proper-prefix pairs.
///
/// When we encounter a node with a value at depth > 0, every descendant that
/// also has a value forms a collision pair.
fn find_collisions<S: Clone, V>(
    node: &TrieNode<S, V>,
    current_key: &mut Vec<S>,
    pairs: &mut Vec<(Vec<S>, Vec<S>)>,
) {
    // If the current node has a value, look for all strictly deeper values.
    if node.value.is_some() && !current_key.is_empty() {
        collect_longer_keys(node, current_key, pairs);
    }

    // Continue searching below regardless, so we also find chains like
    // a < b < c where b is a proper prefix of c.
    for (symbol, child) in &node.children {
        current_key.push(symbol.clone());
        find_collisions(child, current_key, pairs);
        current_key.pop();
    }
}

/// Given `ancestor_key` ending at `node`, emit pairs for every descendant that
/// carries a value.
fn collect_longer_keys<S: Clone, V>(
    node: &TrieNode<S, V>,
    ancestor_key: &[S],
    pairs: &mut Vec<(Vec<S>, Vec<S>)>,
) {
    for (symbol, child) in &node.children {
        let mut descendant_key: Vec<S> = ancestor_key.to_vec();
        descendant_key.push(symbol.clone());
        emit_descendant_values(child, descendant_key, ancestor_key.to_vec(), pairs);
    }
}

fn emit_descendant_values<S: Clone, V>(
    node: &TrieNode<S, V>,
    descendant_key: Vec<S>,
    ancestor_key: Vec<S>,
    pairs: &mut Vec<(Vec<S>, Vec<S>)>,
) {
    if node.value.is_some() {
        pairs.push((ancestor_key.clone(), descendant_key.clone()));
    }

    for (symbol, child) in &node.children {
        let mut next_key = descendant_key.clone();
        next_key.push(symbol.clone());
        emit_descendant_values(child, next_key, ancestor_key.clone(), pairs);
    }
}

// --- tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use {super::Trie, proptest::prelude::*};

    // Exact get / miss ---------------------------------------------------------

    #[test]
    fn exact_get_returns_inserted_value() {
        let mut trie = Trie::new();
        trie.insert("hello".chars(), 42u32);
        assert_eq!(trie.get("hello".chars()), Some(&42));
    }

    #[test]
    fn exact_get_misses_on_prefix_of_stored_key() {
        let mut trie = Trie::new();
        trie.insert("hello".chars(), 1u32);
        // "hell" was never inserted
        assert_eq!(trie.get("hell".chars()), None);
    }

    #[test]
    fn exact_get_misses_on_extension_of_stored_key() {
        let mut trie = Trie::new();
        trie.insert("hi".chars(), 1u32);
        assert_eq!(trie.get("hit".chars()), None);
    }

    #[test]
    fn exact_get_misses_on_empty_trie() {
        let trie: Trie<char, u32> = Trie::new();
        assert_eq!(trie.get("anything".chars()), None);
    }

    #[test]
    fn insert_overwrites_existing_value() {
        let mut trie = Trie::new();
        trie.insert("key".chars(), 1u32);
        trie.insert("key".chars(), 99u32);
        assert_eq!(trie.get("key".chars()), Some(&99));
    }

    // collect_with_prefix ------------------------------------------------------

    #[test]
    fn collect_with_empty_prefix_returns_all_entries() {
        let mut trie = Trie::new();
        trie.insert("ant".chars(), 1u32);
        trie.insert("anteater".chars(), 2);
        trie.insert("bee".chars(), 3);

        let mut all = trie
            .collect_with_prefix(std::iter::empty())
            .into_iter()
            .map(|(k, v)| (k.iter().collect::<String>(), *v))
            .collect::<Vec<_>>();
        all.sort();
        assert_eq!(
            all,
            vec![
                ("ant".to_string(), 1),
                ("anteater".to_string(), 2),
                ("bee".to_string(), 3),
            ]
        );
    }

    #[test]
    fn collect_with_prefix_returns_matching_entries() {
        let mut trie = Trie::new();
        trie.insert("ant".chars(), 1u32);
        trie.insert("anteater".chars(), 2);
        trie.insert("bee".chars(), 3);

        let matches = trie
            .collect_with_prefix("ant".chars())
            .into_iter()
            .map(|(k, v)| (k.iter().collect::<String>(), *v))
            .collect::<Vec<_>>();

        // lexicographic order from BTreeMap
        assert_eq!(
            matches,
            vec![("ant".to_string(), 1), ("anteater".to_string(), 2),]
        );
    }

    #[test]
    fn collect_with_prefix_returns_exact_match_only_when_no_extensions() {
        let mut trie = Trie::new();
        trie.insert("bee".chars(), 3u32);

        let matches = trie
            .collect_with_prefix("bee".chars())
            .into_iter()
            .map(|(k, v)| (k.iter().collect::<String>(), *v))
            .collect::<Vec<_>>();

        assert_eq!(matches, vec![("bee".to_string(), 3)]);
    }

    #[test]
    fn collect_with_prefix_returns_empty_when_prefix_absent() {
        let mut trie = Trie::new();
        trie.insert("ant".chars(), 1u32);

        let matches = trie.collect_with_prefix("xyz".chars());
        assert!(matches.is_empty());
    }

    #[test]
    fn collect_with_prefix_includes_internal_node_with_value() {
        // "an" is stored AND "ant" is stored; both should appear under "an"
        let mut trie = Trie::new();
        trie.insert("an".chars(), 10u32);
        trie.insert("ant".chars(), 11);

        let matches: Vec<_> = trie
            .collect_with_prefix("an".chars())
            .into_iter()
            .map(|(k, v)| (k.iter().collect::<String>(), *v))
            .collect();

        assert_eq!(
            matches,
            vec![("an".to_string(), 10), ("ant".to_string(), 11),]
        );
    }

    // longest_prefix_of --------------------------------------------------------

    #[test]
    fn longest_prefix_exact_match() {
        let mut trie = Trie::new();
        trie.insert(["a", "b", "c"], "leaf");
        let result = trie.longest_prefix_of(["a", "b", "c"]);
        assert_eq!(result, Some((3, &"leaf")));
    }

    #[test]
    fn longest_prefix_key_is_longer_than_stored() {
        let mut trie = Trie::new();
        trie.insert(["a", "b"], "ab");
        // Query ["a","b","c"] — stored "a","b" is the longest prefix
        let result = trie.longest_prefix_of(["a", "b", "c"]);
        assert_eq!(result, Some((2, &"ab")));
    }

    #[test]
    fn longest_prefix_picks_deepest_match() {
        let mut trie = Trie::new();
        trie.insert(["a"], "a");
        trie.insert(["a", "b"], "ab");
        trie.insert(["a", "b", "c"], "abc");
        let result = trie.longest_prefix_of(["a", "b", "c", "d"]);
        assert_eq!(result, Some((3, &"abc")));
    }

    #[test]
    fn longest_prefix_no_match() {
        let mut trie = Trie::new();
        trie.insert(["x", "y"], "xy");
        let result = trie.longest_prefix_of(["a", "b"]);
        assert_eq!(result, None);
    }

    #[test]
    fn longest_prefix_empty_trie() {
        let trie: Trie<&str, u32> = Trie::new();
        assert_eq!(trie.longest_prefix_of(["a"]), None);
    }

    // prefix_collisions --------------------------------------------------------

    #[test]
    fn no_collisions_when_keys_are_disjoint() {
        let mut trie = Trie::new();
        trie.insert("cat".chars(), 1u32);
        trie.insert("dog".chars(), 2);
        assert!(trie.prefix_collisions().is_empty());
    }

    #[test]
    fn reports_one_prefix_collision() {
        let mut trie = Trie::new();
        trie.insert("se".chars(), 1u32);
        trie.insert("sea".chars(), 2);

        let pairs: Vec<_> = trie
            .prefix_collisions()
            .into_iter()
            .map(|(a, b)| (a.iter().collect::<String>(), b.iter().collect::<String>()))
            .collect();

        assert_eq!(pairs, vec![("se".to_string(), "sea".to_string())]);
    }

    #[test]
    fn reports_chain_of_prefix_collisions() {
        let mut trie = Trie::new();
        trie.insert("a".chars(), 1u32);
        trie.insert("ab".chars(), 2);
        trie.insert("abc".chars(), 3);

        let pairs: Vec<_> = trie
            .prefix_collisions()
            .into_iter()
            .map(|(a, b)| (a.iter().collect::<String>(), b.iter().collect::<String>()))
            .collect();

        // "a" is a proper prefix of "ab" and of "abc"; "ab" is a proper prefix of "abc"
        assert!(pairs.contains(&("a".to_string(), "ab".to_string())));
        assert!(pairs.contains(&("a".to_string(), "abc".to_string())));
        assert!(pairs.contains(&("ab".to_string(), "abc".to_string())));
    }

    #[test]
    fn equal_keys_are_not_reported_as_collision() {
        let mut trie = Trie::new();
        // Two insertions at the same key — only one entry survives.
        trie.insert("same".chars(), 1u32);
        trie.insert("same".chars(), 2);
        assert!(trie.prefix_collisions().is_empty());
    }

    #[test]
    fn no_collision_when_longer_key_is_inserted_without_its_prefix() {
        let mut trie = Trie::new();
        // Only "sea" is inserted; "se" is NOT stored, so no collision.
        trie.insert("sea".chars(), 1u32);
        assert!(trie.prefix_collisions().is_empty());
    }

    // byte-sequence variant (simulates PDA seed usage) -------------------------

    #[test]
    fn byte_sequence_exact_get() {
        let mut trie: Trie<u8, &str> = Trie::new();
        trie.insert([0u8, 1, 2], "pda_a");
        assert_eq!(trie.get([0u8, 1, 2]), Some(&"pda_a"));
        assert_eq!(trie.get([0u8, 1]), None);
    }

    #[test]
    fn byte_sequence_prefix_collision() {
        let mut trie: Trie<u8, &str> = Trie::new();
        trie.insert([0u8, 1], "short_pda");
        trie.insert([0u8, 1, 2], "long_pda");

        let pairs = trie.prefix_collisions();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (vec![0u8, 1], vec![0u8, 1, 2]));
    }

    // proptest -----------------------------------------------------------------

    proptest! {
        /// Every key inserted into the trie is retrievable by exact get.
        #[test]
        fn inserted_key_always_retrievable(keys in prop::collection::vec(
            prop::collection::vec(0u8..=127, 0..8),
            1..10,
        )) {
            let mut trie: Trie<u8, usize> = Trie::new();
            for (i, key) in keys.iter().enumerate() {
                trie.insert(key.iter().copied(), i);
            }
            // The last value inserted for each distinct key must be retrievable.
            // Build a reference map to track the final value per key.
            let mut reference: std::collections::HashMap<Vec<u8>, usize> =
                std::collections::HashMap::new();
            for (i, key) in keys.iter().enumerate() {
                reference.insert(key.clone(), i);
            }
            for (key, expected) in &reference {
                prop_assert_eq!(trie.get(key.iter().copied()), Some(expected));
            }
        }

        /// Every inserted key appears in collect_with_prefix of its own key.
        #[test]
        fn inserted_key_appears_under_its_own_prefix(keys in prop::collection::vec(
            prop::collection::vec(0u8..=127, 1..6),
            1..8,
        )) {
            let mut trie: Trie<u8, usize> = Trie::new();
            for (i, key) in keys.iter().enumerate() {
                trie.insert(key.iter().copied(), i);
            }
            for key in &keys {
                let collected = trie.collect_with_prefix(key.iter().copied());
                let found = collected.iter().any(|(k, _)| k == key);
                prop_assert!(found, "key {:?} not found under its own prefix", key);
            }
        }
    }
}
