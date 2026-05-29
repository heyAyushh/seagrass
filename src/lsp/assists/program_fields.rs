use {
    super::{
        account_set_touches_context, Assist, AssistApplicability, AssistContext, AssistId,
        AssistKind, AssistProvider, ADD_ASSOCIATED_TOKEN_PROGRAM_FIELD_ID,
        ADD_SYSTEM_PROGRAM_FIELD_ID, ADD_TOKEN_PROGRAM_FIELD_ID, ASSOCIATED_TOKEN_CONSTRAINT_KEYS,
        ASSOCIATED_TOKEN_PROGRAM_FIELD_NAME, ASSOCIATED_TOKEN_PROGRAM_REASON,
        ASSOCIATED_TOKEN_PROGRAM_TYPE, SYSTEM_PROGRAM_FIELD_NAME, SYSTEM_PROGRAM_FIELD_TEXT,
        TOKEN_INTERFACE_PROGRAM_TYPE, TOKEN_PROGRAM_FIELD_NAME, TOKEN_PROGRAM_REASON,
        TOKEN_PROGRAM_REFERENCE_KEYS, TOKEN_PROGRAM_TYPE,
    },
    crate::{
        actions::common::{account_struct_closing_line, field_indent, single_document_edit},
        evidence::{AccountSetEvidence, EvidenceGraph, FieldEvidence},
    },
    serde_json::json,
    std::collections::HashSet,
    tower_lsp::lsp_types::{Position, Range, TextEdit},
};

pub(super) struct SystemProgramFieldProvider;

impl AssistProvider for SystemProgramFieldProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .filter_map(|accounts| system_program_field_assist(context, accounts))
            .collect()
    }
}

fn system_program_field_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> Option<Assist> {
    if !should_offer_system_program_field(accounts) {
        return None;
    }

    let insert_line = account_struct_closing_line(context.document, accounts.accounts)?;
    let indent = field_indent(context.document, accounts.accounts);
    let edit = TextEdit {
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
        new_text: format!("{indent}{SYSTEM_PROGRAM_FIELD_TEXT}\n"),
    };

    Some(Assist {
        id: AssistId(ADD_SYSTEM_PROGRAM_FIELD_ID),
        title: "Add Anchor system program account".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: accounts.accounts.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": SYSTEM_PROGRAM_FIELD_NAME,
            "reason": "init-like account constraints require the System program account",
        }),
    })
}

fn should_offer_system_program_field(accounts: &AccountSetEvidence<'_>) -> bool {
    accounts.has_payer_candidate()
        && !accounts.has_system_program()
        && accounts
            .fields()
            .iter()
            .any(|field| field.has_init_constraint())
}

pub(super) struct ProgramFieldProvider;

impl AssistProvider for ProgramFieldProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .flat_map(|accounts| companion_program_field_assists(context, accounts))
            .collect()
    }
}

#[derive(Clone, Copy)]
struct CompanionProgramField {
    id: &'static str,
    default_field_name: &'static str,
    title: &'static str,
    requirement: CompanionProgramRequirement,
}

const TOKEN_PROGRAM_FIELD: CompanionProgramField = CompanionProgramField {
    id: ADD_TOKEN_PROGRAM_FIELD_ID,
    default_field_name: TOKEN_PROGRAM_FIELD_NAME,
    title: "Add Anchor token program account",
    requirement: CompanionProgramRequirement::Token,
};

const ASSOCIATED_TOKEN_PROGRAM_FIELD: CompanionProgramField = CompanionProgramField {
    id: ADD_ASSOCIATED_TOKEN_PROGRAM_FIELD_ID,
    default_field_name: ASSOCIATED_TOKEN_PROGRAM_FIELD_NAME,
    title: "Add Anchor associated token program account",
    requirement: CompanionProgramRequirement::AssociatedToken,
};

const COMPANION_PROGRAM_FIELDS: [CompanionProgramField; 2] =
    [TOKEN_PROGRAM_FIELD, ASSOCIATED_TOKEN_PROGRAM_FIELD];

#[derive(Clone, Copy)]
enum CompanionProgramRequirement {
    Token,
    AssociatedToken,
}

struct CompanionProgramEvidence<'a> {
    program_field: CompanionProgramField,
    field_name: &'a str,
    field_type: &'static str,
}

fn companion_program_field_assists(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> Vec<Assist> {
    COMPANION_PROGRAM_FIELDS
        .into_iter()
        .flat_map(|program_field| companion_program_evidence(accounts, program_field))
        .filter_map(|program_evidence| {
            companion_program_field_assist(context, accounts, program_evidence)
        })
        .collect()
}

fn companion_program_field_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    evidence: CompanionProgramEvidence<'_>,
) -> Option<Assist> {
    let insert_line = account_struct_closing_line(context.document, accounts.accounts)?;
    let indent = field_indent(context.document, accounts.accounts);
    let edit = TextEdit {
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
        new_text: format!(
            "{indent}pub {}: {},\n",
            evidence.field_name, evidence.field_type
        ),
    };

    Some(Assist {
        id: AssistId(evidence.program_field.id),
        title: evidence.program_field.title.to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: accounts.accounts.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": evidence.field_name,
            "reason": companion_program_reason(evidence.program_field.requirement),
        }),
    })
}

fn companion_program_evidence<'a>(
    accounts: &'a AccountSetEvidence<'a>,
    program_field: CompanionProgramField,
) -> Vec<CompanionProgramEvidence<'a>> {
    let mut seen_field_names = HashSet::with_capacity(accounts.fields().len());
    accounts
        .fields()
        .iter()
        .filter_map(|field| required_companion_program(field, program_field))
        .filter(|evidence| {
            !accounts.has_account(evidence.field_name)
                && seen_field_names.insert(evidence.field_name.to_string())
        })
        .collect()
}

fn required_companion_program<'a>(
    field: &'a FieldEvidence<'a>,
    program_field: CompanionProgramField,
) -> Option<CompanionProgramEvidence<'a>> {
    if !field.has_init_constraint() {
        return None;
    }

    match program_field.requirement {
        CompanionProgramRequirement::Token if initializes_token_or_mint(field) => {
            Some(CompanionProgramEvidence {
                program_field,
                field_name: token_program_field_name_for_init(
                    field,
                    program_field.default_field_name,
                ),
                field_type: token_program_type_for_initialized_field(field),
            })
        }
        CompanionProgramRequirement::AssociatedToken if initializes_associated_token(field) => {
            Some(CompanionProgramEvidence {
                program_field,
                field_name: program_field.default_field_name,
                field_type: ASSOCIATED_TOKEN_PROGRAM_TYPE,
            })
        }
        _ => None,
    }
}

fn initializes_token_or_mint(field: &FieldEvidence<'_>) -> bool {
    field.has_generic_type("TokenAccount") || field.has_generic_type("Mint")
}

fn initializes_associated_token(field: &FieldEvidence<'_>) -> bool {
    field.has_generic_type("TokenAccount")
        && field.has_any_constraint(&ASSOCIATED_TOKEN_CONSTRAINT_KEYS)
}

fn token_program_field_name_for_init<'a>(
    field: &'a FieldEvidence<'a>,
    default_field_name: &'a str,
) -> &'a str {
    field
        .constraints()
        .iter()
        .flat_map(|constraint| constraint.account_references())
        .find(|reference| TOKEN_PROGRAM_REFERENCE_KEYS.contains(&reference.key))
        .map(|reference| reference.name)
        .unwrap_or(default_field_name)
}

fn token_program_type_for_initialized_field(field: &FieldEvidence<'_>) -> &'static str {
    if field.type_name() == Some("InterfaceAccount") {
        TOKEN_INTERFACE_PROGRAM_TYPE
    } else {
        TOKEN_PROGRAM_TYPE
    }
}

fn companion_program_reason(requirement: CompanionProgramRequirement) -> &'static str {
    match requirement {
        CompanionProgramRequirement::Token => TOKEN_PROGRAM_REASON,
        CompanionProgramRequirement::AssociatedToken => ASSOCIATED_TOKEN_PROGRAM_REASON,
    }
}
