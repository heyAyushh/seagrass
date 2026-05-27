use {
    super::support::current_document_related_information,
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        evidence::{ConstraintEvidence, FieldEvidence},
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, SymbolKind},
};

pub(super) fn diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    let reference = constraint
        .account_references()
        .into_iter()
        .find(|reference| reference.key == "has_one")?;
    let data_struct_name = account_data_type_name_for_field(field)?;
    let data_fields = account_data_fields(document, workspace_index, data_struct_name)?;
    if data_fields
        .names
        .iter()
        .any(|candidate| candidate == reference.name)
    {
        return None;
    }

    let range = constraint
        .value_range(document.source(), reference.key, reference.name)
        .unwrap_or(constraint.range());

    let mut related_information = vec![current_document_related_information(
        field.field.selection_range,
        format!(
            "`{}` is typed as `{data_struct_name}`, so `has_one` checks fields on `{data_struct_name}`.",
            field.field.name
        ),
    )];
    related_information.extend(data_fields.related_information);

    Some(diagnostic_from_range_with_related(
        range,
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `has_one = {}` but `{}` has no `{}` field.",
            field.field.name, reference.name, data_struct_name, reference.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "accountType": data_struct_name,
            "constraint": "has_one",
            "anchorError": "ConstraintHasOne",
            "missingDataField": reference.name,
            "candidates": data_fields.names,
            "quickfix": "replace-has-one-target",
        })),
        Some(related_information),
    ))
}
fn account_data_type_name_for_field<'a>(field: &'a FieldEvidence<'_>) -> Option<&'a str> {
    field.field.generic_type_names.last().map(String::as_str)
}
struct AccountDataFields {
    names: Vec<String>,
    related_information: Vec<DiagnosticRelatedInformation>,
}
fn account_data_fields(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    data_struct_name: &str,
) -> Option<AccountDataFields> {
    if let Some(data_struct) = document
        .symbols()
        .account_data_structs
        .get(data_struct_name)
    {
        let names = data_struct
            .fields
            .iter()
            .map(|data_field| data_field.name.clone())
            .collect::<Vec<_>>();
        let related_information = data_struct
            .fields
            .iter()
            .map(|data_field| {
                current_document_related_information(
                    data_field.selection_range,
                    format!(
                        "`{}` declares candidate field `{}`.",
                        data_struct_name, data_field.name
                    ),
                )
            })
            .collect();
        return Some(AccountDataFields {
            names,
            related_information,
        });
    }

    let workspace_index = workspace_index?;
    let names = workspace_index.field_names_in_container(data_struct_name);
    if names.is_empty() {
        return None;
    }
    let related_information = names
        .iter()
        .flat_map(|name| {
            workspace_index
                .symbol_locations_in_container(name, &[SymbolKind::FIELD], data_struct_name)
                .into_iter()
                .map(move |location| DiagnosticRelatedInformation {
                    location,
                    message: format!("`{data_struct_name}` declares candidate field `{name}`."),
                })
        })
        .collect();
    Some(AccountDataFields {
        names,
        related_information,
    })
}
