use {
    super::{diagnostics_for, ANCHOR_ACCOUNT_USAGE_CODE, ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE},
    proptest::prelude::*,
    tower_lsp::lsp_types::NumberOrString,
};

prop_compose! {
    fn generated_ident()(tail in "[a-z0-9_]{1,10}") -> String {
        format!("sg_{tail}")
    }
}

fn reference_token(reference: bool) -> &'static str {
    if reference {
        "&"
    } else {
        ""
    }
}

fn mutability_token(reference: &str, mutable: bool) -> &'static str {
    if !reference.is_empty() && mutable {
        "mut "
    } else {
        ""
    }
}

proptest! {
    #[test]
    fn reports_generated_missing_account_data_field_through_alias(
        alias in generated_ident(),
        account_field in generated_ident(),
        known_field in generated_ident(),
        missing_field in generated_ident(),
        mutable in any::<bool>(),
        reference in any::<bool>(),
    ) {
        prop_assume!(alias != "ctx" && alias != account_field);
        prop_assume!(known_field != missing_field);
        let reference = reference_token(reference);
        let mutability = mutability_token(reference, mutable);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn read(ctx: Context<Read>) -> Result<()> {{
        let {alias} = {reference}{mutability}ctx.accounts.{account_field};
        {alias}.{missing_field};
        Ok(())
    }}
}}

#[derive(Accounts)]
pub struct Read<'info> {{
    pub {account_field}: Account<'info, AccountData>,
}}

#[account]
pub struct AccountData {{
    pub {known_field}: Pubkey,
}}
"#
        );

        let diagnostics = diagnostics_for(&source);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.code.as_ref(),
                    Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
                ) && diagnostic
                    .message
                    .contains(&format!("`{alias}.{missing_field}` does not resolve"))
                    && diagnostic
                        .message
                        .contains(&format!("`AccountData` has no field `{missing_field}`"))
            }),
            "expected missing account-data member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn accepts_generated_known_account_data_field_through_alias(
        alias in generated_ident(),
        account_field in generated_ident(),
        known_field in generated_ident(),
        mutable in any::<bool>(),
        reference in any::<bool>(),
    ) {
        prop_assume!(alias != "ctx" && alias != account_field);
        let reference = reference_token(reference);
        let mutability = mutability_token(reference, mutable);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn read(ctx: Context<Read>) -> Result<()> {{
        let {alias} = {reference}{mutability}ctx.accounts.{account_field};
        let _ = {alias}.{known_field};
        Ok(())
    }}
}}

#[derive(Accounts)]
pub struct Read<'info> {{
    pub {account_field}: Account<'info, AccountData>,
}}

#[account]
pub struct AccountData {{
    pub {known_field}: Pubkey,
}}
"#
        );

        let diagnostics = diagnostics_for(&source);

        prop_assert!(
            diagnostics.iter().all(|diagnostic| {
                !matches!(
                    diagnostic.code.as_ref(),
                    Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
                ) || !diagnostic
                    .message
                    .contains(&format!("`{alias}.{known_field}` does not resolve"))
            }),
            "known account-data member should resolve, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_alias_mutation_missing_mut_constraint(
        alias in generated_ident(),
        account_field in generated_ident(),
        data_field in generated_ident(),
    ) {
        prop_assume!(alias != "ctx" && alias != account_field);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn update(ctx: Context<Update>) -> Result<()> {{
        let {alias} = &mut ctx.accounts.{account_field};
        {alias}.{data_field} = Pubkey::default();
        Ok(())
    }}
}}

#[derive(Accounts)]
pub struct Update<'info> {{
    pub {account_field}: Account<'info, AccountData>,
}}

#[account]
pub struct AccountData {{
    pub {data_field}: Pubkey,
}}
"#
        );

        let diagnostics = diagnostics_for(&source);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| {
                matches!(
                    diagnostic.code.as_ref(),
                    Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
                ) && diagnostic.message.contains(&format!("`{account_field}`"))
                    && diagnostic.message.contains("missing `#[account(mut)]`")
            }),
            "expected missing mut diagnostic, got {diagnostics:#?}"
        );
    }
}
