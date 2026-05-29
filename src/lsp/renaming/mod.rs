use {
    crate::{
        document::{AccountConstraint, ParsedDocument},
        evidence::ConstraintEvidence,
        navigation,
        range::{matching_word_ranges, word_at_position, word_range_at_position},
        workspace::WorkspaceIndex,
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        Location, Position, PrepareRenameResponse, Range, TextEdit, Url, WorkspaceEdit,
    },
};

pub fn prepare_rename(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<PrepareRenameResponse> {
    let target = rename_target(document, position, workspace_index)?;
    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: target.range,
        placeholder: target.placeholder,
    })
}

pub fn rename_with_workspace(
    document: &ParsedDocument,
    uri: Url,
    position: Position,
    new_name: &str,
    workspace_index: Option<&WorkspaceIndex>,
    source_for_uri: impl Fn(&Url) -> Option<String>,
) -> Option<WorkspaceEdit> {
    if !is_rust_identifier(new_name) {
        return None;
    }

    let target = rename_target(document, position, workspace_index)?;
    let locations = match target.kind {
        RenameTargetKind::AccountField { accounts_type } => workspace_index
            .map(|index| {
                let mut locations = index.references_in_container(
                    &target.placeholder,
                    &[tower_lsp::lsp_types::SymbolKind::FIELD],
                    &accounts_type,
                );
                locations.extend(index.symbol_locations_in_container(
                    &target.placeholder,
                    &[tower_lsp::lsp_types::SymbolKind::FIELD],
                    &accounts_type,
                ));
                push_unique_location(
                    &mut locations,
                    Location {
                        uri: uri.clone(),
                        range: target.range,
                    },
                );
                dedupe_locations(locations)
            })
            .filter(|locations| !locations.is_empty())
            .unwrap_or_else(|| {
                account_field_locations(document, &uri, &accounts_type, &target.placeholder)
            }),
        RenameTargetKind::AccountDataField { account_data_type } => workspace_index
            .map(|index| {
                index.references_in_container(
                    &target.placeholder,
                    &[tower_lsp::lsp_types::SymbolKind::FIELD],
                    &account_data_type,
                )
            })
            .filter(|locations| !locations.is_empty())
            .unwrap_or_else(|| {
                account_data_field_locations(
                    document,
                    &uri,
                    &account_data_type,
                    &target.placeholder,
                )
            }),
        RenameTargetKind::InstructionArgument { context_name } => workspace_index
            .map(|index| {
                index.instruction_argument_references_for_context_with_source(
                    &context_name,
                    &target.placeholder,
                    &source_for_uri,
                )
            })
            .filter(|locations| !locations.is_empty())
            .unwrap_or_else(|| {
                instruction_argument_locations(document, &uri, &context_name, &target.placeholder)
            }),
        RenameTargetKind::AssociatedValue => {
            navigation::references(document, uri.clone(), position).unwrap_or_default()
        }
        RenameTargetKind::AnchorType => workspace_index
            .map(|index| {
                index.references_with_kinds(
                    &target.placeholder,
                    &[tower_lsp::lsp_types::SymbolKind::STRUCT],
                )
            })
            .filter(|locations| !locations.is_empty())
            .unwrap_or_else(|| {
                navigation::references(document, uri.clone(), position).unwrap_or_default()
            }),
    };
    if locations.is_empty() {
        return None;
    }

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for location in locations {
        changes
            .entry(location.uri)
            .or_default()
            .push(TextEdit::new(location.range, new_name.to_string()));
    }
    Some(WorkspaceEdit::new(changes))
}

#[derive(Debug, Clone)]
struct RenameTarget {
    kind: RenameTargetKind,
    range: Range,
    placeholder: String,
}

#[derive(Debug, Clone)]
enum RenameTargetKind {
    AccountField { accounts_type: String },
    AccountDataField { account_data_type: String },
    InstructionArgument { context_name: String },
    AssociatedValue,
    AnchorType,
}

fn rename_target(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<RenameTarget> {
    account_field_target(document, position, workspace_index)
        .or_else(|| account_data_field_target(document, position))
        .or_else(|| instruction_argument_target(document, position))
        .or_else(|| associated_value_target(document, position))
        .or_else(|| anchor_type_target(document, position))
}

fn associated_value_target(document: &ParsedDocument, position: Position) -> Option<RenameTarget> {
    let target = navigation::associated_value_rename_target(document, position)?;
    Some(RenameTarget {
        kind: RenameTargetKind::AssociatedValue,
        range: target.range,
        placeholder: target.value_name,
    })
}

fn instruction_argument_target(
    document: &ParsedDocument,
    position: Position,
) -> Option<RenameTarget> {
    for instruction in document.symbols().callable_functions() {
        let Some(context) = instruction.context.as_ref() else {
            continue;
        };
        if let Some(argument) = instruction
            .arguments
            .iter()
            .find(|argument| contains_position(argument.range, position))
        {
            return Some(RenameTarget {
                kind: RenameTargetKind::InstructionArgument {
                    context_name: context.name.clone(),
                },
                range: argument.range,
                placeholder: argument.name.clone(),
            });
        }
    }

    for accounts in document.symbols().accounts_structs.values() {
        if let Some(argument) = accounts
            .instruction_arguments
            .iter()
            .find(|argument| contains_position(argument.range, position))
        {
            return Some(RenameTarget {
                kind: RenameTargetKind::InstructionArgument {
                    context_name: accounts.name.clone(),
                },
                range: argument.range,
                placeholder: argument.name.clone(),
            });
        }

        for field in &accounts.fields {
            for constraint in &field.account_constraints {
                if let Some((range, name)) =
                    instruction_argument_range_at_position(document.source(), constraint, position)
                {
                    return Some(RenameTarget {
                        kind: RenameTargetKind::InstructionArgument {
                            context_name: accounts.name.clone(),
                        },
                        range,
                        placeholder: name,
                    });
                }
            }
        }
    }

    None
}

fn account_field_target(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<RenameTarget> {
    if let Some(target) = navigation::account_field_path_definition_target(document, position) {
        let range = word_range_at_position(document.source(), position)?;
        return Some(RenameTarget {
            kind: RenameTargetKind::AccountField {
                accounts_type: target.container,
            },
            range,
            placeholder: target.field,
        });
    }

    if let Some(index) = workspace_index {
        let path = navigation::account_path_position(document, position)?;
        let resolved =
            index.resolve_account_field_path(&path.context, &path.segments, path.segment_index)?;
        let range = word_range_at_position(document.source(), position)?;
        return Some(RenameTarget {
            kind: RenameTargetKind::AccountField {
                accounts_type: resolved.container_name,
            },
            range,
            placeholder: resolved.field_name,
        });
    }

    document
        .symbols()
        .accounts_structs
        .iter()
        .find_map(|(accounts_type, accounts)| {
            accounts.fields.iter().find_map(|field| {
                contains_position(field.selection_range, position).then(|| RenameTarget {
                    kind: RenameTargetKind::AccountField {
                        accounts_type: accounts_type.clone(),
                    },
                    range: field.selection_range,
                    placeholder: field.name.clone(),
                })
            })
        })
}

fn account_data_field_target(
    document: &ParsedDocument,
    position: Position,
) -> Option<RenameTarget> {
    if let Some((account_data_type, field_name)) =
        navigation::account_data_field_definition_target(document, position)
    {
        let range = word_range_at_position(document.source(), position)?;
        return Some(RenameTarget {
            kind: RenameTargetKind::AccountDataField { account_data_type },
            range,
            placeholder: field_name,
        });
    }

    document
        .symbols()
        .account_data_structs
        .iter()
        .find_map(|(account_data_type, account)| {
            account.fields.iter().find_map(|field| {
                contains_position(field.selection_range, position).then(|| RenameTarget {
                    kind: RenameTargetKind::AccountDataField {
                        account_data_type: account_data_type.clone(),
                    },
                    range: field.selection_range,
                    placeholder: field.name.clone(),
                })
            })
        })
}

fn anchor_type_target(document: &ParsedDocument, position: Position) -> Option<RenameTarget> {
    let word = word_at_position(document.source(), position)?;
    let range = word_range_at_position(document.source(), position)?;
    if navigation::definition_target_kinds(document, position).is_some_and(|kinds| {
        kinds.contains(&tower_lsp::lsp_types::SymbolKind::CONSTANT)
            || kinds.contains(&tower_lsp::lsp_types::SymbolKind::FUNCTION)
    }) {
        return None;
    }
    if navigation::reference_target_kinds(document, position).is_some()
        || document
            .symbols()
            .all_structs
            .get(&word)
            .is_some_and(|symbol| contains_position(symbol.selection_range, position))
    {
        return Some(RenameTarget {
            kind: RenameTargetKind::AnchorType,
            range,
            placeholder: word,
        });
    }
    None
}

fn account_data_field_locations(
    document: &ParsedDocument,
    uri: &Url,
    account_data_type: &str,
    field_name: &str,
) -> Vec<Location> {
    let mut locations = Vec::new();
    if let Some(field) = document
        .symbols()
        .account_data_structs
        .get(account_data_type)
        .and_then(|account| account.fields.iter().find(|field| field.name == field_name))
    {
        locations.push(Location {
            uri: uri.clone(),
            range: field.selection_range,
        });
    }

    for instruction in document.symbols().callable_functions() {
        let Some(context) = &instruction.context else {
            continue;
        };
        let Some(accounts) = document.symbols().accounts_structs.get(&context.name) else {
            continue;
        };
        for usage in &instruction.account_data_field_usages {
            if usage.field != field_name {
                continue;
            }
            let matches_account_data_type = accounts
                .fields
                .iter()
                .find(|field| field.name == usage.account)
                .and_then(|field| field.generic_type_names.last())
                .is_some_and(|name| name == account_data_type);
            if matches_account_data_type {
                locations.push(Location {
                    uri: uri.clone(),
                    range: usage.range,
                });
            }
        }
    }

    locations
}

fn account_field_locations(
    document: &ParsedDocument,
    uri: &Url,
    accounts_type: &str,
    field_name: &str,
) -> Vec<Location> {
    let mut locations = Vec::new();
    if let Some(field) = document
        .symbols()
        .accounts_structs
        .get(accounts_type)
        .and_then(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == field_name)
        })
    {
        locations.push(Location {
            uri: uri.clone(),
            range: field.selection_range,
        });
    }

    for instruction in document.symbols().callable_functions() {
        let Some(context) = &instruction.context else {
            continue;
        };
        for usage in &instruction.account_path_usages {
            for (segment_index, segment) in usage.segments.iter().enumerate() {
                if segment.name != field_name {
                    continue;
                }
                let path = navigation::AccountPathPosition {
                    context: context.name.clone(),
                    segments: usage
                        .segments
                        .iter()
                        .map(|segment| segment.name.clone())
                        .collect(),
                    segment_index,
                    field: segment.name.clone(),
                };
                if navigation::account_field_path_definition_target_for_position(document, &path)
                    .is_some_and(|target| {
                        target.container == accounts_type && target.field == field_name
                    })
                {
                    locations.push(Location {
                        uri: uri.clone(),
                        range: segment.range,
                    });
                }
            }
        }
    }

    locations
}

fn instruction_argument_locations(
    document: &ParsedDocument,
    uri: &Url,
    context_name: &str,
    argument_name: &str,
) -> Vec<Location> {
    let mut locations = Vec::new();

    for instruction in document.symbols().callable_functions() {
        if instruction
            .context
            .as_ref()
            .is_none_or(|context| context.name != context_name)
        {
            continue;
        }
        for argument in &instruction.arguments {
            if instruction_argument_names_match(&argument.name, argument_name) {
                push_unique_location(
                    &mut locations,
                    Location {
                        uri: uri.clone(),
                        range: argument.range,
                    },
                );
            }
        }
    }

    let Some(accounts) = document.symbols().accounts_structs.get(context_name) else {
        return locations;
    };

    for argument in &accounts.instruction_arguments {
        if instruction_argument_names_match(&argument.name, argument_name) {
            push_unique_location(
                &mut locations,
                Location {
                    uri: uri.clone(),
                    range: argument.range,
                },
            );
        }
    }

    for field in &accounts.fields {
        for constraint in &field.account_constraints {
            for range in instruction_argument_ranges_in_constraint(
                document.source(),
                constraint,
                argument_name,
            ) {
                push_unique_location(
                    &mut locations,
                    Location {
                        uri: uri.clone(),
                        range,
                    },
                );
            }
        }
    }

    locations
}

fn instruction_argument_range_at_position(
    source: &str,
    constraint: &AccountConstraint,
    position: Position,
) -> Option<(Range, String)> {
    let evidence = ConstraintEvidence::new(constraint);
    for reference in evidence.instruction_argument_references() {
        let range = evidence.value_range(source, reference.key, reference.name)?;
        if contains_position(range, position) {
            return Some((range, reference.name.to_string()));
        }
    }

    for range in matching_word_ranges(source, &word_at_position(source, position)?) {
        if contains_position(range, position)
            && range_is_within(range, constraint.range)
            && looks_like_seed_argument(source, range)
        {
            return Some((range, word_at_position(source, position)?));
        }
    }

    None
}

fn instruction_argument_ranges_in_constraint(
    source: &str,
    constraint: &AccountConstraint,
    argument_name: &str,
) -> Vec<Range> {
    let evidence = ConstraintEvidence::new(constraint);
    let mut ranges = Vec::new();

    for reference in evidence.instruction_argument_references() {
        if instruction_argument_names_match(reference.name, argument_name) {
            if let Some(range) = evidence.value_range(source, reference.key, reference.name) {
                ranges.push(range);
            }
        }
    }

    for alias in instruction_argument_aliases(argument_name) {
        for range in matching_word_ranges(source, &alias) {
            if range_is_within(range, constraint.range) && looks_like_seed_argument(source, range) {
                ranges.push(range);
            }
        }
    }

    dedupe_ranges(ranges)
}

fn instruction_argument_aliases(argument_name: &str) -> Vec<String> {
    let mut aliases = vec![argument_name.to_string()];
    let trimmed = argument_name.trim_start_matches('_');
    if trimmed != argument_name && !trimmed.is_empty() {
        aliases.push(trimmed.to_string());
    }
    aliases
}

fn instruction_argument_names_match(left: &str, right: &str) -> bool {
    left == right || left.trim_start_matches('_') == right.trim_start_matches('_')
}

fn looks_like_seed_argument(source: &str, range: Range) -> bool {
    let Some(line) = source.lines().nth(range.start.line as usize) else {
        return false;
    };
    let Ok(end) = usize::try_from(range.end.character) else {
        return false;
    };
    let suffix = line
        .chars()
        .skip(end)
        .collect::<String>()
        .trim_start()
        .to_string();

    suffix.starts_with(".as_ref")
        || suffix.starts_with(".as_bytes")
        || suffix.starts_with(".to_le_bytes")
}

fn range_is_within(inner: Range, outer: Range) -> bool {
    (inner.start.line > outer.start.line
        || inner.start.line == outer.start.line && inner.start.character >= outer.start.character)
        && (inner.end.line < outer.end.line
            || inner.end.line == outer.end.line && inner.end.character <= outer.end.character)
}

fn dedupe_ranges(ranges: Vec<Range>) -> Vec<Range> {
    let mut deduped = Vec::new();
    for range in ranges {
        if !deduped.contains(&range) {
            deduped.push(range);
        }
    }
    deduped
}

fn dedupe_locations(locations: Vec<Location>) -> Vec<Location> {
    let mut deduped = Vec::new();
    for location in locations {
        push_unique_location(&mut deduped, location);
    }
    deduped
}

fn push_unique_location(locations: &mut Vec<Location>, location: Location) {
    if !locations
        .iter()
        .any(|existing| existing.uri == location.uri && existing.range == location.range)
    {
        locations.push(location);
    }
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
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
mod tests;
