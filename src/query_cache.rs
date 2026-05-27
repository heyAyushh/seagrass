use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use tower_lsp::lsp_types::{
    CompletionItem, Diagnostic, DocumentSymbol, FoldingRange, Hover, InlayHint, Location, Position,
    Range, SemanticTokens, Url,
};

/// Pre-allocated capacity for the inner [`DashMap`].
const DEFAULT_CAPACITY: usize = 1024;

/// A thread-safe, versioned cache for expensive LSP query results.
///
/// `QueryCache` stores derived computations (diagnostics, hover text,
/// completions, etc.) keyed by document URI and query kind.  Each entry is
/// tagged with the LSP document version at the time it was computed.  On a
/// cache lookup the caller supplies the *current* document version; if the
/// versions differ the entry is treated as stale and discarded.
///
/// The backing store is a [`DashMap`], so reads and writes are lock-free and
/// safe to use from multiple async LSP handlers concurrently.
///
/// # Examples
///
/// ```
/// use tower_lsp::lsp_types::{Url, Diagnostic, Position};
/// use seagrass::query_cache::{QueryCache, CacheKey, QueryKind, CacheValue};
///
/// let cache = QueryCache::new();
/// let uri = Url::parse("file:///main.rs").unwrap();
/// let key = CacheKey::new(uri.clone(), QueryKind::Diagnostics);
///
/// cache.insert(key, 1, CacheValue::Diagnostics(vec![Diagnostic::default()]));
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
    entries: DashMap<CacheKey, CacheEntry>,
}

impl QueryCache {
    /// Creates a new, empty cache with a pre-allocated capacity of `1024`.
    pub fn new() -> Self {
        Self {
            entries: DashMap::with_capacity(DEFAULT_CAPACITY),
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
        let entry = self.entries.get(&key)?;
        if entry.document_version == current_version {
            Some(entry.value.clone())
        } else {
            drop(entry);
            self.entries.remove(&key);
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
        self.entries.insert(key, CacheEntry::new(version, value));
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
        self.entries
            .retain(|key, _| key.uri.as_str() != uri.as_str());
    }

    /// Clears the entire cache.
    ///
    /// # Examples
    ///
    /// ```
    /// use tower_lsp::lsp_types::Url;
    /// use seagrass::query_cache::{QueryCache, QueryKind, CacheValue};
    ///
    /// let cache = QueryCache::new();
    /// let uri = Url::parse("file:///main.rs").unwrap();
    ///
    /// cache.insert((uri.clone(), QueryKind::Diagnostics), 1, CacheValue::Diagnostics(vec![]));
    /// cache.clear();
    ///
    /// assert!(cache.get((uri, QueryKind::Diagnostics), 1).is_none());
    /// ```
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.entries.clear();
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

impl CacheKey {
    /// Creates a new cache key.
    #[allow(dead_code)]
    pub fn new(uri: Url, kind: QueryKind) -> Self {
        Self { uri, kind }
    }
}

impl From<(Url, QueryKind)> for CacheKey {
    fn from((uri, kind): (Url, QueryKind)) -> Self {
        Self { uri, kind }
    }
}

/// Discriminant for the type of LSP query being cached.
///
/// Variants that operate at a specific cursor position carry a [`Position`];
/// range-based queries carry a [`Range`].
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
    /// Find-references result at a specific position.
    #[allow(dead_code)]
    GotoReferences(Position),
    /// Document-level symbol tree.
    DocumentSymbols,
    /// Folding ranges for the whole document.
    FoldingRanges,
    /// Inlay hints inside a specific range.
    InlayHints(Range),
    /// Full-document semantic tokens.
    SemanticTokens,
}

impl Hash for QueryKind {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            QueryKind::Diagnostics => {}
            QueryKind::Hover(pos)
            | QueryKind::Completion(pos)
            | QueryKind::GotoDefinition(pos)
            | QueryKind::GotoReferences(pos) => {
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
    /// Result of [`QueryKind::GotoReferences`].
    #[allow(dead_code)]
    GotoReferences(Vec<Location>),
    /// Result of [`QueryKind::DocumentSymbols`].
    DocumentSymbols(Vec<DocumentSymbol>),
    /// Result of [`QueryKind::FoldingRanges`].
    FoldingRanges(Vec<FoldingRange>),
    /// Result of [`QueryKind::InlayHints`].
    InlayHints(Vec<InlayHint>),
    /// Result of [`QueryKind::SemanticTokens`].
    SemanticTokens(SemanticTokens),
}
