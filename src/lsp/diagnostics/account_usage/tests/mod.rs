use {
    super::*,
    crate::diagnostics::registry::{
        ANCHOR_ACCOUNT_USAGE_CODE, ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE,
    },
    tower_lsp::lsp_types::NumberOrString,
};

mod generated;
mod workspace;

#[test]
fn reports_mutated_account_missing_mut_constraint() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        ) && diagnostic.message.contains("missing `#[account(mut)]`")
    }));
}

#[test]
fn accepts_mutated_account_with_mut_constraint() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

#[test]
fn reports_mutated_account_through_accounts_alias() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let accounts = &mut ctx.accounts;
        accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        ) && diagnostic.message.contains("`counter`")
            && diagnostic.message.contains("missing `#[account(mut)]`")
    }));
}

#[test]
fn reports_unknown_account_through_accounts_alias() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let accounts = &ctx.accounts;
        let _ = accounts.countr.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        ) && diagnostic.message.contains("`countr`")
            && diagnostic.message.contains("not declared in `Update`")
    }));
}

#[test]
fn reports_unknown_ctx_accounts_field_before_mutability_guess() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, authority: Pubkey) -> Result<()> {
        let counter = &mut ctx.accounts.acc;
        counter.authority = authority;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + 40)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        ) && diagnostic.message.contains("`acc`")
            && diagnostic.message.contains("not declared in `Create`")
    }));
    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        ) || !diagnostic.message.contains("`acc`")
    }));
}

#[test]
fn accepts_account_data_field_mutation_through_local_binding() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, authority: Pubkey) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.authority = authority;
        counter.count = 0;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + 40)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        ) || !diagnostic.message.contains("`count`")
    }));
}

#[test]
fn accepts_initialized_account_data_field_mutation() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, authority: Pubkey) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.authority = authority;
        ctx.accounts.counter.count = 0;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + 40)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code))
                if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
                    || code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

#[test]
fn reports_missing_nested_composite_account_field() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.iner.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
            )
        })
        .expect("nested missing account diagnostic");
    assert!(diagnostic.message.contains("`iner`"));
    assert!(diagnostic.message.contains("not declared in `Wrapped`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("accountsStruct"))
            .and_then(|value| value.as_str()),
        Some("Wrapped")
    );
}

#[test]
fn accepts_declared_nested_composite_account_field() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        )
    }));
}

#[test]
fn keeps_account_data_fields_out_of_nested_account_field_diagnostics() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}

#[account]
pub struct Counter {
    pub count: u64,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        )
    }));
}

#[test]
fn reports_missing_account_data_field_through_local_binding() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundle.s.s;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub position_bundle: Account<'info, PositionBundle>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
            ) && diagnostic
                .message
                .contains("`position_bundle.s` does not resolve")
        })
        .expect("missing account data field diagnostic");
    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}

#[test]
fn reports_missing_account_data_field_through_composite_path() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        ctx.accounts.wrapper.inner.fake;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}

#[account]
pub struct Inner {
    pub value: u64,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
            ) && diagnostic
                .message
                .contains("`wrapper.inner.fake` does not resolve")
        })
        .expect("composite account data field diagnostic");
    assert!(diagnostic.message.contains("`Inner` has no field `fake`"));
}

#[test]
fn reports_missing_account_data_field_through_composite_local_binding() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let wrapper = &ctx.accounts.wrapper;
        wrapper.inner.fake;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}

#[account]
pub struct Inner {
    pub value: u64,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
            ) && diagnostic
                .message
                .contains("`wrapper.inner.fake` does not resolve")
        })
        .expect("composite alias account data field diagnostic");
    assert!(diagnostic.message.contains("`Inner` has no field `fake`"));
}

#[test]
fn accepts_known_account_data_field_through_composite_path() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        ctx.accounts.wrapper.inner.value;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}

#[account]
pub struct Inner {
    pub value: u64,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        )
    }));
}

#[test]
fn reports_mutated_nested_composite_account_missing_mut_constraint() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.wrapper.inner.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
            )
        })
        .expect("nested missing mut diagnostic");

    assert!(diagnostic.message.contains("`inner`"));
    assert!(diagnostic.message.contains("in `Wrapped`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("accountsStruct"))
            .and_then(|value| value.as_str()),
        Some("Wrapped")
    );
}

#[test]
fn accepts_mutated_nested_composite_account_with_mut_constraint() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.wrapper.inner.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    #[account(mut)]
    pub inner: Account<'info, Inner>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

#[test]
fn accepts_mutated_nested_composite_account_with_init_constraint() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.wrapper.inner.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    #[account(init, payer = user, space = 8 + 40)]
    pub inner: Account<'info, Inner>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

#[test]
fn reports_mutated_account_from_called_helper_function() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        update_counter(ctx)
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}

pub fn update_counter(ctx: Context<Update>) -> Result<()> {
    ctx.accounts.counter.count += 1;
    Ok(())
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        ) && diagnostic.message.contains("`update_counter`")
    }));
}

#[test]
fn ignores_mutated_account_from_uncalled_helper_function_when_program_mapping_exists() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Update>) -> Result<()> {
        let _ = ctx.accounts.counter.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}

pub fn update_counter(ctx: Context<Update>) -> Result<()> {
    ctx.accounts.counter.count += 1;
    Ok(())
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

#[test]
fn accepts_mutated_account_with_zero_constraint() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.data.load_init()?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(zero)]
    pub data: AccountLoader<'info, Data>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

fn diagnostics_for(source: &str) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse(source).unwrap();
    collect(&document)
}
