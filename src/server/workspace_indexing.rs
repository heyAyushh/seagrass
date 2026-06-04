use super::*;

impl Backend {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn refresh_workspace_index(&self) {
        if !self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .workspace_index
        {
            *self
                .workspace_index
                .write()
                .unwrap_or_else(|err| err.into_inner()) = workspace::WorkspaceIndex::default();
            return;
        }

        let roots = self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let open_documents = self
            .documents
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().text.clone()))
            .collect::<Vec<_>>();

        let new_index = tokio::task::spawn_blocking(move || {
            crate::measure_hotpath_block!("lsp.workspace.scan", {
                workspace::WorkspaceIndex::build(&roots, open_documents)
            })
        })
        .await
        .unwrap_or_else(|err| {
            eprintln!("Workspace index build panicked: {err}");
            workspace::WorkspaceIndex::default()
        });

        *self
            .workspace_index
            .write()
            .unwrap_or_else(|err| err.into_inner()) = new_index;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub(super) async fn refresh_workspace_index_for_watched_files(
        &self,
        changes: &[tower_lsp::lsp_types::FileEvent],
    ) {
        if !self
            .settings
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .workspace_index
        {
            *self
                .workspace_index
                .write()
                .unwrap_or_else(|err| err.into_inner()) = workspace::WorkspaceIndex::default();
            return;
        }
        if changes.iter().any(requires_full_workspace_refresh) {
            self.refresh_workspace_index().await;
            return;
        }

        let roots = self
            .workspace_roots
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        let open_uris = self
            .documents
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<BTreeSet<_>>();
        let file_changes = changes
            .iter()
            .filter(|change| !open_uris.contains(&change.uri))
            .map(|change| (change.uri.clone(), change.typ))
            .collect::<Vec<_>>();

        let updates = tokio::task::spawn_blocking(move || {
            crate::measure_hotpath_block!("lsp.workspace.incremental_scan", {
                file_changes
                    .into_iter()
                    .map(|(uri, typ)| match typ {
                        FileChangeType::DELETED => (uri, None),
                        FileChangeType::CREATED | FileChangeType::CHANGED => {
                            let update = workspace::WorkspaceIndex::update_for_workspace_file(
                                &roots,
                                uri.clone(),
                            );
                            (uri, update)
                        }
                        _ => (uri, None),
                    })
                    .collect::<Vec<_>>()
            })
        })
        .await
        .unwrap_or_else(|err| {
            eprintln!("Workspace incremental index update panicked: {err}");
            Vec::new()
        });

        let mut workspace_index = self
            .workspace_index
            .write()
            .unwrap_or_else(|err| err.into_inner());
        for (uri, update) in updates {
            match update {
                Some(update) => workspace_index.upsert_open_document_update(update),
                None => workspace_index.remove_document(&uri),
            }
        }
    }
}
