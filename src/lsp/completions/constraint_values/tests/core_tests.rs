use super::*;

#[test]
fn completes_empty_constraint_value_after_space_trigger() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = )]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    let items = completions(&document, position_after(source, "payer = "))
        .expect("expected payer value completions after space");

    assert!(items.iter().any(|item| item.label == "payer"));
}

#[test]
fn payer_value_completion_lists_only_signer_fields() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = )]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    pub user_program: Program<'info, MyProgram>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "payer = "))
        .expect("expected payer completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"payer"));
    assert!(labels.contains(&"authority"));
    assert!(!labels.contains(&"user_program"));
}

#[test]
fn completes_value_inside_unclosed_account_attribute() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = u
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "payer = u"))
        .expect("expected payer completions in unclosed account attribute");

    assert!(items.iter().any(|item| item.label == "user"));
}

#[test]
fn completes_values_inside_broken_account_attribute_shapes() {
    for scenario in [
        BrokenAttributeScenario {
            attribute: "init, payer = u",
            cursor: "payer = u",
            expected: "user",
        },
        BrokenAttributeScenario {
            attribute: "init, space = 8",
            cursor: "space = 8",
            expected: "8 + State::INIT_SPACE",
        },
        BrokenAttributeScenario {
            attribute: "has_one = a",
            cursor: "has_one = a",
            expected: "authority",
        },
        BrokenAttributeScenario {
            attribute: "token::mint = m",
            cursor: "token::mint = m",
            expected: "mint",
        },
        BrokenAttributeScenario {
            attribute: "associated_token::authority = a",
            cursor: "associated_token::authority = a",
            expected: "authority",
        },
        BrokenAttributeScenario {
            attribute: "mint::authority = a",
            cursor: "mint::authority = a",
            expected: "authority",
        },
        BrokenAttributeScenario {
            attribute: "seeds = [u",
            cursor: "seeds = [u",
            expected: "user.key().as_ref()",
        },
        BrokenAttributeScenario {
            attribute: "seeds = [b\"state\", u",
            cursor: "seeds = [b\"state\", u",
            expected: "user.key().as_ref()",
        },
        BrokenAttributeScenario {
            attribute: "seeds = [[b\"state\"], u",
            cursor: "seeds = [[b\"state\"], u",
            expected: "user.key().as_ref()",
        },
        BrokenAttributeScenario {
            attribute: "seeds = [helper(authority.key(), u",
            cursor: "seeds = [helper(authority.key(), u",
            expected: "user.key().as_ref()",
        },
    ] {
        let source = broken_attribute_source(scenario.attribute);
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, scenario.cursor))
            .unwrap_or_else(|| panic!("expected completions for `{}`", scenario.attribute));

        assert!(
            items.iter().any(|item| item.label == scenario.expected),
            "expected `{}` for `{}`; got {:?}",
            scenario.expected,
            scenario.attribute,
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completes_space_for_program_owned_account_data() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "space = 8")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "8 + State::INIT_SPACE"));
}

#[test]
fn does_not_complete_space_for_spl_mint_or_token_accounts() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = 6, mint::authority = payer, space = 8)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(init, payer = payer, token::mint = mint, token::authority = payer, space = 8)]
    pub vault: Account<'info, TokenAccount>,
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    assert!(completions(
        &document,
        position_after(source, "mint::authority = payer, space = 8")
    )
    .is_none());
    assert!(completions(
        &document,
        position_after(source, "token::authority = payer, space = 8")
    )
    .is_none());
}

#[test]
fn completes_extension_account_reference_from_generated_catalog() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(extensions::group_pointer::group_address = g)]
    pub mint: Account<'info, Mint>,
    pub group: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "extensions::group_pointer::group_address = g"),
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "group"));
}

#[test]
fn extension_signer_completion_carries_generated_slot_data() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(extensions::permanent_delegate::delegate = a)]
    pub mint: InterfaceAccount<'info, Mint>,
    pub authority: Signer<'info>,
    pub analytics_program: Program<'info, Analytics>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let completions = completions(
        &document,
        position_after(source, "extensions::permanent_delegate::delegate = a"),
    )
    .unwrap();
    let first = completions.first().expect("expected authority completion");

    assert_eq!(first.label, "authority");
    assert_eq!(
        first
            .data
            .as_ref()
            .and_then(|data| data.get("constraintKey"))
            .and_then(|value| value.as_str()),
        Some("extensions::permanent_delegate::delegate")
    );
}

#[test]
fn completes_values_for_every_generated_account_reference_constraint() {
    for (key, value_kind) in constraint_catalog::account_reference_specs() {
        let scenario = reference_completion_scenario(key, value_kind);
        let source =
            generated_value_completion_source(key, scenario.prefix, "token_decimals: u8, bump: u8");
        let document = ParsedDocument::parse(&source).unwrap();
        let completions = completions(
            &document,
            position_after(&source, &format!("{key} = {}", scenario.prefix)),
        )
        .unwrap_or_else(|| panic!("expected completions for generated constraint `{key}`"));

        assert!(
            completions
                .iter()
                .any(|item| item.label == scenario.expected_label),
            "generated constraint `{key}` did not suggest `{}`; got {:?}",
            scenario.expected_label,
            completions
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completes_values_for_every_generated_instruction_argument_constraint() {
    for key in constraint_catalog::instruction_argument_keys() {
        let (prefix, expected) = match key {
            "bump" => ("bu", "bump"),
            "mint::decimals" => ("to", "token_decimals"),
            _ => panic!("missing generated instruction argument completion scenario for {key}"),
        };
        let source = generated_value_completion_source(key, prefix, "token_decimals: u8, bump: u8");
        let document = ParsedDocument::parse(&source).unwrap();
        let completions = completions(
            &document,
            position_after(&source, &format!("{key} = {prefix}")),
        )
        .unwrap_or_else(|| panic!("expected completions for generated constraint `{key}`"));

        assert!(
            completions.iter().any(|item| item.label == expected),
            "generated constraint `{key}` did not suggest `{expected}`"
        );
    }
}

#[test]
fn completes_instruction_argument_from_workspace_split_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = to)]
    pub mint: Account<'info, Mint>,
}
"#;
    let instruction_source = r#"
#[program]
pub mod demo {
    use super::*;
    pub fn create(ctx: Context<Create>, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/instructions.rs").unwrap(),
            instruction_source.to_string(),
        )],
    );

    let completions = completions_with_workspace(
        &document,
        position_after(source, "mint::decimals = to"),
        Some(&workspace_index),
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "_token_decimals"));
}

#[test]
fn instruction_argument_completion_matches_prefix_without_leading_underscore() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = token_dec)]
    pub mint: Account<'info, Mint>,
}
"#;
    let instruction_source = r#"
#[program]
pub mod demo {
    use super::*;
    pub fn create(ctx: Context<Create>, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/instructions.rs").unwrap(),
            instruction_source.to_string(),
        )],
    );

    let completions = completions_with_workspace(
        &document,
        position_after(source, "mint::decimals = token_dec"),
        Some(&workspace_index),
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "_token_decimals"));
}
