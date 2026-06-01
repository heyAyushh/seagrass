use {
    crate::document::ParsedDocument,
    syn::visit::{self, Visit},
};

#[derive(Default)]
pub(super) struct RawAccountRisk {
    pub(super) missing_owner_check: bool,
    pub(super) missing_discriminator_check: bool,
}

pub(super) fn raw_account_risk(
    document: &ParsedDocument,
    accounts_name: &str,
    field_name: &str,
) -> RawAccountRisk {
    if document.syntax().items.is_empty() {
        return RawAccountRisk::default();
    }

    let mut visitor = RawAccountFileVisitor {
        accounts_name,
        field_name,
        risk: RawAccountRisk::default(),
    };
    visitor.visit_file(document.syntax());
    visitor.risk
}

struct RawAccountFileVisitor<'a> {
    accounts_name: &'a str,
    field_name: &'a str,
    risk: RawAccountRisk,
}

impl<'ast> Visit<'ast> for RawAccountFileVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let context_names = context_argument_names_for_accounts(node, self.accounts_name);
        if context_names.is_empty() {
            return;
        }

        let mut function = RawAccountFunctionVisitor {
            field_name: self.field_name,
            context_names,
            accounts_aliases: Vec::new(),
            field_aliases: Vec::new(),
            data_aliases: Vec::new(),
            evidence: RawAccountFunctionEvidence::default(),
        };
        function.visit_block(&node.block);
        let evidence = function.evidence;
        if evidence.raw_data && !evidence.owner_check {
            self.risk.missing_owner_check = true;
        }
        if evidence.raw_deserialization && !evidence.discriminator_check {
            self.risk.missing_discriminator_check = true;
        }
    }
}

#[derive(Default)]
struct RawAccountFunctionEvidence {
    raw_data: bool,
    raw_deserialization: bool,
    owner_check: bool,
    discriminator_check: bool,
}

struct RawAccountFunctionVisitor<'a> {
    field_name: &'a str,
    context_names: Vec<String>,
    accounts_aliases: Vec<String>,
    field_aliases: Vec<String>,
    data_aliases: Vec<String>,
    evidence: RawAccountFunctionEvidence,
}

impl<'ast> Visit<'ast> for RawAccountFunctionVisitor<'_> {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(local_name) = local_ident(node) {
            if local_aliases_accounts(node, &self.context_names, &self.accounts_aliases) {
                push_unique(&mut self.accounts_aliases, local_name.to_string());
            }
            if local_aliases_field(
                node,
                self.field_name,
                &self.context_names,
                &self.accounts_aliases,
                &self.field_aliases,
            ) {
                push_unique(&mut self.field_aliases, local_name.to_string());
            }
            if local_reads_raw_data(node, self) {
                self.evidence.raw_data = true;
                push_unique(&mut self.data_aliases, local_name.to_string());
            }
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if raw_data_account_from_method(node, self)
            .is_some_and(|account| account == self.field_name)
        {
            self.evidence.raw_data = true;
        }
        if is_safe_deserialize_method(&node.method) {
            self.evidence.discriminator_check = true;
        } else if is_raw_deserialize_method(&node.method)
            && expr_mentions_raw_data(&node.receiver, self)
        {
            self.evidence.raw_deserialization = true;
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if called_ident(&node.func).is_some_and(is_owner_validation_helper)
            && node.args.iter().any(|arg| expr_mentions_field(arg, self))
        {
            self.evidence.owner_check = true;
        }
        if called_ident(&node.func).is_some_and(is_type_validation_helper)
            && node.args.iter().any(|arg| expr_mentions_field(arg, self))
        {
            self.evidence.discriminator_check = true;
        }
        if called_ident(&node.func).is_some_and(is_safe_deserialize_function) {
            self.evidence.discriminator_check = true;
        } else if called_ident(&node.func).is_some_and(is_raw_deserialize_function)
            && node
                .args
                .iter()
                .any(|arg| expr_mentions_raw_data(arg, self))
        {
            self.evidence.raw_deserialization = true;
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
            if owner_account_from_expr(&node.left, self)
                .is_some_and(|account| account == self.field_name)
                || owner_account_from_expr(&node.right, self)
                    .is_some_and(|account| account == self.field_name)
            {
                self.evidence.owner_check = true;
            }
            if (expr_mentions_raw_data(&node.left, self)
                || expr_mentions_raw_data(&node.right, self))
                && (expr_has_discriminator(&node.left) || expr_has_discriminator(&node.right))
            {
                self.evidence.discriminator_check = true;
            }
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        for expression in super::super::macro_expressions::runtime_assertion_macro_arguments(node) {
            self.visit_expr(&expression);
        }
    }
}

fn context_argument_names_for_accounts(item_fn: &syn::ItemFn, accounts_name: &str) -> Vec<String> {
    item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(pat_type) = arg else {
                return None;
            };
            let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
                return None;
            };
            context_type_name(pat_type.ty.as_ref())
                .filter(|context| context == accounts_name)
                .map(|_| pat_ident.ident.to_string())
        })
        .collect()
}

fn context_type_name(ty: &syn::Type) -> Option<String> {
    let ty = match ty {
        syn::Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.iter().find(|segment| {
        segment.ident == "Context" || segment.ident.to_string().ends_with("Context")
    })?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(syn::Type::Path(path)) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    })
}

fn local_ident(local: &syn::Local) -> Option<&syn::Ident> {
    let syn::Pat::Ident(pat_ident) = &local.pat else {
        return None;
    };
    Some(&pat_ident.ident)
}

fn local_aliases_accounts(
    local: &syn::Local,
    context_names: &[String],
    accounts_aliases: &[String],
) -> bool {
    local
        .init
        .as_ref()
        .is_some_and(|init| is_accounts_container_expr(&init.expr, context_names, accounts_aliases))
}

fn local_aliases_field(
    local: &syn::Local,
    field_name: &str,
    context_names: &[String],
    accounts_aliases: &[String],
    field_aliases: &[String],
) -> bool {
    local.init.as_ref().is_some_and(|init| {
        account_name_from_expr(&init.expr, context_names, accounts_aliases, field_aliases)
            .is_some_and(|account| account == field_name)
    })
}

fn local_reads_raw_data(local: &syn::Local, visitor: &RawAccountFunctionVisitor<'_>) -> bool {
    local.init.as_ref().is_some_and(|init| {
        raw_data_account_from_expr(&init.expr, visitor)
            .is_some_and(|account| account == visitor.field_name)
    })
}

fn is_accounts_container_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
) -> bool {
    match expr {
        syn::Expr::Field(field) => {
            matches!(&field.member, syn::Member::Named(ident) if ident == "accounts")
                && matches!(
                    field.base.as_ref(),
                    syn::Expr::Path(path)
                        if context_names.iter().any(|name| path.path.is_ident(name))
                )
        }
        syn::Expr::Path(path) => accounts_aliases
            .iter()
            .any(|alias| path.path.is_ident(alias)),
        syn::Expr::Reference(reference) => {
            is_accounts_container_expr(&reference.expr, context_names, accounts_aliases)
        }
        syn::Expr::Paren(paren) => {
            is_accounts_container_expr(&paren.expr, context_names, accounts_aliases)
        }
        syn::Expr::Group(group) => {
            is_accounts_container_expr(&group.expr, context_names, accounts_aliases)
        }
        syn::Expr::Unary(unary) => {
            is_accounts_container_expr(&unary.expr, context_names, accounts_aliases)
        }
        _ => false,
    }
}

fn account_name_from_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
    field_aliases: &[String],
) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => field_aliases
            .iter()
            .find(|alias| path.path.is_ident(alias.as_str()))
            .cloned(),
        syn::Expr::Field(field)
            if is_accounts_container_expr(field.base.as_ref(), context_names, accounts_aliases) =>
        {
            let syn::Member::Named(ident) = &field.member else {
                return None;
            };
            Some(ident.to_string())
        }
        syn::Expr::Field(field) => {
            account_name_from_expr(&field.base, context_names, accounts_aliases, field_aliases)
        }
        syn::Expr::MethodCall(method) => account_name_from_expr(
            &method.receiver,
            context_names,
            accounts_aliases,
            field_aliases,
        ),
        syn::Expr::Reference(reference) => account_name_from_expr(
            &reference.expr,
            context_names,
            accounts_aliases,
            field_aliases,
        ),
        syn::Expr::Paren(paren) => {
            account_name_from_expr(&paren.expr, context_names, accounts_aliases, field_aliases)
        }
        syn::Expr::Group(group) => {
            account_name_from_expr(&group.expr, context_names, accounts_aliases, field_aliases)
        }
        syn::Expr::Unary(unary) => {
            account_name_from_expr(&unary.expr, context_names, accounts_aliases, field_aliases)
        }
        _ => None,
    }
}

fn raw_data_account_from_method(
    method: &syn::ExprMethodCall,
    visitor: &RawAccountFunctionVisitor<'_>,
) -> Option<String> {
    match method.method.to_string().as_str() {
        "try_borrow_data" | "try_borrow_mut_data" => account_name_from_expr(
            &method.receiver,
            &visitor.context_names,
            &visitor.accounts_aliases,
            &visitor.field_aliases,
        ),
        "borrow" | "borrow_mut" => raw_data_account_from_expr(&method.receiver, visitor),
        _ => None,
    }
}

fn raw_data_account_from_expr(
    expr: &syn::Expr,
    visitor: &RawAccountFunctionVisitor<'_>,
) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method) => raw_data_account_from_method(method, visitor),
        syn::Expr::Field(field) if matches!(&field.member, syn::Member::Named(ident) if ident == "data") => {
            account_name_from_expr(
                &field.base,
                &visitor.context_names,
                &visitor.accounts_aliases,
                &visitor.field_aliases,
            )
        }
        syn::Expr::Path(path) => visitor
            .data_aliases
            .iter()
            .any(|alias| path.path.is_ident(alias.as_str()))
            .then(|| visitor.field_name.to_string()),
        syn::Expr::Index(index) => raw_data_account_from_expr(&index.expr, visitor),
        syn::Expr::Reference(reference) => raw_data_account_from_expr(&reference.expr, visitor),
        syn::Expr::Paren(paren) => raw_data_account_from_expr(&paren.expr, visitor),
        syn::Expr::Group(group) => raw_data_account_from_expr(&group.expr, visitor),
        syn::Expr::Unary(unary) => raw_data_account_from_expr(&unary.expr, visitor),
        _ => None,
    }
}

fn expr_mentions_field(expr: &syn::Expr, visitor: &RawAccountFunctionVisitor<'_>) -> bool {
    account_name_from_expr(
        expr,
        &visitor.context_names,
        &visitor.accounts_aliases,
        &visitor.field_aliases,
    )
    .is_some_and(|account| account == visitor.field_name)
}

fn expr_mentions_raw_data(expr: &syn::Expr, visitor: &RawAccountFunctionVisitor<'_>) -> bool {
    if raw_data_account_from_expr(expr, visitor)
        .is_some_and(|account| account == visitor.field_name)
    {
        return true;
    }
    match expr {
        syn::Expr::Call(call) => call
            .args
            .iter()
            .any(|argument| expr_mentions_raw_data(argument, visitor)),
        syn::Expr::MethodCall(method) => {
            expr_mentions_raw_data(&method.receiver, visitor)
                || method
                    .args
                    .iter()
                    .any(|argument| expr_mentions_raw_data(argument, visitor))
        }
        syn::Expr::Binary(binary) => {
            expr_mentions_raw_data(&binary.left, visitor)
                || expr_mentions_raw_data(&binary.right, visitor)
        }
        syn::Expr::Reference(reference) => expr_mentions_raw_data(&reference.expr, visitor),
        syn::Expr::Paren(paren) => expr_mentions_raw_data(&paren.expr, visitor),
        syn::Expr::Group(group) => expr_mentions_raw_data(&group.expr, visitor),
        syn::Expr::Unary(unary) => expr_mentions_raw_data(&unary.expr, visitor),
        syn::Expr::Index(index) => expr_mentions_raw_data(&index.expr, visitor),
        _ => false,
    }
}

fn owner_account_from_expr(
    expr: &syn::Expr,
    visitor: &RawAccountFunctionVisitor<'_>,
) -> Option<String> {
    match expr {
        syn::Expr::Field(field) if matches!(&field.member, syn::Member::Named(ident) if ident == "owner") => {
            account_name_from_expr(
                &field.base,
                &visitor.context_names,
                &visitor.accounts_aliases,
                &visitor.field_aliases,
            )
        }
        syn::Expr::Reference(reference) => owner_account_from_expr(&reference.expr, visitor),
        syn::Expr::Paren(paren) => owner_account_from_expr(&paren.expr, visitor),
        syn::Expr::Group(group) => owner_account_from_expr(&group.expr, visitor),
        syn::Expr::Unary(unary) => owner_account_from_expr(&unary.expr, visitor),
        _ => None,
    }
}

fn called_ident(func: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(path) = func else {
        return None;
    };
    path.path.segments.last().map(|segment| &segment.ident)
}

fn is_owner_validation_helper(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "assert_owner" | "check_owner" | "validate_owner" | "require_owner"
    )
}

fn is_type_validation_helper(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "assert_type"
            | "check_type"
            | "validate_type"
            | "assert_account_type"
            | "check_account_type"
    )
}

fn is_safe_deserialize_function(ident: &syn::Ident) -> bool {
    ident == "try_deserialize"
}

fn is_raw_deserialize_function(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "try_from_slice" | "deserialize" | "try_deserialize_unchecked" | "from_account_info"
    )
}

fn is_safe_deserialize_method(ident: &syn::Ident) -> bool {
    ident == "try_deserialize"
}

fn is_raw_deserialize_method(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "try_from_slice" | "deserialize" | "try_deserialize_unchecked" | "from_account_info"
    )
}

fn expr_has_discriminator(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Path(path) => path.path.segments.iter().any(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "DISCRIMINATOR" | "discriminator"
            )
        }),
        syn::Expr::Field(field) => {
            matches!(&field.member, syn::Member::Named(ident) if matches!(ident.to_string().as_str(), "DISCRIMINATOR" | "discriminator"))
                || expr_has_discriminator(&field.base)
        }
        syn::Expr::Call(call) => call.args.iter().any(expr_has_discriminator),
        syn::Expr::MethodCall(method) => {
            expr_has_discriminator(&method.receiver)
                || method.args.iter().any(expr_has_discriminator)
        }
        syn::Expr::Binary(binary) => {
            expr_has_discriminator(&binary.left) || expr_has_discriminator(&binary.right)
        }
        syn::Expr::Reference(reference) => expr_has_discriminator(&reference.expr),
        syn::Expr::Paren(paren) => expr_has_discriminator(&paren.expr),
        syn::Expr::Group(group) => expr_has_discriminator(&group.expr),
        syn::Expr::Unary(unary) => expr_has_discriminator(&unary.expr),
        syn::Expr::Index(index) => {
            expr_has_discriminator(&index.expr) || expr_has_discriminator(&index.index)
        }
        _ => false,
    }
}

fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.iter().any(|existing| existing == &item) {
        items.push(item);
    }
}
