use {
    crate::{
        diagnostics::{solana_code_quality_from_span, FrameworkDocument},
        lint::{run_lint_visitor_on_functions, Applicability, Confidence, LintVisitor, Region},
        FrameworkKind,
    },
    quote::ToTokens,
    std::collections::{HashMap, HashSet},
    syn::{
        parse::Parser,
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::Diagnostic,
};

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
const WRITABLE_VALIDATION_HELPERS: &[&str] = &[
    "assert_writable",
    "check_writable",
    "require_writable",
    "validate_writable",
];
const INSTRUCTION_PROGRAM_ID_CONSTRUCTORS: &[&str] =
    &["new_with_bincode", "new_with_borsh", "new_with_bytes"];
const SIGNER_ACCOUNT_META_CONSTRUCTORS: &[&str] = &["new", "new_readonly"];
const SIGNER_INSTRUCTION_ACCOUNT_CONSTRUCTORS: &[&str] = &["readonly_signer", "writable_signer"];
const WRITABLE_ACCOUNT_META_CONSTRUCTORS: &[&str] = &["new"];
const WRITABLE_INSTRUCTION_ACCOUNT_CONSTRUCTORS: &[&str] = &["writable", "writable_signer"];

pub(super) fn diagnostics(
    document: FrameworkDocument<'_>,
    framework_kind: FrameworkKind,
) -> Vec<Diagnostic> {
    if document.syntax().items.is_empty() {
        return Vec::new();
    }

    run_lint_visitor_on_functions(document, |_| {
        NativeAccountValidationVisitor::new(framework_kind)
    })
}

struct NativeAccountValidationVisitor {
    framework_kind: FrameworkKind,
    signer_meta: Vec<SignerMetaEvidence>,
    writable_meta: Vec<WritableMetaEvidence>,
    cpi_programs: Vec<ProgramIdEvidence>,
    signer_validations: AccountValidationSet,
    writable_validations: AccountValidationSet,
    program_id_validations: ExpressionValidationSet,
    account_origins: HashMap<String, usize>,
    saw_invoke_signed: bool,
}

impl NativeAccountValidationVisitor {
    fn new(framework_kind: FrameworkKind) -> Self {
        Self {
            framework_kind,
            signer_meta: Vec::new(),
            writable_meta: Vec::new(),
            cpi_programs: Vec::new(),
            signer_validations: AccountValidationSet::default(),
            writable_validations: AccountValidationSet::default(),
            program_id_validations: ExpressionValidationSet::default(),
            account_origins: HashMap::new(),
            saw_invoke_signed: false,
        }
    }

    fn finish(self) -> Vec<Diagnostic> {
        let Self {
            framework_kind,
            signer_meta,
            writable_meta,
            cpi_programs,
            signer_validations,
            writable_validations,
            program_id_validations,
            account_origins: _,
            saw_invoke_signed,
        } = self;

        let signer = signer_meta
            .into_iter()
            .filter(|evidence| {
                !saw_invoke_signed
                    && !signer_validations.contains(
                        evidence.account_expression.as_deref(),
                        evidence.account_index,
                    )
            })
            .map(|evidence| signer_authorization_diagnostic(framework_kind, evidence));
        let writable = writable_meta
            .into_iter()
            .filter(|evidence| {
                !writable_validations.contains(
                    evidence.account_expression.as_deref(),
                    evidence.account_index,
                )
            })
            .map(|evidence| writable_account_diagnostic(framework_kind, evidence));
        let cpi = cpi_programs
            .into_iter()
            .filter(|evidence| evidence.dynamic)
            .filter(|evidence| !program_id_validations.contains(evidence.expression.as_deref()))
            .map(|evidence| arbitrary_cpi_diagnostic(framework_kind, evidence));
        signer.into_iter().chain(writable).chain(cpi).collect()
    }

    fn record_local_account_origin(&mut self, node: &syn::Local) {
        let Some(init) = node.init.as_ref() else {
            return;
        };
        let Some(name) = local_ident_name(node) else {
            return;
        };
        if let Some(index) = account_index_from_expr(&init.expr) {
            self.account_origins.insert(name, index);
        }
    }

    fn account_index_for_expr(&self, expr: &syn::Expr) -> Option<usize> {
        account_index_from_expr(expr).or_else(|| {
            account_alias_from_expr(expr)
                .and_then(|alias| self.account_origins.get(alias.as_str()).copied())
        })
    }

    fn record_signer_validation(&mut self, expr: Option<&syn::Expr>) {
        self.signer_validations.insert(expr, self.framework_kind);
        if let Some(index) = expr.and_then(|expr| self.account_index_for_expr(expr)) {
            self.signer_validations.insert_index(index);
        }
    }

    fn record_writable_validation(&mut self, expr: Option<&syn::Expr>) {
        self.writable_validations.insert(expr, self.framework_kind);
        if let Some(index) = expr.and_then(|expr| self.account_index_for_expr(expr)) {
            self.writable_validations.insert_index(index);
        }
    }

    fn account_index_for_call_first_arg(&self, node: &syn::ExprCall) -> Option<usize> {
        node.args
            .first()
            .and_then(|expr| self.account_index_for_expr(expr))
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
struct WritableMetaEvidence {
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

#[derive(Default)]
struct AccountValidationSet {
    any: bool,
    expressions: HashSet<String>,
    indexes: HashSet<usize>,
}

impl AccountValidationSet {
    fn insert(&mut self, expr: Option<&syn::Expr>, framework_kind: FrameworkKind) {
        let Some(expr) = expr else {
            self.any = true;
            return;
        };
        self.expressions
            .insert(account_guard_expression(expr, framework_kind));
        if let Some(index) = account_index_from_expr(expr) {
            self.indexes.insert(index);
        }
    }

    fn insert_index(&mut self, index: usize) {
        self.indexes.insert(index);
    }

    fn contains(&self, expression: Option<&str>, index: Option<usize>) -> bool {
        self.any
            || expression.is_some_and(|expression| self.expressions.contains(expression))
            || index.is_some_and(|index| self.indexes.contains(&index))
    }
}

#[derive(Default)]
struct ExpressionValidationSet {
    any: bool,
    expressions: HashSet<String>,
}

impl ExpressionValidationSet {
    fn insert(&mut self, expr: Option<&syn::Expr>) {
        let Some(expr) = expr else {
            self.any = true;
            return;
        };
        self.expressions.insert(program_id_expression(expr));
    }

    fn contains(&self, expression: Option<&str>) -> bool {
        self.any || expression.is_some_and(|expression| self.expressions.contains(expression))
    }
}

impl<'ast> Visit<'ast> for NativeAccountValidationVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_local(&mut self, node: &'ast syn::Local) {
        self.record_local_account_origin(node);
        visit::visit_local(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(mut evidence) = signer_meta_evidence(node, self.framework_kind) {
            evidence.account_index = evidence
                .account_index
                .or_else(|| self.account_index_for_call_first_arg(node));
            self.signer_meta.push(evidence);
        }
        if let Some(mut evidence) = writable_meta_evidence(node, self.framework_kind) {
            evidence.account_index = evidence
                .account_index
                .or_else(|| self.account_index_for_call_first_arg(node));
            self.writable_meta.push(evidence);
        }
        if let Some(evidence) = instruction_constructor_program_id_evidence(node) {
            self.cpi_programs.push(evidence);
        }

        if is_invoke_signed_call(&node.func) {
            self.saw_invoke_signed = true;
        }

        if let Some(ident) = called_ident(&node.func) {
            if ident_matches_any(ident, SIGNER_VALIDATION_HELPERS) {
                self.record_signer_validation(node.args.first());
            }
            if ident_matches_any(ident, PROGRAM_ID_VALIDATION_HELPERS) {
                self.program_id_validations.insert(node.args.first());
            }
            if ident_matches_any(ident, WRITABLE_VALIDATION_HELPERS) {
                self.record_writable_validation(node.args.first());
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
            self.record_signer_validation(Some(node.receiver.as_ref()));
        }
        if method == "is_writable" {
            self.record_writable_validation(Some(node.receiver.as_ref()));
        }
        if method == "check_id" {
            self.program_id_validations.insert(node.args.first());
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if member_is_named(&node.member, "is_signer") {
            self.record_signer_validation(Some(node.base.as_ref()));
        }
        if member_is_named(&node.member, "is_writable") {
            self.record_writable_validation(Some(node.base.as_ref()));
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_))
            && (expr_contains_ident(&node.left, "program_id")
                || expr_contains_ident(&node.right, "program_id"))
        {
            self.program_id_validations.insert(Some(node.left.as_ref()));
            self.program_id_validations
                .insert(Some(node.right.as_ref()));
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if instruction_struct_has_program_id(&node.path) {
            if let Some(field) = node
                .fields
                .iter()
                .find(|field| member_is_named(&field.member, "program_id"))
            {
                self.cpi_programs.push(ProgramIdEvidence {
                    span: field.expr.span(),
                    expression: Some(program_id_expression(&field.expr)),
                    dynamic: is_dynamic_program_id(&field.expr),
                });
            }
        }
        visit::visit_expr_struct(self, node);
    }
}

fn instruction_struct_has_program_id(path: &syn::Path) -> bool {
    path.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "Instruction" | "InstructionView"
        )
    })
}

fn signer_authorization_diagnostic(
    framework_kind: FrameworkKind,
    evidence: SignerMetaEvidence,
) -> Diagnostic {
    solana_code_quality_from_span(
        evidence.span,
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
            "programKind": framework_kind.program_kind_label(),
            "suggestion": "Check `account.is_signer` before using an account as an instruction signer.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn writable_account_diagnostic(
    framework_kind: FrameworkKind,
    evidence: WritableMetaEvidence,
) -> Diagnostic {
    solana_code_quality_from_span(
        evidence.span,
        "Writable CPI account is used without a visible writable check.".to_string(),
        Some(serde_json::json!({
            "quickfix": "add-writable-check",
            "attack": "writable-account",
            "topic": "seagrass/security.writable-account",
            "corpusMode": "legacy-invariant",
            "evidenceSource": "native-account-meta-scan",
            "accountIndex": evidence.account_index,
            "accountExpression": evidence.account_expression,
            "configKey": "security.writableAccounts",
            "strictNative": true,
            "programKind": framework_kind.program_kind_label(),
            "suggestion": "Check the account is writable before passing it as writable CPI metadata.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn arbitrary_cpi_diagnostic(
    framework_kind: FrameworkKind,
    evidence: ProgramIdEvidence,
) -> Diagnostic {
    solana_code_quality_from_span(
        evidence.span,
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
            "programKind": framework_kind.program_kind_label(),
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

fn signer_meta_evidence(
    node: &syn::ExprCall,
    framework_kind: FrameworkKind,
) -> Option<SignerMetaEvidence> {
    if is_account_meta_signer_constructor_call(&node.func) {
        if !second_arg_is_true(node) {
            return None;
        }
        return signer_evidence_from_first_arg(node, framework_kind);
    }
    if is_instruction_account_signer_constructor_call(&node.func) {
        return signer_evidence_from_first_arg(node, framework_kind);
    }
    None
}

fn signer_evidence_from_first_arg(
    node: &syn::ExprCall,
    framework_kind: FrameworkKind,
) -> Option<SignerMetaEvidence> {
    let account_expression = node
        .args
        .first()
        .map(|expr| signer_guard_expression(expr, framework_kind));
    Some(SignerMetaEvidence {
        span: node.func.span(),
        account_index: node.args.first().and_then(account_index_from_expr),
        account_expression,
    })
}

fn signer_guard_expression(expr: &syn::Expr, framework_kind: FrameworkKind) -> String {
    account_guard_expression(expr, framework_kind)
}

fn writable_meta_evidence(
    node: &syn::ExprCall,
    framework_kind: FrameworkKind,
) -> Option<WritableMetaEvidence> {
    if is_account_meta_writable_constructor_call(&node.func)
        || is_instruction_account_writable_constructor_call(&node.func)
    {
        return writable_evidence_from_first_arg(node, framework_kind);
    }
    None
}

fn writable_evidence_from_first_arg(
    node: &syn::ExprCall,
    framework_kind: FrameworkKind,
) -> Option<WritableMetaEvidence> {
    let account_expression = node
        .args
        .first()
        .map(|expr| account_guard_expression(expr, framework_kind));
    Some(WritableMetaEvidence {
        span: node.func.span(),
        account_index: node.args.first().and_then(account_index_from_expr),
        account_expression,
    })
}

fn account_guard_expression(expr: &syn::Expr, framework_kind: FrameworkKind) -> String {
    if framework_kind == FrameworkKind::Pinocchio {
        return pinocchio_account_expression(expr).unwrap_or_else(|| expr_text(expr));
    }
    native_account_expression(expr).unwrap_or_else(|| expr_text(expr))
}

fn native_account_expression(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Field(field) if member_is_named(&field.member, "key") => {
            Some(expr_text(&field.base))
        }
        syn::Expr::MethodCall(method) if method.method == "key" => {
            Some(expr_text(&method.receiver))
        }
        syn::Expr::Unary(unary) => native_account_expression(&unary.expr),
        syn::Expr::Reference(reference) => native_account_expression(&reference.expr),
        syn::Expr::Paren(paren) => native_account_expression(&paren.expr),
        syn::Expr::Group(group) => native_account_expression(&group.expr),
        _ => None,
    }
}

fn pinocchio_account_expression(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method) if method.method == "key" || method.method == "address" => {
            Some(expr_text(&method.receiver))
        }
        syn::Expr::Reference(reference) => pinocchio_account_expression(&reference.expr),
        syn::Expr::Paren(paren) => pinocchio_account_expression(&paren.expr),
        syn::Expr::Group(group) => pinocchio_account_expression(&group.expr),
        _ => None,
    }
}

fn is_account_meta_signer_constructor_call(func: &syn::Expr) -> bool {
    SIGNER_ACCOUNT_META_CONSTRUCTORS
        .iter()
        .any(|constructor| path_ends_with(func, &["AccountMeta", constructor]))
}

fn is_instruction_account_signer_constructor_call(func: &syn::Expr) -> bool {
    SIGNER_INSTRUCTION_ACCOUNT_CONSTRUCTORS
        .iter()
        .any(|constructor| path_ends_with(func, &["InstructionAccount", constructor]))
}

fn is_account_meta_writable_constructor_call(func: &syn::Expr) -> bool {
    WRITABLE_ACCOUNT_META_CONSTRUCTORS
        .iter()
        .any(|constructor| path_ends_with(func, &["AccountMeta", constructor]))
}

fn is_instruction_account_writable_constructor_call(func: &syn::Expr) -> bool {
    WRITABLE_INSTRUCTION_ACCOUNT_CONSTRUCTORS
        .iter()
        .any(|constructor| path_ends_with(func, &["InstructionAccount", constructor]))
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

fn local_ident_name(local: &syn::Local) -> Option<String> {
    match &local.pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        syn::Pat::Reference(reference) => pat_ident_name(&reference.pat),
        syn::Pat::Type(pat_type) => pat_ident_name(&pat_type.pat),
        _ => None,
    }
}

fn pat_ident_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        syn::Pat::Reference(reference) => pat_ident_name(&reference.pat),
        syn::Pat::Type(pat_type) => pat_ident_name(&pat_type.pat),
        _ => None,
    }
}

fn account_alias_from_expr(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(expr_path) => expr_path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => account_alias_from_expr(&field.base),
        syn::Expr::MethodCall(method_call) => account_alias_from_expr(&method_call.receiver),
        syn::Expr::Unary(unary) => account_alias_from_expr(&unary.expr),
        syn::Expr::Reference(reference) => account_alias_from_expr(&reference.expr),
        syn::Expr::Paren(paren) => account_alias_from_expr(&paren.expr),
        syn::Expr::Group(group) => account_alias_from_expr(&group.expr),
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
