use {
    super::{cursor_context, ResolvedCursorContext},
    crate::{
        document::{ParsedDocument, SymbolRange},
        lsp::scope::{
            block_item_value_names, collect_condition_pattern_bindings, collect_pattern_bindings,
            program_module_value_names_from_document, text_block_item_value_names,
            text_enclosing_function_body, TextHandlerBinding, TextHandlerScope,
        },
        workspace::{WorkspaceContextField, WorkspaceIndex},
    },
    std::collections::BTreeMap,
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
        Expr, FnArg, ItemFn, Pat, Stmt,
    },
    tower_lsp::lsp_types::{
        CompletionItem, CompletionItemKind, CompletionTextEdit, Position, Range, TextEdit,
    },
};

const LOCAL_VALUE_RANK: u8 = 0;
const ACCOUNT_FIELD_RANK: u8 = 1;
const GLOBAL_VALUE_RANK: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValueCandidate {
    label: String,
    insert_text: String,
    detail: String,
    kind: CompletionItemKind,
    rank: u8,
}

pub(super) fn completions(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
    _context: &ResolvedCursorContext,
    prefix: &str,
) -> Option<Vec<CompletionItem>> {
    let mut candidates = Vec::new();
    candidates.extend(visible_local_value_candidates(document, position));
    candidates.extend(text_visible_value_candidates(document.source(), position));
    candidates.extend(account_field_value_candidates(
        document,
        position,
        workspace_index,
    ));
    candidates.extend(global_value_candidates(document));

    let replacement_range = super::prefix_replacement_range(position, prefix);
    let mut deduped = BTreeMap::<String, ValueCandidate>::new();
    for candidate in candidates {
        if !super::matches_completion_prefix(&candidate.label, prefix) {
            continue;
        }
        deduped
            .entry(candidate.label.clone())
            .and_modify(|existing| {
                if candidate.rank < existing.rank {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }

    let mut items = deduped
        .into_values()
        .map(|candidate| CompletionItem {
            label: candidate.label.clone(),
            kind: Some(candidate.kind),
            detail: Some(candidate.detail),
            insert_text: Some(candidate.insert_text.clone()),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: replacement_range,
                new_text: candidate.insert_text.clone(),
            })),
            sort_text: Some(format!(
                "{:03}_anchor_handler_value_{}",
                candidate.rank, candidate.label
            )),
            data: Some(serde_json::json!({
                "anchorCompletion": "handler-value",
            })),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
    });
    (!items.is_empty()).then_some(items)
}

fn visible_local_value_candidates(
    document: &ParsedDocument,
    position: Position,
) -> Vec<ValueCandidate> {
    let Some(cursor_offset) = crate::range::byte_offset_at(document.source(), position) else {
        return Vec::new();
    };
    let mut collector = VisibleBindingCollector {
        source: document.source(),
        cursor_offset,
        candidates: Vec::new(),
        found_cursor_function: false,
    };
    collector.visit_file(document.syntax());
    collector.candidates
}

fn account_field_value_candidates(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<ValueCandidate> {
    let Some(context_name) = handler_context_name(document, position)
        .or_else(|| text_handler_context_name(document.source(), position))
    else {
        return Vec::new();
    };
    let mut candidates = document
        .symbols()
        .accounts_structs
        .get(&context_name)
        .map(|accounts| {
            accounts
                .fields
                .iter()
                .map(|field| account_field_candidate(&context_name, field))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(index) = workspace_index {
        candidates.extend(
            index
                .account_context_fields(&context_name)
                .into_iter()
                .map(|field| workspace_account_field_candidate(&context_name, field)),
        );
    }
    candidates.extend(text_account_field_candidates(
        document.source(),
        &context_name,
    ));
    candidates
}

fn global_value_candidates(document: &ParsedDocument) -> Vec<ValueCandidate> {
    document
        .symbols()
        .constants
        .iter()
        .map(|item| ValueCandidate {
            label: item.name.clone(),
            insert_text: item.name.clone(),
            detail: "Anchor file constant".to_string(),
            kind: CompletionItemKind::CONSTANT,
            rank: GLOBAL_VALUE_RANK,
        })
        .chain(
            document
                .symbols()
                .imported_names
                .iter()
                .map(|item| ValueCandidate {
                    label: item.name.clone(),
                    insert_text: item.name.clone(),
                    detail: "Imported value in scope".to_string(),
                    kind: CompletionItemKind::VALUE,
                    rank: GLOBAL_VALUE_RANK,
                }),
        )
        .chain(
            document
                .symbols()
                .value_items
                .iter()
                .map(|item| ValueCandidate {
                    label: item.name.clone(),
                    insert_text: item.name.clone(),
                    detail: "File value item".to_string(),
                    kind: CompletionItemKind::VALUE,
                    rank: GLOBAL_VALUE_RANK,
                }),
        )
        .chain(
            program_module_value_names_from_document(document.source(), &document.syntax().items)
                .into_iter()
                .map(|name| ValueCandidate {
                    label: name.clone(),
                    insert_text: name,
                    detail: "Anchor program module value".to_string(),
                    kind: CompletionItemKind::VALUE,
                    rank: GLOBAL_VALUE_RANK,
                }),
        )
        .collect()
}

fn handler_context_name(document: &ParsedDocument, position: Position) -> Option<String> {
    document
        .symbols()
        .callable_functions()
        .find(|function| contains_position(function.range, position))
        .and_then(|function| function.context.as_ref())
        .map(|context| context.name.clone())
}

fn text_handler_context_name(source: &str, position: Position) -> Option<String> {
    let offset = crate::range::byte_offset_at(source, position)?;
    let before_cursor = &source[..offset.min(source.len())];
    let function_start = cursor_context::last_function_keyword_before(before_cursor)?;
    let function_prefix = &before_cursor[function_start..];
    let signature = function_prefix
        .split_once('{')
        .map_or(function_prefix, |(signature, _)| signature);
    let context_start = signature.rfind("Context<")? + "Context<".len();
    identifier_at_start(&signature[context_start..]).map(str::to_string)
}

fn account_field_candidate(context_name: &str, field: &SymbolRange) -> ValueCandidate {
    let type_display = symbol_type_display(field)
        .map(|display| format!(": {display}"))
        .unwrap_or_default();
    ValueCandidate {
        label: field.name.clone(),
        insert_text: format!("ctx.accounts.{}", field.name),
        detail: format!("Anchor account field in `{context_name}`{type_display}"),
        kind: CompletionItemKind::FIELD,
        rank: ACCOUNT_FIELD_RANK,
    }
}

fn workspace_account_field_candidate(
    context_name: &str,
    field: WorkspaceContextField,
) -> ValueCandidate {
    let type_display = field
        .type_display
        .map(|display| format!(": {display}"))
        .unwrap_or_default();
    ValueCandidate {
        label: field.name.clone(),
        insert_text: format!("ctx.accounts.{}", field.name),
        detail: format!("Anchor account field in `{context_name}`{type_display}"),
        kind: CompletionItemKind::FIELD,
        rank: ACCOUNT_FIELD_RANK,
    }
}

fn text_account_field_candidates(source: &str, context_name: &str) -> Vec<ValueCandidate> {
    let Some(body) = text_struct_body(source, context_name) else {
        return Vec::new();
    };
    body.lines()
        .filter_map(|line| text_account_field_candidate(context_name, line))
        .collect()
}

fn text_account_field_candidate(context_name: &str, line: &str) -> Option<ValueCandidate> {
    let trimmed = line.trim();
    let field = trimmed
        .strip_prefix("pub ")
        .or_else(|| trimmed.strip_prefix("pub(crate) "))?;
    let (name, ty) = field.split_once(':')?;
    let name = name.trim();
    if !crate::syntax::is_ascii_identifier(name) {
        return None;
    }
    let type_display = ty
        .trim()
        .trim_end_matches(',')
        .trim()
        .is_empty()
        .then(String::new)
        .unwrap_or_else(|| format!(": {}", ty.trim().trim_end_matches(',').trim()));
    Some(ValueCandidate {
        label: name.to_string(),
        insert_text: format!("ctx.accounts.{name}"),
        detail: format!("Anchor account field in `{context_name}`{type_display}"),
        kind: CompletionItemKind::FIELD,
        rank: ACCOUNT_FIELD_RANK,
    })
}

fn text_struct_body<'a>(source: &'a str, struct_name: &str) -> Option<&'a str> {
    let struct_needle = format!("struct {struct_name}");
    let struct_start = source.find(&struct_needle)?;
    let open = struct_start + source[struct_start..].find('{')?;
    let close = cursor_context::matching_close_brace(source, open)?;
    source.get(open + '{'.len_utf8()..close)
}

fn symbol_type_display(field: &SymbolRange) -> Option<String> {
    field.type_name.as_ref().map(|type_name| {
        if field.generic_type_names.is_empty() {
            type_name.clone()
        } else {
            format!("{}<{}>", type_name, field.generic_type_names.join(", "))
        }
    })
}

struct VisibleBindingCollector<'a> {
    source: &'a str,
    cursor_offset: usize,
    candidates: Vec<ValueCandidate>,
    found_cursor_function: bool,
}

impl VisibleBindingCollector<'_> {
    fn collect_function(&mut self, item_fn: &ItemFn) {
        self.collect_function_inputs(item_fn);
        self.collect_block_bindings(&item_fn.block);
        self.found_cursor_function = true;
    }

    fn collect_function_inputs(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            match input {
                FnArg::Receiver(receiver) => self.add_candidate(
                    "self".to_string(),
                    Some(type_text(self.source, receiver.ty.span())),
                ),
                FnArg::Typed(pat_type) => {
                    let type_display = type_text(self.source, pat_type.ty.span());
                    self.add_pattern_candidates(&pat_type.pat, Some(type_display));
                }
            }
        }
    }

    fn collect_block_bindings(&mut self, block: &syn::Block) -> bool {
        self.add_block_item_candidates(block);
        for stmt in &block.stmts {
            let stmt_range = crate::range::range_from_span(stmt.span());
            let Some(stmt_start) = crate::range::byte_offset_at(self.source, stmt_range.start)
            else {
                continue;
            };
            let Some(stmt_end) = crate::range::byte_offset_at(self.source, stmt_range.end) else {
                continue;
            };

            if self.cursor_offset < stmt_start {
                return true;
            }
            if self.cursor_offset <= stmt_end {
                return self.collect_bindings_inside_statement(stmt);
            }
            if let Stmt::Local(local) = stmt {
                self.add_pattern_candidates(&local.pat, None);
            }
        }
        false
    }

    fn collect_bindings_inside_statement(&mut self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Expr(expr, _) => self.collect_bindings_inside_expr(expr),
            Stmt::Local(local) => local.init.as_ref().is_none_or(|init| {
                !self.span_contains_cursor(init.expr.span())
                    || self.collect_bindings_inside_expr(&init.expr)
            }),
            Stmt::Item(_) | Stmt::Macro(_) => true,
        }
    }

    fn collect_bindings_inside_expr(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Block(block) => self.collect_block_bindings(&block.block),
            Expr::ForLoop(for_loop) if self.span_contains_cursor(for_loop.body.span()) => {
                self.add_pattern_candidates(&for_loop.pat, None);
                self.collect_block_bindings(&for_loop.body)
            }
            Expr::If(if_expr) => {
                if self.span_contains_cursor(if_expr.then_branch.span()) {
                    self.add_condition_pattern_candidates(&if_expr.cond);
                    return self.collect_block_bindings(&if_expr.then_branch);
                }
                if let Some((_, else_branch)) = &if_expr.else_branch {
                    return self.collect_bindings_inside_expr(else_branch);
                }
                true
            }
            Expr::Loop(loop_expr) if self.span_contains_cursor(loop_expr.body.span()) => {
                self.collect_block_bindings(&loop_expr.body)
            }
            Expr::Closure(closure) if self.span_contains_cursor(closure.body.span()) => {
                self.add_closure_pattern_candidates(closure);
                self.collect_bindings_inside_expr(&closure.body)
            }
            Expr::While(while_expr) if self.span_contains_cursor(while_expr.body.span()) => {
                self.add_condition_pattern_candidates(&while_expr.cond);
                self.collect_block_bindings(&while_expr.body)
            }
            Expr::Match(match_expr) => {
                for arm in &match_expr.arms {
                    if arm
                        .guard
                        .as_ref()
                        .is_some_and(|(_, guard)| self.span_contains_cursor(guard.span()))
                    {
                        self.add_pattern_candidates(&arm.pat, None);
                        return true;
                    }
                    if self.span_contains_cursor(arm.body.span()) {
                        self.add_pattern_candidates(&arm.pat, None);
                        return self.collect_bindings_inside_expr(&arm.body);
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn add_closure_pattern_candidates(&mut self, closure: &syn::ExprClosure) {
        for input in &closure.inputs {
            self.add_pattern_candidates(input, None);
        }
    }

    fn add_block_item_candidates(&mut self, block: &syn::Block) {
        for name in block_item_value_names(block) {
            self.add_candidate(name, None);
        }
    }

    fn span_contains_cursor(&self, span: proc_macro2::Span) -> bool {
        let range = crate::range::range_from_span(span);
        let Some(start) = crate::range::byte_offset_at(self.source, range.start) else {
            return false;
        };
        let Some(end) = crate::range::byte_offset_at(self.source, range.end) else {
            return false;
        };
        start <= self.cursor_offset && self.cursor_offset <= end
    }

    fn add_pattern_candidates(&mut self, pat: &Pat, type_display: Option<String>) {
        let mut names = Vec::new();
        collect_pattern_bindings(pat, &mut names);
        for name in names {
            self.add_candidate(name, type_display.clone());
        }
    }

    fn add_condition_pattern_candidates(&mut self, expr: &Expr) {
        let mut names = Vec::new();
        collect_condition_pattern_bindings(expr, &mut names);
        for name in names {
            self.add_candidate(name, None);
        }
    }

    fn add_candidate(&mut self, name: String, type_display: Option<String>) {
        self.candidates.push(ValueCandidate {
            label: name.clone(),
            insert_text: name,
            detail: type_display
                .map(|display| format!("Anchor handler value: `{display}`"))
                .unwrap_or_else(|| "Anchor handler local value".to_string()),
            kind: CompletionItemKind::VARIABLE,
            rank: LOCAL_VALUE_RANK,
        });
    }
}

fn text_visible_value_candidates(source: &str, position: Position) -> Vec<ValueCandidate> {
    let scope_candidates = TextHandlerScope::at_position(source, position)
        .map(|scope| {
            scope
                .bindings()
                .iter()
                .map(text_value_candidate)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    scope_candidates
        .into_iter()
        .chain(text_enclosing_block_item_candidates(source, position))
        .collect()
}

fn text_enclosing_block_item_candidates(source: &str, position: Position) -> Vec<ValueCandidate> {
    let Some(body) = text_enclosing_function_body(source, position) else {
        return Vec::new();
    };
    text_block_item_value_names(body)
        .into_iter()
        .map(|name| ValueCandidate {
            label: name.clone(),
            insert_text: name,
            detail: "Anchor handler block item".to_string(),
            kind: CompletionItemKind::VALUE,
            rank: LOCAL_VALUE_RANK,
        })
        .collect()
}

fn text_value_candidate(binding: &TextHandlerBinding) -> ValueCandidate {
    ValueCandidate {
        label: binding.name.clone(),
        insert_text: binding.name.clone(),
        detail: binding
            .type_display
            .as_ref()
            .map(|ty| format!("Anchor handler value: `{ty}`"))
            .unwrap_or_else(|| "Anchor handler local value".to_string()),
        kind: CompletionItemKind::VARIABLE,
        rank: LOCAL_VALUE_RANK,
    }
}

impl<'ast> Visit<'ast> for VisibleBindingCollector<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.found_cursor_function || !self.span_contains_cursor(node.block.span()) {
            return;
        }
        self.collect_function(node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !self.found_cursor_function {
            visit::visit_item_mod(self, node);
        }
    }
}

fn type_text(source: &str, span: proc_macro2::Span) -> String {
    let range = crate::range::range_from_span(span);
    text_in_range(source, range)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("value")
        .to_string()
}

fn text_in_range(source: &str, range: Range) -> Option<&str> {
    let start = crate::range::byte_offset_at(source, range.start)?;
    let end = crate::range::byte_offset_at(source, range.end)?;
    source.get(start..end)
}

fn identifier_at_start(value: &str) -> Option<&str> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!crate::syntax::is_ascii_identifier_char(ch)).then_some(idx))
        .unwrap_or(value.len());
    let identifier = &value[..end];
    crate::syntax::is_ascii_identifier(identifier).then_some(identifier)
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}
