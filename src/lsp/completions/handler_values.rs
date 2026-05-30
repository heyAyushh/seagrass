use {
    super::{cursor_context, ResolvedCursorContext},
    crate::{
        document::{ParsedDocument, SymbolRange},
        lsp::scope::collect_pattern_bindings,
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

    let replacement_range = prefix_replacement_range(position, prefix);
    let mut deduped = BTreeMap::<String, ValueCandidate>::new();
    for candidate in candidates {
        if !matches_prefix(&candidate.label, prefix) {
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
    if !is_identifier(name) {
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
            Stmt::Local(_) | Stmt::Item(_) | Stmt::Macro(_) => true,
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
            Expr::While(while_expr) if self.span_contains_cursor(while_expr.body.span()) => {
                self.collect_block_bindings(&while_expr.body)
            }
            Expr::Match(match_expr) => {
                for arm in &match_expr.arms {
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
    let Some(offset) = crate::range::byte_offset_at(source, position) else {
        return Vec::new();
    };
    let before_cursor = &source[..offset.min(source.len())];
    let Some(function_start) = cursor_context::last_function_keyword_before(before_cursor) else {
        return Vec::new();
    };
    let function_prefix = &before_cursor[function_start..];
    let mut candidates = text_function_input_candidates(function_prefix);
    if let Some((_, body_prefix)) = function_prefix.split_once('{') {
        let completed_body_lines = body_prefix
            .rsplit_once('\n')
            .map_or("", |(completed_lines, _)| completed_lines);
        candidates.extend(text_local_binding_candidates(completed_body_lines));
    }
    candidates
}

fn text_function_input_candidates(function_prefix: &str) -> Vec<ValueCandidate> {
    let signature = function_prefix
        .split_once('{')
        .map_or(function_prefix, |(signature, _)| signature);
    let Some(open) = signature.find('(') else {
        return Vec::new();
    };
    let Some(close) = signature[open..].rfind(')').map(|idx| open + idx) else {
        return Vec::new();
    };
    split_top_level_commas(&signature[open + '('.len_utf8()..close])
        .into_iter()
        .filter_map(text_function_input_candidate)
        .collect()
}

fn text_function_input_candidate(input: &str) -> Option<ValueCandidate> {
    let (name, ty) = input.trim().split_once(':')?;
    let name = name.trim();
    if !is_identifier(name) {
        return None;
    }
    let ty = ty.trim();
    Some(ValueCandidate {
        label: name.to_string(),
        insert_text: name.to_string(),
        detail: if ty.is_empty() {
            "Anchor handler value".to_string()
        } else {
            format!("Anchor handler value: `{ty}`")
        },
        kind: CompletionItemKind::VARIABLE,
        rank: LOCAL_VALUE_RANK,
    })
}

fn text_local_binding_candidate(line: &str) -> Option<ValueCandidate> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("let ")?;
    let (left, _) = rest.split_once('=')?;
    let left = left.trim().strip_prefix("mut ").unwrap_or(left.trim());
    let name = left.split_once(':').map_or(left, |(name, _)| name).trim();
    is_identifier(name).then(|| ValueCandidate {
        label: name.to_string(),
        insert_text: name.to_string(),
        detail: "Anchor handler local value".to_string(),
        kind: CompletionItemKind::VARIABLE,
        rank: LOCAL_VALUE_RANK,
    })
}

fn text_local_binding_candidates(body_prefix: &str) -> Vec<ValueCandidate> {
    let mut depth = 0usize;
    let mut candidates = Vec::new();
    for line in body_prefix.lines() {
        if depth == 0 {
            candidates.extend(text_local_binding_candidate(line));
        }
        depth = line.chars().fold(depth, |depth, ch| match ch {
            '{' => depth + 1,
            '}' => depth.saturating_sub(1),
            _ => depth,
        });
    }
    candidates
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(text[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
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

fn prefix_replacement_range(position: Position, prefix: &str) -> Range {
    Range {
        start: Position {
            line: position.line,
            character: position
                .character
                .saturating_sub(u32::try_from(prefix.chars().count()).unwrap_or_default()),
        },
        end: position,
    }
}

fn matches_prefix(candidate: &str, prefix: &str) -> bool {
    candidate
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

fn identifier_at_start(value: &str) -> Option<&str> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!cursor_context::is_identifier_char(ch)).then_some(idx))
        .unwrap_or(value.len());
    let identifier = &value[..end];
    is_identifier(identifier).then_some(identifier)
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(cursor_context::is_identifier_char)
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}
