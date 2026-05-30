use {
    crate::{
        document::{
            summarize_account_field_type, AccountFieldTypeSummary, ParsedDocument, SymbolRange,
        },
        range::line_at,
        workspace::{WorkspaceAccountField, WorkspaceIndex},
    },
    quote::ToTokens,
    tower_lsp::lsp_types::{
        CompletionItem, CompletionItemKind, CompletionTextEdit, Position, Range, TextEdit,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountPathCompletionContext {
    completed_segments: Vec<String>,
    prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldCandidate {
    name: String,
    detail: String,
    kind: CompletionItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextAccountField {
    name: String,
    type_summary: AccountFieldTypeSummary,
    type_display: Option<String>,
}

struct TextStructFields {
    fields: Vec<TextAccountField>,
    derives_accounts: bool,
}

pub fn completions(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<Vec<CompletionItem>> {
    let context = account_path_completion_context(document.source(), position)?;

    let accounts_context = document
        .symbols()
        .callable_functions()
        .filter(|function| contains_position(function.range, position))
        .find_map(|function| {
            function
                .context
                .as_ref()
                .map(|context| context.name.clone())
        })
        .or_else(|| text_enclosing_context_name(document.source(), position))?;

    let mut candidates = local_candidates(document, &accounts_context, &context.completed_segments)
        .or_else(|| {
            text_recovery_candidates(
                document.source(),
                workspace_index,
                &accounts_context,
                &context.completed_segments,
            )
        })
        .or_else(|| {
            workspace_candidates(
                workspace_index?,
                &accounts_context,
                &context.completed_segments,
            )
        })?;
    candidates.retain(|candidate| {
        context.prefix.is_empty() || matches_prefix(&candidate.name, &context.prefix)
    });
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    candidates.dedup_by(|left, right| left.name == right.name);

    (!candidates.is_empty()).then(|| {
        let replacement_range = prefix_replacement_range(position, &context.prefix);
        candidates
            .into_iter()
            .map(|candidate| CompletionItem {
                label: candidate.name.clone(),
                kind: Some(candidate.kind),
                detail: Some(candidate.detail),
                insert_text: Some(candidate.name.clone()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replacement_range,
                    new_text: candidate.name.clone(),
                })),
                sort_text: Some(format!("000_anchor_ctx_account_path_{}", candidate.name)),
                data: Some(serde_json::json!({
                    "anchorCompletion": "ctx-account-path",
                })),
                ..CompletionItem::default()
            })
            .collect()
    })
}

fn local_candidates(
    document: &ParsedDocument,
    accounts_context: &str,
    completed_segments: &[String],
) -> Option<Vec<FieldCandidate>> {
    if completed_segments.is_empty() {
        return document
            .symbols()
            .accounts_structs
            .get(accounts_context)
            .map(|accounts| account_field_candidates(&accounts.name, &accounts.fields))
            .or_else(|| text_account_field_candidates(document.source(), accounts_context));
    }

    let container = local_container_after_segments(document, accounts_context, completed_segments)?;
    match container {
        FieldContainer::Accounts(name) => document
            .symbols()
            .accounts_structs
            .get(&name)
            .map(|accounts| account_field_candidates(&accounts.name, &accounts.fields)),
        FieldContainer::AccountData(name) => document
            .symbols()
            .account_data_structs
            .get(&name)
            .map(|account| account_data_field_candidates(&account.name, &account.fields)),
    }
}

fn workspace_candidates(
    workspace_index: &WorkspaceIndex,
    accounts_context: &str,
    completed_segments: &[String],
) -> Option<Vec<FieldCandidate>> {
    if completed_segments.is_empty() {
        let fields = workspace_index.account_context_fields(accounts_context);
        return (!fields.is_empty()).then(|| {
            fields
                .into_iter()
                .map(|field| FieldCandidate {
                    name: field.name,
                    detail: field
                        .type_display
                        .map(|display| {
                            format!("Anchor account field in `{accounts_context}`: `{display}`")
                        })
                        .unwrap_or_else(|| format!("Anchor account field in `{accounts_context}`")),
                    kind: CompletionItemKind::FIELD,
                })
                .collect()
        });
    }

    let container =
        workspace_container_after_segments(workspace_index, accounts_context, completed_segments)?;
    match container {
        FieldContainer::Accounts(name) => workspace_index.accounts_struct(&name).map(|accounts| {
            accounts
                .fields
                .iter()
                .map(|field| workspace_account_field_candidate(&name, field))
                .collect()
        }),
        FieldContainer::AccountData(name) => {
            workspace_account_data_field_candidates(workspace_index, &name)
        }
    }
}

fn text_recovery_candidates(
    source: &str,
    workspace_index: Option<&WorkspaceIndex>,
    accounts_context: &str,
    completed_segments: &[String],
) -> Option<Vec<FieldCandidate>> {
    if completed_segments.is_empty() {
        return text_account_field_candidates(source, accounts_context);
    }

    let container = text_container_after_segments(
        source,
        workspace_index,
        accounts_context,
        completed_segments,
    )?;
    match container {
        FieldContainer::Accounts(name) => {
            text_account_field_candidates(source, &name).or_else(|| {
                workspace_index.and_then(|index| {
                    index.accounts_struct(&name).map(|accounts| {
                        accounts
                            .fields
                            .iter()
                            .map(|field| workspace_account_field_candidate(&name, field))
                            .collect()
                    })
                })
            })
        }
        FieldContainer::AccountData(name) => text_account_data_field_candidates(source, &name)
            .or_else(|| {
                workspace_index
                    .and_then(|index| workspace_account_data_field_candidates(index, &name))
            }),
    }
}

fn workspace_account_data_field_candidates(
    workspace_index: &WorkspaceIndex,
    name: &str,
) -> Option<Vec<FieldCandidate>> {
    let names = workspace_index.field_names_in_container(name);
    (!names.is_empty()).then(|| {
        names
            .into_iter()
            .map(|field_name| {
                let detail = workspace_index
                    .field_info_in_container(&field_name, name)
                    .and_then(|info| info.type_display)
                    .map(|display| format!("Anchor account data field in `{name}`: `{display}`"))
                    .unwrap_or_else(|| format!("Anchor account data field in `{name}`"));
                FieldCandidate {
                    name: field_name,
                    detail,
                    kind: CompletionItemKind::FIELD,
                }
            })
            .collect()
    })
}

fn text_account_field_candidates(
    source: &str,
    accounts_context: &str,
) -> Option<Vec<FieldCandidate>> {
    let mut candidates = text_accounts_struct_fields(source, accounts_context)?
        .into_iter()
        .map(|field| FieldCandidate {
            name: field.name,
            detail: field
                .type_display
                .map(|display| format!("Anchor account field in `{accounts_context}`: `{display}`"))
                .unwrap_or_else(|| format!("Anchor account field in `{accounts_context}`")),
            kind: CompletionItemKind::FIELD,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    (!candidates.is_empty()).then_some(candidates)
}

fn text_account_data_field_candidates(
    source: &str,
    container: &str,
) -> Option<Vec<FieldCandidate>> {
    let mut candidates = text_struct_fields(source, container)?
        .fields
        .into_iter()
        .map(|field| FieldCandidate {
            name: field.name,
            detail: field
                .type_display
                .map(|display| format!("Anchor account data field in `{container}`: `{display}`"))
                .unwrap_or_else(|| format!("Anchor account data field in `{container}`")),
            kind: CompletionItemKind::FIELD,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.name.cmp(&right.name));
    (!candidates.is_empty()).then_some(candidates)
}

fn text_accounts_struct_fields(
    source: &str,
    accounts_context: &str,
) -> Option<Vec<TextAccountField>> {
    let fields = text_struct_fields(source, accounts_context)?;
    fields.derives_accounts.then_some(fields.fields)
}

fn text_struct_fields(source: &str, accounts_context: &str) -> Option<TextStructFields> {
    let struct_marker = format!("struct {accounts_context}");
    let struct_idx = source.find(&struct_marker)?;
    let open = source[struct_idx..].find('{').map(|idx| struct_idx + idx)?;
    let close = matching_close_brace(source, open).unwrap_or(source.len());
    let body = &source[open + 1..close];
    let synthetic = format!("struct __SeagrassRecovery {{\n{body}\n}}");
    let item = syn::parse_str::<syn::ItemStruct>(&synthetic).ok()?;
    let syn::Fields::Named(fields) = item.fields else {
        return None;
    };
    let fields = fields
        .named
        .into_iter()
        .filter_map(|field| {
            let ident = field.ident?;
            let type_summary = summarize_account_field_type(&field.ty);
            Some(TextAccountField {
                name: ident.to_string(),
                type_display: Some(field.ty.to_token_stream().to_string()),
                type_summary,
            })
        })
        .collect::<Vec<_>>();
    (!fields.is_empty()).then_some(TextStructFields {
        fields,
        derives_accounts: struct_derives_accounts(source, struct_idx),
    })
}

fn matching_close_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in source[open..].char_indices() {
        let absolute = open + idx;
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(absolute);
                }
            }
            _ => {}
        }
    }
    None
}

fn struct_derives_accounts(source: &str, struct_idx: usize) -> bool {
    const DERIVE_ACCOUNTS_MARKER: &str = "#[derive(Accounts";
    let before_struct = &source[..struct_idx.min(source.len())];
    let attribute_block_start = before_struct
        .rfind("\n\n")
        .map(|idx| idx + "\n\n".len())
        .unwrap_or(0);
    before_struct[attribute_block_start..].contains(DERIVE_ACCOUNTS_MARKER)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldContainer {
    Accounts(String),
    AccountData(String),
}

fn local_container_after_segments(
    document: &ParsedDocument,
    accounts_context: &str,
    completed_segments: &[String],
) -> Option<FieldContainer> {
    let mut container = accounts_context.to_string();
    for (index, segment) in completed_segments.iter().enumerate() {
        let accounts = document.symbols().accounts_structs.get(&container)?;
        let field = accounts
            .fields
            .iter()
            .find(|field| field.name == *segment)?;
        let is_last = index + 1 == completed_segments.len();
        if let Some(accounts_type) = field.type_name.as_ref().filter(|type_name| {
            document
                .symbols()
                .accounts_structs
                .contains_key(type_name.as_str())
        }) {
            container = accounts_type.clone();
            if is_last {
                return Some(FieldContainer::Accounts(container));
            }
            continue;
        }
        let account_data_type = field.generic_type_names.last()?.clone();
        if document
            .symbols()
            .account_data_structs
            .contains_key(&account_data_type)
            && is_last
        {
            return Some(FieldContainer::AccountData(account_data_type));
        }
        return None;
    }
    None
}

fn workspace_container_after_segments(
    workspace_index: &WorkspaceIndex,
    accounts_context: &str,
    completed_segments: &[String],
) -> Option<FieldContainer> {
    let mut container = accounts_context.to_string();
    for (index, segment) in completed_segments.iter().enumerate() {
        let accounts = workspace_index.accounts_struct(&container)?;
        let field = accounts
            .fields
            .iter()
            .find(|field| field.name == *segment)?;
        let is_last = index + 1 == completed_segments.len();
        if let Some(accounts_type) = field
            .type_name
            .as_ref()
            .filter(|type_name| workspace_index.accounts_struct(type_name).is_some())
        {
            container = accounts_type.clone();
            if is_last {
                return Some(FieldContainer::Accounts(container));
            }
            continue;
        }
        let account_data_type = field.generic_type_names.last()?.clone();
        if !workspace_index
            .field_names_in_container(&account_data_type)
            .is_empty()
            && is_last
        {
            return Some(FieldContainer::AccountData(account_data_type));
        }
        return None;
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldShape {
    type_name: Option<String>,
    generic_type_names: Vec<String>,
}

fn text_container_after_segments(
    source: &str,
    workspace_index: Option<&WorkspaceIndex>,
    accounts_context: &str,
    completed_segments: &[String],
) -> Option<FieldContainer> {
    let mut container = FieldContainer::Accounts(accounts_context.to_string());
    for (index, segment) in completed_segments.iter().enumerate() {
        let field = field_shape_for_segment(source, workspace_index, &container, segment)?;
        container = next_container_for_field(source, workspace_index, &field)?;
        if index + 1 == completed_segments.len() {
            return Some(container);
        }
    }
    None
}

fn field_shape_for_segment(
    source: &str,
    workspace_index: Option<&WorkspaceIndex>,
    container: &FieldContainer,
    segment: &str,
) -> Option<FieldShape> {
    match container {
        FieldContainer::Accounts(name) => text_field_shape(source, name, segment).or_else(|| {
            workspace_index.and_then(|index| {
                index
                    .accounts_struct(name)?
                    .fields
                    .iter()
                    .find(|field| field.name == segment)
                    .map(field_shape_from_workspace_account_field)
            })
        }),
        FieldContainer::AccountData(name) => {
            text_field_shape(source, name, segment).or_else(|| {
                workspace_index.and_then(|index| {
                    index
                        .field_info_in_container(segment, name)
                        .and_then(|field| field.type_display)
                        .and_then(|display| field_shape_from_type_display(&display))
                })
            })
        }
    }
}

fn text_field_shape(source: &str, container: &str, segment: &str) -> Option<FieldShape> {
    text_struct_fields(source, container)?
        .fields
        .into_iter()
        .find(|field| field.name == segment)
        .map(|field| field_shape_from_type_summary(field.type_summary))
}

fn next_container_for_field(
    source: &str,
    workspace_index: Option<&WorkspaceIndex>,
    field: &FieldShape,
) -> Option<FieldContainer> {
    if let Some(type_name) = field.type_name.as_deref() {
        if has_accounts_struct(source, workspace_index, type_name) {
            return Some(FieldContainer::Accounts(type_name.to_string()));
        }
    }

    field
        .generic_type_names
        .last()
        .or(field.type_name.as_ref())
        .filter(|name| has_account_data_struct(source, workspace_index, name))
        .map(|name| FieldContainer::AccountData(name.clone()))
}

fn has_accounts_struct(source: &str, workspace_index: Option<&WorkspaceIndex>, name: &str) -> bool {
    text_accounts_struct_fields(source, name).is_some()
        || workspace_index.is_some_and(|index| index.accounts_struct(name).is_some())
}

fn has_account_data_struct(
    source: &str,
    workspace_index: Option<&WorkspaceIndex>,
    name: &str,
) -> bool {
    text_struct_fields(source, name).is_some()
        || workspace_index.is_some_and(|index| !index.field_names_in_container(name).is_empty())
}

fn field_shape_from_workspace_account_field(field: &WorkspaceAccountField) -> FieldShape {
    FieldShape {
        type_name: field.type_name.clone(),
        generic_type_names: field.generic_type_names.clone(),
    }
}

fn field_shape_from_type_summary(summary: AccountFieldTypeSummary) -> FieldShape {
    FieldShape {
        type_name: summary.type_name,
        generic_type_names: summary.generic_type_names,
    }
}

fn field_shape_from_type_display(display: &str) -> Option<FieldShape> {
    let ty = syn::parse_str::<syn::Type>(display).ok()?;
    Some(field_shape_from_type_summary(summarize_account_field_type(
        &ty,
    )))
}

fn account_field_candidates(container: &str, fields: &[SymbolRange]) -> Vec<FieldCandidate> {
    fields
        .iter()
        .map(|field| FieldCandidate {
            name: field.name.clone(),
            detail: field_type_display(field)
                .map(|display| format!("Anchor account field in `{container}`: `{display}`"))
                .unwrap_or_else(|| format!("Anchor account field in `{container}`")),
            kind: CompletionItemKind::FIELD,
        })
        .collect()
}

fn account_data_field_candidates(container: &str, fields: &[SymbolRange]) -> Vec<FieldCandidate> {
    fields
        .iter()
        .map(|field| FieldCandidate {
            name: field.name.clone(),
            detail: field_type_display(field)
                .map(|display| format!("Anchor account data field in `{container}`: `{display}`"))
                .unwrap_or_else(|| format!("Anchor account data field in `{container}`")),
            kind: CompletionItemKind::FIELD,
        })
        .collect()
}

fn workspace_account_field_candidate(
    container: &str,
    field: &WorkspaceAccountField,
) -> FieldCandidate {
    FieldCandidate {
        name: field.name.clone(),
        detail: workspace_field_type_display(field)
            .map(|display| format!("Anchor account field in `{container}`: `{display}`"))
            .unwrap_or_else(|| format!("Anchor account field in `{container}`")),
        kind: CompletionItemKind::FIELD,
    }
}

fn field_type_display(field: &SymbolRange) -> Option<String> {
    let type_name = field.type_name.as_ref()?;
    if field.generic_type_names.is_empty() {
        Some(type_name.clone())
    } else {
        Some(format!(
            "{}<{}>",
            type_name,
            field.generic_type_names.join(", ")
        ))
    }
}

fn workspace_field_type_display(field: &WorkspaceAccountField) -> Option<String> {
    let type_name = field.type_name.as_ref()?;
    if field.generic_type_names.is_empty() {
        Some(type_name.clone())
    } else {
        Some(format!(
            "{}<{}>",
            type_name,
            field.generic_type_names.join(", ")
        ))
    }
}

fn account_path_completion_context(
    source: &str,
    position: Position,
) -> Option<AccountPathCompletionContext> {
    ctx_accounts_completion_context(source, position)
        .or_else(|| local_alias_completion_context(source, position))
}

fn ctx_accounts_completion_context(
    source: &str,
    position: Position,
) -> Option<AccountPathCompletionContext> {
    let line = line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let prefix = &line[..cursor];
    let accounts_start = prefix.rfind(".accounts.")? + ".accounts.".len();
    let tail = &prefix[accounts_start..];
    let trailing_dot = tail.ends_with('.');
    let tail = tail.strip_suffix('.').unwrap_or(tail);
    if !tail
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return None;
    }
    let mut segments = if tail.is_empty() {
        Vec::new()
    } else {
        tail.split('.').map(str::to_string).collect::<Vec<_>>()
    };
    let prefix = if trailing_dot || tail.is_empty() {
        String::new()
    } else {
        segments.pop()?
    };
    Some(AccountPathCompletionContext {
        completed_segments: segments,
        prefix,
    })
}

fn local_alias_completion_context(
    source: &str,
    position: Position,
) -> Option<AccountPathCompletionContext> {
    let alias_path = super::account_aliases::local_account_alias_path_at(source, position)?;
    let mut completed_segments = Vec::with_capacity(alias_path.member_chain.len() + 1);
    completed_segments.push(alias_path.account_field);
    completed_segments.extend(alias_path.member_chain);
    Some(AccountPathCompletionContext {
        completed_segments,
        prefix: alias_path.member_prefix,
    })
}

fn matches_prefix(value: &str, prefix: &str) -> bool {
    value
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

fn prefix_replacement_range(position: Position, prefix: &str) -> Range {
    let prefix_len = u32::try_from(prefix.chars().count()).unwrap_or_default();
    Range {
        start: Position {
            line: position.line,
            character: position.character.saturating_sub(prefix_len),
        },
        end: position,
    }
}

fn contains_position(range: tower_lsp::lsp_types::Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
}

fn text_enclosing_context_name(source: &str, position: Position) -> Option<String> {
    let offset = crate::range::byte_offset_at(source, position)?;
    let before_cursor = &source[..offset.min(source.len())];
    let function_start = super::cursor_context::last_function_keyword_before(before_cursor)?;
    let function_prefix = &before_cursor[function_start..];
    let context_open = function_prefix.rfind("Context<")? + "Context<".len();
    let after_context_open = &function_prefix[context_open..];
    let end = after_context_open
        .char_indices()
        .find_map(|(idx, ch)| (!(ch.is_ascii_alphanumeric() || ch == '_')).then_some(idx))
        .unwrap_or(after_context_open.len());
    let context = &after_context_open[..end];
    (!context.is_empty()).then(|| context.to_string())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            document::ParsedDocument, lsp::completions::proptest_support::rust_identifier,
            workspace::WorkspaceIndex,
        },
        proptest::prelude::*,
        tower_lsp::lsp_types::Url,
    };

    #[test]
    fn completes_top_level_ctx_accounts_fields_after_typing() {
        let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.c;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
    pub authority: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let items = completions(&document, position_after(source, "ctx.accounts.c"), None).unwrap();

        assert!(items.iter().any(|item| item.label == "counter"));
        assert!(!items.iter().any(|item| item.label == "authority"));
        let counter = items.iter().find(|item| item.label == "counter").unwrap();
        let Some(CompletionTextEdit::Edit(edit)) = &counter.text_edit else {
            panic!("expected prefix replacement edit");
        };
        assert_eq!(edit.new_text, "counter");
        assert_eq!(edit.range.start.character + 1, edit.range.end.character);
    }

    #[test]
    fn completes_ctx_accounts_fields_after_dot_trigger() {
        let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#;
        let document = ParsedDocument::parse_or_empty(source);

        let items = completions(&document, position_after(source, "ctx.accounts."), None).unwrap();

        assert!(items.iter().any(|item| item.label == "counter"));
    }

    #[test]
    fn completes_nested_composite_account_fields() {
        let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.i;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let items = completions(&document, position_after(source, "wrapper.i"), None).unwrap();

        assert_eq!(items[0].label, "inner");
        assert!(items[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Wrapped")));
    }

    #[test]
    fn completes_account_data_fields_after_account_field() {
        let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.c;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}

#[account]
pub struct Counter {
    pub count: u64,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let items = completions(&document, position_after(source, "counter.c"), None).unwrap();

        assert_eq!(items[0].label, "count");
        assert!(items[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("Counter")));
    }

    #[test]
    fn completes_workspace_split_account_fields() {
        let lib = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.c;
        Ok(())
    }
}
"#,
        )
        .unwrap();
        let index = WorkspaceIndex::build(
            &[],
            [(
                Url::parse("file:///tmp/accounts.rs").unwrap(),
                r#"
#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#
                .to_string(),
            )],
        );

        let items = completions(
            &lib,
            position_after(lib.source(), "ctx.accounts.c"),
            Some(&index),
        )
        .unwrap();

        assert_eq!(items[0].label, "counter");
    }

    #[test]
    fn completes_account_data_members_after_local_account_alias_dot() {
        let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.

    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let index = WorkspaceIndex::build(
            &[],
            [
                (
                    Url::parse("file:///tmp/close_bundled_position.rs").unwrap(),
                    r#"
#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}
"#
                    .to_string(),
                ),
                (
                    Url::parse("file:///tmp/state.rs").unwrap(),
                    r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub owner: Pubkey,
}
"#
                    .to_string(),
                ),
            ],
        );

        let items = completions(
            &document,
            position_after(source, "position_bundle."),
            Some(&index),
        )
        .unwrap();

        assert_eq!(items[0].label, "owner");
        assert!(items
            .iter()
            .any(|item| item.label == "position_bundle_mint"));
    }

    #[test]
    fn completes_same_file_account_alias_members_when_handler_is_incomplete() {
        let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.

    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let index = WorkspaceIndex::build(
            &[],
            [(
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#
                .to_string(),
            )],
        );

        let items = completions(
            &document,
            position_after(source, "position_bundle."),
            Some(&index),
        )
        .expect("same-file account alias member completions should recover");

        assert!(items
            .iter()
            .any(|item| item.label == "position_bundle_mint"));
        assert!(items.iter().any(|item| item.label == "position_bitmap"));
    }

    #[test]
    fn completes_same_file_ctx_account_members_when_handler_is_incomplete() {
        let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    ctx.accounts.position_bundle.

    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let index = WorkspaceIndex::build(
            &[],
            [(
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
                .to_string(),
            )],
        );

        let items = completions(
            &document,
            position_after(source, "ctx.accounts.position_bundle."),
            Some(&index),
        )
        .expect("same-file ctx account member completions should recover");

        assert_eq!(items[0].label, "position_bundle_mint");
    }

    #[test]
    fn completes_account_data_members_after_accounts_alias_dot() {
        let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let afn = &mut ctx.accounts;
    let position_bundle = &mut afn.position_bundle;
    position_bundle.position_bundle_m;

    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let index = WorkspaceIndex::build(
            &[],
            [
                (
                    Url::parse("file:///tmp/close_bundled_position.rs").unwrap(),
                    r#"
#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}
"#
                    .to_string(),
                ),
                (
                    Url::parse("file:///tmp/state.rs").unwrap(),
                    r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub owner: Pubkey,
}
"#
                    .to_string(),
                ),
            ],
        );

        let items = completions(
            &document,
            position_after(source, "position_bundle.position_bundle_m"),
            Some(&index),
        )
        .unwrap();

        assert_eq!(items[0].label, "position_bundle_mint");
    }

    proptest! {
        #[test]
        fn completes_members_for_generated_account_alias_shapes(
            alias in rust_identifier(),
            account_field in rust_identifier(),
            data_field in rust_identifier(),
            mutable in any::<bool>(),
            reference in any::<bool>(),
        ) {
            prop_assume!(alias != "ctx" && alias != account_field && alias != data_field && account_field != data_field);
            let member_prefix = data_field.chars().next().unwrap_or_default().to_string();
            let mutability = mutable.then_some("mut ").unwrap_or_default();
            let reference = reference.then_some("&").unwrap_or_default();
            let source = format!(
                "use anchor_lang::prelude::*;\n\npub fn handler(ctx: Context<Run>) -> Result<()> {{\n    let {alias} = {reference}{mutability}ctx.accounts.{account_field};\n    {alias}.{member_prefix}\n    Ok(())\n}}\n"
            );
            let completion_line = format!("    {alias}.{member_prefix}");
            let document = ParsedDocument::parse_or_empty(&source);
            let index = WorkspaceIndex::build(
                &[],
                [
                    (
                        Url::parse("file:///tmp/accounts.rs").unwrap(),
                        format!(
                            "#[derive(Accounts)]\npub struct Run<'info> {{\n    pub {account_field}: Account<'info, AccountData>,\n}}\n"
                        ),
                    ),
                    (
                        Url::parse("file:///tmp/state.rs").unwrap(),
                        format!(
                            "#[account]\npub struct AccountData {{\n    pub {data_field}: Pubkey,\n}}\n"
                        ),
                    ),
                ],
            );

            let items = completions(
                &document,
                position_after(&source, &completion_line),
                Some(&index),
            )
            .expect("alias member completions should resolve");

            prop_assert!(items.iter().any(|item| item.label == data_field));
        }
    }

    fn position_after(source: &str, needle: &str) -> Position {
        let offset = source.find(needle).expect("needle") + needle.len();
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix
            .rsplit('\n')
            .next()
            .map(|line| line.chars().count())
            .unwrap_or_default() as u32;
        Position { line, character }
    }
}
