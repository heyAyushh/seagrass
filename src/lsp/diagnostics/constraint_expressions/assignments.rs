use {
    super::{bump, expression_before_custom_error, expression_issues, ConstraintExpressionIssue},
    crate::{
        constraint_catalog::{self, ConstraintValueKind},
        constraint_text,
        document::ParsedDocument,
        evidence::{AccountSetEvidence, ConstraintEvidence},
        workspace::WorkspaceIndex,
    },
    syn::{
        visit::{self, Visit},
        Expr, ExprPath,
    },
};

pub(super) fn assignment_value_issues(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<ConstraintExpressionIssue> {
    constraint_text::top_level_assignments(constraint.text())
        .into_iter()
        .filter(|assignment| should_validate_assignment_value(assignment.key, assignment.value))
        .flat_map(|assignment| {
            expression_issues(
                document,
                workspace_index,
                accounts,
                assignment.key,
                assignment.value,
            )
        })
        .collect()
}

fn should_validate_assignment_value(key: &str, value: &str) -> bool {
    if key == bump::EXPLICIT_BUMP_KEY {
        return false;
    }
    match constraint_catalog::value_kind_for_key(key) {
        Some(ConstraintValueKind::Space) => true,
        Some(ConstraintValueKind::ProgramReference) => expression_contains_path_namespace(value),
        None => true,
        Some(
            ConstraintValueKind::None
            | ConstraintValueKind::AnyExpression
            | ConstraintValueKind::AccountReference
            | ConstraintValueKind::SignerReference
            | ConstraintValueKind::InstructionArgument
            | ConstraintValueKind::Keyword
            | ConstraintValueKind::Boolean
            | ConstraintValueKind::Seeds,
        ) => false,
    }
}

fn expression_contains_path_namespace(value: &str) -> bool {
    let expression = expression_before_custom_error(value);
    let Ok(expr) = syn::parse_str::<Expr>(expression) else {
        return false;
    };
    let mut visitor = NamespacedPathVisitor::default();
    visitor.visit_expr(&expr);
    visitor.found
}

#[derive(Default)]
struct NamespacedPathVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for NamespacedPathVisitor {
    fn visit_expr_path(&mut self, path: &'ast ExprPath) {
        if path.qself.is_none() && path.path.segments.len() > 1 {
            self.found = true;
            return;
        }
        visit::visit_expr_path(self, path);
    }
}
