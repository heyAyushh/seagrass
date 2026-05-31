use {
    crate::range::range_from_span,
    quote::ToTokens,
    syn::{Expr, ExprField, Member},
    tower_lsp::lsp_types::{Position, Range},
};

const UNKNOWN_EXPR_RECEIVER_PREFIX: &str = "value";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FieldAccess {
    pub(super) receiver: String,
    pub(super) members: Vec<FieldMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FieldMember {
    pub(super) name: String,
    pub(super) range: Range,
}

pub(super) struct FieldExpressionAccess<'a> {
    receiver: &'a Expr,
    members: Vec<FieldMember>,
}

impl FieldAccess {
    pub(super) fn from_named_expr_field(node: &ExprField) -> Option<Self> {
        let mut members = Vec::new();
        collect_named_field_access(node, &mut members).map(|receiver| Self { receiver, members })
    }

    pub(super) fn end_position(&self) -> Option<Position> {
        self.members.last().map(|member| member.range.end)
    }
}

impl<'a> FieldExpressionAccess<'a> {
    pub(super) fn from_expr_field(node: &'a ExprField) -> Option<Self> {
        let mut members = Vec::new();
        let receiver = collect_expression_field_access(node, &mut members)?;
        Some(Self { receiver, members })
    }

    pub(super) fn receiver(&self) -> &'a Expr {
        self.receiver
    }

    pub(super) fn into_field_access(self, receiver_type: &str) -> FieldAccess {
        FieldAccess {
            receiver: expression_access_path(self.receiver)
                .unwrap_or_else(|| format!("{UNKNOWN_EXPR_RECEIVER_PREFIX}: {receiver_type}")),
            members: self.members,
        }
    }
}

fn collect_named_field_access(node: &ExprField, members: &mut Vec<FieldMember>) -> Option<String> {
    let receiver = match node.base.as_ref() {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            path.path.segments[0].ident.to_string()
        }
        Expr::Field(base) => collect_named_field_access(base, members)?,
        Expr::Paren(paren) => {
            let Expr::Field(field) = paren.expr.as_ref() else {
                return None;
            };
            collect_named_field_access(field, members)?
        }
        Expr::Group(group) => {
            let Expr::Field(field) = group.expr.as_ref() else {
                return None;
            };
            collect_named_field_access(field, members)?
        }
        Expr::Reference(reference) => {
            let Expr::Field(field) = reference.expr.as_ref() else {
                return None;
            };
            collect_named_field_access(field, members)?
        }
        _ => return None,
    };
    members.push(field_member(node)?);
    Some(receiver)
}

fn collect_expression_field_access<'a>(
    node: &'a ExprField,
    members: &mut Vec<FieldMember>,
) -> Option<&'a Expr> {
    let receiver = match node.base.as_ref() {
        Expr::Field(base) => collect_expression_field_access(base, members)?,
        Expr::Paren(paren) => match paren.expr.as_ref() {
            Expr::Field(field) => collect_expression_field_access(field, members)?,
            expr => expr,
        },
        Expr::Group(group) => match group.expr.as_ref() {
            Expr::Field(field) => collect_expression_field_access(field, members)?,
            expr => expr,
        },
        Expr::Reference(reference) => match reference.expr.as_ref() {
            Expr::Field(field) => collect_expression_field_access(field, members)?,
            expr => expr,
        },
        expr => expr,
    };
    members.push(field_member(node)?);
    Some(receiver)
}

fn field_member(node: &ExprField) -> Option<FieldMember> {
    let Member::Named(ident) = &node.member else {
        return None;
    };
    Some(FieldMember {
        name: ident.to_string(),
        range: range_from_span(ident.span()),
    })
}

pub(super) fn expression_access_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Some(path.path.segments[0].ident.to_string())
        }
        Expr::Field(field) => {
            let access = FieldAccess::from_named_expr_field(field)?;
            let mut path = access.receiver;
            for member in access.members {
                path.push('.');
                path.push_str(&member.name);
            }
            Some(path)
        }
        Expr::Index(index) => expression_access_path(&index.expr).map(|receiver| {
            let index = index.index.to_token_stream().to_string();
            format!("{receiver}[{index}]")
        }),
        Expr::MethodCall(method_call) => {
            let receiver = expression_access_path(&method_call.receiver)?;
            Some(format!("{receiver}.{}()", method_call.method))
        }
        Expr::Paren(paren) => expression_access_path(&paren.expr),
        Expr::Group(group) => expression_access_path(&group.expr),
        Expr::Reference(reference) => expression_access_path(&reference.expr),
        _ => None,
    }
}
