use {
    super::{
        diagnostic_from_range,
        lint::{run_lint_visitor, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    crate::{
        account_members,
        document::ParsedDocument,
        lsp::{
            local_types,
            scope::{has_attr, item_fn_has_anchor_context_arg, pattern_binding_name},
        },
        range::range_from_span,
        workspace::WorkspaceIndex,
    },
    std::collections::{HashMap, HashSet},
    syn::{
        visit::{self, Visit},
        Expr, ExprField, FnArg, ItemFn, ItemMod, Member,
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

const TOPIC: &str = "seagrass/anchor.account.usage";
const REASON: &str = "unknown-handler-member";
const EVIDENCE_SOURCE: &str = "parsed-anchor-handler-local-types";

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    run_lint_visitor(
        document,
        HandlerMemberVisitor::new(document, workspace_index),
    )
}

struct HandlerMemberVisitor<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    scopes: TypedScopeStack,
    in_program_module: bool,
    diagnostics: Vec<Diagnostic>,
    emitted: HashSet<(u32, u32)>,
}

impl<'a> HandlerMemberVisitor<'a> {
    fn new(document: &'a ParsedDocument, workspace_index: Option<&'a WorkspaceIndex>) -> Self {
        Self {
            document,
            workspace_index,
            scopes: TypedScopeStack::default(),
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
            let FnArg::Typed(pat_type) = input else {
                continue;
            };
            let Some(type_name) = local_types::shallow_type_name(&pat_type.ty) else {
                continue;
            };
            self.scopes.declare_pat(&pat_type.pat, type_name);
        }
    }

    fn report_unknown_member(&mut self, node: &ExprField) {
        let Some(access) = FieldAccess::from_expr_field(node) else {
            return;
        };
        let Some(receiver_type) = self.scopes.get(&access.receiver) else {
            return;
        };
        let Some(diagnostic) =
            unknown_member_diagnostic(self.document, self.workspace_index, &receiver_type, &access)
        else {
            return;
        };
        let key = (
            diagnostic.range.start.line,
            diagnostic.range.start.character,
        );
        if self.emitted.insert(key) {
            self.diagnostics.push(diagnostic);
        }
    }
}

impl<'ast> LintVisitor<'ast> for HandlerMemberVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Derived;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = TOPIC;

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for HandlerMemberVisitor<'_> {
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
        if let Some(type_name) = local_types::local_type_name(node) {
            self.scopes.declare_pat(&node.pat, type_name);
        }
    }

    fn visit_expr_field(&mut self, node: &'ast ExprField) {
        self.report_unknown_member(node);
        visit::visit_expr_field(self, node);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldAccess {
    receiver: String,
    members: Vec<FieldMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldMember {
    name: String,
    range: Range,
}

impl FieldAccess {
    fn from_expr_field(node: &ExprField) -> Option<Self> {
        let mut members = Vec::new();
        collect_field_access(node, &mut members).map(|receiver| Self { receiver, members })
    }
}

fn collect_field_access(node: &ExprField, members: &mut Vec<FieldMember>) -> Option<String> {
    let receiver = match node.base.as_ref() {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            path.path.segments[0].ident.to_string()
        }
        Expr::Field(base) => collect_field_access(base, members)?,
        Expr::Paren(paren) => {
            let Expr::Field(field) = paren.expr.as_ref() else {
                return None;
            };
            collect_field_access(field, members)?
        }
        Expr::Group(group) => {
            let Expr::Field(field) = group.expr.as_ref() else {
                return None;
            };
            collect_field_access(field, members)?
        }
        Expr::Reference(reference) => {
            let Expr::Field(field) = reference.expr.as_ref() else {
                return None;
            };
            collect_field_access(field, members)?
        }
        _ => return None,
    };
    let Member::Named(ident) = &node.member else {
        return None;
    };
    members.push(FieldMember {
        name: ident.to_string(),
        range: range_from_span(ident.span()),
    });
    Some(receiver)
}

fn unknown_member_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    access: &FieldAccess,
) -> Option<Diagnostic> {
    let mut owner_type = receiver_type.to_string();
    let mut members =
        account_members::resolved_struct_members(document, workspace_index, &owner_type)?;
    let mut receiver_path = access.receiver.clone();

    for member in &access.members {
        let Some(resolved) = members
            .members
            .iter()
            .find(|candidate| candidate.name == member.name)
        else {
            return Some(diagnostic_from_range(
                member.range,
                AnchorDiagnosticKind::AnchorMissingAccountReference,
                format!(
                    "`{receiver_path}.{}` does not resolve; `{}` has no field `{}`.",
                    member.name, members.owner_type, member.name
                ),
                Some(serde_json::json!({
                    "topic": TOPIC,
                    "reason": REASON,
                    "receiver": access.receiver,
                    "receiverType": receiver_type,
                    "field": member.name,
                    "ownerType": members.owner_type,
                    "candidates": members.members.iter().map(|member| member.name.clone()).collect::<Vec<_>>(),
                    "evidenceSource": EVIDENCE_SOURCE,
                    "confidence": Confidence::Derived.as_str(),
                    "applicability": Applicability::Unspecified.as_str(),
                })),
            ));
        };

        receiver_path.push('.');
        receiver_path.push_str(&member.name);
        let Some(next_type) = resolved.type_name.as_ref() else {
            return None;
        };
        owner_type = next_type.clone();
        members = account_members::resolved_struct_members(document, workspace_index, &owner_type)?;
    }

    None
}

#[derive(Default)]
struct TypedScopeStack {
    scopes: Vec<HashMap<String, String>>,
}

impl TypedScopeStack {
    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn declare_pat(&mut self, pat: &syn::Pat, type_name: String) {
        let Some(name) = pattern_binding_name(pat) else {
            return;
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, type_name);
        }
    }

    fn get(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::collect_with_workspace,
        crate::{
            diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
        },
        proptest::prelude::*,
        tower_lsp::lsp_types::NumberOrString,
    };

    prop_compose! {
        fn generated_ident()(tail in "[a-z0-9_]{1,10}") -> String {
            format!("sg_{tail}")
        }
    }

    #[test]
    fn reports_unknown_explicit_handler_local_member() {
        let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let bundle: PositionBundle = PositionBundle { position_bundle_mint: Pubkey::default() };
        bundle.s;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
        )
        .unwrap();

        let diagnostics = collect_with_workspace(&document, None);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("`bundle.s` does not resolve"))
            .unwrap_or_else(|| panic!("missing handler member diagnostic: {diagnostics:#?}"));

        assert_eq!(
            diagnostic.code.as_ref(),
            Some(&NumberOrString::String(
                ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
            ))
        );
        assert!(diagnostic
            .message
            .contains("`PositionBundle` has no field `s`"));
    }

    #[test]
    fn accepts_known_explicit_handler_local_member() {
        let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let bundle: PositionBundle = PositionBundle { position_bundle_mint: Pubkey::default() };
        bundle.position_bundle_mint;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
        )
        .unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        assert!(
            diagnostics.iter().all(|diagnostic| !diagnostic
                .message
                .contains("position_bundle_mint` does not resolve")),
            "known handler member should resolve, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_nested_explicit_handler_local_member() {
        let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
        bundle.inner.fake;
        Ok(())
    }
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real: Pubkey,
}
"#,
        )
        .unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("`bundle.inner.fake` does not resolve")
                    && diagnostic
                        .message
                        .contains("`InnerBundle` has no field `fake`")
            }),
            "missing nested handler member diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn ignores_unknown_external_handler_local_type() {
        let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, external: ExternalType) -> Result<()> {
        external.fake;
        Ok(())
    }
}
"#,
        )
        .unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("external.fake")),
            "external type should stay outside shallow resolver, got {diagnostics:#?}"
        );
    }

    proptest! {
        #[test]
        fn reports_generated_unknown_typed_handler_member(
            local in generated_ident(),
            owner in "[A-Z][A-Za-z0-9_]{1,10}",
            known_field in generated_ident(),
            missing_field in generated_ident(),
        ) {
            prop_assume!(known_field != missing_field);
            let source = format!(
                r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>) -> Result<()> {{
        let {local}: {owner} = {owner} {{ {known_field}: Pubkey::default() }};
        {local}.{missing_field};
        Ok(())
    }}
}}

pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
            );
            let document = ParsedDocument::parse(&source).unwrap();

            let diagnostics = collect_with_workspace(&document, None);

            prop_assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains(&format!("`{local}.{missing_field}` does not resolve"))
                        && diagnostic
                            .message
                            .contains(&format!("`{owner}` has no field `{missing_field}`"))
                }),
                "expected generated handler member diagnostic, got {diagnostics:#?}"
            );
        }
    }
}
