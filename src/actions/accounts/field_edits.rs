use {
    super::super::common::{diagnostic_code, single_document_edit, snippet_text_edit},
    crate::{
        anchor_types,
        constraint_catalog::{self, ConstraintValueKind},
        document::{AccountUsage, InstructionSymbol, ParsedDocument, SymbolRange},
    },
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InferredAccountField {
    name: String,
    type_name: String,
    mutable: bool,
    executable: bool,
}
pub(super) fn inferred_account_fields(
    document: &ParsedDocument,
    context_type: &str,
) -> Vec<InferredAccountField> {
    let mut fields = Vec::new();
    for instruction in document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == context_type)
        })
    {
        for usage in instruction.account_usages.iter().cloned() {
            let signer = account_has_signer_evidence(instruction, &usage.name);
            let cpi_program = account_has_cpi_program_evidence(instruction, &usage.name);
            upsert_inferred_account_field(
                &mut fields,
                inferred_account_field(usage, signer, cpi_program),
            );
        }
        for usage in &instruction.account_path_usages {
            let Some(segment) = usage.segments.first() else {
                continue;
            };
            // AccountPathUsage stores richer paths, but the first segment is still the
            // top-level Accounts field that a generated struct needs to declare.
            upsert_inferred_account_field(
                &mut fields,
                inferred_account_field(
                    AccountUsage {
                        name: segment.name.clone(),
                        range: segment.range,
                        mutable: usage.mutable,
                    },
                    account_has_signer_evidence(instruction, &segment.name),
                    account_has_cpi_program_evidence(instruction, &segment.name),
                ),
            );
        }
        for usage in instruction.cpi_program_usages.iter().cloned() {
            upsert_inferred_account_field(&mut fields, inferred_account_field(usage, false, true));
        }
    }
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    fields
}
fn upsert_inferred_account_field(
    fields: &mut Vec<InferredAccountField>,
    field: InferredAccountField,
) {
    if let Some(existing) = fields
        .iter_mut()
        .find(|existing| existing.name == field.name)
    {
        existing.mutable |= field.mutable;
        existing.executable |= field.executable;
        if existing.type_name == "UncheckedAccount<'info>" && field.type_name != existing.type_name
        {
            existing.type_name = field.type_name;
        }
        return;
    }
    fields.push(field);
}
fn inferred_account_field_for_name(
    document: &ParsedDocument,
    context_type: &str,
    name: &str,
) -> InferredAccountField {
    inferred_account_fields(document, context_type)
        .into_iter()
        .find(|field| field.name == name)
        .unwrap_or_else(|| {
            inferred_account_field(
                AccountUsage {
                    name: name.to_string(),
                    range: Range::default(),
                    mutable: false,
                },
                false,
                false,
            )
        })
}
fn inferred_account_field_for_diagnostic(
    document: &ParsedDocument,
    context_type: &str,
    name: &str,
    data: &serde_json::Value,
) -> InferredAccountField {
    let mut field = inferred_account_field_for_name(document, context_type, name);
    if let Some((type_name, mutable)) = constraint_reference_field_shape(data) {
        field.type_name = type_name.to_string();
        field.mutable |= mutable;
    }
    field
}
fn constraint_reference_field_shape(data: &serde_json::Value) -> Option<(&'static str, bool)> {
    let constraint = data.get("constraint").and_then(|value| value.as_str())?;
    let spec = constraint_catalog::by_key(constraint)?;
    match spec.value_kind {
        ConstraintValueKind::SignerReference => Some(("Signer<'info>", constraint == "payer")),
        ConstraintValueKind::AccountReference if constraint == "payer" => {
            Some(("Signer<'info>", true))
        }
        ConstraintValueKind::None
        | ConstraintValueKind::AnyExpression
        | ConstraintValueKind::AccountReference
        | ConstraintValueKind::ProgramReference
        | ConstraintValueKind::InstructionArgument
        | ConstraintValueKind::Keyword
        | ConstraintValueKind::Boolean
        | ConstraintValueKind::Space
        | ConstraintValueKind::Seeds => None,
    }
}
fn inferred_account_field(
    usage: AccountUsage,
    signer: bool,
    cpi_program: bool,
) -> InferredAccountField {
    let generated_type = anchor_types::field_type_for_field_name(&usage.name);
    let type_name = generated_type
        .or_else(|| signer.then_some("Signer<'info>"))
        .unwrap_or("UncheckedAccount<'info>")
        .to_string();
    InferredAccountField {
        name: usage.name,
        type_name,
        mutable: usage.mutable,
        executable: cpi_program && generated_type.is_none(),
    }
}
fn account_has_signer_evidence(instruction: &InstructionSymbol, name: &str) -> bool {
    instruction
        .signer_usages
        .iter()
        .chain(instruction.signer_checks.iter())
        .any(|usage| usage.name == name)
}
fn account_has_cpi_program_evidence(instruction: &InstructionSymbol, name: &str) -> bool {
    instruction
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == name)
}
pub(super) fn accounts_struct_stub(
    source: &str,
    context_type: &str,
    fields: &[InferredAccountField],
) -> String {
    let separator = if source.is_empty() || source.ends_with("\n\n") {
        ""
    } else if source.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    if fields.is_empty() {
        return format!(
            "{separator}#[derive(Accounts)]\npub struct {context_type}<'info> {{\n}}\n"
        );
    }
    let fields = fields
        .iter()
        .map(|field| {
            let constraint = account_field_constraints(field, "    ");
            format!("{constraint}    pub {}: {},\n", field.name, field.type_name)
        })
        .collect::<String>();
    format!("{separator}#[derive(Accounts)]\npub struct {context_type}<'info> {{\n{fields}}}\n")
}
fn account_field_stub(field: &InferredAccountField, indent: &str) -> String {
    let constraint = account_field_constraints(field, indent);
    format!(
        "{constraint}{indent}pub {}: {},\n",
        field.name, field.type_name
    )
}
fn account_field_constraints(field: &InferredAccountField, indent: &str) -> String {
    match (field.mutable, field.executable) {
        (true, true) => format!("{indent}#[account(mut, executable)]\n"),
        (true, false) => format!("{indent}#[account(mut)]\n"),
        (false, true) => format!("{indent}#[account(executable)]\n"),
        (false, false) => String::new(),
    }
}
pub(super) fn add_missing_account_field_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let account = data.get("account").and_then(|value| value.as_str())?;
            let accounts_name = data
                .get("accountsStruct")
                .and_then(|value| value.as_str())?;
            let accounts = document.symbols().accounts_structs.get(accounts_name)?;
            if accounts.fields.iter().any(|field| field.name == account) {
                return None;
            }
            let field =
                inferred_account_field_for_diagnostic(document, accounts_name, account, data);
            let edit = add_account_field_edit(document, accounts, &field)?;

            Some(CodeAction {
                title: format!("Add `{account}` to `{accounts_name}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "add-missing-account-field",
                    "account": account,
                    "accountsStruct": accounts_name,
                    "type": field.type_name,
                    "mutable": field.mutable,
                })),
            })
        })
        .collect()
}
fn add_account_field_edit(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    field: &InferredAccountField,
) -> Option<TextEdit> {
    let closing_line_number = accounts.range.end.line;
    let closing_line = crate::range::line_at(document.source(), closing_line_number)?;
    let closing_indent = closing_line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let field_indent = format!("{closing_indent}    ");
    Some(snippet_text_edit(
        Range {
            start: Position {
                line: closing_line_number,
                character: 0,
            },
            end: Position {
                line: closing_line_number,
                character: 0,
            },
        },
        &account_field_stub(field, &field_indent),
    ))
}
