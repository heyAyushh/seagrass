use {
    crate::{
        document::{NamedRange, ParsedDocument, SymbolRange},
        range::byte_offset_at,
    },
    syn::{GenericArgument, PathArguments, Type},
    tower_lsp::lsp_types::{DocumentSymbol, Position, Range},
};

const DERIVE_ACCOUNTS_DETAIL: &str = "#[derive(Accounts)]";
const DERIVE_ACCOUNTS_MARKER: &str = "#[derive(Accounts";
const STRUCT_KEYWORD: &str = "struct";
const PUBLIC_FIELD_PREFIX: &str = "pub ";
const OPTION_TYPE_NAME: &str = "Option";

pub(super) fn accounts_struct_at_position(
    document: &ParsedDocument,
    position: Position,
) -> Option<&SymbolRange> {
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

pub(super) fn recover_accounts_struct_at_position(
    document: &ParsedDocument,
    position: Position,
) -> Option<SymbolRange> {
    recover_accounts_struct_from_tree_sitter(document, position)
        .filter(|accounts| !accounts.fields.is_empty())
        .or_else(|| recover_accounts_struct_from_text(document.source(), position))
}

pub(super) fn field_after_attribute(
    accounts: &SymbolRange,
    attribute_line: u32,
) -> Option<&SymbolRange> {
    accounts
        .fields
        .iter()
        .filter(|field| field.selection_range.start.line > attribute_line)
        .min_by_key(|field| field.selection_range.start.line)
}

fn recover_accounts_struct_from_tree_sitter(
    document: &ParsedDocument,
    position: Position,
) -> Option<SymbolRange> {
    let source = document.source();
    let syntax = document.tree_sitter()?;
    syntax
        .anchor_document_symbols(source)
        .into_iter()
        .filter(|symbol| {
            symbol.detail.as_deref() == Some(DERIVE_ACCOUNTS_DETAIL)
                && contains_line(symbol.range, position.line)
        })
        .min_by_key(|symbol| {
            symbol
                .range
                .end
                .line
                .saturating_sub(symbol.range.start.line)
        })
        .map(accounts_symbol_from_document_symbol)
}

fn accounts_symbol_from_document_symbol(symbol: DocumentSymbol) -> SymbolRange {
    let fields = symbol
        .children
        .unwrap_or_default()
        .into_iter()
        .map(field_symbol_from_document_symbol)
        .collect();

    empty_symbol_range(SymbolRangeParts {
        name: symbol.name,
        range: symbol.range,
        selection_range: symbol.selection_range,
        fields,
        ..SymbolRangeParts::default()
    })
}

fn field_symbol_from_document_symbol(symbol: DocumentSymbol) -> SymbolRange {
    let field_type = symbol
        .detail
        .as_deref()
        .map(recovered_field_type)
        .unwrap_or_default();
    empty_symbol_range(SymbolRangeParts {
        name: symbol.name,
        range: symbol.range,
        selection_range: symbol.selection_range,
        type_name: field_type.type_name,
        type_signature: symbol.detail,
        generic_type_names: field_type.generic_type_names,
        is_optional: field_type.is_optional,
        ..SymbolRangeParts::default()
    })
}

fn recover_accounts_struct_from_text(source: &str, position: Position) -> Option<SymbolRange> {
    let cursor = byte_offset_at(source, position)?;
    let prefix = &source[..cursor];
    let derive_start = prefix.rfind(DERIVE_ACCOUNTS_MARKER)?;
    let struct_start = find_word(source, STRUCT_KEYWORD, derive_start)?;
    let name_start = skip_whitespace(source, struct_start + STRUCT_KEYWORD.len())?;
    let name_end = identifier_end(source, name_start)?;
    let name = source[name_start..name_end].to_string();
    let open_brace = source[name_end..].find('{')? + name_end;
    if open_brace > cursor {
        return None;
    }

    let close_brace = matching_close_brace(source, open_brace).unwrap_or(source.len());
    let range = Range {
        start: position_at_byte_offset(source, derive_start),
        end: position_at_byte_offset(source, close_brace.min(source.len())),
    };
    let selection_range = range_for_byte_span(source, name_start, name_end);
    let fields = recover_account_fields_from_text(source, open_brace + '{'.len_utf8(), close_brace);

    (!fields.is_empty()).then(|| {
        empty_symbol_range(SymbolRangeParts {
            name,
            range,
            selection_range,
            fields,
            ..SymbolRangeParts::default()
        })
    })
}

fn recover_account_fields_from_text(
    source: &str,
    body_start: usize,
    body_end: usize,
) -> Vec<SymbolRange> {
    let mut fields = Vec::new();
    let mut line_start = body_start;
    for line in source[body_start..body_end.min(source.len())].split_inclusive('\n') {
        if let Some(field) = recover_account_field_from_line(source, line, line_start) {
            fields.push(field);
        }
        line_start += line.len();
    }
    fields
}

fn recover_account_field_from_line(
    source: &str,
    line: &str,
    line_start: usize,
) -> Option<SymbolRange> {
    let code = line.split_once("//").map_or(line, |(code, _)| code);
    let trimmed = code.trim_start();
    if trimmed.starts_with("#[") {
        return None;
    }

    let leading = code.len().saturating_sub(trimmed.len());
    let declaration = trimmed.strip_prefix(PUBLIC_FIELD_PREFIX).unwrap_or(trimmed);
    let declaration_start = line_start + leading + trimmed.len().saturating_sub(declaration.len());
    let colon = declaration.find(':')?;
    let name = declaration[..colon].trim();
    if !crate::syntax::is_ascii_identifier(name) {
        return None;
    }

    let name_start_in_declaration = declaration[..colon].find(name)?;
    let name_start = declaration_start + name_start_in_declaration;
    let name_end = name_start + name.len();
    let type_start_in_declaration = colon + ':'.len_utf8();
    let raw_type = declaration[type_start_in_declaration..]
        .trim()
        .trim_end_matches(',')
        .trim();
    if raw_type.is_empty() {
        return None;
    }

    let field_type = recovered_field_type(raw_type);
    let line_end = line_start + code.trim_end().len();
    let type_range = field_type.type_name.as_ref().and_then(|type_name| {
        let type_offset = declaration[type_start_in_declaration..].find(type_name)?;
        let start = declaration_start + type_start_in_declaration + type_offset;
        Some(range_for_byte_span(source, start, start + type_name.len()))
    });

    Some(empty_symbol_range(SymbolRangeParts {
        name: name.to_string(),
        range: range_for_byte_span(source, line_start + leading, line_end),
        selection_range: range_for_byte_span(source, name_start, name_end),
        type_name: field_type.type_name,
        type_range,
        generic_type_names: field_type.generic_type_names,
        is_optional: field_type.is_optional,
        ..SymbolRangeParts::default()
    }))
}

#[derive(Default)]
struct RecoveredFieldType {
    type_name: Option<String>,
    generic_type_names: Vec<String>,
    is_optional: bool,
}

fn recovered_field_type(type_text: &str) -> RecoveredFieldType {
    syn::parse_str::<Type>(type_text)
        .ok()
        .and_then(|ty| recovered_field_type_from_syn(&ty))
        .unwrap_or_else(|| recovered_field_type_from_text(type_text))
}

fn recovered_field_type_from_syn(ty: &Type) -> Option<RecoveredFieldType> {
    let (field_ty, is_optional) = unwrap_option_type(ty);
    let Type::Path(type_path) = field_ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let generic_type_names = match &segment.arguments {
        PathArguments::AngleBracketed(arguments) => arguments
            .args
            .iter()
            .filter_map(|argument| match argument {
                GenericArgument::Type(Type::Path(type_path)) => type_path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    Some(RecoveredFieldType {
        type_name: Some(segment.ident.to_string()),
        generic_type_names,
        is_optional,
    })
}

fn unwrap_option_type(ty: &Type) -> (&Type, bool) {
    let Type::Path(type_path) = ty else {
        return (ty, false);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return (ty, false);
    };
    if segment.ident != OPTION_TYPE_NAME {
        return (ty, false);
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return (ty, false);
    };
    let Some(GenericArgument::Type(inner)) = arguments.args.first() else {
        return (ty, false);
    };
    (inner, true)
}

fn recovered_field_type_from_text(type_text: &str) -> RecoveredFieldType {
    let wrapper = type_text
        .split(['<', ' ', '\t'])
        .next()
        .filter(|name| crate::syntax::is_ascii_identifier(name))
        .map(str::to_string);
    let generic_type_names = type_text
        .split(['<', '>', ','])
        .map(str::trim)
        .filter(|part| {
            crate::syntax::is_ascii_identifier(part) && part.as_bytes().first() != Some(&b'\'')
        })
        .skip(1)
        .map(str::to_string)
        .collect();

    RecoveredFieldType {
        type_name: wrapper,
        generic_type_names,
        is_optional: type_text.trim_start().starts_with(OPTION_TYPE_NAME),
    }
}

#[derive(Default)]
struct SymbolRangeParts {
    name: String,
    range: Range,
    selection_range: Range,
    type_name: Option<String>,
    type_range: Option<Range>,
    type_signature: Option<String>,
    generic_type_names: Vec<String>,
    is_optional: bool,
    fields: Vec<SymbolRange>,
}

fn empty_symbol_range(parts: SymbolRangeParts) -> SymbolRange {
    let SymbolRangeParts {
        name,
        range,
        selection_range,
        type_name,
        type_range,
        type_signature,
        generic_type_names,
        is_optional,
        fields,
    } = parts;
    SymbolRange {
        name,
        range,
        selection_range,
        fields,
        variants: Vec::new(),
        type_name,
        type_range,
        type_signature,
        generic_type_ranges: generic_type_names
            .iter()
            .map(|name| NamedRange {
                name: name.clone(),
                range: Range::default(),
            })
            .collect(),
        generic_type_names,
        is_optional,
        max_len_args: Vec::new(),
        account_constraints: Vec::new(),
        pda_constraint: None,
        instruction_arguments: Vec::new(),
        derive_attribute_range: None,
        derive_accounts_range: None,
        derive_init_space_range: None,
        is_zero_copy: false,
    }
}

fn find_word(source: &str, word: &str, start: usize) -> Option<usize> {
    let mut offset = start;
    while let Some(relative) = source[offset..].find(word) {
        let candidate = offset + relative;
        if word_has_boundary(source, candidate, word.len()) {
            return Some(candidate);
        }
        offset = candidate + word.len();
    }
    None
}

fn word_has_boundary(source: &str, start: usize, len: usize) -> bool {
    let previous = source[..start].chars().next_back();
    let next = source[start + len..].chars().next();
    previous
        .map(crate::syntax::is_ascii_identifier_char)
        .is_none_or(|is_ident| !is_ident)
        && next
            .map(crate::syntax::is_ascii_identifier_char)
            .is_none_or(|is_ident| !is_ident)
}

fn skip_whitespace(source: &str, start: usize) -> Option<usize> {
    source[start..]
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(start + idx))
}

fn identifier_end(source: &str, start: usize) -> Option<usize> {
    let mut end = start;
    for (idx, ch) in source[start..].char_indices() {
        if idx == 0 && !crate::syntax::is_ascii_identifier_start(ch) {
            return None;
        }
        if !crate::syntax::is_ascii_identifier_char(ch) {
            break;
        }
        end = start + idx + ch.len_utf8();
    }
    (end > start).then_some(end)
}

fn matching_close_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, ch) in source[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open_brace + idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn range_for_byte_span(source: &str, start: usize, end: usize) -> Range {
    Range {
        start: position_at_byte_offset(source, start),
        end: position_at_byte_offset(source, end),
    }
}

fn position_at_byte_offset(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    Position {
        line,
        character: u32::try_from(source[line_start..offset].chars().count()).unwrap_or_default(),
    }
}

fn contains_line(range: Range, line: u32) -> bool {
    range.start.line <= line && line <= range.end.line
}
