mod account_iter;

use {
    crate::{diagnostics::FrameworkDocument, semantic::SemanticModel, FrameworkKind},
    syn::visit::Visit,
};

pub fn extract_native(document: FrameworkDocument<'_>, kind: FrameworkKind) -> SemanticModel {
    if !matches!(kind, FrameworkKind::Native | FrameworkKind::Pinocchio) {
        return SemanticModel::default();
    }
    let mut extractor = account_iter::NativeAccountExtractor::default();
    extractor.visit_file(document.syntax());
    extractor.finish()
}

#[cfg(test)]
mod tests;
