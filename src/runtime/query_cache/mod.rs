use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use tower_lsp::lsp_types::{
    CodeAction, CompletionItem, Diagnostic, DocumentSymbol, FoldingRange, Hover, InlayHint,
    Location, Position, Range, SemanticTokens, Url,
};

/// Pre-allocated capacity for the outer document cache map.
const DEFAULT_CAPACITY: usize = 1024;

/// A thread-safe, versioned cache for expensive LSP query results.
///
/// `QueryCache` stores derived computations (diagnostics, hover text,
/// completions, etc.) keyed by document URI and query kind.  Each entry is
/// tagged with the LSP document version at the time it was computed.  On a
/// cache lookup the caller supplies the *current* document version; if the
/// versions differ the entry is treated as stale and discarded.
///
/// The backing store is keyed by URI first, so document invalidation removes
/// one URI bucket instead of scanning unrelated files' cached queries.
///
/// # Examples
///
/// ```
/// use tower_lsp::lsp_types::{Url, Diagnostic, Position};
/// use seagrass::query_cache::{QueryCache, QueryKind, CacheValue};
///
/// let cache = QueryCache::new();
/// let uri = Url::parse("file:///main.rs").unwrap();
///
/// cache.insert(
///     (uri.clone(), QueryKind::Diagnostics),
///     1,
///     CacheValue::Diagnostics(vec![Diagnostic::default()]),
/// );
///
/// // Cache hit – version matches.
/// let hit = cache.get((uri.clone(), QueryKind::Diagnostics), 1);
/// assert!(hit.is_some());
///
/// // Cache miss – version changed.
/// let miss = cache.get((uri, QueryKind::Diagnostics), 2);
/// assert!(miss.is_none());
/// ```
#[derive(Debug, Clone)]
pub struct QueryCache {
    entries_by_uri: DashMap<Url, DashMap<QueryKind, CacheEntry>>,
}

impl QueryCache {
    /// Creates a new, empty cache with a pre-allocated capacity of `1024`.
    pub fn new() -> Self {
        Self {
            entries_by_uri: DashMap::with_capacity(DEFAULT_CAPACITY),
        }
    }

    /// Returns the cached value for `key` only when the stored document version
    /// equals `current_version`.
    ///
    /// If the versions differ the entry is removed and `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use tower_lsp::lsp_types::{Url, Position};
    /// use seagrass::query_cache::{QueryCache, QueryKind, CacheValue};
    ///
    /// let cache = QueryCache::new();
    /// let uri = Url::parse("file:///lib.rs").unwrap();
    /// let pos = Position { line: 10, character: 5 };
    ///
    /// cache.insert(
    ///     (uri.clone(), QueryKind::Hover(pos)),
    ///     3,
    ///     CacheValue::Hover(None),
    /// );
    ///
    /// assert!(cache.get((uri.clone(), QueryKind::Hover(pos)), 3).is_some());
    /// assert!(cache.get((uri, QueryKind::Hover(pos)), 4).is_none());
    /// ```
    pub fn get<K: Into<CacheKey>>(&self, key: K, current_version: i32) -> Option<CacheValue> {
        let key = key.into();
        let entries = self.entries_by_uri.get(&key.uri)?;
        let entry = entries.get(&key.kind)?;
        if entry.document_version == current_version {
            Some(entry.value.clone())
        } else {
            drop(entry);
            entries.remove(&key.kind);
            None
        }
    }

    /// Stores `value` in the cache, tagged with `version`.
    ///
    /// Overwrites any existing entry for the same key.
    ///
    /// # Examples
    ///
    /// ```
    /// use tower_lsp::lsp_types::{Url, Diagnostic};
    /// use seagrass::query_cache::{QueryCache, QueryKind, CacheValue};
    ///
    /// let cache = QueryCache::new();
    /// let uri = Url::parse("file:///main.rs").unwrap();
    ///
    /// cache.insert(
    ///     (uri.clone(), QueryKind::Diagnostics),
    ///     1,
    ///     CacheValue::Diagnostics(vec![Diagnostic::default()]),
    /// );
    ///
    /// assert!(cache.get((uri, QueryKind::Diagnostics), 1).is_some());
    /// ```
    pub fn insert<K: Into<CacheKey>>(&self, key: K, version: i32, value: CacheValue) {
        let key = key.into();
        self.entries_by_uri
            .entry(key.uri)
            .or_default()
            .insert(key.kind, CacheEntry::new(version, value));
    }

    /// Removes every cached entry whose URI matches `uri`.
    ///
    /// Typically called when a document is closed or changed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tower_lsp::lsp_types::Url;
    /// use seagrass::query_cache::{QueryCache, QueryKind, CacheValue};
    ///
    /// let cache = QueryCache::new();
    /// let uri = Url::parse("file:///a.rs").unwrap();
    ///
    /// cache.insert((uri.clone(), QueryKind::Diagnostics), 1, CacheValue::Diagnostics(vec![]));
    /// cache.invalidate_for_uri(&uri);
    ///
    /// assert!(cache.get((uri, QueryKind::Diagnostics), 1).is_none());
    /// ```
    pub fn invalidate_for_uri(&self, uri: &Url) {
        self.entries_by_uri.remove(uri);
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache key composed of a document URI and the kind of query performed.
///
/// Two keys are equal when both their URI and query kind match.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub uri: Url,
    pub kind: QueryKind,
}

impl From<(Url, QueryKind)> for CacheKey {
    fn from((uri, kind): (Url, QueryKind)) -> Self {
        Self { uri, kind }
    }
}

/// Discriminant for the type of LSP query being cached.
///
/// Variants that operate at a specific cursor position carry a [`Position`];
/// range-based queries carry a [`Range`].  Document-scoped variants (e.g.
/// [`QueryKind::CodeActions`]) carry a [`Url`] so one entry per document holds
/// the full unfiltered result set — cursor-level filtering is applied on read
/// at the call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryKind {
    /// Full-document diagnostics.
    Diagnostics,
    /// Hover information at a specific position.
    Hover(Position),
    /// Completion items at a specific position.
    Completion(Position),
    /// Goto-definition result at a specific position.
    GotoDefinition(Position),
    /// Document-level symbol tree.
    DocumentSymbols,
    /// Folding ranges for the whole document.
    FoldingRanges,
    /// Inlay hints inside a specific range.
    InlayHints(Range),
    /// Full-document semantic tokens.
    SemanticTokens,
    /// Full unfiltered code-action set for an entire document, keyed by [`Url`].
    ///
    /// A single entry per document stores all actions computed for the last
    /// known document version.  Cursor-aware filtering is applied at the call
    /// site when the caller selects which actions to surface to the editor.
    CodeActions(Url),
}

impl Hash for QueryKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            QueryKind::Diagnostics => {}
            QueryKind::Hover(pos) | QueryKind::Completion(pos) | QueryKind::GotoDefinition(pos) => {
                pos.line.hash(state);
                pos.character.hash(state);
            }
            QueryKind::DocumentSymbols => {}
            QueryKind::FoldingRanges => {}
            QueryKind::InlayHints(range) => {
                range.start.line.hash(state);
                range.start.character.hash(state);
                range.end.line.hash(state);
                range.end.character.hash(state);
            }
            QueryKind::SemanticTokens => {}
            QueryKind::CodeActions(uri) => {
                uri.as_str().hash(state);
            }
        }
    }
}

/// A single cache entry recording the document version and the computed value.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// LSP document version at the time this entry was created.
    pub document_version: i32,
    /// The cached result.
    pub value: CacheValue,
}

impl CacheEntry {
    /// Creates a new cache entry.
    pub fn new(document_version: i32, value: CacheValue) -> Self {
        Self {
            document_version,
            value,
        }
    }
}

/// Typed wrapper around the various LSP result types that can be cached.
///
/// Each variant corresponds to one [`QueryKind`] and holds the native LSP
/// type returned by the Seagrass computation for that query.
#[derive(Debug, Clone)]
pub enum CacheValue {
    /// Result of [`QueryKind::Diagnostics`].
    Diagnostics(Vec<Diagnostic>),
    /// Result of [`QueryKind::Hover`].
    Hover(Option<Hover>),
    /// Result of [`QueryKind::Completion`].
    Completion(Option<Vec<CompletionItem>>),
    /// Result of [`QueryKind::GotoDefinition`].
    GotoDefinition(Option<Location>),
    /// Result of [`QueryKind::DocumentSymbols`].
    DocumentSymbols(Vec<DocumentSymbol>),
    /// Result of [`QueryKind::FoldingRanges`].
    FoldingRanges(Vec<FoldingRange>),
    /// Result of [`QueryKind::InlayHints`].
    InlayHints(Vec<InlayHint>),
    /// Result of [`QueryKind::SemanticTokens`].
    SemanticTokens(SemanticTokens),
    /// Result of [`QueryKind::CodeActions`].
    ///
    /// Holds the **full, unfiltered** action set for the document.  The caller
    /// is responsible for filtering to the actions relevant to the current
    /// cursor range before returning them to the editor.
    CodeActions(Vec<CodeAction>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{CodeAction, Url};

    fn make_uri(path: &str) -> Url {
        Url::parse(&format!("file://{path}")).expect("valid test URI")
    }

    fn make_action(title: &str) -> CodeAction {
        CodeAction {
            title: title.to_owned(),
            ..Default::default()
        }
    }

    /// Inserting at version 7 and reading at version 7 returns the stored vec.
    #[test]
    fn code_actions_round_trip_at_same_version() {
        let cache = QueryCache::new();
        let uri = make_uri("/project/src/lib.rs");
        let actions = vec![make_action("Fix lint")];

        cache.insert(
            (uri.clone(), QueryKind::CodeActions(uri.clone())),
            7,
            CacheValue::CodeActions(actions.clone()),
        );

        let hit = cache.get((uri.clone(), QueryKind::CodeActions(uri.clone())), 7);
        assert!(hit.is_some(), "expected a cache hit at the same version");

        match hit.unwrap() {
            CacheValue::CodeActions(stored) => {
                assert_eq!(stored.len(), actions.len());
                assert_eq!(stored[0].title, actions[0].title);
            }
            other => panic!("unexpected cache value variant: {other:?}"),
        }
    }

    /// Inserting at version 7 and reading at version 8 returns None (stale).
    #[test]
    fn code_actions_miss_after_newer_version() {
        let cache = QueryCache::new();
        let uri = make_uri("/project/src/lib.rs");

        cache.insert(
            (uri.clone(), QueryKind::CodeActions(uri.clone())),
            7,
            CacheValue::CodeActions(vec![make_action("Old action")]),
        );

        let miss = cache.get((uri.clone(), QueryKind::CodeActions(uri.clone())), 8);
        assert!(
            miss.is_none(),
            "expected None because stored version (7) differs from current (8)"
        );
    }

    /// Each URI has an independent slot; inserting for one does not affect the other.
    #[test]
    fn code_actions_per_uri_isolation() {
        let cache = QueryCache::new();
        let uri1 = make_uri("/project/src/a.rs");
        let uri2 = make_uri("/project/src/b.rs");

        let actions_a = vec![make_action("Action A")];
        let actions_b = vec![make_action("Action B1"), make_action("Action B2")];

        cache.insert(
            (uri1.clone(), QueryKind::CodeActions(uri1.clone())),
            1,
            CacheValue::CodeActions(actions_a.clone()),
        );
        cache.insert(
            (uri2.clone(), QueryKind::CodeActions(uri2.clone())),
            1,
            CacheValue::CodeActions(actions_b.clone()),
        );

        let result_a = cache.get((uri1.clone(), QueryKind::CodeActions(uri1.clone())), 1);
        let result_b = cache.get((uri2.clone(), QueryKind::CodeActions(uri2.clone())), 1);

        match result_a.unwrap() {
            CacheValue::CodeActions(stored) => assert_eq!(stored[0].title, "Action A"),
            other => panic!("unexpected variant for uri1: {other:?}"),
        }

        match result_b.unwrap() {
            CacheValue::CodeActions(stored) => {
                assert_eq!(stored.len(), 2);
                assert_eq!(stored[0].title, "Action B1");
                assert_eq!(stored[1].title, "Action B2");
            }
            other => panic!("unexpected variant for uri2: {other:?}"),
        }
    }

    #[test]
    fn invalidate_for_uri_keeps_other_document_entries() {
        let cache = QueryCache::new();
        let changed_uri = make_uri("/project/src/changed.rs");
        let stable_uri = make_uri("/project/src/stable.rs");

        cache.insert(
            (changed_uri.clone(), QueryKind::Diagnostics),
            1,
            CacheValue::Diagnostics(vec![Diagnostic::default()]),
        );
        cache.insert(
            (
                changed_uri.clone(),
                QueryKind::Hover(Position {
                    line: 2,
                    character: 4,
                }),
            ),
            1,
            CacheValue::Hover(None),
        );
        cache.insert(
            (stable_uri.clone(), QueryKind::Diagnostics),
            1,
            CacheValue::Diagnostics(vec![Diagnostic::default()]),
        );

        cache.invalidate_for_uri(&changed_uri);

        assert!(
            cache
                .get((changed_uri.clone(), QueryKind::Diagnostics), 1)
                .is_none(),
            "changed document diagnostics should be invalidated"
        );
        assert!(
            cache
                .get(
                    (
                        changed_uri.clone(),
                        QueryKind::Hover(Position {
                            line: 2,
                            character: 4,
                        }),
                    ),
                    1,
                )
                .is_none(),
            "changed document hover should be invalidated"
        );
        assert!(
            cache
                .get((stable_uri.clone(), QueryKind::Diagnostics), 1)
                .is_some(),
            "unrelated document diagnostics should remain cached"
        );
    }

    /// An empty Vec<CodeAction> stored at version 1 must return Some(empty) on get at version 1.
    #[test]
    fn code_actions_empty_vec_is_still_a_hit() {
        let cache = QueryCache::new();
        let uri = make_uri("/project/src/empty.rs");

        cache.insert(
            (uri.clone(), QueryKind::CodeActions(uri.clone())),
            1,
            CacheValue::CodeActions(vec![]),
        );

        let hit = cache.get((uri.clone(), QueryKind::CodeActions(uri.clone())), 1);
        assert!(hit.is_some(), "empty action list must still be a cache hit");

        match hit.unwrap() {
            CacheValue::CodeActions(stored) => assert!(
                stored.is_empty(),
                "expected empty vec, got {} actions",
                stored.len()
            ),
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
