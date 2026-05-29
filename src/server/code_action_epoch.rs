use super::*;

impl Backend {
    /// Increments the publish epoch for `uri` by one and returns the new epoch value.
    ///
    /// Called exactly once per `publish_analysis` invocation so callers can detect
    /// whether a cached code-action set is still fresh.
    pub(super) fn bump_code_action_epoch(&self, uri: &Url) -> i32 {
        let entry = self
            .code_action_epoch
            .entry(uri.clone())
            .or_insert_with(|| AtomicI32::new(0));
        entry.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Returns the current publish epoch for `uri`, or 0 if no diagnostics have
    /// been published for it yet.
    ///
    /// Consumed by the code-action cache in `code_action` to detect staleness.
    pub(super) fn code_action_epoch(&self, uri: &Url) -> i32 {
        self.code_action_epoch
            .get(uri)
            .map(|entry| entry.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Removes the epoch entry for `uri`.
    ///
    /// Called from `did_close` so that closed-document epochs do not accumulate
    /// in memory for the lifetime of the server process.
    pub(super) fn forget_code_action_epoch(&self, uri: &Url) {
        self.code_action_epoch.remove(uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a minimal Backend suitable for unit-testing pure, in-memory methods.
    ///
    /// The resulting instance is obtained by cloning out of an `LspService`, which is
    /// the only public way to obtain a `Client` outside of the LSP runtime.
    fn make_backend() -> Backend {
        let (service, _socket) = LspService::new(|client| Backend {
            client,
            documents: DashMap::new(),
            workspace_roots: Arc::new(Mutex::new(Vec::new())),
            workspace_index: Arc::new(RwLock::new(workspace::WorkspaceIndex::default())),
            settings: Arc::new(Mutex::new(ServerSettings::default())),
            client_supports_snippet_edits: Arc::new(AtomicBool::new(false)),
            supports_watched_file_registration: Arc::new(AtomicBool::new(false)),
            diagnostics_transport: Arc::new(Mutex::new(DiagnosticsTransport::Push)),
            recent_logs: Arc::new(Mutex::new(VecDeque::new())),
            document_debouncers: DashMap::new(),
            recent_typing_changes: DashMap::new(),
            completion_memo: DashMap::new(),
            code_action_epoch: Arc::new(DashMap::new()),
            query_cache: query_cache::QueryCache::new(),
            salsa_db: Arc::new(Mutex::new(LspSalsaDb::default())),
        });
        service.inner().clone()
    }

    fn test_uri() -> Url {
        Url::parse("file:///test/lib.rs").unwrap()
    }

    #[test]
    fn code_action_epoch_starts_at_zero() {
        let backend = make_backend();
        let uri = test_uri();

        assert_eq!(
            backend.code_action_epoch(&uri),
            0,
            "epoch should be 0 before any diagnostics are published for a URI"
        );
    }

    #[test]
    fn bump_code_action_epoch_increments_monotonically() {
        let backend = make_backend();
        let uri = test_uri();

        let first = backend.bump_code_action_epoch(&uri);
        let second = backend.bump_code_action_epoch(&uri);

        assert_eq!(first, 1, "first bump should yield epoch 1");
        assert_eq!(second, 2, "second bump should yield epoch 2");
        assert_eq!(
            backend.code_action_epoch(&uri),
            2,
            "read-back after two bumps should return 2"
        );
    }

    #[test]
    fn forget_code_action_epoch_resets_reads() {
        let backend = make_backend();
        let uri = test_uri();

        backend.bump_code_action_epoch(&uri);
        backend.bump_code_action_epoch(&uri);
        assert_eq!(backend.code_action_epoch(&uri), 2);

        backend.forget_code_action_epoch(&uri);

        assert_eq!(
            backend.code_action_epoch(&uri),
            0,
            "epoch should read 0 after the entry is forgotten"
        );
    }

    #[test]
    fn bump_is_per_uri() {
        let backend = make_backend();
        let uri1 = Url::parse("file:///test/a.rs").unwrap();
        let uri2 = Url::parse("file:///test/b.rs").unwrap();

        backend.bump_code_action_epoch(&uri1);
        backend.bump_code_action_epoch(&uri1);
        backend.bump_code_action_epoch(&uri1);

        assert_eq!(
            backend.code_action_epoch(&uri2),
            0,
            "bumping uri1 three times must not affect uri2's epoch"
        );
    }

    /// Regression for the Cargo.toml staleness scenario: after a publish bumps
    /// the epoch, the cached code-action entry from the previous epoch must miss
    /// without any explicit cache clear. This is what makes the manifest watcher
    /// fix deliver end-to-end UX: diagnostics AND quickfixes both refresh on
    /// `did_change_watched_files`, even though `document.version` is unchanged.
    #[test]
    fn epoch_bump_invalidates_cached_code_actions_for_uri() {
        use tower_lsp::lsp_types::CodeAction;

        let backend = make_backend();
        let uri = test_uri();
        let cache_key = (
            uri.clone(),
            query_cache::QueryKind::CodeActions(uri.clone()),
        );

        let epoch_before = backend.code_action_epoch(&uri);
        let stale_actions = vec![CodeAction {
            title: "stale fix".to_string(),
            ..CodeAction::default()
        }];
        backend.query_cache.insert(
            cache_key.clone(),
            epoch_before,
            query_cache::CacheValue::CodeActions(stale_actions),
        );

        assert!(
            matches!(
                backend.query_cache.get(cache_key.clone(), epoch_before),
                Some(query_cache::CacheValue::CodeActions(_))
            ),
            "seeded entry should hit at the epoch it was inserted under"
        );

        let epoch_after = backend.bump_code_action_epoch(&uri);
        assert_ne!(
            epoch_before, epoch_after,
            "bump must produce a different freshness token"
        );

        assert!(
            backend.query_cache.get(cache_key, epoch_after).is_none(),
            "cached entry from previous epoch must miss after the bump, with no explicit clear"
        );
    }
}
