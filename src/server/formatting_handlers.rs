use {
    super::Backend,
    crate::formatting,
    tower_lsp::{
        jsonrpc::Result,
        lsp_types::{DocumentFormattingParams, TextEdit},
    },
};

impl Backend {
    pub(super) async fn formatting_impl(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(source) = self.documents.get(&uri).map(|entry| entry.text.clone()) else {
            return Ok(None);
        };

        let edits = tokio::task::spawn_blocking(move || formatting::format_document(&source))
            .await
            .ok()
            .flatten();
        Ok(edits)
    }
}
