use {
    super::constraints,
    crate::{
        anchor_types::{self, AnchorFieldCompletion, AnchorFieldCompletionKind},
        document::{ParsedDocument, PdaBump, PdaConstraint, PdaSeeds, SymbolRange},
        semantic::{
            AccountField, AccountType, AccountsStruct, CompositeRef, ConstraintValue,
            ExtractionConfidence, PdaSeedSet, Populated, PopulatedFields,
        },
    },
    std::collections::{HashMap, HashSet},
};

pub fn extract_accounts_structs(document: &ParsedDocument) -> Vec<Populated<AccountsStruct>> {
    let mut account_symbols = document
        .symbols()
        .accounts_structs
        .iter()
        .collect::<Vec<_>>();
    account_symbols.sort_by(|left, right| left.0.cmp(right.0));
    let all_account_names = account_symbols
        .iter()
        .map(|(name, _)| (*name).clone())
        .collect::<HashSet<_>>();
    let symbols_by_name = account_symbols
        .iter()
        .map(|(name, symbol)| ((*name).clone(), (*symbol).clone()))
        .collect::<HashMap<_, _>>();

    account_symbols
        .into_iter()
        .map(|(_, symbol)| {
            let mut visited = HashSet::new();
            extract_accounts_struct(symbol, &all_account_names, &symbols_by_name, &mut visited)
        })
        .collect()
}

pub fn extract_accounts_struct(
    symbol: &SymbolRange,
    all_account_names: &HashSet<String>,
    symbols_by_name: &HashMap<String, SymbolRange>,
    visited: &mut HashSet<String>,
) -> Populated<AccountsStruct> {
    visited.insert(symbol.name.clone());
    let mut all_composites_resolved = true;
    let fields = symbol.fields.iter().map(extract_account_field).collect();
    let composite_refs = symbol
        .fields
        .iter()
        .filter_map(|field| {
            let target_struct_name = composite_target(field, all_account_names)?;
            let resolved = if visited.contains(&target_struct_name) {
                all_composites_resolved = false;
                None
            } else if let Some(target) = symbols_by_name.get(&target_struct_name) {
                let mut nested_visited = visited.clone();
                Some(Box::new(
                    extract_accounts_struct(
                        target,
                        all_account_names,
                        symbols_by_name,
                        &mut nested_visited,
                    )
                    .inner,
                ))
            } else {
                all_composites_resolved = false;
                None
            };
            Some(CompositeRef {
                field_name: field.name.clone(),
                target_struct_name,
                resolved,
            })
        })
        .collect();
    let mut populated_fields = PopulatedFields::default();
    if all_composites_resolved {
        populated_fields.set(PopulatedFields::COMPOSITE_REFS_RESOLVED);
    }
    if symbol
        .fields
        .iter()
        .any(|field| field.pda_constraint.is_some())
    {
        populated_fields.set(PopulatedFields::PDA_SEEDS);
    }
    Populated::with_fields(
        AccountsStruct {
            name: symbol.name.clone(),
            fields,
            composite_refs,
        },
        ExtractionConfidence::MacroAnnounced,
        populated_fields,
    )
}

pub fn extract_account_field(field: &SymbolRange) -> AccountField {
    let account_type = account_type(field.type_name.as_deref());
    AccountField {
        name: field.name.clone(),
        source_range: field.selection_range,
        account_type,
        constraints: constraints::constraints_from_account_constraints(&field.account_constraints),
        token_interface_candidate: token_interface_candidate(field, account_type),
    }
}

pub fn pda_seed_set(field: &SymbolRange) -> Option<PdaSeedSet> {
    let pda = field.pda_constraint.as_ref()?;
    Some(PdaSeedSet {
        seeds: match &pda.seeds {
            PdaSeeds::List(seeds) => constraints::pda_seeds_from_texts(seeds),
            PdaSeeds::Expr(expr) => vec![crate::semantic::PdaSeed::Expression(expr.clone())],
        },
        bump: bump_value(pda),
        program_id: pda
            .program_seed
            .as_ref()
            .map(|program| ConstraintValue::Expression(program.clone())),
    })
}

fn bump_value(pda: &PdaConstraint) -> Option<ConstraintValue> {
    match &pda.bump {
        PdaBump::Canonical => Some(ConstraintValue::Absent),
        PdaBump::Explicit(value) => Some(ConstraintValue::Expression(value.clone())),
        PdaBump::Missing => None,
    }
}

fn composite_target(field: &SymbolRange, all_account_names: &HashSet<String>) -> Option<String> {
    let type_name = field.type_name.as_ref()?;
    if account_type(Some(type_name.as_str())) != AccountType::Unknown {
        return None;
    }
    all_account_names
        .contains(type_name)
        .then(|| type_name.to_string())
}

fn account_type(type_name: Option<&str>) -> AccountType {
    match type_name {
        Some("AccountInfo") => AccountType::RawAccountInfo,
        Some("Account") => AccountType::Account,
        Some("UncheckedAccount") => AccountType::UncheckedAccount,
        Some("Interface") => AccountType::Interface,
        Some("InterfaceAccount") => AccountType::InterfaceAccount,
        Some("Program") => AccountType::Program,
        Some("Signer") => AccountType::Signer,
        Some("SystemAccount") => AccountType::SystemAccount,
        _ => AccountType::Unknown,
    }
}

fn token_interface_candidate(field: &SymbolRange, account_type: AccountType) -> bool {
    match account_type {
        AccountType::Interface | AccountType::InterfaceAccount => true,
        AccountType::Program => field
            .generic_type_names
            .iter()
            .any(|generic| catalog_token_program_types().contains(generic.as_str())),
        _ => false,
    }
}

fn catalog_token_program_types() -> HashSet<&'static str> {
    anchor_types::field_completions()
        .iter()
        .filter(|completion| {
            completion.kind == AnchorFieldCompletionKind::Program
                && catalog_entry_is_token_program(completion)
        })
        .filter_map(|completion| program_generic(completion.label))
        .collect()
}

fn catalog_entry_is_token_program(completion: &AnchorFieldCompletion) -> bool {
    matches!(
        completion.source_path,
        "spl/src/token.rs" | "spl/src/token_2022.rs" | "spl/src/token_interface.rs"
    )
}

fn program_generic(label: &str) -> Option<&str> {
    label
        .strip_prefix("Program<'info, ")
        .and_then(|rest| rest.strip_suffix('>'))
}
