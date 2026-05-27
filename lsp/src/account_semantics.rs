use {
    crate::{
        constraint_catalog::{self, ConstraintFamily},
        constraint_text,
        document::SymbolRange,
    },
    quote::ToTokens,
    std::collections::HashMap,
    syn::{Attribute, Field, ItemStruct},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountTypeEvidence {
    Constraint,
    AccountReference,
    TokenAccountProperty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedAccountInnerType {
    pub generic: &'static str,
    pub evidence: AccountTypeEvidence,
}

pub fn expected_account_inner_type(
    container: &str,
    _field_name: &str,
    constraint_texts: &[String],
) -> Option<ExpectedAccountInnerType> {
    if !matches!(container, "Account" | "InterfaceAccount") {
        return None;
    }

    if constraint_texts.iter().any(|text| {
        text_uses_any_family(
            text,
            [
                ConstraintFamily::TokenAccount,
                ConstraintFamily::AssociatedTokenAccount,
            ],
        )
    }) {
        return Some(ExpectedAccountInnerType {
            generic: "TokenAccount",
            evidence: AccountTypeEvidence::Constraint,
        });
    }

    if constraint_texts.iter().any(|text| {
        text_uses_any_family(
            text,
            [ConstraintFamily::Mint, ConstraintFamily::MintExtension],
        )
    }) {
        return Some(ExpectedAccountInnerType {
            generic: "Mint",
            evidence: AccountTypeEvidence::Constraint,
        });
    }

    None
}

pub fn expected_account_inner_type_for_syn_field(
    container: &str,
    field: &Field,
) -> Option<ExpectedAccountInnerType> {
    let field_name = field.ident.as_ref()?.to_string();
    let constraint_texts = account_constraint_texts_from_attrs(&field.attrs);
    expected_account_inner_type(container, &field_name, &constraint_texts)
}

pub fn expected_account_inner_types_for_accounts_struct(
    item_struct: &ItemStruct,
) -> HashMap<String, ExpectedAccountInnerType> {
    let syn::Fields::Named(fields) = &item_struct.fields else {
        return HashMap::new();
    };

    expected_account_inner_types_from_field_constraints(
        fields
            .named
            .iter()
            .filter_map(|field| {
                let name = field.ident.as_ref()?.to_string();
                Some((name, account_constraint_texts_from_attrs(&field.attrs)))
            })
            .collect(),
    )
}

pub fn expected_account_inner_types_for_document_accounts_struct(
    accounts: &SymbolRange,
) -> HashMap<String, ExpectedAccountInnerType> {
    expected_account_inner_types_from_field_constraints(
        accounts
            .fields
            .iter()
            .map(|field| {
                (
                    field.name.clone(),
                    field
                        .account_constraints
                        .iter()
                        .map(|constraint| normalize_token_text(&constraint.text))
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
    )
}

pub fn expected_account_inner_type_for_document_field_in_accounts(
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> Option<ExpectedAccountInnerType> {
    expected_account_inner_types_for_document_accounts_struct(accounts)
        .get(&field.name)
        .copied()
}

pub fn declared_or_expected_account_inner_type<'a>(
    accounts: &'a SymbolRange,
    field: &'a SymbolRange,
) -> Option<&'a str> {
    let declared = field.generic_type_names.last().map(String::as_str);
    let expected = expected_account_inner_type_for_document_field_in_accounts(accounts, field);

    match (declared, expected) {
        (Some(generic), Some(expected))
            if expected.evidence == AccountTypeEvidence::TokenAccountProperty
                && !generic.is_empty() =>
        {
            Some(generic)
        }
        (_, Some(expected)) => Some(expected.generic),
        (declared, None) => declared,
    }
}

pub fn field_has_declared_or_expected_account_inner_type(
    accounts: &SymbolRange,
    field: &SymbolRange,
    generic: &str,
) -> bool {
    declared_or_expected_account_inner_type(accounts, field) == Some(generic)
}

#[allow(dead_code)]
pub fn expected_account_inner_type_for_document_field(
    field: &SymbolRange,
) -> Option<ExpectedAccountInnerType> {
    let container = field.type_name.as_deref()?;
    let constraint_texts = field
        .account_constraints
        .iter()
        .map(|constraint| constraint.text.clone())
        .collect::<Vec<_>>();
    expected_account_inner_type(container, &field.name, &constraint_texts)
}

fn text_uses_any_family<const N: usize>(text: &str, families: [ConstraintFamily; N]) -> bool {
    constraint_catalog::CONSTRAINTS.iter().any(|spec| {
        families.contains(&spec.family)
            && constraint_text::has_flag_or_key(text, constraint_catalog::key(spec.label))
    })
}

pub fn expected_account_inner_types_from_field_constraints(
    field_constraints: Vec<(String, Vec<String>)>,
) -> HashMap<String, ExpectedAccountInnerType> {
    let mut expected = HashMap::new();
    let field_constraints = field_constraints
        .into_iter()
        .map(|(field, constraints)| {
            (
                field,
                constraints
                    .into_iter()
                    .map(|constraint| normalize_token_text(&constraint))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    for (field_name, constraints) in &field_constraints {
        for key in ["token::mint", "associated_token::mint"] {
            for reference in account_reference_values(constraints, key) {
                expected.insert(
                    reference,
                    ExpectedAccountInnerType {
                        generic: "Mint",
                        evidence: AccountTypeEvidence::AccountReference,
                    },
                );
            }
        }

        for key in ["token::authority", "associated_token::authority"] {
            if account_reference_values(constraints, key)
                .iter()
                .any(|reference| reference == field_name)
            {
                expected.insert(
                    field_name.clone(),
                    ExpectedAccountInnerType {
                        generic: "TokenAccount",
                        evidence: AccountTypeEvidence::AccountReference,
                    },
                );
            }
        }
    }

    let address_aliases = address_aliases(&field_constraints);
    for expression in mint_reference_expressions(&field_constraints) {
        if let Some(field_name) = field_name_for_expression(&expression, &address_aliases) {
            expected.insert(
                field_name,
                ExpectedAccountInnerType {
                    generic: "Mint",
                    evidence: AccountTypeEvidence::AccountReference,
                },
            );
        }
    }

    for field_name in token_account_property_references(&field_constraints) {
        expected
            .entry(field_name)
            .or_insert(ExpectedAccountInnerType {
                generic: "TokenAccount",
                evidence: AccountTypeEvidence::TokenAccountProperty,
            });
    }

    expected
}

fn account_reference_values(constraint_texts: &[String], key: &str) -> Vec<String> {
    constraint_texts
        .iter()
        .filter_map(|text| constraint_value(text, key))
        .filter_map(simple_identifier)
        .collect()
}

fn address_aliases(field_constraints: &[(String, Vec<String>)]) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    for (field_name, constraints) in field_constraints {
        for constraint in constraints {
            if let Some(expression) = constraint_value(constraint, "address") {
                aliases.insert(expression, field_name.clone());
            }
        }
    }
    aliases
}

fn mint_reference_expressions(field_constraints: &[(String, Vec<String>)]) -> Vec<String> {
    let mut expressions = Vec::new();
    for (_, constraints) in field_constraints {
        for constraint in constraints {
            expressions.extend(expressions_compared_to_mint_property(constraint));
        }
    }
    expressions
}

fn expressions_compared_to_mint_property(text: &str) -> Vec<String> {
    const MINT_EQ: &str = ".mint==";
    let mut expressions = Vec::new();
    let mut search_start = 0usize;
    while let Some(relative) = text[search_start..].find(MINT_EQ) {
        let value_start = search_start + relative + MINT_EQ.len();
        if let Some(value) = take_constraint_value(&text[value_start..]) {
            expressions.push(value);
        }
        search_start = value_start;
    }
    expressions
}

fn token_account_property_references(field_constraints: &[(String, Vec<String>)]) -> Vec<String> {
    const TOKEN_ACCOUNT_PROPERTIES: &[&str] = &[
        "amount",
        "owner",
        "delegate",
        "state",
        "is_native",
        "delegated_amount",
        "close_authority",
    ];

    let field_names = field_constraints
        .iter()
        .map(|(field_name, _)| field_name.as_str())
        .collect::<Vec<_>>();
    let mut references = Vec::new();
    for field_name in &field_names {
        if field_constraints.iter().any(|(_, constraints)| {
            constraints.iter().any(|constraint| {
                TOKEN_ACCOUNT_PROPERTIES
                    .iter()
                    .any(|property| has_path_property_access(constraint, field_name, property))
            })
        }) {
            references.push((*field_name).to_string());
        }
    }
    references
}

fn has_path_property_access(text: &str, field_name: &str, property: &str) -> bool {
    let needle = format!("{field_name}.{property}");
    let mut search_start = 0usize;
    while let Some(relative) = text[search_start..].find(&needle) {
        let start = search_start + relative;
        let end = start + needle.len();
        let previous_ok = text[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_ident_char(ch) && ch != ':');
        let next_ok = text[end..]
            .chars()
            .next()
            .is_none_or(|ch| !is_ident_char(ch));
        if previous_ok && next_ok {
            return true;
        }
        search_start = start + 1;
    }
    false
}

fn field_name_for_expression(
    expression: &str,
    address_aliases: &HashMap<String, String>,
) -> Option<String> {
    address_aliases
        .get(expression)
        .cloned()
        .or_else(|| simple_identifier(expression.to_string()))
}

fn constraint_value(text: &str, key: &str) -> Option<String> {
    let value = constraint_text::value_after_key(text, key)?;
    take_constraint_value(value)
}

fn take_constraint_value(value: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut end = value.len();
    for (idx, ch) in value.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => {
                end = idx;
                break;
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                end = idx;
                break;
            }
            _ => {}
        }
    }
    let trimmed = value[..end].trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn simple_identifier(value: String) -> Option<String> {
    value.chars().all(is_ident_char).then_some(value)
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn account_constraint_texts_from_attrs(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("account"))
        .map(|attr| normalize_token_text(&attr.meta.to_token_stream().to_string()))
        .collect()
}

fn normalize_token_text(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument, proptest::prelude::*};

    #[test]
    fn mint_constraints_imply_mint_inner_type() {
        let constraints = vec!["account(init,mint::decimals=6,mint::authority=payer)".to_string()];

        assert_eq!(
            expected_account_inner_type("InterfaceAccount", "asset", &constraints),
            Some(ExpectedAccountInnerType {
                generic: "Mint",
                evidence: AccountTypeEvidence::Constraint,
            })
        );
    }

    #[test]
    fn token_constraints_imply_token_account_inner_type() {
        let constraints = vec!["account(init,token::mint=mint,token::authority=payer)".to_string()];

        assert_eq!(
            expected_account_inner_type("Account", "vault", &constraints),
            Some(ExpectedAccountInnerType {
                generic: "TokenAccount",
                evidence: AccountTypeEvidence::Constraint,
            })
        );
    }

    #[test]
    fn associated_token_constraints_imply_token_account_inner_type() {
        let constraints = vec![
            "account(init,associated_token::mint=mint,associated_token::authority=payer)"
                .to_string(),
        ];

        assert_eq!(
            expected_account_inner_type("Account", "ata", &constraints),
            Some(ExpectedAccountInnerType {
                generic: "TokenAccount",
                evidence: AccountTypeEvidence::Constraint,
            })
        );
    }

    #[test]
    fn token_mint_reference_infers_mint_account_field() {
        let item: ItemStruct = syn::parse_quote! {
            pub struct Create<'info> {
                #[account(token::mint = mint, token::authority = owner)]
                pub vault: Account<'info, TokenAccount>,
                pub mint: InterfaceAccount<'info, a>,
            }
        };

        let expected = expected_account_inner_types_for_accounts_struct(&item);

        assert_eq!(
            expected.get("mint"),
            Some(&ExpectedAccountInnerType {
                generic: "Mint",
                evidence: AccountTypeEvidence::AccountReference,
            })
        );
    }

    #[test]
    fn mint_pubkey_comparison_infers_address_constrained_mint_account() {
        let item: ItemStruct = syn::parse_quote! {
            pub struct CollectFeesV2<'info> {
                #[account(address = whirlpool.token_mint_b)]
                pub token_mint_b: InterfaceAccount<'info, a>,
                #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
                pub token_owner_account_b: InterfaceAccount<'info, TokenAccount>,
            }
        };

        let expected = expected_account_inner_types_for_accounts_struct(&item);

        assert_eq!(
            expected.get("token_mint_b"),
            Some(&ExpectedAccountInnerType {
                generic: "Mint",
                evidence: AccountTypeEvidence::AccountReference,
            })
        );
    }

    #[test]
    fn custom_token_account_property_access_infers_token_account_field() {
        let item: ItemStruct = syn::parse_quote! {
            pub struct CollectFeesV2<'info> {
                pub position: Account<'info, Position>,
                #[account(
                    constraint = position_token_account.mint == position.position_mint,
                    constraint = position_token_account.amount == 1,
                )]
                pub position_token_account: InterfaceAccount<'info, a>,
            }
        };

        let expected = expected_account_inner_types_for_accounts_struct(&item);

        assert_eq!(
            expected.get("position_token_account"),
            Some(&ExpectedAccountInnerType {
                generic: "TokenAccount",
                evidence: AccountTypeEvidence::TokenAccountProperty,
            })
        );
        assert_eq!(expected.get("position"), None);
    }

    #[test]
    fn token_account_property_evidence_does_not_override_declared_data_type() {
        let source = r#"
#[derive(Accounts)]
pub struct UseState<'info> {
    #[account(constraint = state.amount == 1)]
    pub state: Account<'info, Position>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let accounts = document.symbols().accounts_structs.get("UseState").unwrap();
        let field = accounts
            .fields
            .iter()
            .find(|field| field.name == "state")
            .unwrap();

        assert_eq!(
            declared_or_expected_account_inner_type(accounts, field),
            Some("Position")
        );
    }

    #[test]
    fn declared_or_expected_inner_type_prefers_semantic_account_role() {
        let source = r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, M>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: InterfaceAccount<'info, TokenAccount>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let accounts = document
            .symbols()
            .accounts_structs
            .get("CollectFeesV2")
            .unwrap();
        let field = accounts
            .fields
            .iter()
            .find(|field| field.name == "token_mint_b")
            .unwrap();

        assert_eq!(
            declared_or_expected_account_inner_type(accounts, field),
            Some("Mint")
        );
    }

    prop_compose! {
        fn identifier()(head in "[a-z]", tail in "[a-z0-9_]{0,12}") -> String {
            format!("{head}{tail}")
        }
    }

    proptest! {
        #[test]
        fn token_mint_reference_property_infers_mint_account(
            vault in identifier(),
            mint in identifier(),
            authority in identifier(),
        ) {
            prop_assume!(vault != mint && vault != authority && mint != authority);
            let expected = expected_account_inner_types_from_field_constraints(vec![
                (
                    vault,
                    vec![format!("account(token::mint={mint},token::authority={authority})")],
                ),
                (mint.clone(), Vec::new()),
                (authority, Vec::new()),
            ]);

            prop_assert_eq!(
                expected.get(&mint),
                Some(&ExpectedAccountInnerType {
                    generic: "Mint",
                    evidence: AccountTypeEvidence::AccountReference,
                })
            );
        }

        #[test]
        fn address_alias_mint_comparison_property_infers_mint_account(
            mint_field in identifier(),
            token_account in identifier(),
            source_account in identifier(),
            property in identifier(),
        ) {
            prop_assume!(mint_field != token_account && mint_field != source_account);
            let expression = format!("{source_account}.{property}");
            let expected = expected_account_inner_types_from_field_constraints(vec![
                (
                    mint_field.clone(),
                    vec![format!("account(address={expression})")],
                ),
                (
                    token_account,
                    vec![format!("account(constraint=token_owner.mint=={expression})")],
                ),
            ]);

            prop_assert_eq!(
                expected.get(&mint_field),
                Some(&ExpectedAccountInnerType {
                    generic: "Mint",
                    evidence: AccountTypeEvidence::AccountReference,
                })
            );
        }
    }
}
