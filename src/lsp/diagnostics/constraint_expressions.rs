use {
    crate::{
        account_members::{self, AccountMemberAccess},
        constraint_catalog,
        diagnostics::{
            diagnostic_from_range, registry::AnchorDiagnosticKind,
            REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX,
        },
        document::ParsedDocument,
        evidence::{
            AccountSetEvidence, ConstraintEvidence, EvidenceGraph, SeedExpressionEvidence,
            SeedExpressionKind,
        },
        range::line_at,
        workspace::WorkspaceIndex,
    },
    syn::{
        visit::{self, Visit},
        Expr, ExprField, ExprMethodCall, ExprPath, Member,
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

const CUSTOM_ERROR_SEPARATOR: char = '@';
const ACCOUNT_OWNER_DEREF_ACCESS: &str = ".to_account_info().owner";

#[path = "constraint_expressions/assignments.rs"]
mod assignments;
#[path = "constraint_expressions/bump.rs"]
mod bump;
#[path = "constraint_expressions/resolution.rs"]
mod resolution;

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
        .flat_map(|accounts| {
            accounts.fields().iter().flat_map(|field| {
                field.constraints().iter().flat_map(|constraint| {
                    constraint_expression_diagnostics(
                        document,
                        workspace_index,
                        accounts,
                        constraint,
                    )
                })
            })
        })
        .collect()
}

fn constraint_expression_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut issues = constraint_catalog::expression_value_keys()
        .flat_map(|key| {
            constraint
                .values_after_key(key)
                .into_iter()
                .flat_map(move |expression| {
                    expression_issues(document, workspace_index, accounts, key, expression)
                })
        })
        .collect::<Vec<_>>();

    issues.extend(
        constraint
            .seed_expressions(accounts)
            .iter()
            .flat_map(|seed| seed_expression_issues(document, workspace_index, accounts, seed)),
    );
    issues.extend(
        constraint
            .values_after_key(bump::EXPLICIT_BUMP_KEY)
            .into_iter()
            .flat_map(|expression| {
                bump::expression_issues(document, workspace_index, accounts, expression)
            }),
    );
    issues.extend(assignments::assignment_value_issues(
        document,
        workspace_index,
        accounts,
        constraint,
    ));

    issues
        .into_iter()
        .map(|issue| issue.to_diagnostic(document, accounts, constraint))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConstraintExpressionIssue {
    UnresolvedIdentifier {
        constraint_key: String,
        identifier: String,
    },
    UnknownMember {
        constraint_key: String,
        receiver: String,
        member: String,
        owner_type: String,
        candidates: Vec<String>,
    },
    UnexpectedType {
        constraint_key: String,
        expression: String,
        actual_type: String,
        expected_type: &'static str,
    },
}

impl ConstraintExpressionIssue {
    fn to_diagnostic(
        &self,
        document: &ParsedDocument,
        accounts: &AccountSetEvidence<'_>,
        constraint: &ConstraintEvidence<'_>,
    ) -> Diagnostic {
        match self {
            Self::UnresolvedIdentifier {
                constraint_key,
                identifier,
            } => diagnostic_from_range(
                expression_token_range(document, constraint, identifier)
                    .unwrap_or_else(|| value_range(document, constraint, constraint_key, identifier)),
                AnchorDiagnosticKind::AnchorConstraintExpression,
                format!(
                    "`{identifier}` does not resolve to a field, instruction arg, or const in scope."
                ),
                Some(serde_json::json!({
                    "constraint": constraint_key,
                    "identifier": identifier,
                    "accountsStruct": accounts.accounts.name,
                })),
            ),
            Self::UnknownMember {
                constraint_key,
                receiver,
                member,
                owner_type,
                candidates,
            } => diagnostic_from_range(
                expression_token_range(document, constraint, member)
                    .unwrap_or_else(|| value_range(document, constraint, constraint_key, member)),
                AnchorDiagnosticKind::AnchorConstraintExpression,
                format!(
                    "`{receiver}.{member}` does not resolve; `{owner_type}` has no field `{member}`."
                ),
                Some(serde_json::json!({
                    "constraint": constraint_key,
                    "account": receiver,
                    "field": member,
                    "accountsStruct": accounts.accounts.name,
                    "ownerType": owner_type,
                    "candidates": candidates,
                    "quickfix": REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX,
                })),
            ),
            Self::UnexpectedType {
                constraint_key,
                expression,
                actual_type,
                expected_type,
            } => diagnostic_from_range(
                expression_token_range(document, constraint, expression)
                    .unwrap_or_else(|| value_range(document, constraint, constraint_key, expression)),
                AnchorDiagnosticKind::AnchorConstraintExpression,
                format!(
                    "`{expression}` resolves to `{actual_type}`, but `{constraint_key}` requires `{expected_type}`."
                ),
                Some(serde_json::json!({
                    "constraint": constraint_key,
                    "expression": expression,
                    "actualType": actual_type,
                    "expectedType": expected_type,
                    "accountsStruct": accounts.accounts.name,
                })),
            ),
        }
    }
}

fn expression_issues(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    constraint_key: &str,
    expression: &str,
) -> Vec<ConstraintExpressionIssue> {
    let expression = expression_before_custom_error(expression);
    if is_external_account_owner_deref(expression) {
        return Vec::new();
    }
    let Ok(expr) = syn::parse_str::<Expr>(expression) else {
        return Vec::new();
    };
    let mut visitor = ConstraintExpressionVisitor {
        document,
        workspace_index,
        accounts,
        constraint_key: constraint_key.to_string(),
        issues: Vec::new(),
    };
    visitor.visit_expr(&expr);
    visitor.issues
}

fn is_external_account_owner_deref(expression: &str) -> bool {
    let expression = expression.trim();
    expression.starts_with('*') && expression.contains(ACCOUNT_OWNER_DEREF_ACCESS)
}

fn seed_expression_issues(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    seed: &SeedExpressionEvidence,
) -> Vec<ConstraintExpressionIssue> {
    if !matches!(
        seed.kind,
        SeedExpressionKind::Expression | SeedExpressionKind::AccountKey
    ) {
        return Vec::new();
    }
    let Ok(expr) = syn::parse_str::<Expr>(&seed.expression) else {
        return Vec::new();
    };

    let mut visitor = ConstraintExpressionVisitor {
        document,
        workspace_index,
        accounts,
        constraint_key: "seeds".to_string(),
        issues: Vec::new(),
    };
    visitor.visit_expr(&expr);
    visitor.issues
}

struct ConstraintExpressionVisitor<'a, 'b> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    accounts: &'a AccountSetEvidence<'b>,
    constraint_key: String,
    issues: Vec<ConstraintExpressionIssue>,
}

impl<'ast> Visit<'ast> for ConstraintExpressionVisitor<'_, '_> {
    fn visit_expr_path(&mut self, path: &'ast ExprPath) {
        if let Some(identifier) = resolution::unresolved_path_identifier(
            self.document,
            self.workspace_index,
            self.accounts,
            path,
        ) {
            if !self.has_issue_for_identifier(&identifier) {
                self.issues
                    .push(ConstraintExpressionIssue::UnresolvedIdentifier {
                        constraint_key: self.constraint_key.clone(),
                        identifier,
                    });
            }
        }
        visit::visit_expr_path(self, path);
    }

    fn visit_expr_field(&mut self, field: &'ast ExprField) {
        if let Some(access) = named_field_access(field) {
            self.validate_member(&access);
        }
        visit::visit_expr_field(self, field);
    }
}

impl ConstraintExpressionVisitor<'_, '_> {
    fn validate_member(&mut self, access: &MemberAccess) {
        let Some(field) = self
            .accounts
            .accounts
            .fields
            .iter()
            .find(|field| field.name == access.receiver)
        else {
            return;
        };
        let Some(missing) = account_members::missing_member_in_chain(
            self.document,
            self.workspace_index,
            self.accounts.accounts,
            field,
            access.access,
            &access.receiver,
            &access.member_chain,
        ) else {
            return;
        };
        if !self.has_issue_for_member(&missing.receiver_path, &missing.member) {
            self.issues.push(ConstraintExpressionIssue::UnknownMember {
                constraint_key: self.constraint_key.clone(),
                receiver: missing.receiver_path,
                member: missing.member,
                owner_type: missing.owner_type,
                candidates: missing.candidates,
            });
        }
    }

    fn has_issue_for_identifier(&self, identifier: &str) -> bool {
        self.issues.iter().any(|issue| match issue {
            ConstraintExpressionIssue::UnresolvedIdentifier {
                constraint_key: _,
                identifier: existing,
            } => existing == identifier,
            ConstraintExpressionIssue::UnknownMember { .. }
            | ConstraintExpressionIssue::UnexpectedType { .. } => false,
        })
    }

    fn has_issue_for_member(&self, receiver: &str, member: &str) -> bool {
        self.issues.iter().any(|issue| match issue {
            ConstraintExpressionIssue::UnknownMember {
                constraint_key: _,
                receiver: existing_receiver,
                member: existing_member,
                owner_type: _,
                candidates: _,
            } => existing_receiver == receiver && existing_member == member,
            ConstraintExpressionIssue::UnresolvedIdentifier { .. }
            | ConstraintExpressionIssue::UnexpectedType { .. } => false,
        })
    }

    fn has_resolution_issue(&self) -> bool {
        self.issues.iter().any(|issue| {
            matches!(
                issue,
                ConstraintExpressionIssue::UnresolvedIdentifier { .. }
                    | ConstraintExpressionIssue::UnknownMember { .. }
            )
        })
    }
}

struct MemberAccess {
    receiver: String,
    member_chain: Vec<String>,
    access: AccountMemberAccess,
}

fn named_field_access(field: &ExprField) -> Option<MemberAccess> {
    let Member::Named(member) = &field.member else {
        return None;
    };
    if let Some(mut access) = field_access_base(field.base.as_ref()) {
        access.member_chain.push(member.to_string());
        return Some(access);
    }
    None
}

fn field_access_base(expr: &Expr) -> Option<MemberAccess> {
    direct_field_receiver(expr)
        .map(|receiver| MemberAccess {
            receiver,
            member_chain: Vec::new(),
            access: AccountMemberAccess::Direct,
        })
        .or_else(|| {
            loaded_field_receiver(expr).map(|receiver| MemberAccess {
                receiver,
                member_chain: Vec::new(),
                access: AccountMemberAccess::Loaded,
            })
        })
        .or_else(|| nested_field_access_base(expr))
}

fn nested_field_access_base(expr: &Expr) -> Option<MemberAccess> {
    match expr {
        Expr::Field(field) => named_field_access(field),
        Expr::Paren(paren) => field_access_base(&paren.expr),
        Expr::Group(group) => field_access_base(&group.expr),
        Expr::Reference(reference) => field_access_base(&reference.expr),
        _ => None,
    }
}

fn direct_field_receiver(expr: &Expr) -> Option<String> {
    let Expr::Path(base) = expr else {
        return None;
    };
    simple_path_identifier(base)
}

fn loaded_field_receiver(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Try(expr_try) => account_loader_method_receiver(&expr_try.expr),
        Expr::Paren(paren) => loaded_field_receiver(&paren.expr),
        Expr::Group(group) => loaded_field_receiver(&group.expr),
        Expr::Reference(reference) => loaded_field_receiver(&reference.expr),
        _ => None,
    }
}

fn account_loader_method_receiver(expr: &Expr) -> Option<String> {
    let Expr::MethodCall(method_call) = expr else {
        return None;
    };
    if !is_account_loader_loaded_call(method_call) {
        return None;
    }
    direct_field_receiver(&method_call.receiver)
}

fn is_account_loader_loaded_call(method_call: &ExprMethodCall) -> bool {
    account_members::is_account_loader_loaded_method(&method_call.method.to_string())
        && method_call.args.is_empty()
}

fn simple_path_identifier(path: &ExprPath) -> Option<String> {
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    path.path
        .segments
        .first()
        .map(|segment| segment.ident.to_string())
}

fn expression_before_custom_error(expression: &str) -> &str {
    split_once_top_level(expression, CUSTOM_ERROR_SEPARATOR)
        .map_or(expression, |(before, _)| before)
        .trim()
}

fn split_once_top_level(input: &str, separator: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for (index, ch) in input.char_indices() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ch if ch == separator && depth == 0 => {
                return Some((&input[..index], &input[index + ch.len_utf8()..]));
            }
            _ => {}
        }
    }

    None
}

fn value_range(
    document: &ParsedDocument,
    constraint: &ConstraintEvidence<'_>,
    constraint_key: &str,
    identifier: &str,
) -> Range {
    constraint
        .value_range(document.source(), constraint_key, identifier)
        .unwrap_or(constraint.range())
}

fn expression_token_range(
    document: &ParsedDocument,
    constraint: &ConstraintEvidence<'_>,
    token: &str,
) -> Option<Range> {
    (constraint.range().start.line..=constraint.range().end.line).find_map(|line_number| {
        let line = line_at(document.source(), line_number)?;
        token_range_on_line(line, line_number, token)
    })
}

fn token_range_on_line(line: &str, line_number: u32, token: &str) -> Option<Range> {
    let mut search_start = 0usize;
    while let Some(relative) = line[search_start..].find(token) {
        let start = search_start + relative;
        let end = start + token.len();
        if token_has_identifier_boundaries(line, start, end) {
            return Some(Range {
                start: tower_lsp::lsp_types::Position {
                    line: line_number,
                    character: u32::try_from(line[..start].chars().count()).ok()?,
                },
                end: tower_lsp::lsp_types::Position {
                    line: line_number,
                    character: u32::try_from(line[..end].chars().count()).ok()?,
                },
            });
        }
        search_start = end;
    }
    None
}

fn token_has_identifier_boundaries(line: &str, start: usize, end: usize) -> bool {
    let previous = line[..start].chars().next_back();
    let next = line[end..].chars().next();
    previous.is_none_or(|ch| !is_identifier_char(ch))
        && next.is_none_or(|ch| !is_identifier_char(ch))
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
#[path = "constraint_expressions/expression_value_tests.rs"]
mod expression_value_tests;

#[cfg(test)]
mod tests {
    use {super::*, tower_lsp::lsp_types::NumberOrString};

    #[test]
    fn reports_unresolved_bare_constraint_identifier_on_value_span() {
        let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, amount: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = amount > 0,
        constraint = sd,
    )]
    pub state: Account<'info, State>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("`sd` does not resolve"))
            .expect("unresolved identifier diagnostic");

        assert!(matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-constraint-expression"
        ));
        assert_eq!(diagnostic.range.start.line, 10);
        assert_eq!(diagnostic.range.start.character, 21);
    }

    #[test]
    fn reports_unknown_builtin_member_on_value_span() {
        let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = vault.decimals == 6)]
    pub state: Account<'info, State>,
    pub vault: Account<'info, TokenAccount>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .message
                    .contains("`vault.decimals` does not resolve")
            })
            .expect("unknown TokenAccount member diagnostic");

        assert!(matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-constraint-expression"
        ));
        assert_eq!(diagnostic.range.start.line, 3);
        assert_eq!(diagnostic.range.start.character, 33);
    }

    #[test]
    fn validates_local_account_data_and_composite_members() {
        let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = position.position_mint == mint.key(),
        constraint = position.missing == mint.key(),
        constraint = bundle.inner_mint.key() == mint.key(),
        constraint = bundle.missing.key() == mint.key(),
    )]
    pub state: Account<'info, State>,
    pub position: Account<'info, Position>,
    pub bundle: Bundle<'info>,
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct Bundle<'info> {
    pub inner_mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`position.missing`")));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`bundle.missing`")));
        assert!(!diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("position_mint` does not resolve")));
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("inner_mint` does not resolve")));
    }

    #[test]
    fn validates_account_loader_fields_after_load() {
        let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = position.load()?.position_mint == mint.key(),
        constraint = position.load_mut()?.bump == 1,
        constraint = position.load()?.missing == mint.key(),
    )]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
    pub mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
    pub bump: u8,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`position.missing`")));
        assert!(!diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("position_mint` does not resolve")));
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("bump` does not resolve")));
    }

    #[test]
    fn rejects_direct_account_loader_data_field_access() {
        let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = position.position_mint == mint.key())]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
    pub mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`position.position_mint` does not resolve")
                && diagnostic.message.contains("AccountLoader<Position>")
        }));
    }

    #[test]
    fn validates_nested_account_loader_field_chains() {
        let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        address = state.load()?.config.owner,
        constraint = state.load()?.config.missing == authority.key(),
    )]
    pub authority: Signer<'info>,
    pub state: AccountLoader<'info, State>,
}

#[account]
pub struct State {
    pub config: StateConfig,
}

pub struct StateConfig {
    pub owner: Pubkey,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`state.config.missing`")));
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("owner` does not resolve")));
    }

    #[test]
    fn validates_nested_direct_account_data_field_chains() {
        let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = state.config.missing == authority.key())]
    pub authority: Signer<'info>,
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub config: StateConfig,
}

pub struct StateConfig {
    pub owner: Pubkey,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`state.config.missing`")));
    }
}
