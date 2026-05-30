use {
    crate::{
        diagnostics::lint::{
            run_lint_visitor, run_lint_visitor_on_functions, Applicability, Confidence,
            LintVisitor, Region,
        },
        diagnostics::{
            diagnostic_from_range, diagnostic_from_span, registry::AnchorDiagnosticKind,
        },
        document::{ParsedDocument, PdaConstraint, PdaSeeds, SymbolRange},
        solana::frameworks::{FrameworkContext, FrameworkId},
    },
    quote::ToTokens,
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

mod manual_close;
mod native_raw;
mod native_validation;
mod stale_cpi;

const BALANCE_TERMS: &[&str] = &[
    "amount", "balance", "deposit", "fee", "lamport", "reward", "stake", "supply", "total",
    "value", "withdraw",
];

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_framework(document, FrameworkContext::from_document(document))
}

pub fn collect_with_framework(
    document: &ParsedDocument,
    framework: FrameworkContext,
) -> Vec<Diagnostic> {
    let program_kind = ProgramKind::from_framework(framework);
    if program_kind == ProgramKind::Unknown {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    diagnostics.extend(unsafe_unwrap_diagnostics(document, program_kind));
    diagnostics.extend(unchecked_balance_arithmetic_diagnostics(
        document,
        program_kind,
    ));
    diagnostics.extend(non_canonical_pda_bump_diagnostics(document, program_kind));
    diagnostics.extend(native_raw::diagnostics(document, program_kind));
    diagnostics.extend(manual_close::diagnostics(document));
    diagnostics.extend(stale_cpi::diagnostics(document));
    diagnostics.extend(native_validation::diagnostics(document, program_kind));
    diagnostics.extend(instruction_data_bounds_diagnostics(document, program_kind));
    diagnostics.extend(pda_seed_collision_diagnostics(document));
    diagnostics
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProgramKind {
    Anchor,
    Pinocchio,
    NativeSolana,
    Unknown,
}

impl ProgramKind {
    fn from_framework(framework: FrameworkContext) -> Self {
        match framework.id() {
            FrameworkId::AnchorV1 | FrameworkId::AnchorV2Preview => Self::Anchor,
            FrameworkId::Pinocchio => Self::Pinocchio,
            FrameworkId::NativeSolana => Self::NativeSolana,
            FrameworkId::Unknown => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            ProgramKind::Anchor => "anchor",
            ProgramKind::Pinocchio => "pinocchio",
            ProgramKind::NativeSolana => "native-solana",
            ProgramKind::Unknown => "unknown",
        }
    }
}

fn unsafe_unwrap_diagnostics(
    document: &ParsedDocument,
    program_kind: ProgramKind,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        UnsafeUnwrapVisitor {
            program_kind,
            diagnostics: Vec::new(),
        },
    )
}

struct UnsafeUnwrapVisitor {
    program_kind: ProgramKind,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for UnsafeUnwrapVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/solana.code-quality.unsafe-unwrap";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for UnsafeUnwrapVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if matches!(method.as_str(), "unwrap" | "expect")
            && !is_infallible_try_into_receiver(&node.receiver)
        {
            self.diagnostics.push(unsafe_unwrap_diagnostic(
                self.program_kind,
                node.method.span(),
                &method,
            ));
        }
        visit::visit_expr_method_call(self, node);
    }
}

fn unsafe_unwrap_diagnostic(
    program_kind: ProgramKind,
    span: proc_macro2::Span,
    method: &str,
) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        format!(
            "Avoid `.{method}()` in Solana program code; return a typed error instead of panicking."
        ),
        Some(serde_json::json!({
            "rule": "unsafe-unwrap",
            "topic": "seagrass/solana.code-quality.unsafe-unwrap",
            "method": method,
            "programKind": program_kind.as_str(),
            "suggestion": "Replace the panic path with `ok_or(...) ?` for Option or `map_err(...) ?` for Result.",
            "absorbedFrom": "solana-mcp-official/programAutofixer",
        })),
    )
}

fn is_infallible_try_into_receiver(receiver: &syn::Expr) -> bool {
    match receiver {
        syn::Expr::MethodCall(method) if method.method == "try_into" => {
            is_infallible_try_into_source(&method.receiver)
        }
        syn::Expr::Paren(paren) => is_infallible_try_into_receiver(&paren.expr),
        syn::Expr::Group(group) => is_infallible_try_into_receiver(&group.expr),
        syn::Expr::Reference(reference) => is_infallible_try_into_receiver(&reference.expr),
        _ => false,
    }
}

fn is_infallible_try_into_source(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Index(index) => matches!(index.index.as_ref(), syn::Expr::Range(_)),
        syn::Expr::MethodCall(method) => matches!(
            method.method.to_string().as_str(),
            "to_le_bytes" | "to_be_bytes" | "to_ne_bytes"
        ),
        syn::Expr::Paren(paren) => is_infallible_try_into_source(&paren.expr),
        syn::Expr::Group(group) => is_infallible_try_into_source(&group.expr),
        syn::Expr::Reference(reference) => is_infallible_try_into_source(&reference.expr),
        _ => false,
    }
}

fn unchecked_balance_arithmetic_diagnostics(
    document: &ParsedDocument,
    program_kind: ProgramKind,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        UncheckedArithmeticVisitor {
            program_kind,
            diagnostics: Vec::new(),
        },
    )
}

struct UncheckedArithmeticVisitor {
    program_kind: ProgramKind,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for UncheckedArithmeticVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/solana.code-quality.unchecked-arithmetic";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for UncheckedArithmeticVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if is_unchecked_balance_arithmetic(node) {
            self.diagnostics.push(unchecked_arithmetic_diagnostic(
                self.program_kind,
                node.op.span(),
            ));
        }
        visit::visit_expr_binary(self, node);
    }
}

#[cfg(test)]
fn unchecked_arithmetic_scope() -> &'static [Region] {
    UncheckedArithmeticVisitor::SCOPE
}

fn unchecked_arithmetic_confidence() -> Confidence {
    UncheckedArithmeticVisitor::CONFIDENCE
}

fn unchecked_arithmetic_applicability() -> Applicability {
    UncheckedArithmeticVisitor::APPLICABILITY
}

fn unchecked_arithmetic_topic() -> &'static str {
    UncheckedArithmeticVisitor::TOPIC
}

fn unchecked_arithmetic_diagnostic(
    program_kind: ProgramKind,
    span: proc_macro2::Span,
) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Use checked arithmetic for balance, lamport, token, or amount math in Solana program code."
            .to_string(),
        Some(serde_json::json!({
            "rule": "unchecked-arithmetic",
            "confidence": unchecked_arithmetic_confidence().as_str(),
            "topic": unchecked_arithmetic_topic(),
            "applicability": unchecked_arithmetic_applicability().as_str(),
            "programKind": program_kind.as_str(),
            "suggestion": "Use `checked_add`, `checked_sub`, or `checked_mul` and convert overflow to a program error.",
            "absorbedFrom": "solana-mcp-official/programAutofixer",
        })),
    )
}

fn is_unchecked_balance_arithmetic(node: &syn::ExprBinary) -> bool {
    is_unchecked_arithmetic_operator(&node.op)
        && (expr_contains_balance_term(&node.left) || expr_contains_balance_term(&node.right))
}

fn is_unchecked_arithmetic_operator(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::Add(_)
            | syn::BinOp::Sub(_)
            | syn::BinOp::Mul(_)
            | syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
    )
}

fn expr_contains_balance_term(expr: &syn::Expr) -> bool {
    let tokens = expr.to_token_stream().to_string();
    tokens
        .split(|ch: char| !is_identifier_char(ch))
        .any(identifier_has_balance_term)
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn identifier_has_balance_term(identifier: &str) -> bool {
    identifier
        .split('_')
        .filter(|part| !part.is_empty())
        .any(|part| {
            let part = part.to_ascii_lowercase();
            BALANCE_TERMS.iter().any(|term| part == *term)
        })
}

fn non_canonical_pda_bump_diagnostics(
    document: &ParsedDocument,
    program_kind: ProgramKind,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        NonCanonicalPdaBumpVisitor {
            program_kind,
            diagnostics: Vec::new(),
        },
    )
}

struct NonCanonicalPdaBumpVisitor {
    program_kind: ProgramKind,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> LintVisitor<'ast> for NonCanonicalPdaBumpVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/solana.code-quality.bump-seed-canonicalization";

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for NonCanonicalPdaBumpVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(path) = pubkey_create_program_address_path(&node.func) {
            self.diagnostics.push(non_canonical_pda_bump_diagnostic(
                self.program_kind,
                path.segments
                    .last()
                    .map(|segment| segment.ident.span())
                    .unwrap_or_else(|| node.func.span()),
            ));
        }
        visit::visit_expr_call(self, node);
    }
}

fn pubkey_create_program_address_path(func: &syn::Expr) -> Option<&syn::Path> {
    let syn::Expr::Path(expr_path) = func else {
        return None;
    };
    let mut segments = expr_path.path.segments.iter().rev();
    let method = segments.next()?;
    let receiver = segments.next()?;

    (method.ident == "create_program_address" && receiver.ident == "Pubkey")
        .then_some(&expr_path.path)
}

fn non_canonical_pda_bump_diagnostic(
    program_kind: ProgramKind,
    span: proc_macro2::Span,
) -> Diagnostic {
    diagnostic_from_span(
        span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Verify the canonical PDA bump; `create_program_address` accepts any valid bump for the seeds."
            .to_string(),
        Some(serde_json::json!({
            "attack": "bump-seed-canonicalization",
            "topic": "seagrass/solana.code-quality.bump-seed-canonicalization",
            "corpusMode": "legacy-invariant",
            "versionPolicy": "Do not copy old Anchor templates; apply the invariant only to manual PDA derivation.",
            "programKind": program_kind.as_str(),
            "suggestion": "Prefer `Pubkey::find_program_address` and compare the expected bump, or use Anchor `seeds = [...]` with `bump`.",
            "absorbedFrom": "coral-xyz/sealevel-attacks",
        })),
    )
}

fn instruction_data_bounds_diagnostics(
    document: &ParsedDocument,
    program_kind: ProgramKind,
) -> Vec<Diagnostic> {
    run_lint_visitor_on_functions(document, |item_fn| {
        InstructionDataBoundsVisitor::new(program_kind, item_fn)
    })
}

struct InstructionDataBoundsVisitor {
    program_kind: ProgramKind,
    data_names: Vec<String>,
    first_unchecked_access: Option<DataAccessEvidence>,
    has_bounds_validation: bool,
}

#[derive(Clone)]
struct DataAccessEvidence {
    span: proc_macro2::Span,
    expression: String,
}

impl InstructionDataBoundsVisitor {
    fn new(program_kind: ProgramKind, item_fn: &syn::ItemFn) -> Self {
        Self {
            program_kind,
            data_names: instruction_data_parameter_names(item_fn),
            first_unchecked_access: None,
            has_bounds_validation: false,
        }
    }

    fn finish(self) -> Vec<Diagnostic> {
        self.first_unchecked_access
            .filter(|_| !self.has_bounds_validation)
            .map(|evidence| instruction_data_bounds_diagnostic(self.program_kind, evidence))
            .into_iter()
            .collect()
    }

    fn knows_data_ident(&self, ident: &syn::Ident) -> bool {
        self.data_names
            .iter()
            .any(|candidate| ident == candidate.as_str())
    }
}

impl<'ast> LintVisitor<'ast> for InstructionDataBoundsVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Heuristic;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = "seagrass/solana.code-quality.instruction-data-bounds";

    fn finish(self) -> Vec<Diagnostic> {
        InstructionDataBoundsVisitor::finish(self)
    }
}

impl<'ast> Visit<'ast> for InstructionDataBoundsVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(local_name) = local_ident(node).filter(|_| local_reads_account_data(node)) {
            self.data_names.push(local_name.to_string());
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_index(&mut self, node: &'ast syn::ExprIndex) {
        if let Some(data_name) = path_ident(&node.expr)
            .filter(|name| self.knows_data_ident(name))
            .filter(|_| !matches!(node.index.as_ref(), syn::Expr::Range(_)))
        {
            self.first_unchecked_access
                .get_or_insert_with(|| DataAccessEvidence {
                    span: node.expr.span(),
                    expression: data_name.to_string(),
                });
        }
        visit::visit_expr_index(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if called_ident(&node.func).is_some_and(is_bounds_validation_helper) {
            self.has_bounds_validation = true;
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if path_ident(&node.receiver)
            .filter(|name| self.knows_data_ident(name))
            .is_some()
            && matches!(
                node.method.to_string().as_str(),
                "len" | "get" | "split_at_checked"
            )
        {
            self.has_bounds_validation = true;
        }
        visit::visit_expr_method_call(self, node);
    }
}

fn instruction_data_bounds_diagnostic(
    program_kind: ProgramKind,
    evidence: DataAccessEvidence,
) -> Diagnostic {
    diagnostic_from_span(
        evidence.span,
        AnchorDiagnosticKind::SolanaCodeQuality,
        "Instruction/account data is indexed without a visible bounds check.".to_string(),
        Some(serde_json::json!({
            "quickfix": "use-checked-data-access",
            "attack": "instruction-data-bounds",
            "topic": "seagrass/solana.code-quality.instruction-data-bounds",
            "corpusMode": "program-autofixer-invariant",
            "evidenceSource": "native-data-index-scan",
            "configKey": "security.instructionDataBounds",
            "dataExpression": evidence.expression,
            "strictNative": true,
            "programKind": program_kind.as_str(),
            "suggestion": "Check input length first or use `.get(..)`/checked split helpers before indexing.",
            "absorbedFrom": "solana-mcp-official/programAutofixer",
        })),
    )
}

fn instruction_data_parameter_names(item_fn: &syn::ItemFn) -> Vec<String> {
    item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => pat_ident(&pat_type.pat),
            syn::FnArg::Receiver(_) => None,
        })
        .filter(|name| {
            matches!(
                name.to_string().as_str(),
                "instruction_data" | "_instruction_data" | "data"
            )
        })
        .map(ToString::to_string)
        .collect()
}

fn local_ident(local: &syn::Local) -> Option<&syn::Ident> {
    pat_ident(&local.pat)
}

fn pat_ident(pat: &syn::Pat) -> Option<&syn::Ident> {
    match pat {
        syn::Pat::Ident(ident) => Some(&ident.ident),
        syn::Pat::Reference(reference) => pat_ident(&reference.pat),
        syn::Pat::Type(pat_type) => pat_ident(&pat_type.pat),
        _ => None,
    }
}

fn local_reads_account_data(local: &syn::Local) -> bool {
    local
        .init
        .as_ref()
        .is_some_and(|init| expr_reads_account_data(&init.expr))
}

fn expr_reads_account_data(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(method) => {
            matches!(
                method.method.to_string().as_str(),
                "try_borrow_data" | "try_borrow_mut_data" | "borrow" | "borrow_mut"
            ) || expr_reads_account_data(&method.receiver)
        }
        syn::Expr::Field(field) => {
            matches!(&field.member, syn::Member::Named(ident) if ident == "data")
        }
        syn::Expr::Reference(reference) => expr_reads_account_data(&reference.expr),
        syn::Expr::Paren(paren) => expr_reads_account_data(&paren.expr),
        syn::Expr::Group(group) => expr_reads_account_data(&group.expr),
        _ => false,
    }
}

fn path_ident(expr: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(path) = expr else {
        return None;
    };
    path.path.segments.last().map(|segment| &segment.ident)
}

fn called_ident(func: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(expr_path) = func else {
        return None;
    };
    expr_path.path.segments.last().map(|segment| &segment.ident)
}

fn is_bounds_validation_helper(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "assert_len" | "check_len" | "require_len" | "validate_instruction_data" | "checked_data"
    )
}

const MIN_DYNAMIC_PDA_SEEDS_WITHOUT_DOMAIN: usize = 2;
const DYNAMIC_PDA_SEED_MARKERS: &[&str] = &[".as_ref()", ".to_le_bytes()", ".key()"];
const STATIC_PDA_DOMAIN_PREFIXES: &[&str] = &["b\"", "b'", "br\"", "br#"];

fn pda_seed_collision_diagnostics(document: &ParsedDocument) -> Vec<Diagnostic> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| accounts.fields.iter())
        .filter_map(pda_seed_collision_diagnostic)
        .collect()
}

fn pda_seed_collision_diagnostic(field: &SymbolRange) -> Option<Diagnostic> {
    let pda = field.pda_constraint.as_ref()?;
    pda_seeds_have_collision_risk(pda).then(|| {
        diagnostic_from_range(
            pda_constraint_range(field),
            AnchorDiagnosticKind::SolanaCodeQuality,
            "PDA seeds use multiple dynamic values without a static domain separator; this can make seed domains ambiguous.".to_string(),
            Some(serde_json::json!({
                "attack": "pda-seed-collision",
                "topic": "seagrass/solana.code-quality.pda-seed-collision",
                "corpusMode": "program-autofixer-invariant",
                "quickfix": "add-static-pda-domain-seed",
                "evidenceSource": "parsed-anchor-seeds",
                "configKey": "security.pdaSeedCollision",
                "strictNative": false,
                "suggestion": "Add a fixed namespace seed such as `b\"vault\"` before dynamic seeds and keep variable-length seeds unambiguous.",
                "absorbedFrom": "solana-mcp-official/programAutofixer",
            })),
        )
    })
}

fn pda_constraint_range(field: &SymbolRange) -> Range {
    field
        .account_constraints
        .iter()
        .find(|constraint| constraint.pda.is_some())
        .map(|constraint| constraint.range)
        .unwrap_or(field.selection_range)
}

fn pda_seeds_have_collision_risk(pda: &PdaConstraint) -> bool {
    let PdaSeeds::List(seeds) = &pda.seeds else {
        return false;
    };

    !seeds.iter().any(|seed| is_static_pda_domain_seed(seed))
        && seeds
            .iter()
            .filter(|seed| is_dynamic_pda_seed(seed))
            .count()
            >= MIN_DYNAMIC_PDA_SEEDS_WITHOUT_DOMAIN
}

fn is_static_pda_domain_seed(seed: &str) -> bool {
    STATIC_PDA_DOMAIN_PREFIXES
        .iter()
        .any(|prefix| seed.starts_with(prefix))
}

fn is_dynamic_pda_seed(seed: &str) -> bool {
    DYNAMIC_PDA_SEED_MARKERS
        .iter()
        .any(|marker| seed.contains(marker))
}

#[cfg(test)]
mod tests;
