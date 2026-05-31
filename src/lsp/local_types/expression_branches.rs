use {
    super::{
        expression_optional_item_type_name_with_item_scope, expression_type_name_with_item_scope,
        method_returns, typed_pattern_bindings_with_wrapped_item, TypedLocalValue,
    },
    crate::{account_members, document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{BinOp, Expr, Member, Stmt},
};

pub(super) fn block_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    block: &syn::Block,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let Stmt::Expr(expr, None) = block.stmts.last()? else {
        return None;
    };
    expression_type_name_with_item_scope(
        document,
        workspace_index,
        expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

pub(super) fn if_expression_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr_if: &syn::ExprIf,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let then_values = condition_typed_values(
        document,
        workspace_index,
        &expr_if.cond,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    let then_type = scoped_block_type_name(
        document,
        workspace_index,
        &expr_if.then_branch,
        &then_values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    let (_, else_branch) = expr_if.else_branch.as_ref()?;
    let else_type = expression_type_name_with_item_scope(
        document,
        workspace_index,
        else_branch,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    (then_type == else_type).then_some(then_type)
}

pub(super) fn match_expression_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr_match: &syn::ExprMatch,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let scrutinee_type = expression_type_name_with_item_scope(
        document,
        workspace_index,
        &expr_match.expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    let wrapped_item_type = expression_optional_item_type_name_with_item_scope(
        document,
        workspace_index,
        &expr_match.expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    same_type_name(expr_match.arms.iter().map(|arm| {
        let arm_values = arm_typed_values(
            document,
            workspace_index,
            &arm.pat,
            scrutinee_type.as_deref(),
            wrapped_item_type.as_deref(),
        );
        scoped_expression_type_name(
            document,
            workspace_index,
            &arm.body,
            &arm_values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
    }))
}

fn condition_typed_values(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    condition: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Vec<TypedLocalValue> {
    match condition {
        Expr::Let(expr_let) => {
            let type_name = expression_type_name_with_item_scope(
                document,
                workspace_index,
                &expr_let.expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            );
            let wrapped_item_type = expression_optional_item_type_name_with_item_scope(
                document,
                workspace_index,
                &expr_let.expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            );
            arm_typed_values(
                document,
                workspace_index,
                &expr_let.pat,
                type_name.as_deref(),
                wrapped_item_type.as_deref(),
            )
        }
        Expr::Binary(binary) if matches!(binary.op, BinOp::And(_)) => {
            let mut values = condition_typed_values(
                document,
                workspace_index,
                &binary.left,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            );
            values.extend(condition_typed_values(
                document,
                workspace_index,
                &binary.right,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            ));
            values
        }
        Expr::Group(group) => condition_typed_values(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => condition_typed_values(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => Vec::new(),
    }
}

fn arm_typed_values(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat: &syn::Pat,
    type_name: Option<&str>,
    wrapped_item_type_name: Option<&str>,
) -> Vec<TypedLocalValue> {
    let Some(type_name) = type_name else {
        return Vec::new();
    };
    typed_pattern_bindings_with_wrapped_item(
        document,
        workspace_index,
        pat,
        type_name,
        wrapped_item_type_name,
    )
}

fn scoped_block_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    block: &syn::Block,
    values: &[TypedLocalValue],
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let Stmt::Expr(expr, None) = block.stmts.last()? else {
        return None;
    };
    scoped_expression_type_name(
        document,
        workspace_index,
        expr,
        values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

fn scoped_expression_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    values: &[TypedLocalValue],
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            scoped_value_type_name(values, &path.path.segments[0].ident.to_string()).or_else(|| {
                fallback_expression_type_name(
                    document,
                    workspace_index,
                    expr,
                    scope_type_name,
                    context_type_name,
                    scope_item_type_name,
                )
            })
        }
        Expr::Field(field) => scoped_field_type_name(
            document,
            workspace_index,
            field,
            values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .or_else(|| {
            fallback_expression_type_name(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::MethodCall(method_call) => scoped_method_return_type_name(
            document,
            workspace_index,
            method_call,
            values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .or_else(|| {
            fallback_expression_type_name(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::Block(block) => scoped_block_type_name(
            document,
            workspace_index,
            &block.block,
            values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Reference(reference) => scoped_expression_type_name(
            document,
            workspace_index,
            &reference.expr,
            values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => scoped_expression_type_name(
            document,
            workspace_index,
            &paren.expr,
            values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => scoped_expression_type_name(
            document,
            workspace_index,
            &group.expr,
            values,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => fallback_expression_type_name(
            document,
            workspace_index,
            expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
    }
}

fn scoped_field_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    field: &syn::ExprField,
    values: &[TypedLocalValue],
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let Member::Named(member) = &field.member else {
        return None;
    };
    let receiver_type = scoped_expression_type_name(
        document,
        workspace_index,
        &field.base,
        values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    account_members::resolved_struct_member_type_name(
        document,
        workspace_index,
        &receiver_type,
        &member.to_string(),
    )
}

fn scoped_method_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &syn::ExprMethodCall,
    values: &[TypedLocalValue],
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let receiver_type = scoped_expression_type_name(
        document,
        workspace_index,
        &method_call.receiver,
        values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    method_returns::method_return_type_name_for_receiver_type(
        document,
        workspace_index,
        &receiver_type,
        &method_call.method.to_string(),
    )
}

fn scoped_value_type_name(values: &[TypedLocalValue], name: &str) -> Option<String> {
    values
        .iter()
        .rev()
        .find(|value| value.name == name)
        .map(|value| value.type_name.clone())
}

fn fallback_expression_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    expression_type_name_with_item_scope(
        document,
        workspace_index,
        expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

fn same_type_name(types: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let mut iter = types.into_iter();
    let first = iter.next()??;
    iter.all(|type_name| type_name.as_deref() == Some(first.as_str()))
        .then_some(first)
}
