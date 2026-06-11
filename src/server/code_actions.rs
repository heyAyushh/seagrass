use {
    super::{code_action_uri, Backend},
    crate::{actions, assists, query_cache},
    tower_lsp::{
        jsonrpc::Result,
        lsp_types::{CodeAction, CodeActionParams, CodeActionResponse},
    },
};

impl Backend {
    pub(super) async fn code_action_impl(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };

        let wants_source_action = params
            .context
            .only
            .as_ref()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str().starts_with("source")));
        let context_diagnostics = params.context.diagnostics;
        let has_context_diagnostics = !context_diagnostics.is_empty();
        let diagnostics = self.code_action_diagnostics(
            &uri,
            &document,
            range,
            context_diagnostics,
            wants_source_action,
        );

        let mut actions = self.cursor_independent_code_actions(
            &uri,
            &document,
            &diagnostics,
            has_context_diagnostics,
        );
        {
            let workspace_index = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner());
            actions.extend(actions::cursor_dependent_code_actions_with_workspace(
                &document,
                uri.clone(),
                range,
                &diagnostics,
                Some(&workspace_index),
            ));
        }
        actions.extend(assists::code_actions(&document, uri, range));
        let actions = actions::rank_and_filter_for_cursor(actions, range);
        Ok((!actions.is_empty()).then_some(actions.into_iter().map(Into::into).collect()))
    }

    fn cursor_independent_code_actions(
        &self,
        uri: &tower_lsp::lsp_types::Url,
        document: &crate::document::ParsedDocument,
        diagnostics: &[tower_lsp::lsp_types::Diagnostic],
        has_context_diagnostics: bool,
    ) -> Vec<CodeAction> {
        if has_context_diagnostics {
            return actions::code_actions_unfiltered(document, uri.clone(), diagnostics);
        }

        // Cache the cursor-independent action set per URI per publish-epoch.
        // The epoch bumps on every publish_analysis, so manifest-driven republishes
        // (e.g. Cargo.toml save) invalidate stale entries even though the document
        // version is unchanged.
        let epoch = self.code_action_epoch(uri);
        let cache_key = (
            uri.clone(),
            query_cache::QueryKind::CodeActions(uri.clone()),
        );
        if let Some(query_cache::CacheValue::CodeActions(cached)) =
            self.query_cache.get(cache_key.clone(), epoch)
        {
            return cached;
        }

        let built = actions::code_actions_unfiltered(document, uri.clone(), diagnostics);
        self.query_cache.insert(
            cache_key,
            epoch,
            query_cache::CacheValue::CodeActions(built.clone()),
        );
        built
    }

    pub(super) async fn code_action_resolve_impl(&self, params: CodeAction) -> Result<CodeAction> {
        let Some(uri) = code_action_uri(&params) else {
            return Ok(params);
        };
        let Some(document) = self.document_for(&uri) else {
            return Ok(params);
        };
        let diagnostics = self.collect_diagnostics_for_uri(&uri, &document);
        Ok(actions::resolve(&document, uri, params, &diagnostics))
    }
}
