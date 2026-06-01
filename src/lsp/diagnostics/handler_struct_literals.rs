#[cfg(test)]
mod tests;

use {
    super::{
        diagnostic_from_range,
        lint::{run_lint_visitor, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    crate::{
        account_members,
        document::ParsedDocument,
        lsp::scope::{has_attr, item_fn_has_anchor_context_arg},
        range::range_from_span,
        workspace::WorkspaceIndex,
    },
    std::collections::HashSet,
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
        ExprStruct, ItemFn, ItemMod, Member,
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

const TOPIC: &str = "seagrass/anchor.account.usage";
const REASON: &str = "unknown-struct-literal-field";
const PATTERN_REASON: &str = "unknown-struct-pattern-field";
const EVIDENCE_SOURCE: &str = "parsed-rust-struct-literal-fields";
const PATTERN_EVIDENCE_SOURCE: &str = "parsed-rust-struct-pattern-fields";
const STRUCT_EXPR_DOCS_URL: &str =
    "https://doc.rust-lang.org/reference/expressions/struct-expr.html";
const STRUCT_PATTERN_DOCS_URL: &str =
    "https://doc.rust-lang.org/reference/patterns.html#struct-patterns";
const MAX_MESSAGE_CANDIDATES: usize = 5;

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        HandlerStructLiteralVisitor::new(document, workspace_index),
    )
}

struct HandlerStructLiteralVisitor<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    in_program_module: bool,
    diagnostics: Vec<Diagnostic>,
    emitted: HashSet<(u32, u32)>,
}

impl<'a> HandlerStructLiteralVisitor<'a> {
    fn new(document: &'a ParsedDocument, workspace_index: Option<&'a WorkspaceIndex>) -> Self {
        Self {
            document,
            workspace_index,
            in_program_module: false,
            diagnostics: Vec::new(),
            emitted: HashSet::new(),
        }
    }

    fn visit_anchor_function(&mut self, item_fn: &ItemFn) {
        self.visit_block(&item_fn.block);
    }

    fn report_unknown_fields(&mut self, node: &ExprStruct) {
        let Some(type_name) = struct_literal_type_name(node) else {
            return;
        };
        let Some(candidate_fields) = self.candidate_fields(&type_name) else {
            return;
        };

        for field in &node.fields {
            let Member::Named(field_name) = &field.member else {
                continue;
            };
            let field_name = field_name.to_string();
            if candidate_fields
                .iter()
                .any(|candidate| candidate == &field_name)
            {
                continue;
            }
            self.push_diagnostic_once(unknown_struct_record_field_diagnostic(
                range_from_span(field.member.span()),
                &type_name,
                &field_name,
                &candidate_fields,
                StructRecordKind::Literal,
            ));
        }
    }

    fn report_unknown_pattern_fields(&mut self, node: &syn::PatStruct) {
        let Some(type_name) = struct_pattern_type_name(node) else {
            return;
        };
        let Some(candidate_fields) = self.candidate_fields(&type_name) else {
            return;
        };

        for field in &node.fields {
            let Member::Named(field_name) = &field.member else {
                continue;
            };
            let field_name = field_name.to_string();
            if candidate_fields
                .iter()
                .any(|candidate| candidate == &field_name)
            {
                continue;
            }
            self.push_diagnostic_once(unknown_struct_record_field_diagnostic(
                range_from_span(field.member.span()),
                &type_name,
                &field_name,
                &candidate_fields,
                StructRecordKind::Pattern,
            ));
        }
    }

    fn push_diagnostic_once(&mut self, diagnostic: Diagnostic) {
        let key = (
            diagnostic.range.start.line,
            diagnostic.range.start.character,
        );
        if self.emitted.insert(key) {
            self.diagnostics.push(diagnostic);
        }
    }

    fn candidate_fields(&self, type_name: &str) -> Option<Vec<String>> {
        let members = account_members::resolved_struct_members(
            self.document,
            self.workspace_index,
            type_name,
        )?;
        let candidate_fields = members
            .members
            .iter()
            .filter(|member| {
                member.completion_kind == tower_lsp::lsp_types::CompletionItemKind::FIELD
            })
            .map(|member| member.name.clone())
            .collect::<Vec<_>>();

        (!candidate_fields.is_empty()).then_some(candidate_fields)
    }
}

impl<'ast> LintVisitor<'ast> for HandlerStructLiteralVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Derived;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = TOPIC;

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for HandlerStructLiteralVisitor<'_> {
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

    fn visit_expr_struct(&mut self, node: &'ast ExprStruct) {
        self.report_unknown_fields(node);
        visit::visit_expr_struct(self, node);
    }

    fn visit_pat_struct(&mut self, node: &'ast syn::PatStruct) {
        self.report_unknown_pattern_fields(node);
        visit::visit_pat_struct(self, node);
    }
}

fn struct_literal_type_name(node: &ExprStruct) -> Option<String> {
    unqualified_path_type_name(&node.path)
}

fn struct_pattern_type_name(node: &syn::PatStruct) -> Option<String> {
    unqualified_path_type_name(&node.path)
}

fn unqualified_path_type_name(path: &syn::Path) -> Option<String> {
    (path.segments.len() == 1).then(|| path.segments[0].ident.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructRecordKind {
    Literal,
    Pattern,
}

impl StructRecordKind {
    const fn noun(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::Pattern => "pattern",
        }
    }

    const fn reason(self) -> &'static str {
        match self {
            Self::Literal => REASON,
            Self::Pattern => PATTERN_REASON,
        }
    }

    const fn evidence_source(self) -> &'static str {
        match self {
            Self::Literal => EVIDENCE_SOURCE,
            Self::Pattern => PATTERN_EVIDENCE_SOURCE,
        }
    }

    const fn docs_url(self) -> &'static str {
        match self {
            Self::Literal => STRUCT_EXPR_DOCS_URL,
            Self::Pattern => STRUCT_PATTERN_DOCS_URL,
        }
    }
}

fn unknown_struct_record_field_diagnostic(
    range: Range,
    owner_type: &str,
    field: &str,
    candidates: &[String],
    kind: StructRecordKind,
) -> Diagnostic {
    diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorMissingAccountReference,
        format!(
            "`{owner_type}` has no struct {} field `{field}`; use one of: {}.",
            kind.noun(),
            message_candidates(candidates)
        ),
        Some(serde_json::json!({
            "topic": TOPIC,
            "reason": kind.reason(),
            "field": field,
            "ownerType": owner_type,
            "candidates": candidates,
            "evidenceSource": kind.evidence_source(),
            "docsUrl": kind.docs_url(),
            "confidence": Confidence::Derived.as_str(),
            "applicability": Applicability::Unspecified.as_str(),
        })),
    )
}

fn message_candidates(candidates: &[String]) -> String {
    let mut shown = candidates
        .iter()
        .take(MAX_MESSAGE_CANDIDATES)
        .map(|candidate| format!("`{candidate}`"))
        .collect::<Vec<_>>();
    if candidates.len() > MAX_MESSAGE_CANDIDATES {
        shown.push(format!(
            "{} more",
            candidates.len().saturating_sub(MAX_MESSAGE_CANDIDATES)
        ));
    }
    shown.join(", ")
}
