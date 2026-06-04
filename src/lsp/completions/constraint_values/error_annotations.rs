//! Completions for the error annotation that may follow any constraint value:
//! `#[account(has_one = authority @ MyError::Unauthorized)]`. The cursor sits
//! after an `@`, so we offer the `#[error_code]` enum variants in scope as
//! fully-qualified `Enum::Variant` paths.

use {
    crate::{document::ParsedDocument, range::byte_offset_at},
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position},
};

const ACCOUNT_ATTRIBUTE_OPEN: &str = "#[account(";
const ERROR_ANNOTATION_SEPARATOR: char = '@';

/// Returns the partial error path typed after `@` when the cursor is in an
/// error-annotation position, or `None` when it is anywhere else.
pub(super) fn error_annotation_prefix(source: &str, position: Position) -> Option<String> {
    let cursor = byte_offset_at(source, position)?;
    let before_cursor = source.get(..cursor)?;
    let attribute_start =
        before_cursor.rfind(ACCOUNT_ATTRIBUTE_OPEN)? + ACCOUNT_ATTRIBUTE_OPEN.len();
    let inside = &before_cursor[attribute_start..];
    let annotation_start = error_annotation_start(inside)?;
    let prefix = inside[annotation_start..].trim_start();
    is_error_path_prefix(prefix).then(|| prefix.to_string())
}

pub(super) fn error_variant_items(document: &ParsedDocument) -> Vec<CompletionItem> {
    document
        .symbols()
        .error_codes
        .iter()
        .flat_map(|error_code| {
            error_code
                .variants
                .iter()
                .map(move |variant| error_variant_item(&error_code.name, variant))
        })
        .collect()
}

fn error_variant_item(enum_name: &str, variant: &str) -> CompletionItem {
    CompletionItem {
        label: format!("{enum_name}::{variant}"),
        kind: Some(CompletionItemKind::ENUM_MEMBER),
        detail: Some("Anchor error code".to_string()),
        sort_text: Some(format!("000_anchor_error_{enum_name}_{variant}")),
        data: Some(serde_json::json!({
            "anchorCompletion": "constraint-error-annotation",
        })),
        ..CompletionItem::default()
    }
}

/// The annotation is active only when the last depth-0 separator before the
/// cursor is `@` (a following `,` or `=` means we moved on to another
/// constraint). Returns the byte index just past that `@`.
fn error_annotation_start(inside: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut after_separator: Option<usize> = None;
    let mut separator_is_annotation = false;

    for (idx, ch) in inside.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            // A depth-0 closer means the `#[account(...)]` ended before the
            // cursor, so we are no longer inside the attribute.
            ')' | ']' | '}' if depth == 0 => return None,
            ')' | ']' | '}' => depth -= 1,
            ERROR_ANNOTATION_SEPARATOR if depth == 0 => {
                after_separator = Some(idx + ch.len_utf8());
                separator_is_annotation = true;
            }
            ',' | '=' if depth == 0 => separator_is_annotation = false,
            _ => {}
        }
    }

    separator_is_annotation.then_some(after_separator).flatten()
}

fn is_error_path_prefix(prefix: &str) -> bool {
    prefix
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
}
