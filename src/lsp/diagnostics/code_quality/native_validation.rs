use {
    crate::diagnostics::{
        diagnostic_from_span,
        lint::{run_lint_visitor_on_functions, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    quote::ToTokens,
    syn::{
        parse::Parser,
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::Diagnostic,
};

use super::ProgramKind;

const SIGNER_VALIDATION_HELPERS: &[&str] = &[
    "assert_signer",
    "check_signer",
    "require_signer",
    "validate_signer",
];
const PROGRAM_ID_VALIDATION_HELPERS: &[&str] = &[
    "assert_program_id",
    "check_program_id",
    "require_program_id",
    "validate_program_id",
    "check_id",
];
const INSTRUCTION_PROGRAM_ID_CONSTRUCTORS: &[&str] =
    &["new_with_bincode", "new_with_borsh", "new_with_bytes"];
const SIGNER_ACCOUNT_META_CONSTRUCTORS: &[&str] = &["new", "new_readonly"];

pub(super) fn diagnostics(
    document: &crate::document::ParsedDocument,
    program_kind: ProgramKind,
) -> Vec<Diagnostic> {
    if !matches!(
        program_kind,
        ProgramKind::NativeSolana | ProgramKind::Pinocchio
    ) || document.syntax().items.is_empty()
    {
        return Vec::new();
    }

    run_lint_visitor_on_functions(document, |_| NativeAccountValidationVisitor::default())
}

#[derive(Default)]
struct NativeAccountValidationVisitor {
    signer_meta: Option<SignerMetaEvidence>,
    cpi_program: Option<ProgramIdEvidence>,
    has_signer_validation: bool,
    has_program_id_validation: bool,
    saw_invoke_signed: bool,
}

impl NativeAccountValidationVisitor {
    fn finish(self) -> Vec<Diagnostic> {
        let signer = self
            .signer_meta
            .filter(|_| !self.has_signer_validation && !self.saw_invoke_signed)
            .map(signer_authorization_diagnostic);
        let cpi = self
            .cpi_program
            .filter(|evidence| evidence.dynamic && !self.has_program_id_validation)
            .map(arbitrary_cpi_diagnostic);
        signer.into_iter().chain(cpi).collect()
    }
}

impl<'ast> LintVisitor<'ast> for NativeAccountValidationVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/security.signer.authorization";

    fn finish(self) -> Vec<Diagnostic> {
        NativeAccountValidationVisitor::finish(self)
    }
}

#[derive(Clone)]
struct SignerMetaEvidence {
    span: proc_macro2::Span,
    account_index: Option<usize>,
    account_expression: Option<String>,
}

#[derive(Clone)]
struct ProgramIdEvidence {
    span: proc_macro2::Span,
    expression: Option<String>,
    dynamic: bool,
}

impl<'ast> Visit<'ast> for NativeAccountValidationVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(evidence) = signer_meta_evidence(node) {
            self.signer_meta.get_or_insert(evidence);
        }
        if let Some(evidence) = instruction_constructor_program_id_evidence(node) {
            self.cpi_program.get_or_insert(evidence);
        }

        if is_invoke_signed_call(&node.func) {
            self.saw_invoke_signed = true;
        }

        if let Some(ident) = called_ident(&node.func) {
            if ident_matches_any(ident, SIGNER_VALIDATION_HELPERS) {
                self.has_signer_validation = true;
            }
            if ident_matches_any(ident, PROGRAM_ID_VALIDATION_HELPERS) {
                self.has_program_id_validation = true;
            }
        }

        visit::visit_expr_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if node.mac.path.is_ident("vec") {
            parse_vec_macro_expressions(&node.mac)
                .iter()
                .for_each(|expr| self.visit_expr(expr));
        }
        visit::visit_expr_macro(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if method == "is_signer" {
            self.has_signer_validation = true;
        }
        if method == "check_id" {
            self.has_program_id_validation = true;
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_))
            && (expr_contains_ident(&node.left, "program_id")
                || expr_contains_ident(&node.right, "program_id"))
        {
            self.has_program_id_validation = true;
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if node
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Instruction")
        {
            if let Some(field) = node
                .fields
                .iter()
                .find(|field| member_is_named(&field.member, "program_id"))
            {
                self.cpi_program.get_or_insert(ProgramIdEvidence {
                    span: field.expr.span(),
                    expression: Some(program_id_expression(&field.expr)),
                    dynamic: is_dynamic_program_id(&field.expr),
                });
            }
        }
        visit::visit_expr_struct(self, node);
    }
}

fn signer_authorization_diagnostic(evidence: SignerMetaEvidence) -> Diagnostic {
    diagnostic_from_span(
        evidence.span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Native signer account is used without a visible signer check.".to_string(),
        Some(serde_json::json!({
            "quickfix": "add-signer-check",
            "attack": "signer-authorization",
            "topic": "seagrass/security.signer.authorization",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "native-account-meta-scan",
            "accountIndex": evidence.account_index,
            "accountExpression": evidence.account_expression,
            "configKey": "security.signerAuthorization",
            "strictNative": true,
            "suggestion": "Check `account.is_signer` before using an account as an instruction signer.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn arbitrary_cpi_diagnostic(evidence: ProgramIdEvidence) -> Diagnostic {
    diagnostic_from_span(
        evidence.span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Native CPI program id is used without visible program-id validation.".to_string(),
        Some(serde_json::json!({
            "quickfix": "add-program-id-check",
            "attack": "arbitrary-cpi",
            "topic": "seagrass/security.cpi.program",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "native-instruction-scan",
            "configKey": "security.arbitraryCpi",
            "programIdExpression": evidence.expression,
            "strictNative": true,
            "suggestion": "Compare the supplied program id/account key to the expected program id before invoking.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn instruction_constructor_program_id_evidence(node: &syn::ExprCall) -> Option<ProgramIdEvidence> {
    if !is_instruction_program_id_constructor_call(&node.func) {
        return None;
    }
    let program_id = node.args.first()?;
    Some(ProgramIdEvidence {
        span: program_id.span(),
        expression: Some(program_id_expression(program_id)),
        dynamic: is_dynamic_program_id(program_id),
    })
}

fn is_instruction_program_id_constructor_call(func: &syn::Expr) -> bool {
    INSTRUCTION_PROGRAM_ID_CONSTRUCTORS
        .iter()
        .any(|constructor| path_ends_with(func, &["Instruction", constructor]))
}

fn signer_meta_evidence(node: &syn::ExprCall) -> Option<SignerMetaEvidence> {
    if !is_account_meta_signer_constructor_call(&node.func) || !second_arg_is_true(node) {
        return None;
    }
    let account_expression = node.args.first().map(expr_text);
    Some(SignerMetaEvidence {
        span: node.func.span(),
        account_index: node.args.first().and_then(account_index_from_expr),
        account_expression,
    })
}

fn is_account_meta_signer_constructor_call(func: &syn::Expr) -> bool {
    SIGNER_ACCOUNT_META_CONSTRUCTORS
        .iter()
        .any(|constructor| path_ends_with(func, &["AccountMeta", constructor]))
}

fn second_arg_is_true(node: &syn::ExprCall) -> bool {
    node.args.iter().nth(1).is_some_and(bool_literal_is_true)
}

fn is_invoke_signed_call(func: &syn::Expr) -> bool {
    path_ends_with(func, &["invoke_signed"])
}

fn is_dynamic_program_id(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Path(expr_path) => expr_path
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "ID"),
        syn::Expr::Unary(unary) => is_dynamic_program_id(&unary.expr),
        syn::Expr::Reference(reference) => is_dynamic_program_id(&reference.expr),
        syn::Expr::Paren(paren) => is_dynamic_program_id(&paren.expr),
        syn::Expr::Group(group) => is_dynamic_program_id(&group.expr),
        _ => true,
    }
}

fn program_id_expression(expr: &syn::Expr) -> String {
    expr_text(expr).trim_start_matches('*').to_string()
}

fn called_ident(func: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(expr_path) = func else {
        return None;
    };
    expr_path.path.segments.last().map(|segment| &segment.ident)
}

fn ident_matches_any(ident: &syn::Ident, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| ident == *candidate)
}

fn member_is_named(member: &syn::Member, name: &str) -> bool {
    matches!(member, syn::Member::Named(ident) if ident == name)
}

fn path_ends_with(expr: &syn::Expr, expected: &[&str]) -> bool {
    match expr {
        syn::Expr::Path(expr_path) => {
            let segment_count = expr_path.path.segments.len();
            segment_count >= expected.len()
                && expr_path
                    .path
                    .segments
                    .iter()
                    .skip(segment_count - expected.len())
                    .map(|segment| segment.ident.to_string())
                    .eq(expected.iter().copied())
        }
        syn::Expr::Group(group) => path_ends_with(&group.expr, expected),
        syn::Expr::Paren(paren) => path_ends_with(&paren.expr, expected),
        syn::Expr::Reference(reference) => path_ends_with(&reference.expr, expected),
        _ => false,
    }
}

fn bool_literal_is_true(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(expr_lit) => matches!(&expr_lit.lit, syn::Lit::Bool(lit) if lit.value),
        syn::Expr::Group(group) => bool_literal_is_true(&group.expr),
        syn::Expr::Paren(paren) => bool_literal_is_true(&paren.expr),
        _ => false,
    }
}

fn expr_contains_ident(expr: &syn::Expr, expected: &str) -> bool {
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

    fn visit_member(&mut self, node: &'ast syn::Member) {
        if let syn::Member::Named(ident) = node {
            if ident == self.expected {
                self.found = true;
            }
        }
        visit::visit_member(self, node);
    }
}

fn account_index_from_expr(expr: &syn::Expr) -> Option<usize> {
    match expr {
        syn::Expr::Index(index) => unsigned_literal(&index.index),
        syn::Expr::Field(field) => account_index_from_expr(&field.base),
        syn::Expr::MethodCall(method_call) => account_index_from_expr(&method_call.receiver),
        syn::Expr::Unary(unary) => account_index_from_expr(&unary.expr),
        syn::Expr::Reference(reference) => account_index_from_expr(&reference.expr),
        syn::Expr::Paren(paren) => account_index_from_expr(&paren.expr),
        syn::Expr::Group(group) => account_index_from_expr(&group.expr),
        _ => None,
    }
}

fn unsigned_literal(expr: &syn::Expr) -> Option<usize> {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Int(lit) => lit.base10_parse().ok(),
            _ => None,
        },
        syn::Expr::Group(group) => unsigned_literal(&group.expr),
        syn::Expr::Paren(paren) => unsigned_literal(&paren.expr),
        _ => None,
    }
}

fn expr_text(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn parse_vec_macro_expressions(mac: &syn::Macro) -> Vec<syn::Expr> {
    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(mac.tokens.clone())
        .map(|expressions| expressions.into_iter().collect())
        .unwrap_or_default()
}
