use {
    super::{module_paths, WorkspaceIndex},
    tower_lsp::lsp_types::Url,
};

impl WorkspaceIndex {
    /// True when `segments` name a known `Symbol` reachable via a fully-qualified
    /// `crate::module::Symbol` path.
    ///
    /// Given `["crate", "state", "Escrow"]` the trie finds the URI for the
    /// `crate::state` module, then checks that `"Escrow"` is a symbol declared
    /// in that file.  This resolves multi-segment `crate::...` paths that cross
    /// file boundaries without claiming absence when the symbol lives elsewhere.
    ///
    /// Returns `false` rather than an error when the path is unknown. The caller
    /// should treat that as "cannot confirm presence" rather than "definitely
    /// absent", to stay on the safe side.
    pub fn symbol_exists_at_qualified_path(&self, segments: &[String]) -> bool {
        if segments.len() < 2 {
            return false;
        }
        let symbol_name = segments.last().expect("segments non-empty; checked above");
        let module_prefix = &segments[..segments.len() - 1];

        let Some((depth, file_uri)) = self
            .module_path_trie
            .longest_prefix_of(module_prefix.iter().cloned())
        else {
            return false;
        };
        if depth < module_prefix.len() {
            return false;
        }

        self.symbols_by_name
            .get(symbol_name.as_str())
            .is_some_and(|entries| entries.iter().any(|entry| &entry.location.uri == file_uri))
    }

    /// Record a `crate::...` module-path -> URI mapping in the trie.
    pub(super) fn record_module_path_for_root(&mut self, uri: &Url) {
        if let Some(segments) = module_paths::module_path_segments(uri) {
            self.module_path_trie.insert(segments, uri.clone());
        }
    }

    pub(super) fn remove_module_path_for_root(&mut self, uri: &Url) {
        if let Some(segments) = module_paths::module_path_segments(uri) {
            self.module_path_trie.remove(segments);
        }
    }
}
