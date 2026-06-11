use super::*;

impl Backend {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn hover_impl(
        &self,
        params: HoverParams,
    ) -> Result<Option<tower_lsp::lsp_types::Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::Hover(position));
        if let Some(query_cache::CacheValue::Hover(result)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(result);
        }

        let Some(document) = self.document_for(&uri) else {
            self.query_cache
                .insert(cache_key, version, query_cache::CacheValue::Hover(None));
            return Ok(None);
        };

        {
            let workspace_index = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner());
            if let Some(local_hover) =
                hover::hover_with_workspace(&document, position, Some(&workspace_index))
            {
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::Hover(Some(local_hover.clone())),
                );
                return Ok(Some(local_hover));
            }
        }

        if let Some(path) = navigation::account_path_position(&document, position) {
            let resolved = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .resolve_account_field_path(&path.context, &path.segments, path.segment_index);
            if let Some(resolved) = resolved {
                let result = hover::workspace_account_field_hover(
                    &document,
                    position,
                    &resolved.container_name,
                    &resolved.field_name,
                    resolved.field_info.type_display.as_deref(),
                );
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::Hover(result.clone()),
                );
                return Ok(result);
            }
        }

        if let Some(context) = document
            .symbols()
            .context_references
            .iter()
            .find(|reference| contains_position(reference.range, position))
        {
            let fields = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .account_context_fields(&context.name);
            if !fields.is_empty() {
                let result = hover::workspace_accounts_context_hover(
                    &document,
                    position,
                    &context.name,
                    &fields,
                );
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::Hover(result.clone()),
                );
                return Ok(result);
            }
        }

        if let Some((container, field)) =
            navigation::account_data_field_definition_target(&document, position)
        {
            let field_info = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .field_info_in_container(&field, &container);
            if let Some(field_info) = field_info {
                let result = hover::workspace_account_data_field_hover(
                    &document,
                    position,
                    &container,
                    &field,
                    field_info.type_display.as_deref(),
                );
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::Hover(result.clone()),
                );
                return Ok(result);
            }
        }

        self.query_cache
            .insert(cache_key, version, query_cache::CacheValue::Hover(None));
        Ok(None)
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn goto_definition_impl(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let version = self.current_document_version(&uri);
        let cache_key = (
            uri.clone(),
            query_cache::QueryKind::GotoDefinition(position),
        );
        if let Some(query_cache::CacheValue::GotoDefinition(result)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(result.map(GotoDefinitionResponse::Scalar));
        }

        let Some(document) = self.document_for(&uri) else {
            self.record_navigation_log(
                "textDocument/definition",
                &uri,
                position,
                "missingDocument",
                0,
            );
            self.query_cache.insert(
                cache_key,
                version,
                query_cache::CacheValue::GotoDefinition(None),
            );
            return Ok(None);
        };

        if let Some(range) = navigation::definition_range(&document, position) {
            self.record_navigation_log("textDocument/definition", &uri, position, "document", 1);
            let result = Location { uri, range };
            self.query_cache.insert(
                cache_key,
                version,
                query_cache::CacheValue::GotoDefinition(Some(result.clone())),
            );
            return Ok(Some(GotoDefinitionResponse::Scalar(result)));
        }

        let Some(word) = crate::range::word_at_position(document.source(), position) else {
            self.record_navigation_log("textDocument/definition", &uri, position, "noWord", 0);
            self.query_cache.insert(
                cache_key,
                version,
                query_cache::CacheValue::GotoDefinition(None),
            );
            return Ok(None);
        };
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());
        if let Some(path) = navigation::account_path_position(&document, position) {
            if let Some(resolved) = workspace_index.resolve_account_field_path(
                &path.context,
                &path.segments,
                path.segment_index,
            ) {
                self.record_navigation_log(
                    "textDocument/definition",
                    &uri,
                    position,
                    "workspace",
                    1,
                );
                let result = resolved.field_info.location;
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::GotoDefinition(Some(result.clone())),
                );
                return Ok(Some(GotoDefinitionResponse::Scalar(result)));
            }
        }
        if let Some((container, field)) =
            navigation::account_data_field_definition_target(&document, position)
        {
            let locations = workspace_index.symbol_locations_in_container(
                &field,
                &[SymbolKind::FIELD],
                &container,
            );
            if let Some(location) = locations.into_iter().next() {
                self.record_navigation_log(
                    "textDocument/definition",
                    &uri,
                    position,
                    "workspace",
                    1,
                );
                let result = location;
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::GotoDefinition(Some(result.clone())),
                );
                return Ok(Some(GotoDefinitionResponse::Scalar(result)));
            }
        }
        if let Some(target) = navigation::instruction_argument_target(&document, position) {
            let locations = workspace_index
                .instruction_argument_references_for_context_with_source(
                    &target.context,
                    &target.name,
                    |uri| self.documents.get(uri).map(|doc| doc.text.clone()),
                );
            if let Some(location) = locations.into_iter().next() {
                self.record_navigation_log(
                    "textDocument/definition",
                    &uri,
                    position,
                    "workspace",
                    1,
                );
                let result = location;
                self.query_cache.insert(
                    cache_key,
                    version,
                    query_cache::CacheValue::GotoDefinition(Some(result.clone())),
                );
                return Ok(Some(GotoDefinitionResponse::Scalar(result)));
            }
        }
        let mut locations = navigation::definition_target_kinds(&document, position)
            .map(|kinds| workspace_index.symbol_locations_with_kinds(&word, &kinds))
            .unwrap_or_default();
        if locations.is_empty() {
            locations = workspace_index.symbol_locations(&word);
        }
        let location = locations.into_iter().next();
        self.record_navigation_log(
            "textDocument/definition",
            &uri,
            position,
            "workspace",
            usize::from(location.is_some()),
        );
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::GotoDefinition(location.clone()),
        );
        Ok(location.map(GotoDefinitionResponse::Scalar))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn goto_declaration_impl(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(document) = self.document_for(&uri) else {
            self.record_navigation_log(
                "textDocument/declaration",
                &uri,
                position,
                "missingDocument",
                0,
            );
            return Ok(None);
        };

        if let Some(range) = navigation::declaration_range(&document, position) {
            self.record_navigation_log("textDocument/declaration", &uri, position, "document", 1);
            return Ok(Some(GotoDeclarationResponse::Scalar(Location {
                uri,
                range,
            })));
        }

        let Some(word) = crate::range::word_at_position(document.source(), position) else {
            self.record_navigation_log("textDocument/declaration", &uri, position, "noWord", 0);
            return Ok(None);
        };
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());
        let mut locations = navigation::declaration_target_kinds(&document, position)
            .map(|kinds| workspace_index.symbol_locations_with_kinds(&word, &kinds))
            .unwrap_or_default();
        if locations.is_empty() {
            locations = workspace_index.symbol_locations(&word);
        }
        let location = locations.into_iter().next();
        self.record_navigation_log(
            "textDocument/declaration",
            &uri,
            position,
            "workspace",
            usize::from(location.is_some()),
        );
        Ok(location.map(GotoDeclarationResponse::Scalar))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn goto_type_definition_impl(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> Result<Option<GotoTypeDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(document) = self.document_for(&uri) else {
            self.record_navigation_log(
                "textDocument/typeDefinition",
                &uri,
                position,
                "missingDocument",
                0,
            );
            return Ok(None);
        };

        if let Some(range) = navigation::type_definition_range(&document, position) {
            self.record_navigation_log(
                "textDocument/typeDefinition",
                &uri,
                position,
                "document",
                1,
            );
            return Ok(Some(GotoTypeDefinitionResponse::Scalar(Location {
                uri,
                range,
            })));
        }

        let Some(target) = navigation::type_definition_target(&document, position) else {
            self.record_navigation_log(
                "textDocument/typeDefinition",
                &uri,
                position,
                "noTarget",
                0,
            );
            return Ok(None);
        };
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());
        let location = workspace_index
            .symbol_locations_with_kinds(&target, &[SymbolKind::STRUCT])
            .into_iter()
            .next();
        self.record_navigation_log(
            "textDocument/typeDefinition",
            &uri,
            position,
            "workspace",
            usize::from(location.is_some()),
        );
        Ok(location.map(GotoTypeDefinitionResponse::Scalar))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn goto_implementation_impl(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(document) = self.document_for(&uri) else {
            self.record_navigation_log(
                "textDocument/implementation",
                &uri,
                position,
                "missingDocument",
                0,
            );
            return Ok(None);
        };

        if let Some(ranges) = navigation::implementation_ranges(&document, position) {
            let result_count = ranges.len();
            self.record_navigation_log(
                "textDocument/implementation",
                &uri,
                position,
                "document",
                result_count,
            );
            return Ok(Some(GotoImplementationResponse::Array(
                ranges
                    .into_iter()
                    .map(|range| Location {
                        uri: uri.clone(),
                        range,
                    })
                    .collect(),
            )));
        }

        let Some(context) = navigation::implementation_context_target(&document, position) else {
            self.record_navigation_log(
                "textDocument/implementation",
                &uri,
                position,
                "noTarget",
                0,
            );
            return Ok(None);
        };
        let locations = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .implementation_locations_for_context(&context);

        self.record_navigation_log(
            "textDocument/implementation",
            &uri,
            position,
            "workspace",
            locations.len(),
        );
        Ok((!locations.is_empty()).then_some(GotoImplementationResponse::Array(locations)))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn references_impl(
        &self,
        params: ReferenceParams,
    ) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };

        if let Some(word) = crate::range::word_at_position(document.source(), position) {
            if let Some(path) = navigation::account_path_position(&document, position) {
                let workspace_index = self
                    .workspace_index
                    .read()
                    .unwrap_or_else(|err| err.into_inner());
                if let Some(resolved) = workspace_index.resolve_account_field_path(
                    &path.context,
                    &path.segments,
                    path.segment_index,
                ) {
                    let locations = workspace_index.references_in_container(
                        &resolved.field_name,
                        &[SymbolKind::FIELD],
                        &resolved.container_name,
                    );
                    if !locations.is_empty() {
                        return Ok(Some(locations));
                    }
                }
            }

            if let Some((container, field)) =
                navigation::account_data_field_definition_target(&document, position)
            {
                let locations = self
                    .workspace_index
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .references_in_container(&field, &[SymbolKind::FIELD], &container);
                if !locations.is_empty() {
                    return Ok(Some(locations));
                }
            }

            if let Some(target) = navigation::instruction_argument_target(&document, position) {
                let locations = self
                    .workspace_index
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .instruction_argument_references_for_context_with_source(
                        &target.context,
                        &target.name,
                        |uri| self.documents.get(uri).map(|doc| doc.text.clone()),
                    );
                if !locations.is_empty() {
                    return Ok(Some(locations));
                }
            }

            if let Some(kinds) = navigation::reference_target_kinds(&document, position) {
                let locations = self
                    .workspace_index
                    .read()
                    .unwrap_or_else(|err| err.into_inner())
                    .references_with_kinds(&word, &kinds);
                if !locations.is_empty() {
                    return Ok(Some(locations));
                }
            }

            let locations = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .anchor_type_references(&word);
            if !locations.is_empty() {
                return Ok(Some(locations));
            }

            let locations = self
                .workspace_index
                .read()
                .unwrap_or_else(|err| err.into_inner())
                .function_references(&word);
            if !locations.is_empty() {
                return Ok(Some(locations));
            }
        }

        Ok(navigation::references(&document, uri, position))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn prepare_rename_impl(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());

        Ok(renaming::prepare_rename(
            &document,
            params.position,
            Some(&workspace_index),
        ))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn rename_impl(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());

        Ok(renaming::rename_with_workspace(
            &document,
            uri,
            position,
            &params.new_name,
            Some(&workspace_index),
            |lookup_uri| self.documents.get(lookup_uri).map(|doc| doc.text.clone()),
        ))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn document_highlight_impl(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::DocumentHighlight>>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };

        Ok(navigation::document_highlights(&document, position))
    }
}
