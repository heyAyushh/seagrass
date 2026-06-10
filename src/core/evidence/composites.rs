use {
    crate::document::{ParsedDocument, SymbolRange},
    std::collections::HashSet,
};

pub(super) fn account_names<'a>(
    document: &'a ParsedDocument,
    accounts: &'a SymbolRange,
) -> HashSet<&'a str> {
    accounts
        .fields
        .iter()
        .filter_map(|field| {
            field
                .type_name
                .as_deref()
                .and_then(|name| document.symbols().accounts_structs.get(name))
        })
        .flat_map(|nested| nested.fields.iter().map(|field| field.name.as_str()))
        .collect()
}
