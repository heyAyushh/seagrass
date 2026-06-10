use {
    crate::{
        account_semantics::{self, AccountTypeEvidence},
        anchor_types::{self, AnchorFieldCompletionKind},
        diagnostics::{diagnostic_from_range, diagnostic_from_syn_error},
        document::{derives_accounts, has_attr, ParsedDocument},
        range::range_from_span,
        solana::runtime_catalog,
        workspace::WorkspaceIndex,
    },
    std::collections::HashSet,
    syn::{
        spanned::Spanned, Field, GenericArgument, GenericParam, Item, ItemStruct, PathArguments,
        Type,
    },
    tower_lsp::lsp_types::{Diagnostic, Position, Range, SymbolKind},
};

use super::registry::AnchorDiagnosticKind;

mod program;

use program::validate_program;

enum InvalidSysvarDiagnostic {
    Diagnostic(Box<Diagnostic>),
    Suppress,
    Fallback,
}

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    document
        .syntax()
        .items
        .iter()
        .flat_map(|item| match item {
            Item::Mod(item_mod) if has_attr(&item_mod.attrs, "program") => {
                validate_program(item_mod)
            }
            Item::Struct(item_struct) if derives_accounts(&item_struct.attrs) => {
                validate_accounts(document, item_struct, workspace_index)
            }
            _ => Vec::new(),
        })
        .collect()
}

fn validate_accounts(
    document: &ParsedDocument,
    item_struct: &ItemStruct,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let mut semantic_diagnostics =
        unresolved_account_generic_diagnostics(document, item_struct, workspace_index);
    semantic_diagnostics.extend(account_wrapper_shape_diagnostics(
        item_struct,
        &semantic_diagnostics,
    ));
    let mut diagnostics = match anchor_syn::parser::accounts::parse(item_struct) {
        Ok(_) => Vec::new(),
        Err(err) => {
            if err.to_string().contains("invalid sysvar provided") {
                match invalid_sysvar_diagnostic(document, item_struct, &err) {
                    InvalidSysvarDiagnostic::Diagnostic(diagnostic) => vec![*diagnostic],
                    InvalidSysvarDiagnostic::Suppress => Vec::new(),
                    InvalidSysvarDiagnostic::Fallback => vec![diagnostic_from_syn_error(err)],
                }
            } else if is_anchor_account_shape_error(&err)
                && semantic_diagnostics
                    .iter()
                    .any(is_anchor_account_shape_diagnostic)
            {
                Vec::new()
            } else {
                vec![diagnostic_from_syn_error(err)]
            }
        }
    };
    diagnostics.extend(semantic_diagnostics);
    diagnostics
}

fn is_anchor_account_shape_error(err: &syn::Error) -> bool {
    let message = err.to_string();
    message.contains("bracket arguments must be the lifetime and type")
        || message.contains("expected angle brackets with a lifetime and type")
        || message.contains("first bracket argument must be a lifetime")
        || message.contains("segmented paths are not currently allowed")
        || message.contains("invalid account type given")
}

fn is_anchor_account_shape_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("reason"))
        .and_then(|value| value.as_str())
        .is_some_and(|reason| {
            matches!(
                reason,
                "unresolved-generic-account-type" | "anchor-account-wrapper-shape"
            )
        })
}

fn unresolved_account_generic_diagnostics(
    document: &ParsedDocument,
    item_struct: &ItemStruct,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let declared_type_params = item_struct
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(type_param) => Some(type_param.ident.to_string()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let local_struct_names = document
        .symbols()
        .all_structs
        .keys()
        .cloned()
        .collect::<HashSet<_>>();
    let inferred_expected =
        account_semantics::expected_account_inner_types_for_accounts_struct(item_struct);

    item_struct
        .fields
        .iter()
        .filter_map(|field| {
            unresolved_account_generic_diagnostic(
                field,
                &declared_type_params,
                &local_struct_names,
                &inferred_expected,
                document,
                workspace_index,
            )
        })
        .collect()
}

fn unresolved_account_generic_diagnostic(
    field: &Field,
    declared_type_params: &HashSet<String>,
    local_struct_names: &HashSet<String>,
    inferred_expected: &std::collections::HashMap<
        String,
        account_semantics::ExpectedAccountInnerType,
    >,
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<Diagnostic> {
    let field_name = field.ident.as_ref()?.to_string();
    let (container, generic, generic_range, container_range, boxed_wrapper) =
        account_generic_argument(&field.ty)?;
    let resolved_generic = document.symbols().resolve_type_alias(&generic);

    if declared_type_params.contains(&generic)
        || local_struct_names.contains(&generic)
        || local_struct_names.contains(resolved_generic)
        || is_known_workspace_type(workspace_index, &generic)
        || is_known_workspace_type(workspace_index, resolved_generic)
        || is_generated_anchor_account_inner_type(&container, resolved_generic)
    {
        return None;
    }

    let replacement = inferred_expected.get(&field_name).copied().or_else(|| {
        account_semantics::expected_account_inner_type_for_syn_field(&container, field)
    });
    if !has_unresolved_inner_type_evidence(replacement) {
        return None;
    }

    let message = match (generic.is_empty(), replacement) {
        (true, Some(expected)) => format!(
            "Missing account data type for `{field_name}`; use `{container}<'info, {}>`.",
            expected.generic
        ),
        (false, Some(expected)) => format!(
            "`{generic}` is unresolved in `{container}<'info, {generic}>` for `{field_name}`; use `{container}<'info, {}>`.",
            expected.generic
        ),
        _ => format!(
            "`{generic}` is unresolved in `{container}<'info, {generic}>` for `{field_name}`."
        ),
    };
    let range = if replacement.is_some() && !boxed_wrapper {
        container_range
    } else {
        generic_range
    };

    let data = replacement.map(|expected| {
        serde_json::json!({
            "quickfix": "replace-account-type",
            "account": field_name,
            "expected": format!("{container}<'info, {}>", expected.generic),
            "current": format!("{container}<'info, {generic}>"),
            "reason": "unresolved-generic-account-type",
            "evidence": match expected.evidence {
                AccountTypeEvidence::Constraint => "constraint",
                AccountTypeEvidence::AccountReference => "account-reference",
                AccountTypeEvidence::TokenAccountProperty => "token-account-property",
            },
        })
    });

    Some(diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorSyn,
        message,
        data,
    ))
}

fn account_wrapper_shape_diagnostics(
    item_struct: &ItemStruct,
    existing: &[Diagnostic],
) -> Vec<Diagnostic> {
    item_struct
        .fields
        .iter()
        .filter_map(|field| account_wrapper_shape_diagnostic(field, existing))
        .collect()
}

fn account_wrapper_shape_diagnostic(field: &Field, existing: &[Diagnostic]) -> Option<Diagnostic> {
    let field_name = field.ident.as_ref()?.to_string();
    if existing.iter().any(|diagnostic| {
        is_anchor_account_shape_diagnostic(diagnostic)
            && diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("account"))
                .and_then(|value| value.as_str())
                == Some(field_name.as_str())
    }) {
        return None;
    }

    let Type::Path(type_path) = &field.ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let container = segment.ident.to_string();
    if !is_anchor_account_generic_wrapper(&container) {
        return None;
    }
    let range = range_from_span(field.ty.span());

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Some(account_wrapper_shape_error(
            &field_name,
            &container,
            range,
            format!(
                "`{field_name}` uses `{container}` without generic arguments; use `{container}<'info, AccountType>`."
            ),
            None,
            "missing-generic-arguments",
        ));
    };

    let first_is_lifetime = arguments
        .args
        .first()
        .is_some_and(|argument| matches!(argument, GenericArgument::Lifetime(_)));
    let type_args = generic_type_argument_names(arguments);

    if !first_is_lifetime {
        let expected = type_args
            .first()
            .map(|generic| format!("{container}<'info, {generic}>"));
        let example = expected
            .clone()
            .unwrap_or_else(|| account_wrapper_shape_example(&container));
        return Some(account_wrapper_shape_error(
            &field_name,
            &container,
            range,
            format!(
                "`{field_name}` uses `{container}` without the `'info` lifetime first; use `{example}`."
            ),
            expected,
            "missing-lifetime",
        ));
    }

    if type_args.is_empty() {
        let expected = account_wrapper_missing_type_expected(&field_name, &container);
        let example = expected
            .clone()
            .unwrap_or_else(|| account_wrapper_shape_example(&container));
        return Some(account_wrapper_shape_error(
            &field_name,
            &container,
            range,
            format!("Missing account data type for `{field_name}`; use `{example}`."),
            expected,
            "missing-account-data-type",
        ));
    }

    if type_path.path.segments.len() > 1 {
        let expected = Some(format!("{container}<'info, {}>", type_args.join(", ")));
        return Some(account_wrapper_shape_error(
            &field_name,
            &container,
            range,
            format!(
                "`{field_name}` uses qualified `{container}`; import the Anchor wrapper and write `{}`.",
                expected
                    .clone()
                    .unwrap_or_else(|| account_wrapper_shape_example(&container))
            ),
            expected,
            "qualified-wrapper-path",
        ));
    }

    None
}

fn account_wrapper_shape_error(
    field_name: &str,
    container: &str,
    range: Range,
    message: String,
    expected: Option<String>,
    shape: &'static str,
) -> Diagnostic {
    let data = match expected {
        Some(expected) => serde_json::json!({
            "quickfix": "replace-account-type",
            "account": field_name,
            "container": container,
            "expected": expected,
            "reason": "anchor-account-wrapper-shape",
            "shape": shape,
        }),
        None => serde_json::json!({
            "account": field_name,
            "container": container,
            "reason": "anchor-account-wrapper-shape",
            "shape": shape,
        }),
    };

    diagnostic_from_range(range, AnchorDiagnosticKind::AnchorSyn, message, Some(data))
}

fn account_wrapper_missing_type_expected(field_name: &str, container: &str) -> Option<String> {
    if container == "Sysvar" {
        return anchor_types::sysvar_generic_for_field(field_name)
            .map(|generic| format!("Sysvar<'info, {generic}>"));
    }
    None
}

fn generic_type_argument_names(arguments: &syn::AngleBracketedGenericArguments) -> Vec<String> {
    arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(Type::Path(type_path)) => type_path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        })
        .collect()
}

fn is_anchor_account_generic_wrapper(container: &str) -> bool {
    matches!(
        container,
        "Account"
            | "InterfaceAccount"
            | "Program"
            | "Interface"
            | "AccountLoader"
            | "LazyAccount"
            | "Sysvar"
    )
}

fn account_wrapper_shape_example(container: &str) -> String {
    match container {
        "Sysvar" => format!("Sysvar<'info, {}>", example_sysvar_type_ident()),
        "Program" => "Program<'info, System>".to_string(),
        "Interface" => "Interface<'info, TokenInterface>".to_string(),
        "Account" => "Account<'info, AccountType>".to_string(),
        "InterfaceAccount" => "InterfaceAccount<'info, AccountType>".to_string(),
        "AccountLoader" => "AccountLoader<'info, AccountType>".to_string(),
        "LazyAccount" => "LazyAccount<'info, AccountType>".to_string(),
        _ => "Account<'info, AccountType>".to_string(),
    }
}

fn example_sysvar_type_ident() -> &'static str {
    runtime_catalog::SYSVARS
        .iter()
        .find(|sysvar| !sysvar.is_deprecated)
        .map(|sysvar| sysvar.type_ident)
        .unwrap_or("Sysvar")
}

fn account_generic_argument(ty: &Type) -> Option<(String, String, Range, Range, bool)> {
    account_generic_argument_with_box_state(ty, false)
}

fn account_generic_argument_with_box_state(
    ty: &Type,
    boxed_wrapper: bool,
) -> Option<(String, String, Range, Range, bool)> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let container = segment.ident.to_string();
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };

    if container == "Box" || container == "Option" {
        let nested = arguments.args.iter().find_map(|argument| match argument {
            GenericArgument::Type(inner_ty) => {
                account_generic_argument_with_box_state(inner_ty, container == "Box")
            }
            _ => None,
        })?;
        return Some(nested);
    }

    if !matches!(
        container.as_str(),
        "Account"
            | "InterfaceAccount"
            | "Program"
            | "Interface"
            | "AccountLoader"
            | "LazyAccount"
            | "Migration"
            | "Sysvar"
    ) {
        return None;
    }

    if let Some(generic) = arguments
        .args
        .iter()
        .rev()
        .find_map(|argument| match argument {
            GenericArgument::Type(Type::Path(type_path)) => {
                type_path.path.segments.last().map(|segment| {
                    (
                        container.clone(),
                        segment.ident.to_string(),
                        range_from_span(segment.ident.span()),
                        range_from_span(ty.span()),
                        boxed_wrapper,
                    )
                })
            }
            _ => None,
        })
    {
        return Some(generic);
    }

    let has_lifetime_argument = arguments
        .args
        .iter()
        .any(|argument| matches!(argument, GenericArgument::Lifetime(_)));
    let has_type_argument = arguments
        .args
        .iter()
        .any(|argument| matches!(argument, GenericArgument::Type(_)));

    (has_lifetime_argument && !has_type_argument).then(|| {
        (
            container,
            String::new(),
            range_from_span(arguments.gt_token.span),
            range_from_span(ty.span()),
            boxed_wrapper,
        )
    })
}

fn is_known_workspace_type(workspace_index: Option<&WorkspaceIndex>, generic: &str) -> bool {
    workspace_index.is_some_and(|index| {
        !index
            .symbol_locations_with_kinds(generic, &[SymbolKind::STRUCT, SymbolKind::ENUM])
            .is_empty()
    })
}

fn is_generated_anchor_account_inner_type(container: &str, generic: &str) -> bool {
    let generic_type_names = [generic.to_string()];
    anchor_types::field_type_completion(container, &generic_type_names).is_some()
}

fn has_unresolved_inner_type_evidence(
    replacement: Option<account_semantics::ExpectedAccountInnerType>,
) -> bool {
    match replacement {
        Some(expected) => matches!(
            expected.evidence,
            AccountTypeEvidence::Constraint
                | AccountTypeEvidence::AccountReference
                | AccountTypeEvidence::TokenAccountProperty
        ),
        None => false,
    }
}

fn invalid_sysvar_diagnostic(
    document: &ParsedDocument,
    item_struct: &ItemStruct,
    err: &syn::Error,
) -> InvalidSysvarDiagnostic {
    let err_range = range_from_span(err.span());
    let candidates = item_struct
        .fields
        .iter()
        .filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let (current, current_range) = sysvar_generic_argument(&field.ty)?;
            let replacement = anchor_types::sysvar_generic_for_field(&ident.to_string());
            if replacement.is_some_and(|replacement| current == replacement) {
                return None;
            }
            Some((ident.to_string(), current, replacement, current_range))
        })
        .collect::<Vec<_>>();

    let Some((field_name, current, replacement, range)) = candidates
        .iter()
        .find(|(_, _, _, range)| ranges_overlap(*range, err_range))
        .or_else(|| candidates.first())
    else {
        return InvalidSysvarDiagnostic::Fallback;
    };
    let replacement = *replacement;

    if is_transient_incomplete_sysvar(document.source(), current, *range) {
        return InvalidSysvarDiagnostic::Suppress;
    }

    let example_sysvar = example_sysvar_type_ident();
    let Some(replacement) = replacement else {
        return InvalidSysvarDiagnostic::Diagnostic(Box::new(diagnostic_from_range(
            *range,
            AnchorDiagnosticKind::AnchorSyn,
            format!(
                "`{current}` is not an Anchor sysvar type for `{field_name}`; use a known sysvar such as `{example_sysvar}`."
            ),
            Some(serde_json::json!({
                "account": field_name,
                "current": current,
                "reason": "invalid-sysvar-type",
                "candidates": sysvar_generic_candidates(),
                "generatedFrom": "lang/syn/src/parser/accounts/mod.rs",
            })),
        )));
    };

    InvalidSysvarDiagnostic::Diagnostic(Box::new(diagnostic_from_range(
        *range,
        AnchorDiagnosticKind::AnchorSyn,
        format!(
            "`{current}` is not an Anchor sysvar for `{field_name}`; use `Sysvar<'info, {replacement}>`."
        ),
        Some(serde_json::json!({
            "quickfix": "replace-invalid-sysvar",
            "account": field_name,
            "current": current,
            "replacement": replacement,
            "reason": "invalid-sysvar-type",
            "generatedFrom": "lang/syn/src/parser/accounts/mod.rs",
        })),
    )))
}

fn sysvar_generic_candidates() -> Vec<&'static str> {
    let mut candidates = anchor_types::field_completions()
        .iter()
        .filter(|completion| completion.kind == AnchorFieldCompletionKind::Sysvar)
        .filter_map(|completion| {
            completion
                .label
                .rsplit_once(',')
                .and_then(|(_, generic)| generic.trim().strip_suffix('>'))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn is_transient_incomplete_sysvar(source: &str, current: &str, range: Range) -> bool {
    starts_lowercase_identifier(current) && type_argument_still_open(source, range)
}

fn starts_lowercase_identifier(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
}

fn type_argument_still_open(source: &str, range: Range) -> bool {
    let Some(line) = crate::range::line_at(source, range.end.line) else {
        return false;
    };
    let Ok(end) = usize::try_from(range.end.character) else {
        return false;
    };
    let after = line
        .char_indices()
        .nth(end)
        .map_or("", |(offset, _)| &line[offset..])
        .trim_start();

    after.is_empty() || after.starts_with("//")
}

fn sysvar_generic_argument(ty: &Type) -> Option<(String, Range)> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Sysvar" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(Type::Path(type_path)) => {
            type_path.path.segments.last().map(|segment| {
                (
                    segment.ident.to_string(),
                    range_from_span(segment.ident.span()),
                )
            })
        }
        _ => None,
    })
}

fn ranges_overlap(left: Range, right: Range) -> bool {
    position_le(left.start, right.end) && position_le(right.start, left.end)
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || left.line == right.line && left.character <= right.character
}

#[cfg(test)]
mod tests;
