use {
    crate::{
        collections::Trie,
        diagnostics::lint::{
            run_lint_visitor, run_lint_visitor_on_functions, Applicability, Confidence,
            FunctionBody, LintVisitor, Region,
        },
        diagnostics::{
            diagnostic_from_range, diagnostic_from_span, registry::AnchorDiagnosticKind,
        },
        document::{ParsedDocument, PdaSeeds, SymbolRange},
        solana::frameworks::{FrameworkContext, FrameworkId},
        syntax::{expr_path_last_ident, member_is_named},
    },
    quote::ToTokens,
    seagrass_framework::diagnostics::FrameworkDocument,
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

mod manual_close;
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
    diagnostics.extend(framework_crate_diagnostics(document, framework));
    diagnostics.extend(manual_close::diagnostics(document));
    diagnostics.extend(stale_cpi::diagnostics(document));
    diagnostics.extend(instruction_data_bounds_diagnostics(document, program_kind));
    diagnostics.extend(pda_seed_collision_diagnostics(document));
    diagnostics
}

fn framework_crate_diagnostics(
    document: &ParsedDocument,
    framework: FrameworkContext,
) -> Vec<Diagnostic> {
    let framework_document = FrameworkDocument::new(document.source(), document.syntax());
    match framework.id() {
        FrameworkId::Pinocchio => seagrass_pinocchio::diagnostics(framework_document),
        FrameworkId::NativeSolana => seagrass_native::diagnostics(framework_document),
        FrameworkId::AnchorV1 | FrameworkId::AnchorV2Preview | FrameworkId::Unknown => Vec::new(),
    }
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
        .split(|ch: char| !crate::syntax::is_ascii_identifier_char(ch))
        .any(identifier_has_balance_term)
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
    run_lint_visitor_on_functions(document, |function| {
        InstructionDataBoundsVisitor::new(program_kind, function)
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
    fn new(program_kind: ProgramKind, function: FunctionBody<'_>) -> Self {
        Self {
            program_kind,
            data_names: instruction_data_parameter_names(function.inputs),
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
        if expr_path_last_ident(&node.func).is_some_and(is_bounds_validation_helper) {
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

fn instruction_data_parameter_names(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> Vec<String> {
    inputs
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
        syn::Expr::Field(field) => member_is_named(&field.member, "data"),
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

fn is_bounds_validation_helper(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "assert_len" | "check_len" | "require_len" | "validate_instruction_data" | "checked_data"
    )
}

/// A diagnostic site for a PDA seed sequence found in one accounts struct field.
///
/// The `range` points at the `#[account(seeds = [...])]` attribute so the
/// editor can highlight the exact location.  `display` is a human-readable
/// representation of the seed list used in the warning message.
struct PdaSeedSite {
    range: Range,
    display: String,
}

/// Build trie-based PDA prefix-collision diagnostics for the whole document.
///
/// The algorithm:
///  1. For every accounts-struct field that carries a `seeds = [...]` constraint,
///     compute the *literal-byte prefix* of the concatenated seed sequence — that
///     is, the bytes contributed by leading byte-string literals before the first
///     non-literal (opaque) component.
///  2. Insert those byte sequences into a `Trie<u8, PdaSeedSite>`.  Fields whose
///     literal prefix is empty (because the first seed is already opaque) are
///     skipped: no static information is available, so we cannot flag a false positive.
///  3. Call `prefix_collisions()`.  Each reported pair represents a *genuine*
///     Zellic-class ambiguity: the shorter concatenated-literal sequence is a proper
///     byte prefix of the longer one, so the Solana runtime cannot distinguish which
///     PDA was intended when both anchor keys overlap.
///  4. Emit one WARNING per colliding site (both the shorter and the longer).
fn pda_seed_collision_diagnostics(document: &ParsedDocument) -> Vec<Diagnostic> {
    let mut trie: Trie<u8, PdaSeedSite> = Trie::new();

    for accounts_struct in document.symbols().accounts_structs.values() {
        for field in &accounts_struct.fields {
            let Some(pda) = &field.pda_constraint else {
                continue;
            };
            let PdaSeeds::List(seeds) = &pda.seeds else {
                continue;
            };

            let literal_prefix = concatenated_literal_byte_prefix(seeds);

            // An empty literal prefix carries no static information — skip to
            // avoid false positives on purely-dynamic seed lists.
            if literal_prefix.is_empty() {
                continue;
            }

            let site = PdaSeedSite {
                range: pda_constraint_range(field),
                display: format!("[{}]", seeds.join(", ")),
            };
            // `insert` silently overwrites duplicate keys; for collision detection
            // what matters is that the key exists in the trie, not which site wins.
            trie.insert(literal_prefix, site);
        }
    }

    trie.prefix_collisions()
        .into_iter()
        .flat_map(|(shorter_key, longer_key)| {
            // Both sites are guaranteed present because `prefix_collisions` only
            // reports keys that were actually inserted.
            let shorter_site = trie
                .get(shorter_key.iter().copied())
                .expect("shorter site must be in trie");
            let longer_site = trie
                .get(longer_key.iter().copied())
                .expect("longer site must be in trie");

            [
                pda_seed_prefix_collision_diagnostic(shorter_site, &longer_site.display),
                pda_seed_prefix_collision_diagnostic(longer_site, &shorter_site.display),
            ]
        })
        .collect()
}

fn pda_seed_prefix_collision_diagnostic(site: &PdaSeedSite, other_display: &str) -> Diagnostic {
    diagnostic_from_range(
        site.range,
        AnchorDiagnosticKind::SolanaCodeQuality,
        format!(
            "PDA seed prefix collision: the literal-byte prefix of seeds {} is a prefix of {}; \
             the runtime cannot distinguish which PDA was intended.",
            site.display, other_display,
        ),
        Some(serde_json::json!({
            "attack": "pda-seed-collision",
            "topic": "seagrass/solana.code-quality.pda-seed-collision",
            "corpusMode": "program-autofixer-invariant",
            "quickfix": "add-static-pda-domain-seed",
            "evidenceSource": "trie-prefix-collision",
            "configKey": "security.pdaSeedCollision",
            "strictNative": false,
            "suggestion": "Ensure each PDA uses a distinct fixed-length namespace seed so no seed list is a byte-prefix of another.",
            "absorbedFrom": "solana-mcp-official/programAutofixer",
        })),
    )
}

fn pda_constraint_range(field: &SymbolRange) -> Range {
    field
        .account_constraints
        .iter()
        .find(|constraint| constraint.pda.is_some())
        .map(|constraint| constraint.range)
        .unwrap_or(field.selection_range)
}

/// Return the bytes contributed by the leading byte-string-literal seeds,
/// stopping at the first non-literal component.
///
/// For example:
/// - `["b\"pr\"", "user.key().as_ref()"]` → `[b'p', b'r']`
/// - `["b\"product\""]`                   → `[b'p', b'r', b'o', b'd', b'u', b'c', b't']`
/// - `["user.key().as_ref()"]`             → `[]`  (stops immediately)
///
/// Only plain `b"..."` literals are parsed; raw literals and byte-char literals
/// are treated as opaque (conservative — prefer silence over a false positive).
fn concatenated_literal_byte_prefix(seeds: &[String]) -> Vec<u8> {
    let mut result = Vec::new();
    for seed in seeds {
        match parse_byte_string_literal(seed) {
            Some(bytes) => result.extend(bytes),
            // First opaque component — stop here; we have no static information
            // about the bytes it contributes at runtime.
            None => break,
        }
    }
    result
}

/// Parse a `b"..."` byte-string literal into its raw bytes.
///
/// Returns `None` when the token is not a plain byte-string literal (raw
/// literals, byte-char literals, or non-literal expressions).  Being
/// conservative here prevents false positives: if we cannot determine the
/// exact bytes a seed contributes, we stop the literal prefix rather than
/// guessing.
///
/// Supported escape sequences: `\\`, `\"`, `\n`, `\r`, `\t`, `\0`,
/// `\x??` (two-digit hex).  Unicode escapes (`\u{...}`) are not byte-valid
/// and will cause the function to return `None`.
fn parse_byte_string_literal(token: &str) -> Option<Vec<u8>> {
    // Must start with b" and end with " (plain, non-raw byte string).
    let inner = token.strip_prefix("b\"")?;
    let inner = inner.strip_suffix('\"')?;

    let mut bytes = Vec::new();
    let mut chars = inner.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            // Guard: only accept printable ASCII (the common case for PDA seeds).
            // Non-ASCII byte-string content is unusual and we treat it as opaque.
            if ch.is_ascii() {
                bytes.push(ch as u8);
            } else {
                return None;
            }
            continue;
        }

        // Escape sequence
        let escaped = chars.next()?;
        let byte = match escaped {
            '\\' => b'\\',
            '"' => b'"',
            'n' => b'\n',
            'r' => b'\r',
            't' => b'\t',
            '0' => b'\0',
            'x' => {
                // Two-digit hex escape: \xFF
                let hi = chars.next()?.to_digit(16)? as u8;
                let lo = chars.next()?.to_digit(16)? as u8;
                hi * 16 + lo
            }
            // Unicode escapes (\u{...}) are not valid in byte strings by Rust's
            // grammar, but treat anything unrecognised as opaque.
            _ => return None,
        };
        bytes.push(byte);
    }

    Some(bytes)
}

#[cfg(test)]
mod tests;
