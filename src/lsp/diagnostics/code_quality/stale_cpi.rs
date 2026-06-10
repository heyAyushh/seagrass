use {
    crate::diagnostics::{
        diagnostic_from_span,
        lint::{run_lint_visitor_on_functions, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    crate::syntax::{expr_path_ends_with, member_is_named, member_name},
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::Diagnostic,
};

pub(super) fn diagnostics(document: &crate::document::ParsedDocument) -> Vec<Diagnostic> {
    run_lint_visitor_on_functions(document, |_| StaleAccountAfterCpiVisitor::default())
}

#[derive(Default)]
struct StaleAccountAfterCpiVisitor {
    cpi_span: Option<proc_macro2::Span>,
    reloaded_accounts: Vec<String>,
    diagnostics: Vec<Diagnostic>,
}

impl StaleAccountAfterCpiVisitor {
    fn record_cpi(&mut self, span: proc_macro2::Span) {
        self.cpi_span.get_or_insert(span);
    }

    fn record_reload(&mut self, account: String) {
        if !self
            .reloaded_accounts
            .iter()
            .any(|existing| existing == &account)
        {
            self.reloaded_accounts.push(account);
        }
    }

    fn record_account_read(&mut self, account: String) {
        let Some(cpi_span) = self.cpi_span else {
            return;
        };
        if self
            .reloaded_accounts
            .iter()
            .any(|existing| existing == &account)
            || self.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("account"))
                    == Some(&serde_json::json!(account))
            })
        {
            return;
        }
        self.diagnostics
            .push(stale_account_after_cpi_diagnostic(cpi_span, account));
    }
}

impl<'ast> LintVisitor<'ast> for StaleAccountAfterCpiVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/solana.code-quality.stale-account-after-cpi";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for StaleAccountAfterCpiVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        let is_cpi = is_cpi_call(&node.func);
        visit::visit_expr_call(self, node);
        if is_cpi {
            self.record_cpi(node.func.span());
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "reload" {
            if let Some(account) = ctx_account_name(&node.receiver) {
                self.record_reload(account);
            }
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if let Some(account) = ctx_account_field_read_name(node) {
            self.record_account_read(account);
        }
        visit::visit_expr_field(self, node);
    }
}

fn stale_account_after_cpi_diagnostic(span: proc_macro2::Span, account: String) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Account data read after CPI may be stale; reload accounts that the CPI can mutate before reading cached fields.".to_string(),
        Some(serde_json::json!({
            "attack": "stale-account-after-cpi",
            "topic": "seagrass/solana.code-quality.stale-account-after-cpi",
            "corpusMode": "solana-invariant",
            "quickfix": "insert-reload-after-cpi",
            "evidenceSource": "cpi-then-account-read-scan",
            "account": account,
            "configKey": "security.staleCpiReload",
            "strictNative": false,
            "suggestion": "Call `.reload()?` on Anchor accounts after CPI when subsequent logic reads fields that the CPI may have changed.",
            "absorbedFrom": "solana-program-security",
        })),
    )
}

fn is_cpi_call(func: &syn::Expr) -> bool {
    expr_path_ends_with(func, &["CpiContext", "new"])
        || expr_path_ends_with(func, &["CpiContext", "new_with_signer"])
        || expr_path_ends_with(func, &["invoke"])
}

fn ctx_account_field_read_name(field: &syn::ExprField) -> Option<String> {
    ctx_account_name(&field.base)
}

fn ctx_account_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Field(field) if expr_is_ctx_accounts(&field.base) => member_name(&field.member),
        syn::Expr::Group(group) => ctx_account_name(&group.expr),
        syn::Expr::Paren(paren) => ctx_account_name(&paren.expr),
        syn::Expr::Reference(reference) => ctx_account_name(&reference.expr),
        _ => None,
    }
}

fn expr_is_ctx_accounts(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Field(field) if member_is_named(&field.member, "accounts") => {
            expr_is_ident(&field.base, "ctx")
        }
        syn::Expr::Group(group) => expr_is_ctx_accounts(&group.expr),
        syn::Expr::Paren(paren) => expr_is_ctx_accounts(&paren.expr),
        syn::Expr::Reference(reference) => expr_is_ctx_accounts(&reference.expr),
        _ => false,
    }
}

fn expr_is_ident(expr: &syn::Expr, expected: &str) -> bool {
    matches!(expr, syn::Expr::Path(path) if path.path.is_ident(expected))
}
