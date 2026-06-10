use {
    crate::{
        diagnostics::{
            diagnostic_from_range,
            lint::{run_lint_visitor, Applicability, Confidence, LintVisitor, Region},
            registry::AnchorDiagnosticKind,
        },
        document::{ParsedDocument, SymbolRange},
        syntax::member_name,
        workspace::{WorkspaceAccountField, WorkspaceIndex},
    },
    std::collections::{HashMap, HashSet},
    syn::visit::{self, Visit},
    tower_lsp::lsp_types::Diagnostic,
};

use super::{constraint_has_flag, has_constraint};

const DISTINCT_CONSTRAINT_KEY: &str = "constraint";
const CONSTRAINT_DELIMITER_LEN: usize = 1;

pub(super) fn duplicate_account_checks(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        DuplicateMutableAccountVisitor {
            document,
            workspace_index,
            diagnostics: Vec::new(),
        },
    )
}

struct DuplicateMutableAccountVisitor<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for DuplicateMutableAccountVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::AccountsStructField];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.account.duplicate-mutable";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for DuplicateMutableAccountVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if let Some(accounts) = self
            .document
            .symbols()
            .accounts_structs
            .get(&node.ident.to_string())
        {
            self.diagnostics
                .extend(duplicate_account_checks_for_accounts(
                    self.document,
                    self.workspace_index,
                    accounts,
                ));
        }
        visit::visit_item_struct(self, node);
    }
}

fn duplicate_account_checks_for_accounts(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
) -> Vec<Diagnostic> {
    let mut visited = HashSet::new();
    let candidates =
        duplicate_mutable_candidates(document, workspace_index, accounts, &mut visited);
    let mut by_data_type: HashMap<String, Vec<DuplicateMutableCandidate>> = HashMap::new();
    for candidate in candidates {
        by_data_type
            .entry(candidate.data_type.clone())
            .or_default()
            .push(candidate);
    }

    by_data_type
        .into_values()
        .filter(|fields| fields.len() > 1)
        .flat_map(|fields| {
            fields
                .iter()
                .filter_map(|candidate| {
                    let peer = unchecked_duplicate_mutable_peer(
                        document,
                        accounts,
                        &fields,
                        candidate,
                    )?;
                    let range = candidate.range?;
                    Some(diagnostic_from_range(
                        range,
                        AnchorDiagnosticKind::SecurityDuplicateAccount,
                        format!(
                            "`{}` in `{}` is a mutable `{}` account that may duplicate `{}` in `{}`; add `dup` when intentional or a key inequality check when they must be distinct.",
                            candidate.field_path,
                            candidate.accounts_name,
                            candidate.data_type,
                            peer.field_path,
                            peer.accounts_name,
                        ),
                        Some(serde_json::json!({
                            "account": candidate.field_name,
                            "accountPath": candidate.field_path,
                            "accountsStruct": candidate.accounts_name,
                            "peer": peer.field_name,
                            "peerPath": peer.field_path,
                            "peerAccountsStruct": peer.accounts_name,
                            "allowDistinctConstraint": !candidate.field_path.contains('.'),
                            "quickfix": "duplicate-account-remediation",
                            "reason": "anchor-v1-duplicate-mutable-account",
                        })),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[derive(Debug, Clone)]
struct DuplicateMutableCandidate {
    field_name: String,
    field_path: String,
    accounts_name: String,
    data_type: String,
    range: Option<tower_lsp::lsp_types::Range>,
}

fn duplicate_mutable_candidates(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    visited: &mut HashSet<String>,
) -> Vec<DuplicateMutableCandidate> {
    if !visited.insert(accounts.name.clone()) {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for field in &accounts.fields {
        if is_duplicate_mutable_candidate(field) {
            if let Some(data_type) = field.generic_type_names.last() {
                candidates.push(DuplicateMutableCandidate {
                    field_name: field.name.clone(),
                    field_path: field.name.clone(),
                    accounts_name: accounts.name.clone(),
                    data_type: data_type.clone(),
                    range: Some(duplicate_account_range(field)),
                });
            }
            continue;
        }

        if let Some(nested) = local_composite_accounts(document, field) {
            candidates.extend(
                duplicate_mutable_candidates(document, workspace_index, nested, visited)
                    .into_iter()
                    .map(|candidate| candidate.with_prefix(&field.name)),
            );
        } else if let Some(nested) = workspace_composite_accounts(workspace_index, field) {
            candidates.extend(
                workspace_duplicate_mutable_candidates(workspace_index, nested, visited)
                    .into_iter()
                    .map(|candidate| candidate.with_prefix(&field.name)),
            );
        }
    }

    visited.remove(accounts.name.as_str());
    candidates
}

impl DuplicateMutableCandidate {
    fn with_prefix(mut self, prefix: &str) -> Self {
        self.field_path = format!("{}.{}", prefix, self.field_path);
        self
    }
}

fn local_composite_accounts<'a>(
    document: &'a ParsedDocument,
    field: &SymbolRange,
) -> Option<&'a SymbolRange> {
    field
        .type_name
        .as_deref()
        .and_then(|name| document.symbols().accounts_structs.get(name))
}

fn workspace_composite_accounts<'a>(
    workspace_index: Option<&'a WorkspaceIndex>,
    field: &SymbolRange,
) -> Option<&'a crate::workspace::WorkspaceAccountsStruct> {
    let name = field.type_name.as_deref()?;
    workspace_index?.accounts_struct(name)
}

fn workspace_duplicate_mutable_candidates(
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &crate::workspace::WorkspaceAccountsStruct,
    visited: &mut HashSet<String>,
) -> Vec<DuplicateMutableCandidate> {
    if !visited.insert(accounts.name.clone()) {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for field in &accounts.fields {
        if is_workspace_duplicate_mutable_candidate(field) {
            if let Some(data_type) = field.generic_type_names.last() {
                candidates.push(DuplicateMutableCandidate {
                    field_name: field.name.clone(),
                    field_path: field.name.clone(),
                    accounts_name: accounts.name.clone(),
                    data_type: data_type.clone(),
                    range: None,
                });
            }
            continue;
        }

        if let Some(nested) = field
            .type_name
            .as_deref()
            .and_then(|name| workspace_index?.accounts_struct(name))
        {
            candidates.extend(
                workspace_duplicate_mutable_candidates(workspace_index, nested, visited)
                    .into_iter()
                    .map(|candidate| candidate.with_prefix(&field.name)),
            );
        }
    }

    visited.remove(accounts.name.as_str());
    candidates
}

fn is_duplicate_mutable_candidate(field: &SymbolRange) -> bool {
    is_serializing_account_type(field)
        && has_parser_effective_mut_constraint(field)
        && !has_constraint(field, |text| constraint_has_flag(text, "dup"))
        && !is_pure_init(field)
}

fn is_workspace_duplicate_mutable_candidate(field: &WorkspaceAccountField) -> bool {
    is_workspace_serializing_account_type(field)
        && workspace_has_parser_effective_mut_constraint(field)
        && !has_workspace_constraint(field, |text| constraint_has_flag(text, "dup"))
        && !is_workspace_pure_init(field)
}

fn is_workspace_serializing_account_type(field: &WorkspaceAccountField) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Account") | Some("LazyAccount") | Some("InterfaceAccount") | Some("Migration")
    )
}

fn workspace_has_parser_effective_mut_constraint(field: &WorkspaceAccountField) -> bool {
    has_workspace_constraint(field, |text| {
        constraint_has_flag(text, "mut")
            || constraint_has_flag(text, "init")
            || constraint_has_flag(text, "init_if_needed")
            || constraint_has_flag(text, "zero")
    })
}

fn is_workspace_pure_init(field: &WorkspaceAccountField) -> bool {
    has_workspace_constraint(field, |text| {
        constraint_has_flag(text, "init") && !constraint_has_flag(text, "init_if_needed")
    })
}

fn has_workspace_constraint(field: &WorkspaceAccountField, matches: impl Fn(&str) -> bool) -> bool {
    field
        .account_constraints
        .iter()
        .any(|constraint| matches(&constraint.text))
}

fn is_serializing_account_type(field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Account") | Some("LazyAccount") | Some("InterfaceAccount") | Some("Migration")
    )
}

fn has_parser_effective_mut_constraint(field: &SymbolRange) -> bool {
    // Source-driven: use the same mutability-implying keys that the rest of the
    // LSP derives from the generated catalog (instead of a local hardcoded list).
    let keys = crate::constraint_catalog::mutability_implying_keys();
    has_constraint(field, |text| {
        keys.iter().any(|k| constraint_has_flag(text, k))
    })
}

fn is_pure_init(field: &SymbolRange) -> bool {
    has_constraint(field, |text| {
        constraint_has_flag(text, "init") && !constraint_has_flag(text, "init_if_needed")
    })
}

fn duplicate_account_range(field: &SymbolRange) -> tower_lsp::lsp_types::Range {
    field
        .generic_type_ranges
        .last()
        .map(|generic| generic.range)
        .unwrap_or(field.selection_range)
}

fn unchecked_duplicate_mutable_peer<'a>(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    fields: &'a [DuplicateMutableCandidate],
    candidate: &DuplicateMutableCandidate,
) -> Option<&'a DuplicateMutableCandidate> {
    fields.iter().find(|peer| {
        peer.field_path != candidate.field_path
            && !has_distinct_key_check(document, accounts, &candidate.field_path, &peer.field_path)
    })
}

fn has_distinct_key_check(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    left: &str,
    right: &str,
) -> bool {
    if has_pairwise_distinct_constraint(accounts, left, right) {
        return true;
    }
    document.symbols().callable_functions().any(|instruction| {
        instruction
            .context
            .as_ref()
            .is_some_and(|context| context.name == accounts.name)
            && instruction
                .account_key_comparisons
                .iter()
                .any(|comparison| comparison.matches(left, right))
    })
}

fn has_pairwise_distinct_constraint(accounts: &SymbolRange, left: &str, right: &str) -> bool {
    accounts
        .fields
        .iter()
        .flat_map(|field| field.account_constraints.iter())
        .flat_map(|constraint| distinct_constraint_expressions(&constraint.text))
        .any(|expr| is_distinct_key_check(&expr, left, right))
}

fn distinct_constraint_expressions(text: &str) -> Vec<syn::Expr> {
    let mut remaining = text;
    let mut expressions = Vec::new();

    while let Some(value) =
        crate::constraint_text::value_after_key(remaining, DISTINCT_CONSTRAINT_KEY)
    {
        let expr_text = leading_constraint_expression(value);
        if expr_text.is_empty() {
            break;
        }
        if let Ok(expr) = syn::parse_str::<syn::Expr>(expr_text) {
            expressions.push(expr);
        }

        let value_start = remaining.len() - value.len();
        let consumed = value_start + expr_text.len();
        remaining = remaining
            .get(consumed.saturating_add(CONSTRAINT_DELIMITER_LEN)..)
            .unwrap_or_default();
    }

    expressions
}

fn leading_constraint_expression(value: &str) -> &str {
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;
    let mut delimiter = value.len();

    for (index, ch) in value.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            '[' => bracket_depth += 1,
            '{' => brace_depth += 1,
            ')' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                delimiter = index;
                break;
            }
            ']' if bracket_depth == 0 && paren_depth == 0 && brace_depth == 0 => {
                delimiter = index;
                break;
            }
            '}' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                delimiter = index;
                break;
            }
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                delimiter = index;
                break;
            }
            _ => {}
        }
    }

    value[..delimiter].trim()
}

fn is_distinct_key_check(expr: &syn::Expr, left: &str, right: &str) -> bool {
    match expr {
        syn::Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_)) => {
            is_distinct_key_check(&binary.left, left, right)
                || is_distinct_key_check(&binary.right, left, right)
        }
        syn::Expr::Binary(binary) if matches!(binary.op, syn::BinOp::Ne(_)) => {
            let Some(left_path) = key_call_account_path(&binary.left) else {
                return false;
            };
            let Some(right_path) = key_call_account_path(&binary.right) else {
                return false;
            };
            account_paths_match(&left_path, &right_path, left, right)
        }
        syn::Expr::Group(group) => is_distinct_key_check(&group.expr, left, right),
        syn::Expr::Paren(paren) => is_distinct_key_check(&paren.expr, left, right),
        _ => false,
    }
}

fn key_call_account_path(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method_call)
            if method_call.method == "key" && method_call.args.is_empty() =>
        {
            account_path(&method_call.receiver)
        }
        syn::Expr::Group(group) => key_call_account_path(&group.expr),
        syn::Expr::Paren(paren) => key_call_account_path(&paren.expr),
        syn::Expr::Reference(reference) => key_call_account_path(&reference.expr),
        _ => None,
    }
}

fn account_path(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => {
            let mut base = account_path(&field.base)?;
            base.push('.');
            base.push_str(&member_name(&field.member)?);
            Some(base)
        }
        syn::Expr::Group(group) => account_path(&group.expr),
        syn::Expr::Paren(paren) => account_path(&paren.expr),
        syn::Expr::Reference(reference) => account_path(&reference.expr),
        syn::Expr::Unary(unary) => account_path(&unary.expr),
        _ => None,
    }
}

fn account_paths_match(
    left_path: &str,
    right_path: &str,
    expected_left: &str,
    expected_right: &str,
) -> bool {
    (left_path == expected_left && right_path == expected_right)
        || (left_path == expected_right && right_path == expected_left)
}
