use {
    crate::{
        document::{AccountPathUsage, ParsedDocument},
        range::{matching_word_ranges, word_at_position},
    },
    tower_lsp::lsp_types::{
        DocumentHighlight, DocumentHighlightKind, Location, Position, Range, SymbolKind, Url,
    },
};

mod instruction_arguments;

pub use instruction_arguments::instruction_argument_target;
use instruction_arguments::{
    instruction_argument_definition_range, instruction_argument_reference_ranges,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPathPosition {
    pub context: String,
    pub segments: Vec<String>,
    pub segment_index: usize,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFieldPathTarget {
    pub container: String,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionArgumentTarget {
    pub context: String,
    pub name: String,
    pub range: Range,
}

pub fn definition_range(document: &ParsedDocument, position: Position) -> Option<Range> {
    let word = word_at_position(document.source(), position)?;
    if let Some(range) = account_field_path_definition_range(document, &word, position) {
        return Some(range);
    }
    if let Some(range) = account_data_field_definition_range(document, &word, position) {
        return Some(range);
    }
    if let Some(range) = instruction_argument_definition_range(document, position) {
        return Some(range);
    }
    if let Some(range) = field_definition_range(document, &word, position) {
        return Some(range);
    }

    let is_context_reference = document
        .symbols()
        .context_references
        .iter()
        .any(|reference| reference.name == word && contains_position(reference.range, position));
    if !is_context_reference && !document.symbols().all_structs.contains_key(&word) {
        return None;
    }

    document
        .symbols()
        .all_structs
        .get(&word)
        .map(|symbol| symbol.selection_range)
}

pub fn declaration_range(document: &ParsedDocument, position: Position) -> Option<Range> {
    definition_range(document, position)
}

pub fn declaration_target_kinds(
    document: &ParsedDocument,
    position: Position,
) -> Option<Vec<SymbolKind>> {
    definition_target_kinds(document, position)
}

pub fn definition_target_kinds(
    document: &ParsedDocument,
    position: Position,
) -> Option<Vec<SymbolKind>> {
    if account_field_path_definition_target(document, position).is_some() {
        return Some(vec![SymbolKind::FIELD]);
    }

    if account_data_field_definition_target(document, position).is_some() {
        return Some(vec![SymbolKind::FIELD]);
    }

    if instruction_argument_target(document, position).is_some() {
        return Some(vec![SymbolKind::VARIABLE]);
    }

    if is_anchor_type_position(document, position) {
        return Some(vec![SymbolKind::STRUCT]);
    }

    let word = word_at_position(document.source(), position)?;
    field_definition_range(document, &word, position).map(|_| vec![SymbolKind::FIELD])
}

pub fn type_definition_target(document: &ParsedDocument, position: Position) -> Option<String> {
    let word = word_at_position(document.source(), position)?;

    if is_anchor_type_position(document, position) && document.symbols().knows_type(&word) {
        return Some(word);
    }

    if let Some(path) = account_path_position(document, position) {
        let target = resolve_local_account_path(document, &path)?;
        return document
            .symbols()
            .accounts_structs
            .get(&target.container)?
            .fields
            .iter()
            .find(|field| field.name == target.field)
            .and_then(anchor_field_type_target);
    }

    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| &accounts.fields)
        .find(|field| contains_position(field.selection_range, position))
        .and_then(anchor_field_type_target)
}

pub fn type_definition_range(document: &ParsedDocument, position: Position) -> Option<Range> {
    let target = type_definition_target(document, position)?;
    document
        .symbols()
        .all_structs
        .get(&target)
        .map(|symbol| symbol.selection_range)
}

pub fn implementation_context_target(
    document: &ParsedDocument,
    position: Position,
) -> Option<String> {
    let word = word_at_position(document.source(), position)?;
    let is_context_type = document
        .symbols()
        .context_references
        .iter()
        .any(|reference| reference.name == word && contains_position(reference.range, position));
    let is_accounts_struct = document
        .symbols()
        .accounts_structs
        .get(&word)
        .is_some_and(|accounts| contains_position(accounts.selection_range, position));

    (is_context_type || is_accounts_struct).then_some(word)
}

pub fn implementation_ranges(document: &ParsedDocument, position: Position) -> Option<Vec<Range>> {
    let context = implementation_context_target(document, position)?;
    let ranges = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|reference| reference.name == context)
        })
        .map(|instruction| instruction.selection_range)
        .collect::<Vec<_>>();

    (!ranges.is_empty()).then_some(ranges)
}

pub fn account_data_field_definition_target(
    document: &ParsedDocument,
    position: Position,
) -> Option<(String, String)> {
    let word = word_at_position(document.source(), position)?;
    let usage = account_data_field_usage_at_position(document, &word, position)?;
    let account_field = account_field_for_usage(document, usage)?;
    let account_data_type = account_field.generic_type_names.last()?.clone();
    Some((account_data_type, usage.field.clone()))
}

pub fn account_path_position(
    document: &ParsedDocument,
    position: Position,
) -> Option<AccountPathPosition> {
    let word = word_at_position(document.source(), position)?;
    document
        .symbols()
        .callable_functions()
        .filter(|instruction| contains_position(instruction.range, position))
        .filter_map(|instruction| {
            let context = instruction.context.as_ref()?;
            instruction.account_path_usages.iter().find_map(|usage| {
                account_path_segment_index(usage, &word, position).map(|segment_index| {
                    AccountPathPosition {
                        context: context.name.clone(),
                        segments: usage
                            .segments
                            .iter()
                            .map(|segment| segment.name.clone())
                            .collect(),
                        segment_index,
                        field: word.clone(),
                    }
                })
            })
        })
        .next()
}

pub fn account_field_path_definition_target(
    document: &ParsedDocument,
    position: Position,
) -> Option<AccountFieldPathTarget> {
    let path = account_path_position(document, position)?;
    resolve_local_account_path(document, &path)
}

pub fn account_field_path_definition_target_for_position(
    document: &ParsedDocument,
    path: &AccountPathPosition,
) -> Option<AccountFieldPathTarget> {
    resolve_local_account_path(document, path)
}

#[cfg(test)]
pub fn allows_workspace_references(document: &ParsedDocument, position: Position) -> bool {
    reference_target_kinds(document, position).is_some()
}

pub fn reference_target_kinds(
    document: &ParsedDocument,
    position: Position,
) -> Option<Vec<SymbolKind>> {
    if instruction_argument_target(document, position).is_some() {
        return Some(vec![SymbolKind::VARIABLE]);
    }

    if is_anchor_type_position(document, position) {
        return Some(vec![SymbolKind::STRUCT]);
    }

    let word = word_at_position(document.source(), position)?;
    document
        .symbols()
        .all_structs
        .get(&word)
        .filter(|symbol| contains_position(symbol.selection_range, position))
        .map(|_| vec![SymbolKind::STRUCT])
}

pub fn references(
    document: &ParsedDocument,
    uri: Url,
    position: Position,
) -> Option<Vec<Location>> {
    let word = word_at_position(document.source(), position)?;
    if let Some(ranges) = account_field_path_reference_ranges(document, &word, position) {
        return Some(
            ranges
                .into_iter()
                .map(|range| Location {
                    uri: uri.clone(),
                    range,
                })
                .collect(),
        );
    }
    if let Some(ranges) = account_data_field_reference_ranges(document, &word, position) {
        return Some(
            ranges
                .into_iter()
                .map(|range| Location {
                    uri: uri.clone(),
                    range,
                })
                .collect(),
        );
    }
    if let Some(ranges) = instruction_argument_reference_ranges(document, position) {
        return Some(
            ranges
                .into_iter()
                .map(|range| Location {
                    uri: uri.clone(),
                    range,
                })
                .collect(),
        );
    }
    if !document.symbols().knows_type(&word) && !document_knows_field(document, &word, position) {
        return None;
    }

    let ranges = matching_word_ranges(document.source(), &word);
    (!ranges.is_empty()).then(|| {
        ranges
            .into_iter()
            .map(|range| Location {
                uri: uri.clone(),
                range,
            })
            .collect()
    })
}

pub fn document_highlights(
    document: &ParsedDocument,
    position: Position,
) -> Option<Vec<DocumentHighlight>> {
    let word = word_at_position(document.source(), position)?;
    if let Some(ranges) = account_field_path_reference_ranges(document, &word, position) {
        return Some(
            ranges
                .into_iter()
                .map(|range| DocumentHighlight {
                    range,
                    kind: Some(DocumentHighlightKind::TEXT),
                })
                .collect(),
        );
    }
    if let Some(ranges) = account_data_field_reference_ranges(document, &word, position) {
        return Some(
            ranges
                .into_iter()
                .map(|range| DocumentHighlight {
                    range,
                    kind: Some(DocumentHighlightKind::TEXT),
                })
                .collect(),
        );
    }
    if let Some(ranges) = instruction_argument_reference_ranges(document, position) {
        return Some(
            ranges
                .into_iter()
                .map(|range| DocumentHighlight {
                    range,
                    kind: Some(DocumentHighlightKind::TEXT),
                })
                .collect(),
        );
    }
    if !document.symbols().knows_type(&word) && !document_knows_field(document, &word, position) {
        return None;
    }

    let ranges = matching_word_ranges(document.source(), &word);
    (!ranges.is_empty()).then(|| {
        ranges
            .into_iter()
            .map(|range| DocumentHighlight {
                range,
                kind: Some(DocumentHighlightKind::TEXT),
            })
            .collect()
    })
}

fn field_definition_range(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    if let Some(range) = field_in_enclosing_struct(document, word, position) {
        return Some(range);
    }

    if let Some(range) = field_in_enclosing_instruction_context(document, word, position) {
        return Some(range);
    }

    unique_anchor_field(document, word)
}

fn account_data_field_definition_range(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    let (account_data_type, field_name) = account_data_field_definition_target(document, position)?;
    if field_name != word {
        return None;
    }
    document
        .symbols()
        .account_data_structs
        .get(&account_data_type)?
        .fields
        .iter()
        .find(|field| field.name == field_name)
        .map(|field| field.selection_range)
}

fn account_field_path_definition_range(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    let target = account_field_path_definition_target(document, position)?;
    if target.field != word {
        return None;
    }
    document
        .symbols()
        .accounts_structs
        .get(&target.container)?
        .fields
        .iter()
        .find(|field| field.name == target.field)
        .map(|field| field.selection_range)
}

fn account_field_path_reference_ranges(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Vec<Range>> {
    let target = account_field_path_definition_target(document, position)?;
    if target.field != word {
        return None;
    }

    let mut ranges = Vec::new();
    if let Some(field) = document
        .symbols()
        .accounts_structs
        .get(&target.container)
        .and_then(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == target.field)
        })
    {
        ranges.push(field.selection_range);
    }

    for instruction in document.symbols().callable_functions() {
        let Some(context) = instruction.context.as_ref() else {
            continue;
        };
        for usage in &instruction.account_path_usages {
            for (segment_index, segment) in usage.segments.iter().enumerate() {
                if segment.name != target.field {
                    continue;
                }
                let path = AccountPathPosition {
                    context: context.name.clone(),
                    segments: usage
                        .segments
                        .iter()
                        .map(|segment| segment.name.clone())
                        .collect(),
                    segment_index,
                    field: segment.name.clone(),
                };
                if resolve_local_account_path(document, &path).is_some_and(|candidate| {
                    candidate.container == target.container && candidate.field == target.field
                }) {
                    ranges.push(segment.range);
                }
            }
        }
    }

    (!ranges.is_empty()).then_some(ranges)
}

fn account_data_field_reference_ranges(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Vec<Range>> {
    let (account_data_type, field_name) = account_data_field_definition_target(document, position)?;
    if field_name != word {
        return None;
    }

    let mut ranges = Vec::new();
    if let Some(field) = document
        .symbols()
        .account_data_structs
        .get(&account_data_type)
        .and_then(|account| account.fields.iter().find(|field| field.name == field_name))
    {
        ranges.push(field.selection_range);
    }

    ranges.extend(
        document
            .symbols()
            .callable_functions()
            .flat_map(|instruction| {
                let account_data_type = account_data_type.clone();
                let field_name = field_name.clone();
                instruction
                    .account_data_field_usages
                    .iter()
                    .filter(move |usage| {
                        usage.field == field_name
                            && account_field_for_usage(document, usage)
                                .and_then(|field| field.generic_type_names.last())
                                .is_some_and(|name| name == &account_data_type)
                    })
                    .map(|usage| usage.range)
            }),
    );

    (!ranges.is_empty()).then_some(ranges)
}

fn account_data_field_usage_at_position<'a>(
    document: &'a ParsedDocument,
    word: &str,
    position: Position,
) -> Option<&'a crate::document::AccountDataFieldUsage> {
    document
        .symbols()
        .callable_functions()
        .filter(|instruction| contains_position(instruction.range, position))
        .flat_map(|instruction| instruction.account_data_field_usages.iter())
        .find(|usage| usage.field == word && contains_position(usage.range, position))
}

fn account_path_segment_index(
    usage: &AccountPathUsage,
    word: &str,
    position: Position,
) -> Option<usize> {
    usage
        .segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.name == word && contains_position(segment.range, position))
        .map(|(index, _)| index)
}

fn resolve_local_account_path(
    document: &ParsedDocument,
    path: &AccountPathPosition,
) -> Option<AccountFieldPathTarget> {
    let mut container = path.context.clone();
    for (index, segment) in path.segments.iter().enumerate() {
        let accounts = document.symbols().accounts_structs.get(&container)?;
        let field = accounts
            .fields
            .iter()
            .find(|field| field.name == *segment)?;
        if index == path.segment_index {
            return Some(AccountFieldPathTarget {
                container,
                field: field.name.clone(),
            });
        }
        let next_container = field.type_name.as_ref()?;
        if !document
            .symbols()
            .accounts_structs
            .contains_key(next_container)
        {
            return None;
        }
        container = next_container.clone();
    }
    None
}

fn account_field_for_usage<'a>(
    document: &'a ParsedDocument,
    usage: &crate::document::AccountDataFieldUsage,
) -> Option<&'a crate::document::SymbolRange> {
    document
        .symbols()
        .callable_functions()
        .find(|instruction| {
            instruction
                .account_data_field_usages
                .iter()
                .any(|candidate| candidate.range == usage.range)
        })
        .and_then(|instruction| instruction.context.as_ref())
        .and_then(|context| document.symbols().accounts_structs.get(&context.name))
        .and_then(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == usage.account)
        })
}

fn document_knows_field(document: &ParsedDocument, word: &str, position: Position) -> bool {
    field_definition_range(document, word, position).is_some()
        || account_field_path_definition_range(document, word, position).is_some()
        || account_data_field_definition_range(document, word, position).is_some()
}

fn is_anchor_type_position(document: &ParsedDocument, position: Position) -> bool {
    document
        .symbols()
        .context_references
        .iter()
        .any(|reference| contains_position(reference.range, position))
        || document
            .symbols()
            .accounts_structs
            .values()
            .chain(document.symbols().account_data_structs.values())
            .flat_map(|symbol| symbol.fields.iter())
            .any(|field| {
                field
                    .type_range
                    .is_some_and(|range| contains_position(range, position))
                    || field
                        .generic_type_ranges
                        .iter()
                        .any(|generic| contains_position(generic.range, position))
            })
}

fn field_in_enclosing_struct(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .filter(|symbol| contains_position(symbol.range, position))
        .find_map(|symbol| {
            symbol
                .fields
                .iter()
                .find(|field| field.name == word)
                .map(|field| field.selection_range)
        })
}

fn field_in_enclosing_instruction_context(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    document
        .symbols()
        .callable_functions()
        .filter(|instruction| contains_position(instruction.range, position))
        .filter_map(|instruction| instruction.context.as_ref())
        .filter_map(|context| document.symbols().accounts_structs.get(&context.name))
        .find_map(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == word)
                .map(|field| field.selection_range)
        })
}

fn unique_anchor_field(document: &ParsedDocument, word: &str) -> Option<Range> {
    let mut matches = document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .flat_map(|symbol| &symbol.fields)
        .filter(|field| field.name == word)
        .map(|field| field.selection_range);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn anchor_field_type_target(field: &crate::document::SymbolRange) -> Option<String> {
    field
        .generic_type_names
        .last()
        .or(field.type_name.as_ref())
        .cloned()
}

pub(super) fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
}

#[cfg(test)]
mod tests;
