use {
    crate::{
        account_members::{self, AccountMemberAccess},
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        document::{AccountPathUsage, InstructionSymbol, ParsedDocument, SymbolRange},
        evidence::EvidenceGraph,
        workspace::{WorkspaceAccountField, WorkspaceAccountsStruct, WorkspaceIndex},
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let graph = match workspace_index {
        Some(index) => EvidenceGraph::from_document_with_reachable_functions(document, |name| {
            index.reachable_function_names_for_context(name)
        }),
        None => EvidenceGraph::from_document(document),
    };

    let mut diagnostics = graph
        .account_sets()
        .iter()
        .flat_map(|accounts| {
            let unknown_account_diagnostics = accounts.unknown_account_usages().into_iter().map(
                |usage| {
                    diagnostic_from_range(
                        usage.range,
                        AnchorDiagnosticKind::AnchorMissingAccountReference,
                        format!(
                            "`{}` is used through `ctx.accounts` in `{}` but is not declared in `{}`.",
                            usage.account, usage.instruction, accounts.accounts.name
                        ),
                        Some(serde_json::json!({
                            "account": usage.account,
                            "accountsStruct": accounts.accounts.name,
                            "instruction": usage.instruction,
                            "reason": "unknown-ctx-account-field",
                        })),
                    )
                },
            );
            let mutability_diagnostics = accounts.fields().iter().flat_map(|field| {
                field
                    .used_by_instructions()
                    .iter()
                    .filter(|usage| usage.mutable)
                    .filter(|_| should_require_mut(field))
                    .filter(|_| !has_effective_mut_constraint(field))
                    .map(|usage| {
                        diagnostic_from_range(
                            usage.range,
                            AnchorDiagnosticKind::AnchorAccountUsage,
                            format!(
                                "`{}` is mutated in `{}` but its account field is missing `#[account(mut)]`.",
                                field.field.name, usage.instruction
                            ),
                            Some(serde_json::json!({
                                "account": field.field.name,
                                "instruction": usage.instruction,
                                "missing": "mut",
                                "quickfix": "add-mut-constraint",
                            })),
                        )
                    })
            });

            unknown_account_diagnostics
                .chain(mutability_diagnostics)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    diagnostics.extend(nested_account_path_diagnostics(document, workspace_index));
    diagnostics.extend(nested_account_data_path_diagnostics(
        document,
        workspace_index,
    ));
    diagnostics.extend(account_data_field_diagnostics(document, workspace_index));
    diagnostics
}

fn has_effective_mut_constraint(field: &crate::evidence::FieldEvidence<'_>) -> bool {
    // Source-driven: ask the generated catalog which constraint keys imply
    // mutability, then use the existing evidence scanner (which knows how to
    // find "mut", "init", etc. inside possibly composite #[account(...)] text).
    field.has_any_constraint(crate::constraint_catalog::mutability_implying_keys())
}

fn should_require_mut(field: &crate::evidence::FieldEvidence<'_>) -> bool {
    matches!(
        field.type_name(),
        Some("Account")
            | Some("AccountLoader")
            | Some("InterfaceAccount")
            | Some("UncheckedAccount")
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountPathField {
    name: String,
    type_name: Option<String>,
    constraints: Vec<String>,
}

fn nested_account_path_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    document
        .symbols()
        .callable_functions()
        .flat_map(|instruction| {
            instruction
                .account_path_usages
                .iter()
                .filter(|usage| usage.segments.len() > 1)
                .filter_map(move |usage| {
                    nested_account_path_diagnostic(document, workspace_index, instruction, usage)
                })
        })
        .collect()
}

fn account_data_field_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    document
        .symbols()
        .callable_functions()
        .flat_map(|instruction| {
            instruction
                .account_data_field_usages
                .iter()
                .filter_map(move |usage| {
                    let context = instruction.context.as_ref()?;
                    let (accounts, field) = account_data_field_context(
                        document,
                        workspace_index,
                        &context.name,
                        &usage.account,
                    )?;
                    let missing = account_members::missing_member_in_chain(
                        document,
                        workspace_index,
                        &accounts,
                        &field,
                        AccountMemberAccess::Direct,
                        &usage.source_account,
                        std::slice::from_ref(&usage.field),
                    )?;
                    Some(diagnostic_from_range(
                        usage.range,
                        AnchorDiagnosticKind::AnchorMissingAccountReference,
                        format!(
                            "`{}.{}` does not resolve; `{}` has no field `{}`.",
                            missing.receiver_path,
                            missing.member,
                            missing.owner_type,
                            missing.member
                        ),
                        Some(serde_json::json!({
                            "account": usage.account,
                            "field": usage.field,
                            "accountsStruct": accounts.name,
                            "instruction": instruction.name,
                            "reason": "unknown-account-data-field",
                            "ownerType": missing.owner_type,
                            "candidates": missing.candidates,
                        })),
                    ))
                })
        })
        .collect()
}

fn nested_account_data_path_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    document
        .symbols()
        .callable_functions()
        .flat_map(|instruction| {
            instruction
                .account_path_usages
                .iter()
                .filter(|usage| usage.segments.len() > 2)
                .filter_map(move |usage| {
                    nested_account_data_path_diagnostic(
                        document,
                        workspace_index,
                        instruction,
                        usage,
                    )
                })
        })
        .collect()
}

fn nested_account_data_path_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    instruction: &InstructionSymbol,
    usage: &AccountPathUsage,
) -> Option<Diagnostic> {
    let context = instruction.context.as_ref()?;
    let mut accounts = accounts_symbol(document, workspace_index, &context.name)?;
    let mut receiver_segments = Vec::new();

    for (index, segment) in usage.segments.iter().enumerate() {
        let field = account_field_symbol(&accounts, &segment.name)?;
        receiver_segments.push(segment.name.clone());

        if index + 1 == usage.segments.len() {
            return None;
        }

        if let Some(next_container) = field
            .type_name
            .as_ref()
            .filter(|type_name| has_accounts_struct(document, workspace_index, type_name))
        {
            accounts = accounts_symbol(document, workspace_index, next_container)?;
            continue;
        }

        let remaining = usage.segments[index + 1..]
            .iter()
            .map(|segment| segment.name.clone())
            .collect::<Vec<_>>();
        let missing = account_members::missing_member_in_chain(
            document,
            workspace_index,
            &accounts,
            &field,
            AccountMemberAccess::Direct,
            &receiver_segments.join("."),
            &remaining,
        )?;
        let missing_range = usage.segments[index + 1..]
            .iter()
            .find(|segment| segment.name == missing.member)
            .map(|segment| segment.range)
            .unwrap_or(segment.range);

        return Some(diagnostic_from_range(
            missing_range,
            AnchorDiagnosticKind::AnchorMissingAccountReference,
            format!(
                "`{}.{}` does not resolve; `{}` has no field `{}`.",
                missing.receiver_path, missing.member, missing.owner_type, missing.member
            ),
            Some(serde_json::json!({
                "account": receiver_segments.join("."),
                "field": missing.member,
                "accountsStruct": accounts.name,
                "instruction": instruction.name,
                "reason": "unknown-account-data-field",
                "ownerType": missing.owner_type,
                "candidates": missing.candidates,
            })),
        ));
    }

    None
}

fn account_data_field_context(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts_name: &str,
    account_name: &str,
) -> Option<(SymbolRange, SymbolRange)> {
    let accounts = accounts_symbol(document, workspace_index, accounts_name)?;
    let field = account_field_symbol(&accounts, account_name)?;
    Some((accounts, field))
}

fn accounts_symbol(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts_name: &str,
) -> Option<SymbolRange> {
    if let Some(accounts) = document.symbols().accounts_structs.get(accounts_name) {
        return Some(accounts.clone());
    }
    workspace_index
        .and_then(|index| index.accounts_struct(accounts_name))
        .map(workspace_accounts_symbol)
}

fn account_field_symbol(accounts: &SymbolRange, account_name: &str) -> Option<SymbolRange> {
    accounts
        .fields
        .iter()
        .find(|field| field.name == account_name)
        .cloned()
}

fn workspace_accounts_symbol(accounts: &WorkspaceAccountsStruct) -> SymbolRange {
    SymbolRange {
        name: accounts.name.clone(),
        range: Range::default(),
        selection_range: Range::default(),
        fields: accounts
            .fields
            .iter()
            .map(workspace_account_field_symbol)
            .collect(),
        variants: Vec::new(),
        type_name: None,
        type_range: None,
        type_signature: None,
        generic_type_names: Vec::new(),
        generic_type_ranges: Vec::new(),
        is_optional: false,
        max_len_args: Vec::new(),
        account_constraints: Vec::new(),
        pda_constraint: None,
        instruction_arguments: accounts.instruction_arguments.clone(),
        derive_attribute_range: None,
        derive_accounts_range: None,
        derive_init_space_range: None,
        is_zero_copy: false,
    }
}

fn workspace_account_field_symbol(field: &WorkspaceAccountField) -> SymbolRange {
    SymbolRange {
        name: field.name.clone(),
        range: Range::default(),
        selection_range: Range::default(),
        fields: Vec::new(),
        variants: Vec::new(),
        type_name: field.type_name.clone(),
        type_range: None,
        type_signature: field.type_signature.clone(),
        generic_type_names: field.generic_type_names.clone(),
        generic_type_ranges: Vec::new(),
        is_optional: false,
        max_len_args: Vec::new(),
        account_constraints: field.account_constraints.clone(),
        pda_constraint: None,
        instruction_arguments: Vec::new(),
        derive_attribute_range: None,
        derive_accounts_range: None,
        derive_init_space_range: None,
        is_zero_copy: false,
    }
}

fn nested_account_path_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    instruction: &InstructionSymbol,
    usage: &AccountPathUsage,
) -> Option<Diagnostic> {
    let context = instruction.context.as_ref()?;
    let mut container = context.name.clone();

    for (index, segment) in usage.segments.iter().enumerate() {
        let fields = account_fields(document, workspace_index, &container)?;
        let Some(field) = fields.iter().find(|field| field.name == segment.name) else {
            let candidates = fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>();
            return Some(diagnostic_from_range(
                segment.range,
                AnchorDiagnosticKind::AnchorMissingAccountReference,
                format!(
                    "`{}` is used through `ctx.accounts` in `{}` but is not declared in `{}`.",
                    segment.name, instruction.name, container
                ),
                Some(serde_json::json!({
                    "account": segment.name,
                    "accountsStruct": container,
                    "instruction": instruction.name,
                    "reason": "unknown-ctx-account-field",
                    "accountPath": usage.segments.iter().map(|segment| segment.name.as_str()).collect::<Vec<_>>(),
                    "candidates": candidates,
                })),
            ));
        };

        if index > 0
            && usage.mutable
            && should_require_mut_path_field(field)
            && !has_effective_mut_path_constraint(field)
        {
            return Some(diagnostic_from_range(
                segment.range,
                AnchorDiagnosticKind::AnchorAccountUsage,
                format!(
                    "`{}` is mutated in `{}` but its account field in `{}` is missing `#[account(mut)]`.",
                    field.name, instruction.name, container
                ),
                Some(serde_json::json!({
                    "account": field.name,
                    "accountsStruct": container,
                    "instruction": instruction.name,
                    "missing": "mut",
                    "quickfix": "add-mut-constraint",
                    "accountPath": usage.segments.iter().map(|segment| segment.name.as_str()).collect::<Vec<_>>(),
                })),
            ));
        }

        if index + 1 == usage.segments.len() {
            return None;
        }

        let next_container = field
            .type_name
            .as_ref()
            .filter(|type_name| has_accounts_struct(document, workspace_index, type_name))?;
        container = next_container.clone();
    }

    None
}

fn account_fields(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    container: &str,
) -> Option<Vec<AccountPathField>> {
    if let Some(accounts) = document.symbols().accounts_structs.get(container) {
        return Some(
            accounts
                .fields
                .iter()
                .map(local_account_path_field)
                .collect(),
        );
    }
    workspace_index
        .and_then(|index| index.accounts_struct(container))
        .map(|accounts| {
            accounts
                .fields
                .iter()
                .map(workspace_account_path_field)
                .collect()
        })
}

fn local_account_path_field(field: &SymbolRange) -> AccountPathField {
    AccountPathField {
        name: field.name.clone(),
        type_name: field.type_name.clone(),
        constraints: field
            .account_constraints
            .iter()
            .map(|constraint| constraint.text.clone())
            .collect(),
    }
}

fn workspace_account_path_field(field: &WorkspaceAccountField) -> AccountPathField {
    AccountPathField {
        name: field.name.clone(),
        type_name: field.type_name.clone(),
        constraints: field
            .account_constraints
            .iter()
            .map(|constraint| constraint.text.clone())
            .collect(),
    }
}

fn should_require_mut_path_field(field: &AccountPathField) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Account")
            | Some("AccountLoader")
            | Some("InterfaceAccount")
            | Some("UncheckedAccount")
    )
}

fn has_effective_mut_path_constraint(field: &AccountPathField) -> bool {
    field.constraints.iter().any(|constraint| {
        constraint_has_any_key(constraint, &["init", "init_if_needed", "mut", "zero"])
    })
}

fn constraint_has_any_key(text: &str, keys: &[&str]) -> bool {
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
        .any(|token| keys.contains(&token))
}

fn has_accounts_struct(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    name: &str,
) -> bool {
    document.symbols().accounts_structs.contains_key(name)
        || workspace_index.is_some_and(|index| index.accounts_struct(name).is_some())
}

#[cfg(test)]
mod tests;
