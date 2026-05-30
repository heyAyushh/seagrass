use {
    super::super::collect_with_workspace,
    crate::{
        diagnostics::registry::{ANCHOR_ACCOUNT_USAGE_CODE, ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE},
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{NumberOrString, Url},
};

#[test]
fn local_program_instruction_evidence_survives_stale_workspace_reachability() {
    let document = ParsedDocument::parse(
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
    )
    .unwrap();
    let stale_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/stale.rs").unwrap(),
            r#"
#[program]
pub mod stale {
    pub fn unrelated(ctx: Context<Update>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub authority: Signer<'info>,
}
"#
            .to_string(),
        )],
    );

    let diagnostics = collect_with_workspace(&document, Some(&stale_index));

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        ) && diagnostic.message.contains("`counter`")
            && diagnostic.message.contains("missing `#[account(mut)]`")
    }));
}

#[test]
fn reports_missing_nested_composite_account_field_from_workspace_index() {
    let lib = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.iner.key();
        Ok(())
    }
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/accounts.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#
            .to_string(),
        )],
    );

    let diagnostics = collect_with_workspace(&lib, Some(&index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
            )
        })
        .expect("workspace nested missing account diagnostic");

    assert!(diagnostic.message.contains("`iner`"));
    assert!(diagnostic.message.contains("not declared in `Wrapped`"));
}

#[test]
fn reports_missing_account_data_field_from_workspace_index() {
    let lib = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundle.s.s;
        Ok(())
    }
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/accounts.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut)]
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
            .to_string(),
        )],
    );

    let diagnostics = collect_with_workspace(&lib, Some(&index));
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
        .expect("workspace account data field diagnostic");

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}

#[test]
fn reports_mutated_nested_composite_account_missing_mut_from_workspace_index() {
    let lib = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.wrapper.inner.count += 1;
        Ok(())
    }
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/accounts.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct Update<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#
            .to_string(),
        )],
    );

    let diagnostics = collect_with_workspace(&lib, Some(&index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
            )
        })
        .expect("workspace nested missing mut diagnostic");

    assert!(diagnostic.message.contains("`inner`"));
    assert!(diagnostic.message.contains("in `Wrapped`"));
}

#[test]
fn workspace_reachability_reports_called_split_helper_mutation() {
    let helper = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}

pub fn update_counter(ctx: Context<Update>) -> Result<()> {
    ctx.accounts.counter.count += 1;
    Ok(())
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/lib.rs").unwrap(),
                r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        instructions::update_counter(ctx)
    }
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/instructions.rs").unwrap(),
                helper.source().to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&helper, Some(&index));
    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}

#[test]
fn workspace_reachability_ignores_uncalled_split_helper_mutation() {
    let helper = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}

pub fn update_counter(ctx: Context<Update>) -> Result<()> {
    ctx.accounts.counter.count += 1;
    Ok(())
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/lib.rs").unwrap(),
                r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Update>) -> Result<()> {
        let _ = ctx.accounts.counter.key();
        Ok(())
    }
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/instructions.rs").unwrap(),
                helper.source().to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&helper, Some(&index));
    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_ACCOUNT_USAGE_CODE
        )
    }));
}
