use {
    crate::constants::SERVER_ID,
    zed_extension_api::{
        self as zed,
        lsp::{Completion, CompletionKind, Symbol, SymbolKind},
    },
};

const FUNCTION_PREFIX: &str = "fn ";
const FUNCTION_SUFFIX: &str = "()";
const FUNCTION_PARSE_SUFFIX: &str = " {}";
const MODULE_PREFIX: &str = "mod ";
const MODULE_PARSE_SUFFIX: &str = ";";
const STRUCT_PREFIX: &str = "struct ";
const STRUCT_PARSE_SUFFIX: &str = ";";
const ENUM_PREFIX: &str = "enum ";
const ENUM_PARSE_SUFFIX: &str = " {}";
const FIELD_TYPE_SEPARATOR: &str = ": ";

pub(crate) fn label_for_completion(
    language_server_id: &zed::LanguageServerId,
    completion: Completion,
) -> Option<zed::CodeLabel> {
    if language_server_id.as_ref() != SERVER_ID {
        return None;
    }

    completion_code_label(completion)
}

pub(crate) fn label_for_symbol(
    language_server_id: &zed::LanguageServerId,
    symbol: Symbol,
) -> Option<zed::CodeLabel> {
    if language_server_id.as_ref() != SERVER_ID {
        return None;
    }

    symbol_code_label(symbol)
}

fn completion_code_label(completion: Completion) -> Option<zed::CodeLabel> {
    let label = completion.label.trim();
    if label.is_empty() {
        return None;
    }

    let detail = completion
        .label_details
        .as_ref()
        .and_then(|details| details.detail.as_deref())
        .or(completion.detail.as_deref())
        .map(str::trim)
        .filter(|detail| !detail.is_empty() && *detail != label);

    let kind = completion.kind.as_ref();
    Some(match kind {
        Some(CompletionKind::Function | CompletionKind::Method | CompletionKind::Constructor)
            if is_rust_identifier(label) =>
        {
            parsed_code_label(
                FUNCTION_PREFIX,
                label,
                FUNCTION_SUFFIX,
                FUNCTION_PARSE_SUFFIX,
            )
        }
        Some(CompletionKind::Module) if is_rust_identifier(label) => {
            parsed_code_label(MODULE_PREFIX, label, "", MODULE_PARSE_SUFFIX)
        }
        Some(CompletionKind::Struct | CompletionKind::Class | CompletionKind::Interface)
            if is_rust_identifier(label) =>
        {
            parsed_code_label(STRUCT_PREFIX, label, "", STRUCT_PARSE_SUFFIX)
        }
        Some(CompletionKind::Enum) if is_rust_identifier(label) => {
            parsed_code_label(ENUM_PREFIX, label, "", ENUM_PARSE_SUFFIX)
        }
        Some(
            CompletionKind::Field
            | CompletionKind::Property
            | CompletionKind::Variable
            | CompletionKind::Constant
            | CompletionKind::EnumMember,
        ) => field_like_label(label, detail),
        _ => plain_label(label),
    })
}

fn symbol_code_label(symbol: Symbol) -> Option<zed::CodeLabel> {
    let name = symbol.name.trim();
    if name.is_empty() {
        return None;
    }

    Some(match symbol.kind {
        SymbolKind::Function | SymbolKind::Method if is_rust_identifier(name) => parsed_code_label(
            FUNCTION_PREFIX,
            name,
            FUNCTION_SUFFIX,
            FUNCTION_PARSE_SUFFIX,
        ),
        SymbolKind::Module | SymbolKind::Namespace if is_rust_identifier(name) => {
            parsed_code_label(MODULE_PREFIX, name, "", MODULE_PARSE_SUFFIX)
        }
        SymbolKind::Struct | SymbolKind::Class | SymbolKind::Interface
            if is_rust_identifier(name) =>
        {
            parsed_code_label(STRUCT_PREFIX, name, "", STRUCT_PARSE_SUFFIX)
        }
        SymbolKind::Enum if is_rust_identifier(name) => {
            parsed_code_label(ENUM_PREFIX, name, "", ENUM_PARSE_SUFFIX)
        }
        _ => plain_label(name),
    })
}

fn field_like_label(label: &str, detail: Option<&str>) -> zed::CodeLabel {
    if let Some(detail) = detail {
        return literal_label(
            &format!("{label}{FIELD_TYPE_SEPARATOR}{detail}"),
            label.len(),
        );
    }

    plain_label(label)
}

fn plain_label(label: &str) -> zed::CodeLabel {
    literal_label(label, label.len())
}

fn parsed_code_label(
    prefix: &str,
    label: &str,
    display_suffix: &str,
    parse_suffix: &str,
) -> zed::CodeLabel {
    let display = format!("{prefix}{label}{display_suffix}");
    let code = format!("{display}{parse_suffix}");
    let label_start = prefix.len() as u32;
    let label_end = label_start + label.len() as u32;
    zed::CodeLabel {
        code,
        spans: vec![zed::CodeLabelSpan::code_range(0..display.len() as u32)],
        filter_range: zed::Range {
            start: label_start,
            end: label_end,
        },
    }
}

fn literal_label(text: &str, filter_end: usize) -> zed::CodeLabel {
    zed::CodeLabel {
        code: text.to_string(),
        spans: vec![zed::CodeLabelSpan::literal(text, None)],
        filter_range: zed::Range {
            start: 0,
            end: filter_end as u32,
        },
    }
}

fn is_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_label_formats_anchor_function_like_rows() {
        let label = completion_code_label(Completion {
            label: "initialize".to_string(),
            label_details: None,
            detail: None,
            kind: Some(CompletionKind::Function),
            insert_text_format: None,
        })
        .expect("function label");

        assert_eq!(label.code, "fn initialize() {}");
        assert_eq!(label.filter_range.start, FUNCTION_PREFIX.len() as u32);
        assert_eq!(
            label.filter_range.end,
            (FUNCTION_PREFIX.len() + "initialize".len()) as u32
        );
    }

    #[test]
    fn completion_label_formats_field_detail_without_filtering_on_type() {
        let label = completion_code_label(Completion {
            label: "payer".to_string(),
            label_details: None,
            detail: Some("Signer<'info>".to_string()),
            kind: Some(CompletionKind::Field),
            insert_text_format: None,
        })
        .expect("field label");

        assert_eq!(label.code, "payer: Signer<'info>");
        assert!(matches!(
            label.spans.first(),
            Some(zed::CodeLabelSpan::Literal(_))
        ));
        assert_eq!(label.filter_range.start, 0);
        assert_eq!(label.filter_range.end, "payer".len() as u32);
    }

    #[test]
    fn completion_label_keeps_non_identifier_constraint_rows_literal() {
        let label = completion_code_label(Completion {
            label: "token::authority =".to_string(),
            label_details: None,
            detail: None,
            kind: Some(CompletionKind::Snippet),
            insert_text_format: None,
        })
        .expect("constraint label");

        assert_eq!(label.code, "token::authority =");
        assert!(matches!(
            label.spans.first(),
            Some(zed::CodeLabelSpan::Literal(_))
        ));
        assert_eq!(label.filter_range.start, 0);
        assert_eq!(label.filter_range.end, "token::authority =".len() as u32);
    }

    #[test]
    fn symbol_label_formats_anchor_structs() {
        let label = symbol_code_label(Symbol {
            name: "Create".to_string(),
            kind: SymbolKind::Struct,
        })
        .expect("symbol label");

        assert_eq!(label.code, "struct Create;");
    }

    #[test]
    fn completion_label_skips_empty_rows() {
        let label = completion_code_label(Completion {
            label: " ".to_string(),
            label_details: None,
            detail: None,
            kind: Some(CompletionKind::Function),
            insert_text_format: None,
        });

        assert!(label.is_none());
    }
}
