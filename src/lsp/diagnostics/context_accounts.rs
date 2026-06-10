use {
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        range::range_from_span,
        workspace::WorkspaceIndex,
    },
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Range, Url},
};

#[cfg(test)]
fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    // The standard multi-file Anchor layout defines account structs in sibling
    // instruction modules and pulls them into the program module with a glob
    // `use` (e.g. `use instructions::*;`). When the file glob-imports names and
    // we have no workspace-wide visibility, we cannot prove a `Context<T>` type
    // is missing — emitting a "create the struct" error would be a false
    // positive on this canonical layout.
    let resolution_may_be_incomplete =
        document_has_local_glob_import(document) && workspace_lacks_visibility(workspace_index);
    let mut diagnostics = empty_context_type_diagnostics(document, workspace_index);
    diagnostics.extend(
        document
            .symbols()
            .context_references
            .iter()
            .filter_map(|reference| {
                if document
                    .symbols()
                    .accounts_structs
                    .contains_key(&reference.name)
                    || workspace_has_accounts_struct(workspace_index, &reference.name)
                {
                    None
                } else if document.symbols().all_structs.contains_key(&reference.name) {
                    Some(diagnostic_from_range_with_related(
                        reference.range,
                        AnchorDiagnosticKind::AnchorContextAccounts,
                        format!(
                            "`Context<{}>` resolves to struct `{}` but it is missing `#[derive(Accounts)]`.",
                            reference.name,
                            reference.name
                        ),
                        Some(serde_json::json!({
                            "contextType": reference.name,
                            "quickfix": "derive-accounts",
                        })),
                        Some(context_related_information(
                            document,
                            &reference.name,
                            Some(format!("struct `{}` is declared here.", reference.name)),
                        )),
                    ))
                } else if resolution_may_be_incomplete {
                    None
                } else {
                    Some(diagnostic_from_range_with_related(
                        reference.range,
                        AnchorDiagnosticKind::AnchorContextAccounts,
                        format!(
                            "`Context<{}>` has no matching `#[derive(Accounts)]` struct; create `{}<'info>`.",
                            reference.name, reference.name
                        ),
                        Some(serde_json::json!({
                            "contextType": reference.name,
                            "quickfix": "create-accounts-struct",
                        })),
                        Some(context_related_information(
                            document,
                            &reference.name,
                            Some(format!(
                                "`Context<{}>` is used by this Anchor handler.",
                                reference.name
                            )),
                        )),
                    ))
                }
            })
            .collect::<Vec<_>>(),
    );
    diagnostics
}

#[derive(Debug)]
struct EmptyContextType {
    function_name: String,
    range: Range,
}

fn empty_context_type_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let candidates = context_type_candidates(document, workspace_index);
    empty_context_types(document)
        .into_iter()
        .map(|empty| {
            let inferred = pascal_case(&empty.function_name);
            let suggested = best_context_candidate(&candidates, &inferred)
                .unwrap_or(inferred.as_str())
                .to_string();
            let context_type_exists = document.symbols().accounts_structs.contains_key(&suggested)
                || workspace_has_accounts_struct(workspace_index, &suggested);
            diagnostic_from_range_with_related(
                empty.range,
                AnchorDiagnosticKind::AnchorContextAccounts,
                format!(
                    "`{}` uses empty `Context<>`; use `Context<{suggested}>`.",
                    empty.function_name
                ),
                Some(serde_json::json!({
                    "quickfix": "fill-context-type",
                    "contextType": suggested,
                    "contextTypeExists": context_type_exists,
                    "candidates": candidates,
                    "function": empty.function_name,
                })),
                Some(empty_context_related_information(
                    document,
                    &empty.function_name,
                )),
            )
        })
        .collect()
}

fn context_type_candidates(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<String> {
    let mut candidates = document
        .symbols()
        .accounts_structs
        .keys()
        .cloned()
        .chain(
            workspace_index
                .map(WorkspaceIndex::accounts_struct_names)
                .unwrap_or_default(),
        )
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn best_context_candidate<'a>(candidates: &'a [String], inferred: &str) -> Option<&'a str> {
    candidates
        .iter()
        .find(|candidate| candidate.as_str() == inferred)
        .or_else(|| (candidates.len() == 1).then(|| &candidates[0]))
        .map(String::as_str)
}

fn empty_context_types(document: &ParsedDocument) -> Vec<EmptyContextType> {
    if document.syntax().items.is_empty() {
        return Vec::new();
    }

    let mut visitor = EmptyContextVisitor::default();
    visitor.visit_file(document.syntax());
    visitor.empty_contexts
}

#[derive(Default)]
struct EmptyContextVisitor {
    empty_contexts: Vec<EmptyContextType>,
}

impl<'ast> Visit<'ast> for EmptyContextVisitor {
    fn visit_attribute(&mut self, _node: &'ast syn::Attribute) {}

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let function_name = node.sig.ident.to_string();
        self.empty_contexts
            .extend(node.sig.inputs.iter().filter_map(|input| {
                empty_context_range_from_argument(input).map(|range| EmptyContextType {
                    function_name: function_name.clone(),
                    range,
                })
            }));
        visit::visit_item_fn(self, node);
    }
}

fn empty_context_range_from_argument(argument: &syn::FnArg) -> Option<Range> {
    let syn::FnArg::Typed(pat_type) = argument else {
        return None;
    };
    empty_context_range_from_type(&pat_type.ty)
}

fn empty_context_range_from_type(ty: &syn::Type) -> Option<Range> {
    match ty {
        syn::Type::Path(type_path) => type_path.path.segments.iter().find_map(|segment| {
            if segment.ident != "Context" {
                return None;
            }
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            arguments
                .args
                .is_empty()
                .then(|| range_from_span(arguments.span()))
        }),
        syn::Type::Reference(reference) => empty_context_range_from_type(&reference.elem),
        syn::Type::Paren(paren) => empty_context_range_from_type(&paren.elem),
        syn::Type::Group(group) => empty_context_range_from_type(&group.elem),
        _ => None,
    }
}

fn pascal_case(value: &str) -> String {
    let mut output = String::new();
    for part in value.split('_').filter(|part| !part.is_empty()) {
        let mut chars = part.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        output.extend(first.to_uppercase());
        output.push_str(chars.as_str());
    }
    output
}

/// True when the workspace index gives us no cross-file visibility into account
/// structs — either it is absent, or it has indexed no `#[derive(Accounts)]`
/// structs at all. In that state we cannot trust a single file's view of which
/// `Context<T>` types exist.
fn workspace_lacks_visibility(workspace_index: Option<&WorkspaceIndex>) -> bool {
    workspace_index.is_none_or(|index| index.accounts_struct_names().is_empty())
}

/// Roots whose glob imports re-export names from elsewhere in this crate rather
/// than from an external dependency.
const LOCAL_GLOB_ROOTS: &[&str] = &["crate", "super", "self"];

/// True when the document contains a glob `use` that could bring **local**
/// account structs into scope — i.e. one rooted at `crate`/`super`/`self` or at
/// a module declared in this crate (e.g. `use instructions::*;` paired with
/// `mod instructions;`). The ubiquitous `use anchor_lang::prelude::*;` is
/// deliberately excluded: it re-exports an external crate's prelude, not local
/// account structs, so it must not gate this diagnostic.
fn document_has_local_glob_import(document: &ParsedDocument) -> bool {
    #[derive(Default)]
    struct GlobScan {
        module_names: std::collections::HashSet<String>,
        glob_roots: Vec<String>,
    }

    impl<'ast> Visit<'ast> for GlobScan {
        fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
            self.module_names.insert(node.ident.to_string());
            visit::visit_item_mod(self, node);
        }

        fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
            if let Some(root) = glob_root_segment(&node.tree) {
                self.glob_roots.push(root);
            }
            visit::visit_item_use(self, node);
        }
    }

    let mut scan = GlobScan::default();
    scan.visit_file(document.syntax());
    scan.glob_roots
        .iter()
        .any(|root| LOCAL_GLOB_ROOTS.contains(&root.as_str()) || scan.module_names.contains(root))
}

/// The leading path segment of a `use` tree that terminates in a glob, e.g.
/// `"super"` for `use super::*;` or `"anchor_lang"` for
/// `use anchor_lang::prelude::*;`. Returns `None` when the tree imports no glob.
fn glob_root_segment(tree: &syn::UseTree) -> Option<String> {
    fn ends_in_glob(tree: &syn::UseTree) -> bool {
        match tree {
            syn::UseTree::Glob(_) => true,
            syn::UseTree::Path(path) => ends_in_glob(&path.tree),
            syn::UseTree::Group(group) => group.items.iter().any(ends_in_glob),
            _ => false,
        }
    }

    match tree {
        syn::UseTree::Path(path) if ends_in_glob(&path.tree) => Some(path.ident.to_string()),
        _ => None,
    }
}

fn workspace_has_accounts_struct(workspace_index: Option<&WorkspaceIndex>, name: &str) -> bool {
    workspace_index.is_some_and(|index| {
        index
            .accounts_struct_names()
            .into_iter()
            .any(|candidate| candidate == name)
    })
}

fn context_related_information(
    document: &ParsedDocument,
    context_name: &str,
    fallback_message: Option<String>,
) -> Vec<DiagnosticRelatedInformation> {
    let mut related = Vec::new();
    if let Some(instruction) = document.symbols().callable_functions().find(|instruction| {
        instruction
            .context
            .as_ref()
            .is_some_and(|context| context.name == context_name)
    }) {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: current_document_uri(),
                range: instruction.selection_range,
            },
            message: format!(
                "`{}` uses `Context<{context_name}>` here.",
                instruction.name
            ),
        });
    }
    if let Some(existing_struct) = document.symbols().all_structs.get(context_name) {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: current_document_uri(),
                range: existing_struct.selection_range,
            },
            message: fallback_message
                .unwrap_or_else(|| format!("struct `{context_name}` is declared here.")),
        });
    } else if related.is_empty() {
        if let Some(message) = fallback_message {
            related.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: current_document_uri(),
                    range: Range::default(),
                },
                message,
            });
        }
    }
    related
}

fn empty_context_related_information(
    document: &ParsedDocument,
    function_name: &str,
) -> Vec<DiagnosticRelatedInformation> {
    document
        .symbols()
        .callable_functions()
        .find(|instruction| instruction.name == function_name)
        .map(|instruction| {
            vec![DiagnosticRelatedInformation {
                location: Location {
                    uri: current_document_uri(),
                    range: instruction.selection_range,
                },
                message: format!(
                    "`{}` has an empty Anchor `Context<>` parameter.",
                    instruction.name
                ),
            }]
        })
        .unwrap_or_default()
}

fn current_document_uri() -> Url {
    Url::parse("file:///seagrass/current-document.rs").unwrap()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::workspace::WorkspaceIndex, tower_lsp::lsp_types::Url};

    #[test]
    fn accepts_accounts_struct_from_workspace_index() {
        let lib = ParsedDocument::parse(
            r#"
#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();
        let accounts_uri = Url::parse("file:///tmp/instructions/make_offer.rs").unwrap();
        let index = WorkspaceIndex::build(
            &[],
            [(
                accounts_uri,
                r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#
                .to_string(),
            )],
        );

        assert!(collect_with_workspace(&lib, Some(&index)).is_empty());
    }

    #[test]
    fn still_reports_missing_accounts_struct_without_workspace_evidence() {
        // No glob import: the file's view of which structs exist is complete, so
        // a missing `Context<T>` struct is a real error even without a workspace.
        let lib = ParsedDocument::parse(
            r#"
#[program]
pub mod escrow {
    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&lib);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("`Context<MakeOffer>` has no matching `#[derive(Accounts)]` struct"));
        assert!(diagnostics[0]
            .related_information
            .as_ref()
            .expect("expected related handler context")
            .iter()
            .any(|info| info.message.contains("uses `Context<MakeOffer>`")));
    }

    #[test]
    fn suppresses_missing_struct_when_glob_import_hides_resolution() {
        // Regression: the canonical multi-file Anchor layout defines account
        // structs in sibling modules and pulls them in via `use instructions::*;`.
        // Without workspace visibility we cannot prove the struct is absent, so
        // we must not emit a confident "create the struct" error.
        let lib = ParsedDocument::parse(
            r#"
use instructions::*;

#[program]
pub mod escrow {
    use super::*;

    pub fn make(ctx: Context<Make>) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();

        assert!(
            collect(&lib).is_empty(),
            "glob import without workspace evidence must not be reported as a missing struct: {:#?}",
            collect(&lib)
        );
    }

    #[test]
    fn reports_missing_struct_through_glob_when_workspace_confirms_absence() {
        // A populated workspace index that has crawled sibling files gives real
        // visibility: a `Context<T>` it does not know about is genuinely missing,
        // glob import or not.
        let lib = ParsedDocument::parse(
            r#"
use instructions::*;

#[program]
pub mod escrow {
    use super::*;

    pub fn make(ctx: Context<Make>) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();
        let accounts_uri = Url::parse("file:///tmp/instructions/take.rs").unwrap();
        let index = WorkspaceIndex::build(
            &[],
            [(
                accounts_uri,
                r#"
#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: Signer<'info>,
}
"#
                .to_string(),
            )],
        );

        let diagnostics = collect_with_workspace(&lib, Some(&index));
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("`Context<Make>` has no matching `#[derive(Accounts)]` struct"));
    }

    #[test]
    fn reports_empty_context_type_with_inferred_accounts_struct() {
        let lib = ParsedDocument::parse(
            r#"
#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(
        context: Context<>,
        id: u64,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&lib);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("empty `Context<>`"));
        assert_eq!(
            diagnostics[0]
                .data
                .as_ref()
                .and_then(|data| data.get("contextType"))
                .and_then(|value| value.as_str()),
            Some("MakeOffer")
        );
        assert!(diagnostics[0]
            .related_information
            .as_ref()
            .expect("expected related empty Context handler")
            .iter()
            .any(|info| info.message.contains("empty Anchor `Context<>`")));
    }

    #[test]
    fn ignores_empty_context_text_in_comments_strings_and_attributes() {
        let lib = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[doc = "Context<>"]
pub fn helper() {
    // Context<>
    let _template = "Context<>";
}
"#,
        )
        .unwrap();

        assert!(collect(&lib).is_empty());
    }

    #[test]
    fn reports_context_struct_missing_accounts_derive_with_related_struct() {
        let lib = ParsedDocument::parse(
            r#"
#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> {
        Ok(())
    }
}

pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&lib);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("missing `#[derive(Accounts)]`"));
        assert!(diagnostics[0]
            .related_information
            .as_ref()
            .expect("expected related struct declaration")
            .iter()
            .any(|info| info.message.contains("struct `MakeOffer` is declared here")));
    }

    #[test]
    fn reports_empty_context_type_with_workspace_accounts_struct() {
        let lib = ParsedDocument::parse(
            r#"
#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(context: Context<>, id: u64) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();
        let accounts_uri = Url::parse("file:///tmp/instructions/make_offer.rs").unwrap();
        let index = WorkspaceIndex::build(
            &[],
            [(
                accounts_uri,
                r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#
                .to_string(),
            )],
        );

        let diagnostics = collect_with_workspace(&lib, Some(&index));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0]
                .data
                .as_ref()
                .and_then(|data| data.get("contextType"))
                .and_then(|value| value.as_str()),
            Some("MakeOffer")
        );
    }
}
