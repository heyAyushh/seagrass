use {
    super::{type_names, wrapper_constructor_value_type_name, wrapper_value_type_name_with_scope},
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::Expr,
};

type TypeLookup<'a> = dyn Fn(&str) -> Option<String> + 'a;

struct WrapperBranchValue {
    kind: type_names::ValueWrapperKind,
    type_name: Option<String>,
}

pub(super) fn if_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr_if: &syn::ExprIf,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<super::WrapperValue> {
    let then_branch = block_tail_expr(&expr_if.then_branch)?;
    let (_, else_branch) = expr_if.else_branch.as_ref()?;
    same_wrapper_branch_value_type_name([
        branch_value_type_name(
            document,
            workspace_index,
            then_branch,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        branch_value_type_name(
            document,
            workspace_index,
            else_branch,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
    ])
}

pub(super) fn match_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr_match: &syn::ExprMatch,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<super::WrapperValue> {
    same_wrapper_branch_value_type_name(expr_match.arms.iter().map(|arm| {
        branch_value_type_name(
            document,
            workspace_index,
            &arm.body,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
    }))
}

fn branch_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperBranchValue> {
    match expr {
        Expr::Call(call) => wrapper_constructor_value_type_name(
            document,
            workspace_index,
            call,
            None,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(WrapperBranchValue::from)
        .or_else(|| non_value_call_branch(call)),
        Expr::Path(path) => type_names::wrapper_non_value_path_kind(path).map(non_value_branch),
        Expr::If(expr_if) => if_value_type_name(
            document,
            workspace_index,
            expr_if,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(WrapperBranchValue::from),
        Expr::Match(expr_match) => match_value_type_name(
            document,
            workspace_index,
            expr_match,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(WrapperBranchValue::from),
        Expr::Block(block) => block_tail_expr(&block.block).and_then(|expr| {
            branch_value_type_name(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::Reference(reference) => branch_value_type_name(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => branch_value_type_name(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => branch_value_type_name(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => wrapper_value_type_name_with_scope(
            document,
            workspace_index,
            expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(WrapperBranchValue::from),
    }
}

fn non_value_call_branch(call: &syn::ExprCall) -> Option<WrapperBranchValue> {
    (call.args.len() == 1)
        .then(|| type_names::wrapper_non_value_constructor_kind(call))
        .flatten()
        .map(non_value_branch)
}

fn non_value_branch(kind: type_names::ValueWrapperKind) -> WrapperBranchValue {
    WrapperBranchValue {
        kind,
        type_name: None,
    }
}

impl From<super::WrapperValue> for WrapperBranchValue {
    fn from(value: super::WrapperValue) -> Self {
        Self {
            kind: value.kind,
            type_name: Some(value.type_name),
        }
    }
}

fn same_wrapper_branch_value_type_name(
    branches: impl IntoIterator<Item = Option<WrapperBranchValue>>,
) -> Option<super::WrapperValue> {
    let mut branches = branches.into_iter();
    let first = branches.next()??;
    let kind = first.kind;
    let mut type_name = first.type_name;

    for branch in branches {
        let branch = branch?;
        if branch.kind != kind {
            return None;
        }
        let Some(branch_type_name) = branch.type_name else {
            continue;
        };
        if type_name
            .as_ref()
            .is_some_and(|type_name| type_name != &branch_type_name)
        {
            return None;
        }
        type_name = Some(branch_type_name);
    }

    Some(super::WrapperValue {
        kind,
        type_name: type_name?,
    })
}

pub(super) fn block_tail_expr(block: &syn::Block) -> Option<&Expr> {
    let syn::Stmt::Expr(expr, None) = block.stmts.last()? else {
        return None;
    };
    Some(expr)
}
