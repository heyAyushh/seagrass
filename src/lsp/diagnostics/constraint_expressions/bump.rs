use {
    super::{
        expression_before_custom_error, named_field_access, ConstraintExpressionIssue,
        ConstraintExpressionVisitor, MemberAccess,
    },
    crate::{
        account_members, document::ParsedDocument, evidence::AccountSetEvidence,
        workspace::WorkspaceIndex,
    },
    syn::{visit::Visit, Expr},
};

pub(super) const EXPLICIT_BUMP_KEY: &str = "bump";
const EXPECTED_BUMP_TYPE: &str = "u8";

pub(super) fn expression_issues(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    expression: &str,
) -> Vec<ConstraintExpressionIssue> {
    let expression = expression_before_custom_error(expression);
    let Ok(expr) = syn::parse_str::<Expr>(expression) else {
        return Vec::new();
    };

    let mut visitor = ConstraintExpressionVisitor {
        document,
        workspace_index,
        accounts,
        constraint_key: EXPLICIT_BUMP_KEY.to_string(),
        issues: Vec::new(),
    };
    visitor.visit_expr(&expr);
    if visitor.has_resolution_issue() {
        return visitor.issues;
    }

    if let Some(access) = expression_member_access(&expr) {
        if let Some(actual_type) =
            resolved_member_type(document, workspace_index, accounts, &access)
        {
            if actual_type != EXPECTED_BUMP_TYPE {
                visitor
                    .issues
                    .push(ConstraintExpressionIssue::UnexpectedType {
                        constraint_key: EXPLICIT_BUMP_KEY.to_string(),
                        expression: member_access_path(&access),
                        actual_type,
                        expected_type: EXPECTED_BUMP_TYPE,
                    });
            }
        }
    }

    visitor.issues
}

fn expression_member_access(expr: &Expr) -> Option<MemberAccess> {
    match expr {
        Expr::Field(field) => named_field_access(field),
        Expr::Paren(paren) => expression_member_access(&paren.expr),
        Expr::Group(group) => expression_member_access(&group.expr),
        Expr::Reference(reference) => expression_member_access(&reference.expr),
        _ => None,
    }
}

fn resolved_member_type(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    access: &MemberAccess,
) -> Option<String> {
    let (member, parent_chain) = access.member_chain.split_last()?;
    let field = accounts
        .accounts
        .fields
        .iter()
        .find(|field| field.name == access.receiver)?;
    let parent_members = account_members::resolved_field_chain_members(
        document,
        workspace_index,
        accounts.accounts,
        field,
        access.access,
        parent_chain,
    )?;
    parent_members
        .members
        .iter()
        .find(|candidate| candidate.name == *member)?
        .type_name
        .clone()
}

fn member_access_path(access: &MemberAccess) -> String {
    std::iter::once(access.receiver.as_str())
        .chain(access.member_chain.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(".")
}
