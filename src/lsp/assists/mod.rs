mod canonical_seeds;
mod cpi_programs;
mod program_fields;
mod structural;
#[cfg(test)]
mod tests;
mod text;

use {
    crate::{document::ParsedDocument, evidence::AccountSetEvidence},
    canonical_seeds::CanonicalSeedsProvider,
    cpi_programs::SafeCpiProgramProvider,
    program_fields::{ProgramFieldProvider, SystemProgramFieldProvider},
    serde_json::json,
    structural::{InstructionArgsAttributeProvider, MutConstraintProvider, PdaBumpProvider},
    tower_lsp::lsp_types::{CodeAction, CodeActionKind, Position, Range, Url, WorkspaceEdit},
};

pub(super) const ADD_SYSTEM_PROGRAM_FIELD_ID: &str = "add-system-program-field";
pub(super) const ADD_TOKEN_PROGRAM_FIELD_ID: &str = "add-token-program-field";
pub(super) const ADD_ASSOCIATED_TOKEN_PROGRAM_FIELD_ID: &str = "add-associated-token-program-field";
pub(super) const ADD_PDA_BUMP_CONSTRAINT_ID: &str = "add-pda-bump-constraint";
pub(super) const ADD_MUT_CONSTRAINT_ID: &str = "add-mut-constraint";
pub(super) const ADD_INSTRUCTION_ARGS_ATTRIBUTE_ID: &str = "add-instruction-args-attribute";
pub(super) const ADD_CANONICAL_SEEDS_STRUCT_ID: &str = "add-canonical-seeds-struct";
pub(super) const USE_TYPED_CPI_PROGRAM_ACCOUNT_ID: &str = "use-typed-cpi-program-account";
pub(super) const ADD_CPI_PROGRAM_EXECUTABLE_CONSTRAINT_ID: &str =
    "add-cpi-program-executable-constraint";
pub(super) const SYSTEM_PROGRAM_FIELD_NAME: &str = "system_program";
pub(super) const TOKEN_PROGRAM_FIELD_NAME: &str = "token_program";
pub(super) const ASSOCIATED_TOKEN_PROGRAM_FIELD_NAME: &str = "associated_token_program";
pub(super) const SYSTEM_PROGRAM_FIELD_TEXT: &str = "pub system_program: Program<'info, System>,";
pub(super) const TOKEN_PROGRAM_TYPE: &str = "Program<'info, Token>";
pub(super) const TOKEN_INTERFACE_PROGRAM_TYPE: &str = "Interface<'info, TokenInterface>";
pub(super) const ASSOCIATED_TOKEN_PROGRAM_TYPE: &str = "Program<'info, AssociatedToken>";
pub(super) const SEEDS_CONSTRAINT_NAME: &str = "seeds";
pub(super) const BUMP_CONSTRAINT_NAME: &str = "bump";
pub(super) const MUT_CONSTRAINT_NAME: &str = "mut";
pub(super) const EXECUTABLE_CONSTRAINT_NAME: &str = "executable";
pub(super) const TOKEN_PROGRAM_REASON: &str =
    "token or mint init constraints require the Token program account";
pub(super) const ASSOCIATED_TOKEN_PROGRAM_REASON: &str =
    "associated token constraints require the Associated Token program account";
pub(super) const PDA_BUMP_REASON: &str = "PDA seeds should validate the canonical bump";
pub(super) const MUT_CONSTRAINT_REASON: &str = "instruction code mutates this account";
pub(super) const INSTRUCTION_ARGS_REASON: &str =
    "account constraints reference instruction arguments";
pub(super) const CANONICAL_SEEDS_REASON: &str =
    "PDA seeds can be reused through a canonical helper";
pub(super) const TYPED_CPI_PROGRAM_REASON: &str =
    "known CPI program accounts should use typed Anchor program accounts";
pub(super) const EXECUTABLE_CPI_PROGRAM_REASON: &str =
    "CPI program accounts should be executable or constrained by program id";
pub(super) const TOKEN_PROGRAM_REFERENCE_KEYS: [&str; 3] = [
    "token::token_program",
    "associated_token::token_program",
    "mint::token_program",
];
pub(super) const ASSOCIATED_TOKEN_CONSTRAINT_KEYS: [&str; 3] = [
    "associated_token::mint",
    "associated_token::authority",
    "associated_token::token_program",
];
pub(super) const MUTABILITY_CONSTRAINT_KEYS: [&str; 5] =
    ["mut", "zero", "init", "init_if_needed", "realloc"];
pub(super) const CPI_PROGRAM_VALIDATION_CONSTRAINT_KEYS: [&str; 4] =
    ["address", "owner", "executable", "constraint"];
const POSITION_CHARACTER_STRIDE: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AssistId(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssistKind {
    Refactor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssistApplicability {
    MachineApplicable,
}

#[derive(Debug, Clone)]
pub(crate) struct Assist {
    pub id: AssistId,
    pub title: String,
    pub kind: AssistKind,
    pub applicability: AssistApplicability,
    pub range: Range,
    pub edit: Option<WorkspaceEdit>,
    pub data: serde_json::Value,
}

pub(crate) struct AssistContext<'a> {
    pub document: &'a ParsedDocument,
    pub uri: &'a Url,
    pub range: Option<Range>,
}

pub(crate) trait AssistProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist>;
}

pub(crate) fn code_actions(document: &ParsedDocument, uri: Url, range: Range) -> Vec<CodeAction> {
    collect_assists(document, &uri, Some(range))
        .into_iter()
        .filter(|assist| assist_touches_range(assist.range, range))
        .map(assist_to_code_action)
        .collect()
}

pub(crate) fn proposed_assists(document: &ParsedDocument, uri: &Url) -> Vec<serde_json::Value> {
    collect_assists(document, uri, None)
        .into_iter()
        .map(assist_summary)
        .collect()
}

fn collect_assists<'a>(
    document: &'a ParsedDocument,
    uri: &'a Url,
    range: Option<Range>,
) -> Vec<Assist> {
    let context = AssistContext {
        document,
        uri,
        range,
    };

    let mut assists = Vec::new();
    collect_provider_assists(SystemProgramFieldProvider, &context, &mut assists);
    collect_provider_assists(ProgramFieldProvider, &context, &mut assists);
    collect_provider_assists(PdaBumpProvider, &context, &mut assists);
    collect_provider_assists(CanonicalSeedsProvider, &context, &mut assists);
    collect_provider_assists(MutConstraintProvider, &context, &mut assists);
    collect_provider_assists(InstructionArgsAttributeProvider, &context, &mut assists);
    collect_provider_assists(SafeCpiProgramProvider, &context, &mut assists);
    assists
}

fn collect_provider_assists(
    provider: impl AssistProvider,
    context: &AssistContext<'_>,
    assists: &mut Vec<Assist>,
) {
    assists.extend(provider.assists(context));
}

fn assist_summary(assist: Assist) -> serde_json::Value {
    json!({
        "id": assist.id.0,
        "title": assist.title,
        "kind": assist_kind_name(assist.kind),
        "applicability": applicability_name(assist.applicability),
        "range": assist.range,
        "hasEdit": assist.edit.is_some(),
        "edit": assist.edit,
        "evidence": assist.data,
    })
}

fn assist_to_code_action(assist: Assist) -> CodeAction {
    CodeAction {
        title: assist.title,
        kind: Some(code_action_kind(assist.kind)),
        diagnostics: None,
        edit: assist.edit,
        command: None,
        is_preferred: Some(matches!(
            assist.applicability,
            AssistApplicability::MachineApplicable
        )),
        disabled: None,
        data: Some(json!({
            "seagrassAssist": assist.id.0,
            "applicability": applicability_name(assist.applicability),
            "seagrassAssistRange": assist.range,
            "evidence": assist.data,
        })),
    }
}

fn code_action_kind(kind: AssistKind) -> CodeActionKind {
    match kind {
        AssistKind::Refactor => CodeActionKind::REFACTOR,
    }
}

fn assist_kind_name(kind: AssistKind) -> &'static str {
    match kind {
        AssistKind::Refactor => "refactor",
    }
}

fn applicability_name(applicability: AssistApplicability) -> &'static str {
    match applicability {
        AssistApplicability::MachineApplicable => "machineApplicable",
    }
}

pub(super) fn assist_touches_range(assist_range: Range, cursor_range: Range) -> bool {
    position_key(assist_range.start) <= position_key(cursor_range.end)
        && position_key(cursor_range.start) <= position_key(assist_range.end)
}

fn position_key(position: Position) -> u64 {
    u64::from(position.line)
        .saturating_mul(POSITION_CHARACTER_STRIDE)
        .saturating_add(u64::from(position.character))
}

pub(super) fn account_set_touches_context(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> bool {
    context
        .range
        .is_none_or(|range| assist_touches_range(accounts.accounts.range, range))
}
