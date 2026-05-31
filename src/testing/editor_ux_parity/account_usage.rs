use {
    super::position_after,
    crate::{
        completions,
        diagnostics::{self, ANCHOR_SECURITY_SIGNER_CODE},
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{NumberOrString, Url},
};

#[test]
fn editor_ux_completes_member_on_boxed_account_alias() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.position_
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "position_bundle.position_"),
    )
    .expect("editor-visible context account alias completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

#[test]
fn editor_ux_completes_member_through_accounts_alias() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub bundled_position: Box<Account<'info, PositionBundle>>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let accounts = &mut ctx.accounts;
    accounts.bundled_position.position_
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "accounts.bundled_position.position_"),
    )
    .expect("editor-visible accounts alias member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

#[test]
fn editor_ux_completes_context_bump_members() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(seeds = [b"bundle"], bump)]
    pub bundled_position: Account<'info, Position>,
    pub receiver: Signer<'info>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    ctx.bumps.bundled_
}

#[account]
pub struct Position {
    pub value: u64,
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "ctx.bumps.bundled_"),
    )
    .expect("editor-visible context bump completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"bundled_position"));
    assert!(!labels.contains(&"receiver"));
}

#[test]
fn editor_ux_flags_unknown_member_on_boxed_account_alias() {
    let document = ParsedDocument::parse_or_empty(
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
    );
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/close_accounts.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#
            .to_string(),
        )],
    );

    let diagnostics = diagnostics::collect_with_workspace(&document, Some(&workspace_index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`position_bundle.s` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing boxed account alias diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}

#[test]
fn editor_ux_flags_unknown_context_bump_member() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(seeds = [b"bundle"], bump)]
    pub bundled_position: Account<'info, Position>,
    pub receiver: Signer<'info>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    ctx.bumps.receiver;
    Ok(())
}

#[account]
pub struct Position {
    pub value: u64,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`ctx.bumps.receiver` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing context bump diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`CloseBumps` has no field `receiver`"));
}

#[test]
fn editor_ux_flags_unknown_member_on_typed_handler_local() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        bundle.s.s;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`bundle.s` does not resolve"))
        .unwrap_or_else(|| panic!("missing typed handler local diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unknown-handler-member")
    );
}

#[test]
fn editor_ux_flags_unknown_member_on_typed_handler_alias() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        let alias = bundle;
        alias.s.s;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`alias.s` does not resolve"))
        .unwrap_or_else(|| panic!("missing typed handler alias diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unknown-handler-member")
    );
}

#[test]
fn editor_ux_flags_split_helper_signer_usage() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(accounts_source);
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/accounts.rs").unwrap(),
                accounts_source.to_string(),
            ),
            (
                Url::parse("file:///tmp/instructions/log_message.rs").unwrap(),
                r#"
use anchor_lang::solana_program::instruction::AccountMeta;

pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
    let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
    Ok(())
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/lib.rs").unwrap(),
                r#"
#[program]
pub mod demo {
    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        instructions::log_message(ctx)
    }
}
"#
                .to_string(),
            ),
        ],
    );

    let diagnostics = diagnostics::collect_with_workspace(&document, Some(&workspace_index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_SECURITY_SIGNER_CODE
            )
        })
        .unwrap_or_else(|| panic!("missing split helper signer diagnostic: {diagnostics:#?}"));

    assert!(diagnostic.message.contains("`authority`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|value| value.as_str()),
        Some("authority")
    );
}

#[test]
fn editor_ux_flags_unresolved_anchor_handler_identifier() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundle = position_bundle = sd;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`sd` does not resolve"))
        .unwrap_or_else(|| {
            panic!("missing unresolved handler identifier diagnostic: {diagnostics:#?}")
        });

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unresolved-handler-identifier")
    );
}
