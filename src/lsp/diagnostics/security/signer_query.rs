use {crate::semantic::SemanticModel, tower_lsp::lsp_types::Diagnostic};

pub fn missing_signer_diagnostics(model: &SemanticModel) -> Vec<Diagnostic> {
    crate::semantic::queries::missing_signer_diagnostics(model)
}
