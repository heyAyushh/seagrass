use {
    super::contains_position,
    crate::{
        document::{ParsedDocument, SymbolRange},
        navigation,
    },
    tower_lsp::lsp_types::Position,
};

pub(super) fn field_at_position<'a>(
    document: &'a ParsedDocument,
    word: &str,
    position: Position,
) -> Option<(String, &'a SymbolRange)> {
    if let Some(field) = document
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
                .map(|field| (symbol.name.clone(), field))
        })
    {
        return Some(field);
    }

    if let Some(field) = document
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
                .map(|field| (accounts.name.clone(), field))
        })
    {
        return Some(field);
    }

    let mut matches = document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .flat_map(|symbol| {
            symbol
                .fields
                .iter()
                .filter(move |field| field.name == word)
                .map(|field| (symbol.name.clone(), field))
        });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

pub(super) fn field_type_display(field: &SymbolRange) -> Option<String> {
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

pub(super) fn field_names(symbol: &SymbolRange) -> String {
    if symbol.fields.is_empty() {
        "none".to_string()
    } else {
        symbol
            .fields
            .iter()
            .map(|field| format!("`{}`", field.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(super) fn account_usage_summary(
    document: &ParsedDocument,
    accounts_name: &str,
    field_name: &str,
) -> Option<String> {
    let usages = document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts_name)
        })
        .flat_map(|instruction| {
            instruction
                .account_usages
                .iter()
                .filter(move |usage| usage.name == field_name)
                .map(move |usage| {
                    if usage.mutable {
                        format!("`{}` mutates", instruction.name)
                    } else {
                        format!("`{}` reads", instruction.name)
                    }
                })
        })
        .collect::<Vec<_>>();
    (!usages.is_empty()).then(|| usages.join(", "))
}

pub(super) fn account_data_field_usage_summary(
    document: &ParsedDocument,
    account_data_type: &str,
    field_name: &str,
) -> Option<String> {
    let usages = document
        .symbols()
        .callable_functions()
        .flat_map(|instruction| {
            instruction
                .account_data_field_usages
                .iter()
                .filter_map(move |usage| {
                    let accounts = instruction.context.as_ref().and_then(|context| {
                        document.symbols().accounts_structs.get(&context.name)
                    })?;
                    let usage_type = accounts
                        .fields
                        .iter()
                        .find(|field| field.name == usage.account)
                        .and_then(|field| field.generic_type_names.last())?;
                    (usage.field == field_name && usage_type == account_data_type).then(|| {
                        if usage.mutable {
                            format!("`{}` mutates", instruction.name)
                        } else {
                            format!("`{}` reads", instruction.name)
                        }
                    })
                })
        })
        .collect::<Vec<_>>();
    (!usages.is_empty()).then(|| usages.join(", "))
}

pub(super) fn account_path_usage_summary(
    document: &ParsedDocument,
    accounts_name: &str,
    field_name: &str,
) -> Option<String> {
    let usages = document
        .symbols()
        .callable_functions()
        .filter_map(|instruction| {
            let context = instruction.context.as_ref()?;
            let used = instruction
                .account_path_usages
                .iter()
                .flat_map(|usage| {
                    usage
                        .segments
                        .iter()
                        .enumerate()
                        .filter_map(|(index, segment)| {
                            let position = navigation::AccountPathPosition {
                                context: context.name.clone(),
                                segments: usage
                                    .segments
                                    .iter()
                                    .map(|segment| segment.name.clone())
                                    .collect(),
                                segment_index: index,
                                field: segment.name.clone(),
                            };
                            navigation::account_field_path_definition_target_for_position(
                                document, &position,
                            )
                            .filter(|target| {
                                target.container == accounts_name && target.field == field_name
                            })
                            .map(|_| usage.mutable)
                        })
                })
                .next()?;
            Some(if used {
                format!("`{}` mutates", instruction.name)
            } else {
                format!("`{}` reads", instruction.name)
            })
        })
        .collect::<Vec<_>>();
    (!usages.is_empty()).then(|| usages.join(", "))
}
