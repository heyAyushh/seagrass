use {
    super::collect_with_workspace,
    crate::{
        diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
    },
    tower_lsp::lsp_types::NumberOrString,
};

#[path = "generated.rs"]
mod generated;

#[test]
fn reports_unknown_explicit_handler_local_member() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let bundle: PositionBundle = PositionBundle { position_bundle_mint: Pubkey::default() };
        bundle.s;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`bundle.s` does not resolve"))
        .unwrap_or_else(|| panic!("missing handler member diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
        ))
    );
    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}

#[test]
fn reports_field_called_as_handler_method() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
        bundle.position_bundle_mint();
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("calls `position_bundle_mint` as a method")
        })
        .unwrap_or_else(|| panic!("missing field-as-method diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("field-called-as-method")
    );
    assert!(diagnostic
        .message
        .contains("use `bundle.position_bundle_mint`"));
}

#[test]
fn reports_text_recovered_unknown_member_when_rhs_is_missing() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.missing = ;
    Ok(())
}

pub struct PositionBundle {
    pub known: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`bundle.missing` does not resolve")
                && diagnostic
                    .message
                    .contains("`PositionBundle` has no field `missing`")
        }),
        "missing text-recovered handler member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_text_recovered_unknown_member_through_context_account_alias() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.missing = ;
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub known: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`position_bundle.missing` does not resolve")
                && diagnostic
                    .message
                    .contains("`PositionBundle` has no field `missing`")
        }),
        "missing context-account alias member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_context_bump_member() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    ctx.bumps.payer;
    Ok(())
}

#[account]
pub struct State {
    pub value: u64,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`ctx.bumps.payer` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing context bump diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`RunBumps` has no field `payer`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("candidates"))
            .and_then(|value| value.as_array())
            .map(|values| values
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["state"])
    );
}

#[test]
fn accepts_known_context_bump_member() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    let state_bump = ctx.bumps.state;
    Ok(())
}

#[account]
pub struct State {
    pub value: u64,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("ctx.bumps.state")),
        "known context bump should resolve, got {diagnostics:#?}"
    );
}

#[test]
fn reports_context_bump_member_when_context_has_no_pdas() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub payer: Signer<'info>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    ctx.bumps.payer;
    Ok(())
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`ctx.bumps.payer` does not resolve")
                && diagnostic
                    .message
                    .contains("`RunBumps` has no field `payer`")
        }),
        "empty generated Bumps struct should still reject fields: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_context_bump_member_after_alias() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    let bumps = ctx.bumps;
    bumps.payer;
    Ok(())
}

#[account]
pub struct State {
    pub value: u64,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`bumps.payer` does not resolve")
                && diagnostic
                    .message
                    .contains("`RunBumps` has no field `payer`")
        }),
        "missing context bump alias diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn accepts_known_explicit_handler_local_member() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let bundle: PositionBundle = PositionBundle { position_bundle_mint: Pubkey::default() };
        bundle.position_bundle_mint;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("position_bundle_mint` does not resolve")),
        "known handler member should resolve, got {diagnostics:#?}"
    );
}

#[test]
fn reports_nested_explicit_handler_local_member() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
        bundle.inner.fake;
        Ok(())
    }
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`bundle.inner.fake` does not resolve")
                && diagnostic
                    .message
                    .contains("`InnerBundle` has no field `fake`")
        }),
        "missing nested handler member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_through_typed_handler_alias() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
        let alias = bundle;
        alias.fake;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`alias.fake` does not resolve")
                && diagnostic
                    .message
                    .contains("`PositionBundle` has no field `fake`")
        }),
        "missing alias handler member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_through_typed_field_alias() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
        let inner = bundle.inner;
        inner.fake;
        Ok(())
    }
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("`inner.fake` does not resolve")
                && diagnostic
                    .message
                    .contains("`InnerBundle` has no field `fake`")
        }),
        "missing field alias handler member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_through_context_account_alias() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.fake;
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub real: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`position_bundle.fake` does not resolve")
                && diagnostic
                    .message
                    .contains("`PositionBundle` has no field `fake`")
        }),
        "missing context-account alias handler member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_through_accounts_alias_field() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub bundle_account: Box<Account<'info, PositionBundle>>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    let accounts = &mut ctx.accounts;
    accounts.bundle_account.fake;
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`accounts.bundle_account.fake` does not resolve")
                && diagnostic
                    .message
                    .contains("`PositionBundle` has no field `fake`")
        }),
        "missing accounts-alias field member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_through_account_alias_from_accounts_alias() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub bundle_account: Box<Account<'info, PositionBundle>>,
}

pub fn run(ctx: Context<Run>) -> Result<()> {
    let accounts = &mut ctx.accounts;
    let bundle = &mut accounts.bundle_account;
    bundle.fake;
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`bundle.fake` does not resolve")
                && diagnostic
                    .message
                    .contains("`PositionBundle` has no field `fake`")
        }),
        "missing account alias from accounts-alias diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn ignores_unknown_handler_alias_type() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let alias = unknown_value;
        alias.fake;
        Ok(())
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("alias.fake")),
        "unknown alias type should stay outside shallow resolver, got {diagnostics:#?}"
    );
}

#[test]
fn ignores_unknown_external_handler_local_type() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, external: ExternalType) -> Result<()> {
        external.fake;
        Ok(())
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("external.fake")),
        "external type should stay outside shallow resolver, got {diagnostics:#?}"
    );
}
