use {
    super::CURRENT_DOCUMENT_PLACEHOLDER_URI,
    crate::{
        constraint_ranges,
        evidence::{ConstraintEvidence, FieldEvidence},
    },
    tower_lsp::lsp_types::{DiagnosticRelatedInformation, Location, Range, Url},
};

pub(super) fn current_document_related_information(
    range: Range,
    message: String,
) -> DiagnosticRelatedInformation {
    DiagnosticRelatedInformation {
        location: Location {
            uri: Url::parse(CURRENT_DOCUMENT_PLACEHOLDER_URI).unwrap(),
            range,
        },
        message,
    }
}
pub(super) fn constraint_key_range(
    source: &str,
    constraint: &ConstraintEvidence<'_>,
    key: &str,
) -> Option<Range> {
    constraint_ranges::constraint_key_ranges(source, constraint.range())
        .into_iter()
        .find_map(|range| (range.key == key).then_some(range.range))
}
pub(super) fn field_related_information(
    field: &FieldEvidence<'_>,
    message: String,
) -> DiagnosticRelatedInformation {
    current_document_related_information(field.field.selection_range, message)
}
pub(super) fn constraint_related_information(
    source: &str,
    constraint: &ConstraintEvidence<'_>,
    key: &str,
    message: String,
) -> DiagnosticRelatedInformation {
    current_document_related_information(
        constraint_key_range(source, constraint, key).unwrap_or(constraint.range()),
        message,
    )
}
