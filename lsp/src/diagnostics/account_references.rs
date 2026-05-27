use {
    crate::{
        account_semantics,
        constraint_catalog::ConstraintValueKind,
        constraint_text,
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::{ParsedDocument, SymbolRange},
        evidence::{AccountSetEvidence, EvidenceGraph},
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Url},
};

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    EvidenceGraph::from_document(document)
        .account_sets()
        .iter()
        .flat_map(|accounts| missing_account_references(document, accounts, workspace_index))
        .collect()
}

fn missing_account_references(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let workspace_instruction_args = workspace_index
        .map(|index| index.instruction_argument_names_for_context(&accounts.accounts.name))
        .unwrap_or_default();
    let has_workspace_instruction_mapping = workspace_index
        .is_some_and(|index| index.has_program_instruction_for_context(&accounts.accounts.name));

    let mut diagnostics = accounts
        .fields()
        .iter()
        .flat_map(|field| {
            field.constraints().iter().flat_map(|constraint| {
                constraint
                    .account_references()
                    .into_iter()
                    .filter(|reference| {
                        if accounts.has_account(reference.name) {
                            return false;
                        }
                        if reference.value_kind != ConstraintValueKind::ProgramReference {
                            return true;
                        }
                        !accounts.has_instruction_argument(reference.name)
                            && !workspace_instruction_args.contains(reference.name)
                            && !document_has_constant(document, reference.name)
                    })
                    .map(|reference| {
                        let range = constraint
                            .value_range(document.source(), reference.key, reference.name)
                            .unwrap_or(constraint.range());
                        let candidates =
                            replacement_candidates(accounts, reference.key, reference.name);
                        let mut data = serde_json::json!({
                            "constraint": reference.key,
                            "account": reference.name,
                            "accountsStruct": accounts.accounts.name,
                        });
                        if !candidates.is_empty() {
                            data["candidates"] = serde_json::Value::Array(
                                candidates
                                    .iter()
                                    .cloned()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            );
                        }

                        diagnostic_from_range_with_related(
                            range,
                            AnchorDiagnosticKind::AnchorMissingAccountReference,
                            format!(
                                "`{}` is referenced by `{}` but is not declared in `{}`.",
                                reference.name, reference.key, accounts.accounts.name
                            ),
                            Some(data),
                            Some(missing_account_related_information(accounts, &candidates)),
                        )
                    })
            })
        })
        .collect::<Vec<_>>();

    if accounts.has_instruction_mapping() || has_workspace_instruction_mapping {
        diagnostics.extend(accounts.fields().iter().flat_map(|field| {
            field.constraints().iter().flat_map(|constraint| {
                constraint
                    .instruction_argument_references()
                    .into_iter()
                    .filter(|reference| {
                        let declared_for_accounts = accounts
                            .accounts
                            .instruction_arguments
                            .iter()
                            .any(|argument| instruction_arg_names_match(&argument.name, reference.name));
                        !declared_for_accounts
                    })
                    .map(|reference| {
                        let range = constraint
                            .value_range(document.source(), reference.key, reference.name)
                            .unwrap_or(constraint.range());
                        let mut data = serde_json::json!({
                            "constraint": reference.key,
                            "argument": reference.name,
                            "accountsStruct": accounts.accounts.name,
                            "quickfix": "add-instruction-argument",
                        });
                        if let Some(type_name) = accounts.instruction_argument_type(reference.name) {
                            data["argumentType"] = serde_json::Value::String(type_name.to_string());
                        } else if let Some(type_name) = workspace_index.and_then(|index| {
                            index.instruction_argument_type_for_context(
                                &accounts.accounts.name,
                                reference.name,
                            )
                        }) {
                            data["argumentType"] = serde_json::Value::String(type_name);
                        }

                        diagnostic_from_range_with_related(
                            range,
                            AnchorDiagnosticKind::AnchorMissingInstructionArgument,
                            format!(
                                "`{}` is referenced by `{}` but is not an instruction argument for `{}`.",
                                reference.name, reference.key, accounts.accounts.name
                            ),
                            Some(data),
                            Some(missing_instruction_argument_related_information(
                                accounts,
                                reference.name,
                            )),
                        )
                    })
            })
        }));
    }

    diagnostics
}

fn missing_instruction_argument_related_information(
    accounts: &AccountSetEvidence<'_>,
    argument: &str,
) -> Vec<DiagnosticRelatedInformation> {
    let mut related = Vec::with_capacity(2);
    let uri = current_document_uri();
    related.push(DiagnosticRelatedInformation {
        location: Location {
            uri: uri.clone(),
            range: accounts.accounts.selection_range,
        },
        message: format!(
            "`{}` is the Accounts struct whose #[instruction(...)] arguments are checked.",
            accounts.accounts.name
        ),
    });
    if let Some((instruction, range)) = accounts.instruction_argument_location(argument) {
        related.push(DiagnosticRelatedInformation {
            location: Location { uri, range },
            message: format!("handler `{instruction}` declares `{argument}` here."),
        });
    }
    related
}

fn missing_account_related_information(
    accounts: &AccountSetEvidence<'_>,
    candidates: &[String],
) -> Vec<DiagnosticRelatedInformation> {
    let max_candidates = candidates.len().min(5);
    let mut related = Vec::with_capacity(max_candidates + 1);
    let uri = current_document_uri();
    related.push(DiagnosticRelatedInformation {
        location: Location {
            uri: uri.clone(),
            range: accounts.accounts.selection_range,
        },
        message: format!(
            "`{}` is the Accounts struct being checked.",
            accounts.accounts.name
        ),
    });
    for candidate in candidates.iter().take(5) {
        if let Some(field) = accounts
            .fields()
            .iter()
            .find(|field| field.field.name == *candidate)
        {
            related.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: field.field.selection_range,
                },
                message: format!("candidate account field `{candidate}` is declared here."),
            });
        }
    }
    related
}

fn replacement_candidates(
    accounts: &AccountSetEvidence<'_>,
    constraint_key: &str,
    missing: &str,
) -> Vec<String> {
    let mut candidates = accounts
        .fields()
        .iter()
        .map(|field| &field.field)
        .filter(|field| field.name != missing)
        .map(|field| {
            (
                candidate_role_rank(accounts.accounts, field, constraint_key),
                edit_distance(&field.name, missing),
                field.name.clone(),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });
    candidates.truncate(8);
    candidates.into_iter().map(|(_, _, name)| name).collect()
}

fn candidate_role_rank(accounts: &SymbolRange, field: &SymbolRange, constraint_key: &str) -> u8 {
    let key = constraint_key.to_ascii_lowercase();

    if constraint_key_has_segment(&key, "authority") {
        return if is_signer_like_field(field) { 0 } else { 1 };
    }
    if constraint_key_has_segment(&key, "payer") {
        return if is_signer_like_field(field) { 0 } else { 1 };
    }
    if constraint_key_has_segment(&key, "mint") {
        return if is_mint_account_field(accounts, field) {
            0
        } else {
            1
        };
    }
    if constraint_key_has_segment(&key, "program") {
        return if is_program_field(field) { 0 } else { 1 };
    }
    0
}

fn constraint_key_has_segment(key: &str, expected: &str) -> bool {
    key.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|segment| segment == expected)
}

fn is_signer_like_field(field: &SymbolRange) -> bool {
    field.type_name.as_deref() == Some("Signer")
        || field
            .account_constraints
            .iter()
            .any(|constraint| constraint_text::has_flag_or_key(&constraint.text, "signer"))
}

fn is_mint_account_field(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Account") | Some("InterfaceAccount")
    ) && account_semantics::field_has_declared_or_expected_account_inner_type(
        accounts, field, "Mint",
    )
}

fn is_program_field(field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Program") | Some("Interface")
    )
}

fn edit_distance(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    if left.is_empty() {
        return right.chars().count();
    }
    if right.is_empty() {
        return left.chars().count();
    }

    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0usize; right_chars.len() + 1];

    for (left_idx, left_char) in left.chars().enumerate() {
        current[0] = left_idx + 1;
        for (right_idx, right_char) in right_chars.iter().enumerate() {
            let replace_cost = usize::from(left_char != *right_char);
            current[right_idx + 1] = (previous[right_idx + 1] + 1)
                .min(current[right_idx] + 1)
                .min(previous[right_idx] + replace_cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}

fn current_document_uri() -> Url {
    Url::parse("file:///seagrass/current-document.rs").unwrap()
}

fn instruction_arg_names_match(attribute_name: &str, handler_name: &str) -> bool {
    attribute_name == handler_name
        || attribute_name.trim_start_matches('_') == handler_name.trim_start_matches('_')
}

fn document_has_constant(document: &ParsedDocument, name: &str) -> bool {
    document
        .symbols()
        .constants
        .iter()
        .any(|constant| constant.name == name)
}

#[cfg(test)]
#[path = "account_references_tests.rs"]
mod tests;
