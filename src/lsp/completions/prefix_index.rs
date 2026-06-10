//! Static trie-backed prefix indexes for completion candidate sets that are
//! fixed at compile time.
//!
//! Why a trie here instead of a linear scan?
//!
//! Both the constraint-key catalog (~43 entries) and the anchor field-type
//! catalog (~60 entries) are queried on *every keystroke* while the user types
//! inside an `#[account(…)]` attribute or an accounts-struct field.  The linear
//! `.filter(|s| s.starts_with(prefix))` approach is O(n) per query.  A trie
//! reduces that to O(|prefix| + |results|), which is constant for the common
//! single-character prefix case.
//!
//! The indexes are built once via `OnceLock` and reused for the lifetime of the
//! server process, so the one-time construction cost is paid only on the first
//! completion request of each type.
//!
//! Keys are stored in lowercase so that prefix queries can be answered with a
//! single case-fold of the typed prefix, matching the normalisation already used
//! throughout the completion providers.

use {
    crate::{anchor_types, collections::Trie, constraint_catalog},
    std::{collections::BTreeMap, sync::OnceLock},
};

// ---------------------------------------------------------------------------
// Constraint-key trie
// ---------------------------------------------------------------------------

/// Trie over the constraint catalog.
///
/// Each entry maps a **lowercase constraint key** (the label with any trailing
/// ` =` removed, e.g. `"payer"`, `"seeds::program"`) to the index of the
/// corresponding `ConstraintSpec` in `constraint_catalog::CONSTRAINTS`.
pub(super) struct ConstraintKeyIndex {
    trie: Trie<char, usize>,
}

impl ConstraintKeyIndex {
    fn build() -> Self {
        let mut trie = Trie::new();
        for (index, spec) in constraint_catalog::CONSTRAINTS.iter().enumerate() {
            // Key on the full lowercase label (e.g. `"payer ="`, `"bump ="`,
            // `"bump"`).  Two entries share the same stripped key when a
            // constraint exists both as a flag (`bump`) and as an assignment
            // (`bump =`).  Keying on the full label avoids the overwrite problem
            // while still matching correctly: `collect_with_prefix("bump")`
            // returns both `"bump"` and `"bump ="` because `"bump ="` starts
            // with `"bump"`.
            let key = spec.label.to_ascii_lowercase();
            trie.insert(key.chars(), index);
        }
        Self { trie }
    }

    /// Return every `ConstraintSpec` whose label starts with `prefix`
    /// (case-insensitive).
    ///
    /// This mirrors the original `constraint_matches_prefix` behaviour: a
    /// prefix of `"bump"` matches both `"bump"` and `"bump ="` because the
    /// trie traversal finds all entries whose lowercased label begins with
    /// `"bump"`.
    ///
    /// An empty prefix returns all entries in lexicographic label order.
    pub(super) fn specs_with_prefix(
        &self,
        prefix: &str,
    ) -> Vec<&'static constraint_catalog::ConstraintSpec> {
        let normalized = prefix.to_ascii_lowercase();
        self.trie
            .collect_with_prefix(normalized.chars())
            .into_iter()
            .map(|(_, &index)| &constraint_catalog::CONSTRAINTS[index])
            .collect()
    }
}

/// The singleton constraint-key index, built once on first use.
static CONSTRAINT_KEY_INDEX: OnceLock<ConstraintKeyIndex> = OnceLock::new();

pub(super) fn constraint_key_index() -> &'static ConstraintKeyIndex {
    CONSTRAINT_KEY_INDEX.get_or_init(ConstraintKeyIndex::build)
}

// ---------------------------------------------------------------------------
// Field-name trie (for `FieldCompletionContext::FieldName`)
// ---------------------------------------------------------------------------

/// Trie over the anchor field-completion catalog, keyed by **field name**.
///
/// Each entry maps the lowercase field name extracted from `field_label`
/// (e.g. `"rent"` from `"rent: Sysvar<'info, Rent>"`) to the *set* of
/// indices into `anchor_types::field_completions()` that share that name.
///
/// Multiple completions can share the same field name — for instance both
/// `"mint: Account<'info, Mint>"` and `"mint: InterfaceAccount<'info, Mint>"`
/// produce field name `"mint"`.  A `Vec<usize>` value at each trie node
/// preserves all of them.
///
/// Entries whose `field_label` does not contain a `:` separator are excluded —
/// they carry no named field.
pub(super) struct FieldNameIndex {
    trie: Trie<char, Vec<usize>>,
}

impl FieldNameIndex {
    fn build() -> Self {
        // Group indices by lowercase field name so that entries sharing a name
        // are stored together under one trie key.
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, completion) in anchor_types::field_completions().iter().enumerate() {
            if let Some(name) = field_name_from_label(completion.field_label) {
                groups
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push(index);
            }
        }

        let mut trie = Trie::new();
        for (key, indices) in groups {
            trie.insert(key.chars(), indices);
        }
        Self { trie }
    }

    /// Return every `AnchorFieldCompletion` whose field name starts with
    /// `prefix` (case-insensitive).
    ///
    /// When multiple completions share the same field name (e.g. `"mint"` for
    /// `Account` and `InterfaceAccount` variants) all of them are returned.
    pub(super) fn completions_with_prefix(
        &self,
        prefix: &str,
    ) -> Vec<&'static anchor_types::AnchorFieldCompletion> {
        let normalized = prefix.to_ascii_lowercase();
        let all_completions = anchor_types::field_completions();
        self.trie
            .collect_with_prefix(normalized.chars())
            .into_iter()
            .flat_map(|(_, indices)| indices.iter().map(|&i| &all_completions[i]))
            .collect()
    }
}

static FIELD_NAME_INDEX: OnceLock<FieldNameIndex> = OnceLock::new();

pub(super) fn field_name_index() -> &'static FieldNameIndex {
    FIELD_NAME_INDEX.get_or_init(FieldNameIndex::build)
}

// ---------------------------------------------------------------------------
// Field-type trie (for `FieldCompletionContext::FieldType`)
// ---------------------------------------------------------------------------

/// Trie over the anchor field-completion catalog, keyed by **full label**.
///
/// Each entry maps the lowercase full label (e.g. `"sysvar<'info, rent>"`)
/// to the index in `anchor_types::field_completions()`.  This powers prefix
/// queries for the `FieldType` completion context where the user types the
/// Rust type name directly, e.g. `Sy` → matches `Sysvar<'info, …>`.
pub(super) struct FieldTypeIndex {
    trie: Trie<char, usize>,
}

impl FieldTypeIndex {
    fn build() -> Self {
        let mut trie = Trie::new();
        for (index, completion) in anchor_types::field_completions().iter().enumerate() {
            let key = completion.label.to_ascii_lowercase();
            trie.insert(key.chars(), index);
        }
        Self { trie }
    }

    /// Return every `AnchorFieldCompletion` whose label starts with `prefix`
    /// (case-insensitive).
    pub(super) fn completions_with_prefix(
        &self,
        prefix: &str,
    ) -> Vec<&'static anchor_types::AnchorFieldCompletion> {
        let normalized = prefix.to_ascii_lowercase();
        self.trie
            .collect_with_prefix(normalized.chars())
            .into_iter()
            .map(|(_, &index)| &anchor_types::field_completions()[index])
            .collect()
    }
}

static FIELD_TYPE_INDEX: OnceLock<FieldTypeIndex> = OnceLock::new();

pub(super) fn field_type_index() -> &'static FieldTypeIndex {
    FIELD_TYPE_INDEX.get_or_init(FieldTypeIndex::build)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Extract the field name from a `field_label` string like
/// `"rent: Sysvar<'info, Rent>"`.  Returns `None` when no `:` separator is
/// present.
fn field_name_from_label(field_label: &str) -> Option<&str> {
    field_label.split_once(':').map(|(name, _)| name.trim())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- constraint key index -------------------------------------------------

    #[test]
    fn constraint_key_prefix_returns_all_on_empty_prefix() {
        let index = constraint_key_index();
        let all = index.specs_with_prefix("");
        assert_eq!(
            all.len(),
            constraint_catalog::CONSTRAINTS.len(),
            "empty prefix must return the full catalog"
        );
    }

    #[test]
    fn constraint_key_prefix_filters_to_matching_entries() {
        let index = constraint_key_index();
        let results = index.specs_with_prefix("pa");
        // "payer =" key starts with "pa"
        assert!(
            results.iter().any(|spec| spec.label == "payer ="),
            "expected 'payer =' in results for prefix 'pa'"
        );
        // "mut" does not start with "pa"
        assert!(
            !results.iter().any(|spec| spec.label == "mut"),
            "'mut' must not appear for prefix 'pa'"
        );
    }

    #[test]
    fn constraint_key_prefix_is_case_insensitive() {
        let index = constraint_key_index();
        let lower = index.specs_with_prefix("pa");
        let upper = index.specs_with_prefix("PA");
        let labels_lower: Vec<_> = lower.iter().map(|s| s.label).collect();
        let labels_upper: Vec<_> = upper.iter().map(|s| s.label).collect();
        assert_eq!(
            labels_lower, labels_upper,
            "prefix query must be case-insensitive"
        );
    }

    #[test]
    fn constraint_key_prefix_returns_empty_for_no_match() {
        let index = constraint_key_index();
        let results = index.specs_with_prefix("zzznomatch");
        assert!(results.is_empty(), "unmatched prefix must yield no results");
    }

    #[test]
    fn constraint_key_prefix_matches_namespaced_constraints() {
        let index = constraint_key_index();
        let results = index.specs_with_prefix("seeds::");
        assert!(
            results
                .iter()
                .any(|spec| spec.label == "seeds::program ="),
            "expected 'seeds::program =' for prefix 'seeds::'"
        );
    }

    // --- field-name index -----------------------------------------------------

    #[test]
    fn field_name_prefix_returns_all_on_empty_prefix() {
        let index = field_name_index();
        let all = index.completions_with_prefix("");
        // Every completion that has a parseable field_label must appear.
        let expected = anchor_types::field_completions()
            .iter()
            .filter(|c| field_name_from_label(c.field_label).is_some())
            .count();
        assert_eq!(all.len(), expected);
    }

    #[test]
    fn field_name_prefix_filters_correctly() {
        let index = field_name_index();
        let results = index.completions_with_prefix("sy");
        // Field names starting with "sy" should include sysvars.
        // In the catalog field_label starts with "sysvar:" or similar.
        // The key used is the field name (before the colon), not the type.
        // All returned labels should have a field name starting with "sy".
        for completion in &results {
            let name = field_name_from_label(completion.field_label)
                .expect("all index entries have a field name");
            assert!(
                name.to_ascii_lowercase().starts_with("sy"),
                "unexpected entry '{name}' for prefix 'sy'"
            );
        }
    }

    // --- field-type index -----------------------------------------------------

    #[test]
    fn field_type_prefix_returns_all_on_empty_prefix() {
        let index = field_type_index();
        let all = index.completions_with_prefix("");
        assert_eq!(all.len(), anchor_types::field_completions().len());
    }

    #[test]
    fn field_type_prefix_filters_to_sysvar_entries() {
        let index = field_type_index();
        let results = index.completions_with_prefix("Sysvar");
        assert!(
            !results.is_empty(),
            "expected sysvar completions for prefix 'Sysvar'"
        );
        for completion in &results {
            assert!(
                completion
                    .label
                    .to_ascii_lowercase()
                    .starts_with("sysvar"),
                "unexpected label '{}' for prefix 'Sysvar'",
                completion.label
            );
        }
    }

    #[test]
    fn field_type_prefix_returns_empty_for_no_match() {
        let index = field_type_index();
        let results = index.completions_with_prefix("ZzzNoMatch");
        assert!(results.is_empty());
    }
}
