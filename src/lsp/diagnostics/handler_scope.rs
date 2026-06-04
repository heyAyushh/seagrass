use {
    super::{
        diagnostic_from_range,
        lint::{run_lint_visitor, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    crate::{
        document::ParsedDocument,
        lsp::scope::{has_attr, item_fn_has_anchor_context_arg, TextHandlerScope},
        range::{byte_offset_at, range_from_span},
    },
    std::collections::{BTreeSet, HashSet},
    syn::{
        visit::{self, Visit},
        BinOp, Expr, ExprCall, ExprPath, FnArg, ItemFn, ItemMod, Pat, PathArguments,
    },
    tower_lsp::lsp_types::{Diagnostic, Position, Range},
};

const TOPIC: &str = "seagrass/anchor.account.usage";
const REASON: &str = "unresolved-handler-identifier";
const EVIDENCE_SOURCE: &str = "parsed-anchor-handler-scope";
const RECOVERED_EVIDENCE_SOURCE: &str = "recovered-anchor-handler-scope";
const KNOWN_SINGLE_SEGMENT_VALUES: &[&str] = &["self", "crate", "super"];

pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    run_lint_visitor(document, HandlerScopeVisitor::new(document))
}

pub(super) fn parse_error_unresolved_identifier_diagnostic(
    source: &str,
    range: Range,
) -> Option<Diagnostic> {
    if !crate::solana::frameworks::source_has_anchor_framework_hint(source) {
        return None;
    }
    let identifier = parse_error_identifier(source, range)?;
    if !crate::lsp::scope::handler_identifier_should_be_resolved(&identifier)
        || !parse_error_identifier_is_value(source, range)
    {
        return None;
    }
    let scope = RecoveredHandlerScope::at_position(source, range.start)?;
    if scope.identifier_resolves(&identifier) {
        return None;
    }

    Some(unresolved_handler_identifier_diagnostic(
        &identifier,
        range,
        scope.visible_candidates(),
        RECOVERED_EVIDENCE_SOURCE,
    ))
}

struct HandlerScopeVisitor {
    global_values: HashSet<String>,
    scopes: ScopeStack,
    in_program_module: bool,
    diagnostics: Vec<Diagnostic>,
    emitted: HashSet<(String, u32, u32)>,
}

impl HandlerScopeVisitor {
    fn new(document: &ParsedDocument) -> Self {
        Self {
            global_values: global_values(document),
            scopes: ScopeStack::default(),
            in_program_module: false,
            diagnostics: Vec::new(),
            emitted: HashSet::new(),
        }
    }

    fn visit_anchor_function(&mut self, item_fn: &ItemFn) {
        self.scopes.push();
        self.declare_function_inputs(item_fn);
        self.visit_block(&item_fn.block);
        self.scopes.pop();
    }

    fn declare_function_inputs(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            match input {
                FnArg::Receiver(_) => self.scopes.declare("self"),
                FnArg::Typed(pat_type) => self.scopes.declare_pat(&pat_type.pat),
            }
        }
    }

    fn report_unresolved_identifier(&mut self, identifier: &str, range: Range) {
        let key = (
            identifier.to_string(),
            range.start.line,
            range.start.character,
        );
        if !self.emitted.insert(key) {
            return;
        }

        self.diagnostics
            .push(unresolved_handler_identifier_diagnostic(
                identifier,
                range,
                self.visible_candidates(),
                EVIDENCE_SOURCE,
            ));
    }

    fn report_unresolved_path_identifier(&mut self, path: &ExprPath) {
        let Some(identifier) = bare_value_identifier(path) else {
            return;
        };
        if crate::lsp::scope::handler_identifier_should_be_resolved(&identifier)
            && !self.identifier_resolves(&identifier)
        {
            self.report_unresolved_identifier(
                &identifier,
                range_from_span(path.path.segments[0].ident.span()),
            );
        }
    }

    fn visit_condition_with_pattern_scope(&mut self, expr: &Expr) {
        match expr {
            Expr::Let(expr_let) => {
                self.visit_expr(&expr_let.expr);
                self.scopes.declare_pat(&expr_let.pat);
            }
            Expr::Binary(binary) if matches!(binary.op, BinOp::And(_)) => {
                self.visit_condition_with_pattern_scope(&binary.left);
                self.visit_condition_with_pattern_scope(&binary.right);
            }
            Expr::Group(group) => self.visit_condition_with_pattern_scope(&group.expr),
            Expr::Paren(paren) => self.visit_condition_with_pattern_scope(&paren.expr),
            _ => self.visit_expr(expr),
        }
    }

    fn identifier_resolves(&self, identifier: &str) -> bool {
        self.scopes.contains(identifier)
            || self.global_values.contains(identifier)
            || KNOWN_SINGLE_SEGMENT_VALUES.contains(&identifier)
    }

    fn visible_candidates(&self) -> Vec<String> {
        self.scopes
            .visible_names()
            .into_iter()
            .chain(self.global_values.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

impl<'ast> LintVisitor<'ast> for HandlerScopeVisitor {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Derived;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = TOPIC;

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for HandlerScopeVisitor {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let was_program_module = self.in_program_module;
        if has_attr(&node.attrs, "program") {
            self.in_program_module = true;
        }
        visit::visit_item_mod(self, node);
        self.in_program_module = was_program_module;
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.in_program_module || item_fn_has_anchor_context_arg(node) {
            self.visit_anchor_function(node);
        }
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.scopes.push();
        for name in crate::lsp::scope::block_item_value_names(node) {
            self.scopes.declare(&name);
        }
        visit::visit_block(self, node);
        self.scopes.pop();
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(init) = &node.init {
            self.visit_expr(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.visit_expr(diverge);
            }
        }
        self.scopes.declare_pat(&node.pat);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);
        self.scopes.push();
        self.scopes.declare_pat(&node.pat);
        self.visit_block(&node.body);
        self.scopes.pop();
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.scopes.push();
        self.visit_condition_with_pattern_scope(&node.cond);
        self.visit_block(&node.then_branch);
        self.scopes.pop();
        if let Some((_, else_branch)) = &node.else_branch {
            self.visit_expr(else_branch);
        }
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.scopes.push();
        self.visit_condition_with_pattern_scope(&node.cond);
        self.visit_block(&node.body);
        self.scopes.pop();
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.visit_expr(&node.expr);
        for arm in &node.arms {
            self.scopes.push();
            self.scopes.declare_pat(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                self.visit_expr(guard);
            }
            self.visit_expr(&arm.body);
            self.scopes.pop();
        }
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        self.scopes.push();
        for input in &node.inputs {
            self.scopes.declare_pat(input);
        }
        self.visit_expr(&node.body);
        self.scopes.pop();
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(path) = node.func.as_ref() {
            self.report_unresolved_path_identifier(path);
        } else {
            self.visit_expr(&node.func);
        }
        for arg in &node.args {
            self.visit_expr(arg);
        }
    }

    fn visit_expr_path(&mut self, node: &'ast ExprPath) {
        self.report_unresolved_path_identifier(node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        for expression in crate::lsp::assertion_macros::assertion_macro_arguments(node) {
            self.visit_expr(&expression);
        }
    }
}

fn unresolved_handler_identifier_diagnostic(
    identifier: &str,
    range: Range,
    candidates: Vec<String>,
    evidence_source: &str,
) -> Diagnostic {
    diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorAccountUsage,
        format!(
            "`{identifier}` does not resolve in this Anchor handler; declare a local, add an argument, or import the value before using it."
        ),
        Some(serde_json::json!({
            "topic": TOPIC,
            "reason": REASON,
            "identifier": identifier,
            "candidates": candidates,
            "evidenceSource": evidence_source,
            "confidence": Confidence::Derived.as_str(),
            "applicability": Applicability::Unspecified.as_str(),
        })),
    )
}

#[derive(Default)]
struct ScopeStack {
    scopes: Vec<HashSet<String>>,
}

impl ScopeStack {
    fn push(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn declare_pat(&mut self, pat: &Pat) {
        let mut names = Vec::new();
        crate::lsp::scope::collect_pattern_bindings(pat, &mut names);
        for name in names {
            self.declare(&name);
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn visible_names(&self) -> Vec<String> {
        self.scopes
            .iter()
            .flat_map(|scope| scope.iter().cloned())
            .collect()
    }
}

fn global_values(document: &ParsedDocument) -> HashSet<String> {
    use crate::lsp::scope::program_module_value_names_from_document as module_values;
    let mut values = document
        .symbols()
        .value_items
        .iter()
        .chain(document.symbols().constants.iter())
        .chain(document.symbols().imported_names.iter())
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();
    values.extend(module_values(document.source(), &document.syntax().items));
    if document.symbols().declared_program_id.is_some() {
        values.insert("ID".to_string());
    }
    values
}

struct RecoveredHandlerScope {
    names: BTreeSet<String>,
}

impl RecoveredHandlerScope {
    fn at_position(source: &str, position: Position) -> Option<Self> {
        let text_scope = TextHandlerScope::at_position(source, position)?;
        if !text_scope.has_anchor_context() {
            return None;
        }

        let mut names = text_scope
            .bindings()
            .iter()
            .map(|binding| binding.name.clone())
            .collect::<BTreeSet<_>>();
        names.extend(crate::lsp::scope::text_file_value_names(source));
        names.extend(
            KNOWN_SINGLE_SEGMENT_VALUES
                .iter()
                .map(|name| name.to_string()),
        );
        Some(Self { names })
    }

    fn identifier_resolves(&self, identifier: &str) -> bool {
        self.names.contains(identifier)
    }

    fn visible_candidates(&self) -> Vec<String> {
        self.names.iter().cloned().collect()
    }
}

fn parse_error_identifier(source: &str, range: Range) -> Option<String> {
    let offset = byte_offset_at(source, range.start)?;
    let (start, end) = identifier_bounds_at(source, offset)?;
    source.get(start..end).map(str::to_string)
}

fn parse_error_identifier_is_value(source: &str, range: Range) -> bool {
    let Some(offset) = byte_offset_at(source, range.start) else {
        return false;
    };
    let Some((start, end)) = identifier_bounds_at(source, offset) else {
        return false;
    };
    if previous_non_whitespace(source, start).is_some_and(|ch| matches!(ch, '.' | ':')) {
        return false;
    }
    if next_non_whitespace(source, end).is_some_and(|ch| matches!(ch, '(' | '!' | ':')) {
        return false;
    }
    !previous_word(source, start).is_some_and(|word| matches!(word, "fn" | "let" | "struct"))
}

fn identifier_bounds_at(source: &str, offset: usize) -> Option<(usize, usize)> {
    let byte = source.as_bytes().get(offset).copied()?;
    if !is_identifier_byte(byte) {
        return None;
    }

    let mut start = offset;
    while start > 0
        && source
            .as_bytes()
            .get(start - 1)
            .is_some_and(|byte| is_identifier_byte(*byte))
    {
        start -= 1;
    }

    let mut end = offset;
    while source
        .as_bytes()
        .get(end)
        .is_some_and(|byte| is_identifier_byte(*byte))
    {
        end += 1;
    }

    Some((start, end))
}

fn previous_non_whitespace(source: &str, offset: usize) -> Option<char> {
    source
        .get(..offset)?
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
}

fn next_non_whitespace(source: &str, offset: usize) -> Option<char> {
    source.get(offset..)?.chars().find(|ch| !ch.is_whitespace())
}

fn previous_word(source: &str, offset: usize) -> Option<&str> {
    let before = source.get(..offset)?.trim_end();
    let end = before.len();
    let start = before
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_identifier_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    before.get(start..end).filter(|word| !word.is_empty())
}

fn bare_value_identifier(path: &ExprPath) -> Option<String> {
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    if !matches!(&segment.arguments, PathArguments::None) {
        return None;
    }
    Some(segment.ident.to_string())
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

#[cfg(test)]
#[path = "handler_scope/pattern_tests.rs"]
mod pattern_tests;

#[cfg(test)]
#[path = "handler_scope/macro_tests.rs"]
mod macro_tests;

#[cfg(test)]
#[path = "handler_scope/tests.rs"]
mod tests;
