use {
    super::super::collect_with_workspace,
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_type_identifier},
    proptest::prelude::*,
};

prop_compose! {
    fn generated_ident()(tail in "[a-z0-9_]{1,10}") -> String {
        format!("sg_{tail}")
    }
}

proptest! {
    #[test]
    fn reports_generated_unknown_typed_handler_member(
        local in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>) -> Result<()> {{
        let {local}: {owner} = {owner} {{ {known_field}: Pubkey::default() }};
        {local}.{missing_field};
        Ok(())
    }}
}}

pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{local}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated handler member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_block_item_members(
        declaration in prop_oneof![Just("const"), Just("static")],
        local in "[A-Z][A-Z0-9_]{1,10}",
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>) -> Result<()> {{
        {local}.{missing_field};

        {declaration} {local}: {owner} = {owner} {{
            {known_field}: Pubkey::default(),
        }};

        Ok(())
    }}
}}

pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{local}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated block item member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_field_called_as_method(
        local in generated_ident(),
        owner in rust_type_identifier(),
        field in generated_ident(),
    ) {
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
        {local}.{field}();
        Ok(())
    }}
}}

pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("calls `{field}` as a method"))
                    && diagnostic
                        .message
                        .contains(&format!("use `{local}.{field}`"))
            }),
            "expected generated field-as-method diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_text_recovered_unknown_typed_handler_member(
        local in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
    {local}.{missing_field} = ;
    Ok(())
}}

pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{local}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated text-recovered handler member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_member_through_context_account_alias(
        account_field in generated_ident(),
        alias in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: Box<Account<'info, {owner}>>,
}}

pub fn run(ctx: Context<Run>) -> Result<()> {{
    let {alias} = &mut ctx.accounts.{account_field};
    {alias}.{missing_field};
    Ok(())
}}

#[account]
pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{alias}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated context alias member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_member_through_accounts_alias(
        account_field in generated_ident(),
        accounts_alias in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(account_field != accounts_alias);
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: Box<Account<'info, {owner}>>,
}}

pub fn run(ctx: Context<Run>) -> Result<()> {{
    let {accounts_alias} = &mut ctx.accounts;
    {accounts_alias}.{account_field}.{missing_field};
    Ok(())
}}

#[account]
pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "`{accounts_alias}.{account_field}.{missing_field}` does not resolve"
                )) && diagnostic
                    .message
                    .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated accounts alias member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_context_bump_member(
        pda_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(pda_field != missing_field);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(seeds = [b"generated"], bump)]
    pub {pda_field}: Account<'info, GeneratedState>,
}}

pub fn run(ctx: Context<Run>) -> Result<()> {{
    ctx.bumps.{missing_field};
    Ok(())
}}

#[account]
pub struct GeneratedState {{
    pub value: u64,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`ctx.bumps.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`RunBumps` has no field `{missing_field}`"))
            }),
            "expected generated context bump diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_member_through_typed_alias(
        local in generated_ident(),
        alias in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(local != alias);
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
        let {alias} = {local};
        {alias}.{missing_field};
        Ok(())
    }}
}}

pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{alias}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated alias member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_account_loader_direct_alias_member(
        account_field in generated_ident(),
        alias in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: AccountLoader<'info, {owner}>,
}}

pub fn run(ctx: Context<Run>) -> Result<()> {{
    let {alias} = &ctx.accounts.{account_field};
    {alias}.{missing_field};
    Ok(())
}}

#[account(zero_copy)]
pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{alias}.{missing_field}` does not resolve"))
                    && diagnostic.message.contains(&format!(
                        "`AccountLoader<{owner}>` has no field `{missing_field}`"
                    ))
            }),
            "expected generated AccountLoader direct alias diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_member_after_as_ref_alias(
        account_field in generated_ident(),
        alias in generated_ident(),
        owner in rust_type_identifier(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(known_field != missing_field);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: Account<'info, {owner}>,
}}

pub fn run(ctx: Context<Run>) -> Result<()> {{
    let {alias} = ctx.accounts.{account_field}.as_ref();
    {alias}.{missing_field};
    Ok(())
}}

#[account]
pub struct {owner} {{
    pub {known_field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();

        let diagnostics = collect_with_workspace(&document, None);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("`{alias}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`{owner}` has no field `{missing_field}`"))
            }),
            "expected generated as_ref alias diagnostic, got {diagnostics:#?}"
        );
    }
}
