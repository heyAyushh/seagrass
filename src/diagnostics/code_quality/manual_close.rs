use {
    crate::diagnostics::{
        diagnostic_from_span,
        lint::{run_lint_visitor_on_functions, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
        Expr, FieldValue, Lit, Member,
    },
    tower_lsp::lsp_types::Diagnostic,
};

const ZERO_LITERAL: &str = "0";

pub(super) fn diagnostics(document: &crate::document::ParsedDocument) -> Vec<Diagnostic> {
    run_lint_visitor_on_functions(document, |_| ManualCloseReinitVisitor::default())
}

#[derive(Default)]
struct ManualCloseReinitVisitor {
    lamports_mutation: Option<proc_macro2::Span>,
    close_action: Option<proc_macro2::Span>,
    unchecked_initialization: Option<proc_macro2::Span>,
    closed_discriminator: bool,
}

impl ManualCloseReinitVisitor {
    fn finish(self) -> Vec<Diagnostic> {
        let close = self
            .lamports_mutation
            .zip(self.close_action)
            .filter(|_| !self.closed_discriminator)
            .map(|(lamports, close)| {
                account_closing_diagnostic(close.join(lamports).unwrap_or(close))
            });

        let init = self.unchecked_initialization.map(initialization_diagnostic);

        close.into_iter().chain(init).collect()
    }

    fn record_close_action(&mut self, span: proc_macro2::Span) {
        self.close_action.get_or_insert(span);
    }

    fn record_lamports_mutation(&mut self, span: proc_macro2::Span) {
        self.lamports_mutation.get_or_insert(span);
    }
}

impl<'ast> LintVisitor<'ast> for ManualCloseReinitVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/solana.code-quality.account-closing";

    fn finish(self) -> Vec<Diagnostic> {
        ManualCloseReinitVisitor::finish(self)
    }
}

impl<'ast> Visit<'ast> for ManualCloseReinitVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        match method.as_str() {
            "try_borrow_mut_lamports" => self.record_lamports_mutation(node.method.span()),
            "assign" if method_args_contain_system_program_id(node) => {
                self.record_close_action(node.method.span());
            }
            "realloc" if first_method_arg_is_zero(node) => {
                self.record_close_action(node.method.span());
            }
            "fill" if first_method_arg_is_zero(node) && receiver_is_mut_data(node) => {
                self.record_close_action(node.method.span());
            }
            "try_deserialize_unchecked" => {
                self.unchecked_initialization
                    .get_or_insert(node.method.span());
            }
            _ => {}
        }

        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if expr_contains_ident(&node.left, "lamports") && expr_is_zero(&node.right) {
            self.record_lamports_mutation(node.eq_token.spans[0]);
        }
        if expr_contains_ident(&node.left, "discriminator") && expr_is_false(&node.right) {
            self.unchecked_initialization
                .get_or_insert(node.eq_token.spans[0]);
        }
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if called_ident(&node.func).is_some_and(|ident| ident == "try_deserialize_unchecked") {
            self.unchecked_initialization
                .get_or_insert(node.func.span());
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == "CLOSED_ACCOUNT_DISCRIMINATOR")
        {
            self.closed_discriminator = true;
        }
        visit::visit_expr_path(self, node);
    }
}

fn account_closing_diagnostic(span: proc_macro2::Span) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Manual account close/reinit logic should use a safe close invariant or closed discriminator."
            .to_string(),
        Some(serde_json::json!({
            "quickfix": "prefer-anchor-close",
            "attack": "account-closing",
            "topic": "seagrass/solana.code-quality.account-closing",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "manual-close-scan",
            "configKey": "security.accountClosing",
            "strictNative": false,
            "suggestion": "Prefer Anchor `close = ...`; for manual close, drain lamports, make the account unusable, and write/verify a closed discriminator.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn initialization_diagnostic(span: proc_macro2::Span) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Unchecked initialization/deserialization can allow reinitialization unless a type marker is enforced."
            .to_string(),
        Some(serde_json::json!({
            "quickfix": "reject-reinit",
            "attack": "initialization",
            "topic": "seagrass/solana.code-quality.initialization",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "manual-init-scan",
            "configKey": "security.initialization",
            "strictNative": false,
            "suggestion": "Use checked Anchor account initialization/deserialization or explicitly reject already-initialized accounts.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn method_args_contain_system_program_id(node: &syn::ExprMethodCall) -> bool {
    node.args.iter().any(is_system_program_id_expr)
}

fn first_method_arg_is_zero(node: &syn::ExprMethodCall) -> bool {
    node.args.first().is_some_and(expr_is_zero)
}

fn receiver_is_mut_data(node: &syn::ExprMethodCall) -> bool {
    expr_is_method_call(&node.receiver, "try_borrow_mut_data")
        || (expr_is_method_call(&node.receiver, "borrow_mut")
            && expr_contains_ident(&node.receiver, "data"))
}

fn called_ident(func: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(expr_path) = func else {
        return None;
    };
    expr_path.path.segments.last().map(|segment| &segment.ident)
}

fn is_system_program_id_expr(expr: &Expr) -> bool {
    expr_path_ends_with(expr, &["system_program", "ID"])
        || expr_path_ends_with(expr, &["System", "id"])
}

fn expr_path_ends_with(expr: &Expr, expected: &[&str]) -> bool {
    match expr {
        Expr::Path(expr_path) => path_ends_with(&expr_path.path, expected),
        Expr::Call(expr_call) => expr_path_ends_with(&expr_call.func, expected),
        Expr::Group(expr_group) => expr_path_ends_with(&expr_group.expr, expected),
        Expr::Paren(expr_paren) => expr_path_ends_with(&expr_paren.expr, expected),
        Expr::Reference(expr_reference) => expr_path_ends_with(&expr_reference.expr, expected),
        _ => false,
    }
}

fn path_ends_with(path: &syn::Path, expected: &[&str]) -> bool {
    let actual = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let Some(suffix) = actual.get(actual.len().saturating_sub(expected.len())..) else {
        return false;
    };
    suffix.len() == expected.len()
        && suffix
            .iter()
            .map(String::as_str)
            .zip(expected.iter().copied())
            .all(|(actual, expected)| actual == expected)
}

fn expr_is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(expr_lit) if matches!(&expr_lit.lit, Lit::Int(lit) if lit.base10_digits() == ZERO_LITERAL))
}

fn expr_is_false(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(expr_lit) if matches!(&expr_lit.lit, Lit::Bool(lit) if !lit.value))
}

fn expr_is_method_call(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::MethodCall(method_call) if method_call.method == expected)
}

fn expr_contains_ident(expr: &Expr, expected: &str) -> bool {
    let mut visitor = IdentSearch {
        expected,
        found: false,
    };
    visitor.visit_expr(expr);
    visitor.found
}

struct IdentSearch<'a> {
    expected: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for IdentSearch<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == self.expected {
            self.found = true;
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == self.expected)
        {
            self.found = true;
        }
        visit::visit_expr_path(self, node);
    }

    fn visit_field_value(&mut self, node: &'ast FieldValue) {
        if let Member::Named(ident) = &node.member {
            if ident == self.expected {
                self.found = true;
            }
        }
        visit::visit_field_value(self, node);
    }

    fn visit_member(&mut self, node: &'ast Member) {
        if let Member::Named(ident) = node {
            if ident == self.expected {
                self.found = true;
            }
        }
        visit::visit_member(self, node);
    }
}
