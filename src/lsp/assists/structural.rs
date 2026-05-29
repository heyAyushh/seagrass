use {
    super::{
        account_set_touches_context,
        text::{leading_seed_identifier, line_indent},
        Assist, AssistApplicability, AssistContext, AssistId, AssistKind, AssistProvider,
        ADD_INSTRUCTION_ARGS_ATTRIBUTE_ID, ADD_MUT_CONSTRAINT_ID, ADD_PDA_BUMP_CONSTRAINT_ID,
        BUMP_CONSTRAINT_NAME, INSTRUCTION_ARGS_REASON, MUTABILITY_CONSTRAINT_KEYS,
        MUT_CONSTRAINT_NAME, MUT_CONSTRAINT_REASON, PDA_BUMP_REASON, SEEDS_CONSTRAINT_NAME,
    },
    crate::{
        actions::common::{add_constraint_to_field_edit, single_document_edit, struct_line},
        document::{ParsedDocument, PdaBump, SymbolRange},
        evidence::{
            AccountSetEvidence, EvidenceGraph, FieldEvidence, SeedExpressionEvidence,
            SeedExpressionKind,
        },
    },
    serde_json::json,
    std::collections::HashSet,
    tower_lsp::lsp_types::{Position, Range, TextEdit},
};

pub(super) struct PdaBumpProvider;

impl AssistProvider for PdaBumpProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .flat_map(|accounts| pda_bump_assists(context, accounts))
            .collect()
    }
}

fn pda_bump_assists(context: &AssistContext<'_>, accounts: &AccountSetEvidence<'_>) -> Vec<Assist> {
    accounts
        .fields()
        .iter()
        .filter(|field| field_requires_bump_constraint(field))
        .filter_map(|field| pda_bump_assist(context, accounts, field))
        .collect()
}

fn field_requires_bump_constraint(field: &FieldEvidence<'_>) -> bool {
    field
        .pda()
        .is_some_and(|pda| matches!(&pda.bump, PdaBump::Missing))
        || field.constraints().iter().any(|constraint| {
            constraint.has_key(SEEDS_CONSTRAINT_NAME)
                && !constraint.has_flag_or_key(BUMP_CONSTRAINT_NAME)
        })
}

fn pda_bump_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Option<Assist> {
    let edit = add_constraint_to_field_edit(context.document, field.field, BUMP_CONSTRAINT_NAME)?;

    Some(Assist {
        id: AssistId(ADD_PDA_BUMP_CONSTRAINT_ID),
        title: "Add Anchor PDA bump constraint".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: field.field.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": field.field.name,
            "constraint": BUMP_CONSTRAINT_NAME,
            "reason": PDA_BUMP_REASON,
        }),
    })
}

pub(super) struct MutConstraintProvider;

impl AssistProvider for MutConstraintProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .flat_map(|accounts| mut_constraint_assists(context, accounts))
            .collect()
    }
}

fn mut_constraint_assists(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> Vec<Assist> {
    accounts
        .fields()
        .iter()
        .filter_map(|field| mut_constraint_assist(context, accounts, field))
        .collect()
}

fn mut_constraint_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Option<Assist> {
    if field.has_any_constraint(&MUTABILITY_CONSTRAINT_KEYS) {
        return None;
    }
    let usage = field
        .used_by_instructions()
        .iter()
        .find(|usage| usage.mutable)?;
    let edit = add_constraint_to_field_edit(context.document, field.field, MUT_CONSTRAINT_NAME)?;

    Some(Assist {
        id: AssistId(ADD_MUT_CONSTRAINT_ID),
        title: "Add Anchor mut constraint".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: field.field.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": field.field.name,
            "constraint": MUT_CONSTRAINT_NAME,
            "instruction": usage.instruction,
            "reason": MUT_CONSTRAINT_REASON,
        }),
    })
}

pub(super) struct InstructionArgsAttributeProvider;

impl AssistProvider for InstructionArgsAttributeProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .filter_map(|accounts| instruction_args_attribute_assist(context, accounts))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstructionArgAssistEvidence {
    name: String,
    type_name: String,
    source: String,
}

fn instruction_args_attribute_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> Option<Assist> {
    let arguments = missing_instruction_arguments(accounts);
    if arguments.is_empty() {
        return None;
    }

    let edit = instruction_args_attribute_edit(context.document, accounts.accounts, &arguments)?;

    Some(Assist {
        id: AssistId(ADD_INSTRUCTION_ARGS_ATTRIBUTE_ID),
        title: "Add Anchor instruction arguments attribute".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: accounts.accounts.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "arguments": arguments.iter().map(|argument| json!({
                "name": argument.name,
                "type": argument.type_name,
                "source": argument.source,
            })).collect::<Vec<_>>(),
            "reason": INSTRUCTION_ARGS_REASON,
        }),
    })
}

fn instruction_args_attribute_edit(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    arguments: &[InstructionArgAssistEvidence],
) -> Option<TextEdit> {
    let argument_list = instruction_argument_list(arguments);
    if let Some(last_argument) = accounts.instruction_arguments.last() {
        return Some(TextEdit {
            range: Range {
                start: last_argument.range.end,
                end: last_argument.range.end,
            },
            new_text: format!(", {argument_list}"),
        });
    }

    let insert_line = struct_line(document.source(), &accounts.name)?;
    let indent = crate::range::line_at(document.source(), insert_line)
        .map(line_indent)
        .unwrap_or_default();
    Some(TextEdit {
        range: Range {
            start: Position {
                line: insert_line,
                character: 0,
            },
            end: Position {
                line: insert_line,
                character: 0,
            },
        },
        new_text: format!("{indent}#[instruction({argument_list})]\n"),
    })
}

fn instruction_argument_list(arguments: &[InstructionArgAssistEvidence]) -> String {
    arguments
        .iter()
        .map(|argument| format!("{}: {}", argument.name, argument.type_name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn missing_instruction_arguments(
    accounts: &AccountSetEvidence<'_>,
) -> Vec<InstructionArgAssistEvidence> {
    let mut seen = HashSet::new();
    let mut arguments = Vec::new();

    for field in accounts.fields() {
        for constraint in field.constraints() {
            for reference in constraint.instruction_argument_references() {
                push_instruction_argument(
                    accounts,
                    reference.name,
                    reference.key,
                    &mut seen,
                    &mut arguments,
                );
            }

            for seed in field.seed_expressions(accounts, constraint) {
                if let Some(name) = seed_instruction_argument_name(&seed) {
                    push_instruction_argument(accounts, &name, "seeds", &mut seen, &mut arguments);
                }
            }
        }
    }

    arguments
}

fn push_instruction_argument(
    accounts: &AccountSetEvidence<'_>,
    name: &str,
    source: &str,
    seen: &mut HashSet<String>,
    arguments: &mut Vec<InstructionArgAssistEvidence>,
) {
    if account_attribute_has_instruction_argument(accounts, name) || !seen.insert(name.to_string())
    {
        return;
    }
    let Some(type_name) = accounts.instruction_argument_type(name) else {
        return;
    };
    arguments.push(InstructionArgAssistEvidence {
        name: name.to_string(),
        type_name: type_name.to_string(),
        source: source.to_string(),
    });
}

fn account_attribute_has_instruction_argument(
    accounts: &AccountSetEvidence<'_>,
    name: &str,
) -> bool {
    accounts
        .accounts
        .instruction_arguments
        .iter()
        .any(|argument| {
            argument.name == name
                || argument.name.trim_start_matches('_') == name.trim_start_matches('_')
        })
}

fn seed_instruction_argument_name(seed: &SeedExpressionEvidence) -> Option<String> {
    (seed.kind == SeedExpressionKind::InstructionArgument)
        .then(|| leading_seed_identifier(&seed.expression))
        .flatten()
        .map(ToString::to_string)
}
