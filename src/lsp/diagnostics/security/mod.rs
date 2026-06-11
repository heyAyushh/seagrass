use {
    crate::{
        anchor_types,
        diagnostics::{
            diagnostic_from_range,
            lint::{run_lint_visitor, Applicability, Confidence, LintVisitor, Region},
            registry::AnchorDiagnosticKind,
        },
        document::{ParsedDocument, SymbolRange},
        workspace::{IndexedFunctionEntry, WorkspaceIndex, MAX_REACHABILITY_DEPTH},
    },
    std::collections::{HashSet, VecDeque},
    syn::visit::{self, Visit},
    tower_lsp::lsp_types::Diagnostic,
};

mod raw_account;
#[cfg(test)]
mod signer_query;

use raw_account::raw_account_risk;

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(signer_authorization_diagnostics(document, workspace_index));
    diagnostics.extend(sysvar_address_diagnostics(document));
    diagnostics.extend(raw_owner_checking_diagnostics(document));
    diagnostics.extend(raw_type_cosplay_diagnostics(document));
    diagnostics.extend(arbitrary_cpi_program_diagnostics(document, workspace_index));
    diagnostics
}

fn signer_authorization_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        SignerAuthorizationVisitor {
            document,
            workspace_index,
            diagnostics: Vec::new(),
        },
    )
}

struct SignerAuthorizationVisitor<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for SignerAuthorizationVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::AccountsStructField];
    const CONFIDENCE: Confidence = Confidence::Authoritative;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.signer.authorization";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for SignerAuthorizationVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if let Some(accounts) = accounts_for_item(self.document, node) {
            self.diagnostics
                .extend(accounts.fields.iter().filter_map(|field| {
                    signer_authorization(self.document, self.workspace_index, accounts, field)
                }));
        }
        visit::visit_item_struct(self, node);
    }
}

fn signer_authorization(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> Option<Diagnostic> {
    if !is_unchecked_account(field)
        || !account_used_as_signer(document, workspace_index, accounts, field)
        || has_signer_constraint(field)
        || has_manual_signer_check(document, workspace_index, accounts, field)
    {
        return None;
    }

    Some(diagnostic_from_range(
        unsafe_account_range(field),
        AnchorDiagnosticKind::SecuritySigner,
        format!(
            "`{}` is used as a signer account without Anchor signer validation; use `Signer<'info>` or add `#[account(signer)]`.",
            field.name
        ),
        anchor_types::core_account_type("Signer<'info>").map(|expected| {
            serde_json::json!({
                "quickfix": "replace-account-type",
                "account": field.name,
                "expected": expected,
                "missing": "signer",
                "reason": "typed-signer",
            })
        }),
    ))
}

fn sysvar_address_diagnostics(document: &ParsedDocument) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        SysvarAddressVisitor {
            document,
            diagnostics: Vec::new(),
        },
    )
}

struct SysvarAddressVisitor<'a> {
    document: &'a ParsedDocument,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for SysvarAddressVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::AccountsStructField];
    const CONFIDENCE: Confidence = Confidence::Authoritative;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.sysvar.address";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for SysvarAddressVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if let Some(accounts) = accounts_for_item(self.document, node) {
            self.diagnostics
                .extend(accounts.fields.iter().filter_map(sysvar_address_checking));
        }
        visit::visit_item_struct(self, node);
    }
}

fn sysvar_address_checking(field: &SymbolRange) -> Option<Diagnostic> {
    if !is_unchecked_account(field)
        || anchor_types::sysvar_type_for_field(&field.name).is_none()
        || has_address_constraint(field)
    {
        return None;
    }

    Some(diagnostic_from_range(
        unsafe_account_range(field),
        AnchorDiagnosticKind::SecuritySysvar,
        format!(
            "`{}` is an unchecked sysvar account; use the typed `Sysvar<'info, _>` account or constrain its address.",
            field.name
        ),
        anchor_types::sysvar_type_for_field(&field.name).map(|expected| {
            serde_json::json!({
                "quickfix": "replace-account-type",
                "account": field.name,
                "expected": expected,
                "reason": "typed-sysvar",
            })
        }),
    ))
}

fn raw_owner_checking_diagnostics(document: &ParsedDocument) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        RawOwnerCheckVisitor {
            document,
            diagnostics: Vec::new(),
        },
    )
}

struct RawOwnerCheckVisitor<'a> {
    document: &'a ParsedDocument,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for RawOwnerCheckVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::AccountsStructField];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.owner-check";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for RawOwnerCheckVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if let Some(accounts) = accounts_for_item(self.document, node) {
            self.diagnostics.extend(
                accounts
                    .fields
                    .iter()
                    .filter_map(|field| raw_owner_checking(self.document, accounts, field)),
            );
        }
        visit::visit_item_struct(self, node);
    }
}

fn raw_owner_checking(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> Option<Diagnostic> {
    let risk = raw_account_risk(document, &accounts.name, &field.name);
    if !is_unchecked_account(field) || has_owner_constraint(field) || !risk.missing_owner_check {
        return None;
    }

    Some(diagnostic_from_range(
        unsafe_account_range(field),
        AnchorDiagnosticKind::SecurityOwnerCheck,
        format!(
            "`{}` is read as raw account data, but no owner check is visible.",
            field.name
        ),
        Some(serde_json::json!({
            "quickfix": "add-owner-constraint",
            "account": field.name,
            "attack": "owner-checks",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "parsed-anchor-accounts-plus-alias-scan",
            "configKey": "security.ownerChecks",
            "suggestion": "Prefer `Account<'info, T>` when the owner is this program, or add `#[account(owner = ...)]` / an explicit runtime owner check for raw accounts.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    ))
}

fn raw_type_cosplay_diagnostics(document: &ParsedDocument) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        RawTypeCosplayVisitor {
            document,
            diagnostics: Vec::new(),
        },
    )
}

struct RawTypeCosplayVisitor<'a> {
    document: &'a ParsedDocument,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for RawTypeCosplayVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::AccountsStructField];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.type-cosplay";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for RawTypeCosplayVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if let Some(accounts) = accounts_for_item(self.document, node) {
            self.diagnostics.extend(
                accounts
                    .fields
                    .iter()
                    .filter_map(|field| raw_type_cosplay_checking(self.document, accounts, field)),
            );
        }
        visit::visit_item_struct(self, node);
    }
}

fn raw_type_cosplay_checking(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> Option<Diagnostic> {
    let risk = raw_account_risk(document, &accounts.name, &field.name);
    if !is_unchecked_account(field) || !risk.missing_discriminator_check {
        return None;
    }

    Some(diagnostic_from_range(
        unsafe_account_range(field),
        AnchorDiagnosticKind::SecurityTypeCosplay,
        format!(
            "`{}` is deserialized from raw data, but no discriminator/type check is visible.",
            field.name
        ),
        Some(serde_json::json!({
            "quickfix": "typed-account-or-discriminator",
            "account": field.name,
            "attack": "type-cosplay",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "parsed-anchor-accounts-plus-alias-scan",
            "configKey": "security.typeCosplay",
            "suggestion": "Prefer Anchor account deserialization with a typed account, or validate the first discriminator/type bytes before decoding raw data.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    ))
}

fn arbitrary_cpi_program_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        ArbitraryCpiProgramVisitor {
            document,
            workspace_index,
            diagnostics: Vec::new(),
        },
    )
}

struct ArbitraryCpiProgramVisitor<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for ArbitraryCpiProgramVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::AccountsStructField];
    const CONFIDENCE: Confidence = Confidence::Authoritative;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.cpi.program";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for ArbitraryCpiProgramVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if let Some(accounts) = accounts_for_item(self.document, node) {
            self.diagnostics
                .extend(accounts.fields.iter().filter_map(|field| {
                    arbitrary_cpi_program(self.document, self.workspace_index, accounts, field)
                }));
        }
        visit::visit_item_struct(self, node);
    }
}

fn arbitrary_cpi_program(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> Option<Diagnostic> {
    if !account_used_as_cpi_program(document, workspace_index, accounts, field)
        || !is_unchecked_account(field)
        || has_address_constraint(field)
        || has_owner_constraint(field)
        || has_reachable_program_account_check(document, workspace_index, accounts, field)
    {
        return None;
    }

    Some(diagnostic_from_range(
        unsafe_account_range(field),
        AnchorDiagnosticKind::SecurityCpiProgram,
        format!(
            "`{}` is used as a CPI program account without Anchor program-id validation; use `Program<'info, _>` or constrain the program id.",
            field.name
        ),
        typed_cpi_program_quickfix(field),
    ))
}

fn accounts_for_item<'a>(
    document: &'a ParsedDocument,
    node: &syn::ItemStruct,
) -> Option<&'a SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .get(&node.ident.to_string())
}

fn is_unchecked_account(field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("AccountInfo") | Some("UncheckedAccount")
    )
}

fn unsafe_account_range(field: &SymbolRange) -> tower_lsp::lsp_types::Range {
    field.type_range.unwrap_or(field.selection_range)
}

fn has_signer_constraint(field: &SymbolRange) -> bool {
    has_constraint(field, |text| constraint_has_flag(text, "signer"))
}

fn has_address_constraint(field: &SymbolRange) -> bool {
    has_constraint(field, |text| {
        constraint_has_assignment(text, "address") || constraint_has_assignment(text, "constraint")
    })
}

fn has_owner_constraint(field: &SymbolRange) -> bool {
    has_constraint(field, |text| {
        constraint_has_assignment(text, "owner") || constraint_has_assignment(text, "constraint")
    })
}

pub(super) fn has_constraint(field: &SymbolRange, matches: impl Fn(&str) -> bool) -> bool {
    field.account_constraints.iter().any(|constraint| {
        constraint.range.start.line <= field.selection_range.start.line && matches(&constraint.text)
    })
}

fn constraint_has_assignment(text: &str, key: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        if constraint_key_boundary(text, idx, key.len()) {
            let after_key = &text[idx + key.len()..];
            if after_key.trim_start().starts_with('=') {
                return true;
            }
        }
        search_start = idx + 1;
    }
    false
}

pub(super) fn constraint_has_flag(text: &str, key: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        if constraint_key_boundary(text, idx, key.len()) {
            let after_key = text[idx + key.len()..].trim_start();
            if after_key.is_empty() || after_key.starts_with(',') || after_key.starts_with(')') {
                return true;
            }
        }
        search_start = idx + 1;
    }
    false
}

fn constraint_key_boundary(text: &str, idx: usize, key_len: usize) -> bool {
    let previous = text[..idx].chars().next_back();
    let next = text[idx + key_len..].chars().next();
    previous
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .unwrap_or(true)
        && next
            .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
            .unwrap_or(true)
}

fn account_used_as_cpi_program(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> bool {
    if let Some(usages) =
        reachable_local_account_usage_names_for_context(document, &accounts.name, |function| {
            &function.cpi_program_usages
        })
    {
        return !usages.truncated && usages.names.contains(&field.name);
    }

    if local_account_usage_exists(document, accounts, field, |function| {
        &function.cpi_program_usages
    }) {
        return true;
    }

    workspace_index
        .and_then(|index| {
            reachable_workspace_account_usage_names_for_context(
                index,
                &accounts.name,
                ReachabilityUse::Offense,
                None,
                |function| &function.cpi_program_usages,
            )
        })
        .is_some_and(|usages| !usages.truncated && usages.names.contains(&field.name))
}

fn account_used_as_signer(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> bool {
    if let Some(usages) =
        reachable_local_account_usage_names_for_context(document, &accounts.name, |function| {
            &function.signer_usages
        })
    {
        return !usages.truncated && usages.names.contains(&field.name);
    }

    if local_account_usage_exists(document, accounts, field, |function| {
        &function.signer_usages
    }) {
        return true;
    }

    workspace_index
        .and_then(|index| {
            reachable_workspace_account_usage_names_for_context(
                index,
                &accounts.name,
                ReachabilityUse::Defense,
                None,
                |function| &function.signer_usages,
            )
        })
        .is_some_and(|usages| usages.names.contains(&field.name))
}

fn has_manual_signer_check(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> bool {
    if let Some(checks) =
        reachable_local_account_usage_names_for_context(document, &accounts.name, |function| {
            &function.signer_checks
        })
    {
        return checks.truncated || checks.names.contains(&field.name);
    }

    if local_account_usage_exists(document, accounts, field, |function| {
        &function.signer_checks
    }) {
        return true;
    }

    workspace_index
        .and_then(|index| {
            reachable_workspace_account_usage_names_for_context(
                index,
                &accounts.name,
                ReachabilityUse::Defense,
                Some(&field.name),
                |function| &function.signer_checks,
            )
        })
        .is_some_and(|checks| checks.truncated || checks.names.contains(&field.name))
}

fn has_reachable_program_account_check(
    _document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> bool {
    workspace_index
        .and_then(|index| index.reachable_function_entries_for_context(&accounts.name))
        .is_some_and(|(entries, truncated)| {
            truncated
                || entries.iter().any(|function| {
                    function.account_key_comparisons.iter().any(|comparison| {
                        comparison.left == field.name || comparison.right == field.name
                    })
                })
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReachabilityUse {
    Defense,
    Offense,
}

struct ReachableAccountUsageNames {
    names: HashSet<String>,
    truncated: bool,
}

fn reachable_workspace_account_usage_names_for_context(
    index: &WorkspaceIndex,
    context_name: &str,
    reachability_use: ReachabilityUse,
    exact_field_name: Option<&str>,
    usages: impl Fn(&IndexedFunctionEntry) -> &[crate::document::AccountUsage],
) -> Option<ReachableAccountUsageNames> {
    let (entries, truncated) = match reachability_use {
        ReachabilityUse::Defense => {
            index.reachable_function_entries_for_context_matching(context_name, |function| {
                function.context_name.as_deref() == Some(context_name)
                    || exact_field_name.is_some_and(|field_name| {
                        usages(function)
                            .iter()
                            .any(|usage| usage.name == field_name)
                    })
            })?
        }
        ReachabilityUse::Offense => {
            index.unambiguous_reachable_function_entries_for_context(context_name)?
        }
    };
    Some(ReachableAccountUsageNames {
        names: entries
            .iter()
            .flat_map(|function| usages(function))
            .map(|usage| usage.name.clone())
            .collect(),
        truncated,
    })
}

fn local_account_usage_exists(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    field: &SymbolRange,
    usages: impl Fn(&crate::document::InstructionSymbol) -> &[crate::document::AccountUsage],
) -> bool {
    document.symbols().callable_functions().any(|function| {
        function
            .context
            .as_ref()
            .is_some_and(|context| context.name == accounts.name)
            && usages(function)
                .iter()
                .any(|usage| usage.name == field.name)
    })
}

fn reachable_local_account_usage_names_for_context(
    document: &ParsedDocument,
    context_name: &str,
    usages: impl Fn(&crate::document::InstructionSymbol) -> &[crate::document::AccountUsage],
) -> Option<ReachableAccountUsageNames> {
    let instructions = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == context_name)
        })
        .collect::<Vec<_>>();
    if instructions.is_empty() {
        return None;
    }

    let mut reachable = instructions
        .iter()
        .map(|instruction| instruction.name.clone())
        .collect::<HashSet<_>>();
    let mut pending = instructions
        .iter()
        .flat_map(|instruction| instruction.function_calls.iter())
        .map(|call| (call.name.clone(), 1_usize))
        .collect::<VecDeque<_>>();
    let mut truncated = false;

    while let Some((name, depth)) = pending.pop_front() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        let Some(function) = document
            .symbols()
            .functions
            .iter()
            .find(|function| function.name == name)
        else {
            continue;
        };
        if depth >= MAX_REACHABILITY_DEPTH {
            truncated |= !function.function_calls.is_empty();
            continue;
        }
        pending.extend(
            function
                .function_calls
                .iter()
                .map(|call| (call.name.clone(), depth + 1)),
        );
    }

    Some(ReachableAccountUsageNames {
        names: document
            .symbols()
            .callable_functions()
            .filter(|function| reachable.contains(&function.name))
            .flat_map(usages)
            .map(|usage| usage.name.clone())
            .collect(),
        truncated,
    })
}

fn typed_cpi_program_quickfix(field: &SymbolRange) -> Option<serde_json::Value> {
    if let Some(expected) =
        anchor_types::field_type_for_field_name(&field.name).filter(|expected| {
            expected.starts_with("Program<'info,") || expected.starts_with("Interface<'info,")
        })
    {
        return Some(serde_json::json!({
            "quickfix": "replace-account-type",
            "account": field.name,
            "expected": expected,
            "reason": "typed-cpi-program",
        }));
    }

    Some(serde_json::json!({
        "quickfix": "add-missing-constraint",
        "account": field.name,
        "missing": "executable",
        "reason": "executable-cpi-program",
    }))
}

#[cfg(test)]
mod tests;
