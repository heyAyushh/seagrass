use {
    super::{
        account_set_touches_context, Assist, AssistApplicability, AssistContext, AssistId,
        AssistKind, AssistProvider, ADD_CPI_PROGRAM_EXECUTABLE_CONSTRAINT_ID,
        CPI_PROGRAM_VALIDATION_CONSTRAINT_KEYS, EXECUTABLE_CONSTRAINT_NAME,
        EXECUTABLE_CPI_PROGRAM_REASON, TYPED_CPI_PROGRAM_REASON, USE_TYPED_CPI_PROGRAM_ACCOUNT_ID,
    },
    crate::{
        actions::common::{add_constraint_to_field_edit, single_document_edit},
        anchor_types,
        document::SymbolRange,
        evidence::{AccountSetEvidence, EvidenceGraph, FieldEvidence},
    },
    serde_json::json,
    tower_lsp::lsp_types::{Position, Range, TextEdit},
};

pub(super) struct SafeCpiProgramProvider;

impl AssistProvider for SafeCpiProgramProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .flat_map(|accounts| safe_cpi_program_assists(context, accounts))
            .collect()
    }
}

fn safe_cpi_program_assists(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> Vec<Assist> {
    accounts
        .fields()
        .iter()
        .filter_map(|field| safe_cpi_program_assist(context, accounts, field))
        .collect()
}

fn safe_cpi_program_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Option<Assist> {
    if !field.is_unchecked_account() || cpi_program_field_is_validated(field) {
        return None;
    }
    let usage = field.used_as_cpi_programs().first()?;

    if let Some(expected_type) = typed_cpi_program_type(field) {
        return typed_cpi_program_assist(
            context,
            accounts,
            field,
            usage.instruction,
            expected_type,
        );
    }

    executable_cpi_program_assist(context, accounts, field, usage.instruction)
}

fn typed_cpi_program_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    instruction: &str,
    expected_type: &'static str,
) -> Option<Assist> {
    let edit = TextEdit {
        range: full_field_type_range(field.field)?,
        new_text: expected_type.to_string(),
    };

    Some(Assist {
        id: AssistId(USE_TYPED_CPI_PROGRAM_ACCOUNT_ID),
        title: "Use typed CPI program account".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: field.field.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": field.field.name,
            "expectedType": expected_type,
            "instruction": instruction,
            "reason": TYPED_CPI_PROGRAM_REASON,
        }),
    })
}

fn executable_cpi_program_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    instruction: &str,
) -> Option<Assist> {
    let edit =
        add_constraint_to_field_edit(context.document, field.field, EXECUTABLE_CONSTRAINT_NAME)?;

    Some(Assist {
        id: AssistId(ADD_CPI_PROGRAM_EXECUTABLE_CONSTRAINT_ID),
        title: "Add executable CPI program constraint".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: field.field.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": field.field.name,
            "constraint": EXECUTABLE_CONSTRAINT_NAME,
            "instruction": instruction,
            "reason": EXECUTABLE_CPI_PROGRAM_REASON,
        }),
    })
}

fn typed_cpi_program_type(field: &FieldEvidence<'_>) -> Option<&'static str> {
    anchor_types::field_type_for_field_name(&field.field.name).filter(|expected| {
        expected.starts_with("Program<'info,") || expected.starts_with("Interface<'info,")
    })
}

fn cpi_program_field_is_validated(field: &FieldEvidence<'_>) -> bool {
    matches!(field.type_name(), Some("Program" | "Interface"))
        || field.has_any_constraint(&CPI_PROGRAM_VALIDATION_CONSTRAINT_KEYS)
}

fn full_field_type_range(field: &SymbolRange) -> Option<Range> {
    let type_range = field.type_range?;
    let end = field
        .generic_type_ranges
        .last()
        .map(|range| Position {
            line: range.range.end.line,
            character: range.range.end.character.saturating_add(1),
        })
        .unwrap_or(type_range.end);

    Some(Range {
        start: type_range.start,
        end,
    })
}
