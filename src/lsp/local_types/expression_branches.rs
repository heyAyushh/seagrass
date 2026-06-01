use {
    super::{
        explicit_pattern_type_name, expression_optional_item_type_name_with_item_scope,
        expression_type_name_with_item_scope, method_returns,
        typed_pattern_bindings_with_wrapped_item, wrapper_pattern_type_name, TypedLocalValue,
    },
    crate::{account_members, document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{BinOp, Expr, Member, Stmt},
};

const DIVERGING_MACROS: &[&str] = &["panic", "todo", "unimplemented", "unreachable"];

#[derive(Debug, Clone, PartialEq, Eq)]
enum BranchOutcome {
    Value(String),
    Diverges,
    Unknown,
}

pub(super) fn block_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    block: &syn::Block,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    scoped_block_type_name(
        document,
        workspace_index,
        block,
        &[],
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
    let then_type = scoped_block_branch_outcome(
        document,
        workspace_index,
        &expr_if.then_branch,
        &then_values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    let (_, else_branch) = expr_if.else_branch.as_ref()?;
    let else_type = expression_branch_outcome(
        document,
        workspace_index,
        else_branch,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    same_branch_type_name([then_type, else_type])
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
    same_branch_type_name(expr_match.arms.iter().map(|arm| {
        let arm_values = arm_typed_values(
            document,
            workspace_index,
            &arm.pat,
            scrutinee_type.as_deref(),
            wrapped_item_type.as_deref(),
        );
        scoped_expression_branch_outcome(
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

fn expression_branch_outcome(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> BranchOutcome {
    expression_type_name_with_item_scope(
        document,
        workspace_index,
        expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
    .map(BranchOutcome::Value)
    .unwrap_or_else(|| diverging_expression_outcome(expr))
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
    let (tail, leading_statements) = block.stmts.split_last()?;
    let Stmt::Expr(expr, None) = tail else {
        return None;
    };
    let mut block_values = values.to_vec();
    collect_block_local_values(
        document,
        workspace_index,
        leading_statements,
        &mut block_values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    scoped_expression_type_name(
        document,
        workspace_index,
        expr,
        &block_values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

fn scoped_block_branch_outcome(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    block: &syn::Block,
    values: &[TypedLocalValue],
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> BranchOutcome {
    scoped_block_type_name(
        document,
        workspace_index,
        block,
        values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
    .map(BranchOutcome::Value)
    .unwrap_or_else(|| block_diverges_outcome(block))
}

fn scoped_expression_branch_outcome(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    values: &[TypedLocalValue],
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> BranchOutcome {
    scoped_expression_type_name(
        document,
        workspace_index,
        expr,
        values,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
    .map(BranchOutcome::Value)
    .unwrap_or_else(|| diverging_expression_outcome(expr))
}

fn collect_block_local_values(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    statements: &[Stmt],
    values: &mut Vec<TypedLocalValue>,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) {
    for statement in statements {
        let Stmt::Local(local) = statement else {
            continue;
        };
        let wrapped_item_type = local.init.as_ref().and_then(|init| {
            expression_optional_item_type_name_with_item_scope(
                document,
                workspace_index,
                &init.expr,
                &|name| scoped_value_type_name(values, name).or_else(|| scope_type_name(name)),
                context_type_name,
                scope_item_type_name,
            )
        });
        let type_name = explicit_pattern_type_name(&local.pat)
            .or_else(|| {
                local.init.as_ref().and_then(|init| {
                    scoped_expression_type_name(
                        document,
                        workspace_index,
                        &init.expr,
                        values,
                        scope_type_name,
                        context_type_name,
                        scope_item_type_name,
                    )
                })
            })
            .or_else(|| wrapper_pattern_type_name(&local.pat).map(str::to_string));
        let Some(type_name) = type_name else {
            continue;
        };
        let new_values = typed_pattern_bindings_with_wrapped_item(
            document,
            workspace_index,
            &local.pat,
            &type_name,
            wrapped_item_type.as_deref(),
        );
        values.extend(new_values);
    }
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

fn same_branch_type_name(outcomes: impl IntoIterator<Item = BranchOutcome>) -> Option<String> {
    let mut expected = None;
    for outcome in outcomes {
        match outcome {
            BranchOutcome::Value(type_name) => {
                if expected.as_ref().is_some_and(|known| known != &type_name) {
                    return None;
                }
                expected = Some(type_name);
            }
            BranchOutcome::Diverges => {}
            BranchOutcome::Unknown => return None,
        }
    }
    expected
}

fn diverging_expression_outcome(expr: &Expr) -> BranchOutcome {
    if expression_diverges(expr) {
        BranchOutcome::Diverges
    } else {
        BranchOutcome::Unknown
    }
}

fn block_diverges_outcome(block: &syn::Block) -> BranchOutcome {
    if block_diverges(block) {
        BranchOutcome::Diverges
    } else {
        BranchOutcome::Unknown
    }
}

fn expression_diverges(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Break(_) | Expr::Continue(_) => true,
        Expr::Block(block) => block_diverges(&block.block),
        Expr::If(expr_if) => {
            let Some((_, else_branch)) = &expr_if.else_branch else {
                return false;
            };
            block_diverges(&expr_if.then_branch) && expression_diverges(else_branch)
        }
        Expr::Match(expr_match) => {
            !expr_match.arms.is_empty()
                && expr_match
                    .arms
                    .iter()
                    .all(|arm| expression_diverges(&arm.body))
        }
        Expr::Macro(expr_macro) => macro_path_diverges(&expr_macro.mac.path),
        Expr::Paren(paren) => expression_diverges(&paren.expr),
        Expr::Group(group) => expression_diverges(&group.expr),
        _ => false,
    }
}

fn block_diverges(block: &syn::Block) -> bool {
    block.stmts.iter().any(statement_diverges)
}

fn statement_diverges(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(expr, _) => expression_diverges(expr),
        Stmt::Macro(stmt_macro) => macro_path_diverges(&stmt_macro.mac.path),
        Stmt::Local(_) | Stmt::Item(_) => false,
    }
}

fn macro_path_diverges(path: &syn::Path) -> bool {
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
        .is_some_and(|name| DIVERGING_MACROS.contains(&name.as_str()))
}
