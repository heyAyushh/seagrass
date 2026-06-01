use {
    super::{
        INIT_CONSTRAINT_KEY, INIT_IF_NEEDED_CONSTRAINT_KEY, PAYER_CONSTRAINT_KEY,
        SPACE_CONSTRAINT_KEY,
    },
    crate::document::{AccountAttributeCursor, AccountAttributeSlot, ParsedDocument},
    tower_lsp::lsp_types::{Position, Range},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSignatureKind {
    AccountPath,
    ContextType,
    AccountConstraintKey,
    AccountConstraintValue,
    InstructionAttribute,
    AccountsField,
    HandlerValue,
    HandlerMember,
    HandlerStructField,
}

impl CompletionSignatureKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::AccountPath => "accountPath",
            Self::ContextType => "contextType",
            Self::AccountConstraintKey => "accountConstraintKey",
            Self::AccountConstraintValue => "accountConstraintValue",
            Self::InstructionAttribute => "instructionAttribute",
            Self::AccountsField => "accountsField",
            Self::HandlerValue => "handlerValue",
            Self::HandlerMember => "handlerMember",
            Self::HandlerStructField => "handlerStructField",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSignature {
    pub line: u32,
    pub kind: CompletionSignatureKind,
    pub prefix: String,
}

impl CompletionSignature {
    fn new(line: u32, kind: CompletionSignatureKind, prefix: &str) -> Self {
        Self {
            line,
            kind,
            prefix: prefix.to_string(),
        }
    }

    pub fn cache_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.line,
            self.kind.as_str(),
            self.prefix.to_ascii_lowercase()
        )
    }
}

pub fn completion_signature(source: &str, position: Position) -> Option<CompletionSignature> {
    CursorContext::classify_source(source, position).signature(position.line)
}

#[allow(dead_code)]
pub fn should_offer_completion(source: &str, position: Position) -> bool {
    completion_signature(source, position).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CursorContext {
    kind: CursorContextKind,
    context: ResolvedCursorContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CursorContextKind {
    AccountPath {
        prefix: String,
    },
    ContextType {
        prefix: String,
    },
    AccountConstraintKey {
        context: AccountConstraintCompletionContext,
    },
    AccountConstraintValue {
        prefix: String,
    },
    InstructionAttribute {
        prefix: String,
    },
    AccountsField {
        prefix: String,
    },
    HandlerValue {
        prefix: String,
    },
    HandlerMember {
        prefix: String,
    },
    HandlerStructField {
        context: super::handler_struct_fields::StructLiteralFieldContext,
    },
    NotAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountConstraintCompletionContext {
    pub(super) prefix: String,
    pub(super) account_field: Option<String>,
    pub(super) has_init_constraint: bool,
    pub(super) has_payer_constraint: bool,
    pub(super) has_space_constraint: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ResolvedCursorContext {
    pub accounts_struct: Option<ResolvedAccountsStructHandle>,
    pub account_field: Option<ResolvedFieldHandle>,
    pub enclosing_instruction: Option<ResolvedInstructionHandle>,
    pub instruction_arguments: Vec<ResolvedInstructionArgumentHandle>,
    pub cpi_sites: Vec<ResolvedCpiSiteHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedAccountsStructHandle {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedFieldHandle {
    pub name: String,
    pub type_name: Option<String>,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedInstructionHandle {
    pub name: String,
    pub context_name: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedInstructionArgumentHandle {
    pub name: String,
    pub type_name: Option<String>,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedCpiSiteHandle {
    pub instruction_name: String,
    pub account_name: String,
    pub range: Range,
}

const HANDLER_EMPTY_VALUE_SLOT_CHARS: &[char] = &[
    '(', '[', ',', '=', '>', '<', '&', '|', '!', '+', '-', '*', '/', '%',
];
const HANDLER_EMPTY_VALUE_SLOT_KEYWORDS: &[&str] = &["return", "break", "if", "while", "match"];

impl CursorContext {
    pub(crate) fn classify_document(document: &ParsedDocument, position: Position) -> Self {
        let account_cursor = document.account_attribute_cursor(position);
        let mut kind =
            Self::classify_with_account_cursor(document.source(), position, account_cursor.clone());
        if matches!(kind, CursorContextKind::NotAnchor) {
            if let Some(recovered) = document.tree_sitter().and_then(|syntax| {
                syntax.context_type_prefix_at_position(document.source(), position)
            }) {
                kind = CursorContextKind::ContextType {
                    prefix: recovered.prefix,
                };
            }
        }
        let context =
            ResolvedCursorContext::from_document(document, position, account_cursor.as_ref());
        Self { kind, context }
    }

    fn classify_source(source: &str, position: Position) -> Self {
        let kind = Self::classify_with_account_cursor(
            source,
            position,
            AccountAttributeCursor::from_source(source, position),
        );
        Self {
            kind,
            context: ResolvedCursorContext::default(),
        }
    }

    pub(crate) fn kind(&self) -> &CursorContextKind {
        &self.kind
    }

    pub(crate) fn context(&self) -> &ResolvedCursorContext {
        &self.context
    }

    fn classify_with_account_cursor(
        source: &str,
        position: Position,
        account_cursor: Option<AccountAttributeCursor>,
    ) -> CursorContextKind {
        let Some((offset, line, cursor)) = completion_line(source, position) else {
            return CursorContextKind::NotAnchor;
        };
        if crate::range::is_in_comment_or_string(source, position) {
            return CursorContextKind::NotAnchor;
        }

        let line_prefix = &line[..cursor];
        if let Some(cursor) = account_cursor {
            return match cursor.slot {
                AccountAttributeSlot::Value => CursorContextKind::AccountConstraintValue {
                    prefix: cursor.prefix,
                },
                AccountAttributeSlot::Key => CursorContextKind::AccountConstraintKey {
                    context: AccountConstraintCompletionContext::from_cursor(source, cursor),
                },
            };
        }

        if let Some(prefix) = account_path_typed_prefix(source, offset, line_prefix) {
            return CursorContextKind::AccountPath {
                prefix: prefix.to_string(),
            };
        }
        if has_enclosing_anchor_context(source, offset) {
            if let Some(path) =
                super::account_aliases::local_account_alias_path_at(source, position)
            {
                return CursorContextKind::AccountPath {
                    prefix: path.member_prefix,
                };
            }
        }

        if let Some(prefix) = context_type_typed_prefix(source, offset, line_prefix) {
            return CursorContextKind::ContextType {
                prefix: prefix.to_string(),
            };
        }

        if let Some(prefix) = instruction_attribute_typed_prefix(line_prefix) {
            return CursorContextKind::InstructionAttribute {
                prefix: prefix.to_string(),
            };
        }

        if looks_like_accounts_field_completion(source, offset, line_prefix) {
            return CursorContextKind::AccountsField {
                prefix: account_field_hint_prefix(line_prefix)
                    .unwrap_or_default()
                    .to_string(),
            };
        }

        if has_enclosing_anchor_context(source, offset)
            && !has_enclosing_function_signature(source, offset)
        {
            if let Some(context) = super::handler_struct_fields::context_at(source, offset) {
                return CursorContextKind::HandlerStructField { context };
            }
        }

        if let Some(prefix) = handler_value_typed_prefix(source, offset, line_prefix) {
            return CursorContextKind::HandlerValue {
                prefix: prefix.to_string(),
            };
        }
        if let Some(prefix) = handler_member_typed_prefix(source, offset, line_prefix) {
            return CursorContextKind::HandlerMember {
                prefix: prefix.to_string(),
            };
        }

        CursorContextKind::NotAnchor
    }

    fn signature(&self, line: u32) -> Option<CompletionSignature> {
        let (kind, prefix) = match self {
            Self {
                kind: CursorContextKind::AccountPath { prefix },
                ..
            } => (CompletionSignatureKind::AccountPath, prefix),
            Self {
                kind: CursorContextKind::ContextType { prefix },
                ..
            } => (CompletionSignatureKind::ContextType, prefix),
            Self {
                kind: CursorContextKind::AccountConstraintKey { context },
                ..
            } => (
                CompletionSignatureKind::AccountConstraintKey,
                &context.prefix,
            ),
            Self {
                kind: CursorContextKind::AccountConstraintValue { prefix },
                ..
            } => (CompletionSignatureKind::AccountConstraintValue, prefix),
            Self {
                kind: CursorContextKind::InstructionAttribute { prefix },
                ..
            } => (CompletionSignatureKind::InstructionAttribute, prefix),
            Self {
                kind: CursorContextKind::AccountsField { prefix },
                ..
            } => (CompletionSignatureKind::AccountsField, prefix),
            Self {
                kind: CursorContextKind::HandlerValue { prefix },
                ..
            } => (CompletionSignatureKind::HandlerValue, prefix),
            Self {
                kind: CursorContextKind::HandlerMember { prefix },
                ..
            } => (CompletionSignatureKind::HandlerMember, prefix),
            Self {
                kind: CursorContextKind::HandlerStructField { context },
                ..
            } => (CompletionSignatureKind::HandlerStructField, &context.prefix),
            Self {
                kind: CursorContextKind::NotAnchor,
                ..
            } => return None,
        };
        Some(CompletionSignature::new(line, kind, prefix))
    }
}

impl AccountConstraintCompletionContext {
    fn from_cursor(source: &str, cursor: AccountAttributeCursor) -> Self {
        let attribute_text = text_in_range(source, cursor.range).unwrap_or_default();
        Self {
            prefix: cursor.prefix,
            account_field: cursor.field_name,
            has_init_constraint: crate::constraint_text::has_flag_or_key(
                attribute_text,
                INIT_CONSTRAINT_KEY,
            ) || crate::constraint_text::has_flag_or_key(
                attribute_text,
                INIT_IF_NEEDED_CONSTRAINT_KEY,
            ),
            has_payer_constraint: crate::constraint_text::has_key(
                attribute_text,
                PAYER_CONSTRAINT_KEY,
            ),
            has_space_constraint: crate::constraint_text::has_key(
                attribute_text,
                SPACE_CONSTRAINT_KEY,
            ),
        }
    }
}

impl ResolvedCursorContext {
    fn from_document(
        document: &ParsedDocument,
        position: Position,
        account_cursor: Option<&AccountAttributeCursor>,
    ) -> Self {
        let Some(accounts) = accounts_struct_for_cursor(document, position) else {
            return Self::default();
        };
        let account_field = account_cursor
            .and_then(|cursor| cursor.field_name.as_deref())
            .and_then(|field_name| {
                accounts
                    .fields
                    .iter()
                    .find(|field| field.name == field_name)
            })
            .or_else(|| {
                accounts
                    .fields
                    .iter()
                    .find(|field| contains_position(field.range, position))
            });
        let instructions = document
            .symbols()
            .callable_functions()
            .filter(|instruction| {
                instruction
                    .context
                    .as_ref()
                    .is_some_and(|context| context.name == accounts.name)
            })
            .collect::<Vec<_>>();
        let enclosing_instruction = instructions
            .iter()
            .copied()
            .find(|instruction| contains_position(instruction.range, position))
            .or_else(|| instructions.first().copied());

        let mut instruction_arguments = enclosing_instruction
            .into_iter()
            .flat_map(|instruction| instruction.arguments.iter())
            .map(|argument| ResolvedInstructionArgumentHandle {
                name: argument.name.clone(),
                type_name: argument.type_name.clone(),
                range: argument.range,
            })
            .collect::<Vec<_>>();
        instruction_arguments.extend(accounts.instruction_arguments.iter().map(|argument| {
            ResolvedInstructionArgumentHandle {
                name: argument.name.clone(),
                type_name: argument.type_name.clone(),
                range: argument.range,
            }
        }));
        instruction_arguments.sort_by(|left, right| left.name.cmp(&right.name));
        instruction_arguments.dedup_by(|left, right| left.name == right.name);

        let cpi_sites = instructions
            .iter()
            .flat_map(|instruction| {
                instruction
                    .cpi_program_usages
                    .iter()
                    .map(|usage| ResolvedCpiSiteHandle {
                        instruction_name: instruction.name.clone(),
                        account_name: usage.name.clone(),
                        range: usage.range,
                    })
            })
            .collect::<Vec<_>>();

        Self {
            accounts_struct: Some(ResolvedAccountsStructHandle {
                name: accounts.name.clone(),
                range: accounts.range,
            }),
            account_field: account_field.map(|field| ResolvedFieldHandle {
                name: field.name.clone(),
                type_name: field_type_display(field),
                range: field.range,
            }),
            enclosing_instruction: enclosing_instruction.and_then(|instruction| {
                instruction
                    .context
                    .as_ref()
                    .map(|context| ResolvedInstructionHandle {
                        name: instruction.name.clone(),
                        context_name: context.name.clone(),
                        range: instruction.range,
                    })
            }),
            instruction_arguments,
            cpi_sites,
        }
    }
}

fn accounts_struct_for_cursor(
    document: &ParsedDocument,
    position: Position,
) -> Option<&crate::document::SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .filter(|accounts| contains_line(accounts.range, position.line))
        .min_by_key(|accounts| {
            accounts
                .range
                .end
                .line
                .saturating_sub(accounts.range.start.line)
        })
}

fn field_type_display(field: &crate::document::SymbolRange) -> Option<String> {
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

fn contains_line(range: Range, line: u32) -> bool {
    range.start.line <= line && line <= range.end.line
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

fn text_in_range(source: &str, range: Range) -> Option<&str> {
    let start = crate::range::byte_offset_at(source, range.start)?;
    let end = crate::range::byte_offset_at(source, range.end)?;
    source.get(start..end)
}

fn completion_line(source: &str, position: Position) -> Option<(usize, &str, usize)> {
    let line = crate::range::line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let offset = crate::range::byte_offset_at(source, position)?;
    Some((offset, line, cursor))
}

fn account_path_typed_prefix<'a>(source: &str, offset: usize, prefix: &'a str) -> Option<&'a str> {
    if !has_enclosing_anchor_context(source, offset) {
        return None;
    }
    let accounts_start = prefix.rfind(".accounts.")? + ".accounts.".len();
    let receiver = &prefix[..accounts_start - ".accounts.".len()];
    if !receiver
        .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        .next()
        .and_then(|path| path.rsplit('.').next())
        .is_some_and(is_anchor_context_receiver)
    {
        return None;
    }
    let tail = &prefix[accounts_start..];
    if tail.ends_with('.') {
        return Some("");
    }
    tail.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
        .then(|| tail.rsplit('.').next().unwrap_or_default())
}

fn context_type_typed_prefix<'a>(source: &str, offset: usize, prefix: &'a str) -> Option<&'a str> {
    if !has_anchor_framework_hint(source, offset) {
        return None;
    }
    let open = prefix.rfind('<')?;
    let before_open = prefix[..open].trim_end();
    let context_start = before_open
        .rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    if !before_open[..context_start].trim_end().ends_with(':') {
        return None;
    }
    if !has_enclosing_function_signature(source, offset) {
        return None;
    }
    let context_name = before_open[context_start..].rsplit("::").next()?;
    if context_name != "Context" {
        return None;
    }

    let tail = prefix[open + 1..].trim_start();
    if tail.contains('>') || tail.contains(',') {
        return None;
    }
    let end = tail
        .char_indices()
        .find_map(|(idx, ch)| (!(ch.is_ascii_alphanumeric() || ch == '_')).then_some(idx))
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

fn has_enclosing_function_signature(source: &str, offset: usize) -> bool {
    let before_cursor = &source[..offset.min(source.len())];
    let Some(function_start) = last_function_keyword_before(before_cursor) else {
        return false;
    };
    let function_prefix = &before_cursor[function_start..];
    function_prefix.contains('(') && !function_prefix.contains('{')
}

fn has_enclosing_anchor_context(source: &str, offset: usize) -> bool {
    if !has_anchor_framework_hint(source, offset) {
        return false;
    }
    let before_cursor = &source[..offset.min(source.len())];
    let Some(function_start) = last_function_keyword_before(before_cursor) else {
        return false;
    };
    before_cursor[function_start..].contains("Context<")
}

pub(crate) fn last_function_keyword_before(source: &str) -> Option<usize> {
    crate::lsp::scope::last_function_keyword_before(source)
}

fn has_anchor_framework_hint(source: &str, offset: usize) -> bool {
    let before_cursor = &source[..offset.min(source.len())];
    crate::solana::frameworks::source_has_anchor_framework_hint(before_cursor)
}

fn is_anchor_context_receiver(receiver: &str) -> bool {
    matches!(receiver, "ctx" | "context")
}

pub(crate) fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn instruction_attribute_typed_prefix(prefix: &str) -> Option<&str> {
    let start = prefix.rfind("#[instruction(")? + "#[instruction(".len();
    typed_tail(&prefix[start..])
}

fn typed_tail(text: &str) -> Option<&str> {
    let tail_start = text
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| matches!(ch, ',' | '(' | '[' | '=').then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let tail = text[tail_start..].trim_start();
    let end = tail
        .char_indices()
        .find_map(|(idx, ch)| {
            (!(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')).then_some(idx)
        })
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

fn looks_like_accounts_field_completion(source: &str, offset: usize, line_prefix: &str) -> bool {
    if account_field_hint_prefix(line_prefix).is_none() {
        return false;
    }

    let before = &source[..offset.min(source.len())];
    let Some(derive_idx) = before.rfind("#[derive(Accounts") else {
        return false;
    };
    let after_derive = &before[derive_idx..];
    let Some(struct_idx) = after_derive.find("struct ") else {
        return false;
    };
    let Some(open_idx) = after_derive[struct_idx..].find('{') else {
        return false;
    };
    let open = derive_idx + struct_idx + open_idx;
    matching_close_brace(source, open).is_none_or(|close| close >= offset)
}

fn handler_value_typed_prefix<'a>(
    source: &str,
    offset: usize,
    line_prefix: &'a str,
) -> Option<&'a str> {
    if !has_enclosing_anchor_context(source, offset)
        || has_enclosing_function_signature(source, offset)
    {
        return None;
    }

    let tail_start = line_prefix
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| (!is_identifier_char(ch)).then_some(idx + ch.len_utf8()))
        .unwrap_or(0);
    let prefix = &line_prefix[tail_start..];
    if prefix.is_empty() && !empty_handler_value_prefix_allowed(source, offset, line_prefix) {
        return None;
    }
    let previous = line_prefix[..tail_start].chars().next_back();
    if matches!(previous, Some('.') | Some(':')) {
        return None;
    }
    if line_prefix[..tail_start]
        .rsplit([';', '{', '}'])
        .next()
        .is_some_and(|segment| segment.contains("let ") && !segment.contains('='))
    {
        return None;
    }

    Some(prefix)
}

fn empty_handler_value_prefix_allowed(source: &str, offset: usize, line_prefix: &str) -> bool {
    if crate::lsp::assertion_macros::has_open_expression_assertion_macro(source, offset)
        && line_prefix
            .chars()
            .rev()
            .find(|ch| !ch.is_whitespace())
            .is_none_or(|ch| matches!(ch, '(' | ',' | '!' | '=' | '>' | '<' | '&' | '|'))
    {
        return true;
    }

    let Some(previous) = previous_non_whitespace_char(line_prefix) else {
        return false;
    };
    HANDLER_EMPTY_VALUE_SLOT_CHARS.contains(&previous)
        || HANDLER_EMPTY_VALUE_SLOT_KEYWORDS
            .iter()
            .any(|keyword| line_prefix_ends_with_keyword(line_prefix, keyword))
}

fn handler_member_typed_prefix<'a>(
    source: &str,
    offset: usize,
    line_prefix: &'a str,
) -> Option<&'a str> {
    if !has_enclosing_anchor_context(source, offset)
        || has_enclosing_function_signature(source, offset)
    {
        return None;
    }

    let (receiver, member_prefix) = line_prefix.rsplit_once('.')?;
    if receiver.trim_end().is_empty() {
        return None;
    }
    if !member_prefix.chars().all(is_identifier_char) {
        return None;
    }
    Some(member_prefix)
}

fn account_field_hint_prefix(line_prefix: &str) -> Option<&str> {
    let trimmed = line_prefix.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("#[") {
        return None;
    }

    if line_prefix.contains(':') {
        let field_prefix = line_prefix
            .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .next()
            .unwrap_or_default();
        return Some(field_prefix);
    }

    if let Some(field_prefix) = trimmed
        .strip_prefix("pub ")
        .or_else(|| trimmed.strip_prefix("pub(crate) "))
    {
        return Some(field_prefix);
    }

    (!trimmed.is_empty()).then_some(trimmed)
}

pub(crate) fn matching_close_brace(source: &str, open: usize) -> Option<usize> {
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

fn previous_non_whitespace_char(text: &str) -> Option<char> {
    text.chars().rev().find(|ch| !ch.is_whitespace())
}

fn line_prefix_ends_with_keyword(line_prefix: &str, keyword: &str) -> bool {
    let trimmed = line_prefix.trim_end();
    let Some(before_keyword) = trimmed.strip_suffix(keyword) else {
        return false;
    };
    before_keyword
        .chars()
        .next_back()
        .is_none_or(|ch| !is_identifier_char(ch))
}
