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
fn editor_ux_completes_typed_handler_methods() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
    bundle.ver
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

impl PositionBundle {
    pub fn verify_bundle(&self) -> bool {
        true
    }

    pub fn static_helper() -> bool {
        true
    }
}
"#,
    );

    let items =
        completions::completions(&document, position_after(document.source(), "bundle.ver"))
            .expect("editor-visible typed handler method completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"verify_bundle()"));
    assert!(!labels.contains(&"static_helper()"));
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
fn editor_ux_flags_field_called_as_handler_method() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        bundle.position_bundle_mint();
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
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
}

#[test]
fn editor_ux_completes_loaded_account_loader_members() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub position: AccountLoader<'info, Position>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position = ctx.accounts.position.load()?;
    position.position_
}

#[account(zero_copy)]
pub struct Position {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "position.position_"),
    )
    .expect("editor-visible loaded AccountLoader completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
}

#[test]
fn editor_ux_hides_account_loader_data_before_load() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let position = &ctx.accounts.position;
        position.position_bundle_mint;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub position: AccountLoader<'info, Position>,
}

#[account(zero_copy)]
pub struct Position {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`position.position_bundle_mint` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing direct AccountLoader diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`AccountLoader<Position>` has no field `position_bundle_mint`"));
}

#[test]
fn editor_ux_completes_account_members_after_as_ref() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position_bundle = ctx.accounts.position_bundle.as_ref();
    position_bundle.position_
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "position_bundle.position_"),
    )
    .expect("editor-visible as_ref account completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn editor_ux_completes_members_from_result_helper_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position_bundle = load_position_bundle()?;
    position_bundle.position_
}

fn load_position_bundle() -> Result<PositionBundle> {
    unreachable!()
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "position_bundle.position_"),
    )
    .expect("editor-visible helper return completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn editor_ux_completes_members_from_split_helper_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position_bundle = load_position_bundle()?;
    position_bundle.position_
}
"#,
    );
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/helpers.rs").unwrap(),
                r#"
use anchor_lang::prelude::*;

pub fn load_position_bundle() -> Result<PositionBundle> {
    unreachable!()
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let items = completions::completions_with_workspace(
        &document,
        position_after(document.source(), "position_bundle.position_"),
        Some(&workspace_index),
    )
    .expect("editor-visible split helper return completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn editor_ux_completes_members_from_result_method_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
    let metadata = bundle.metadata()?;
    metadata.position_
}

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}

pub struct BundleMetadata {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let items = completions::completions(
        &document,
        position_after(document.source(), "metadata.position_"),
    )
    .expect("editor-visible method return completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn editor_ux_completes_members_from_split_method_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
    let metadata = bundle.metadata()?;
    metadata.position_
}
"#,
    );
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/bundle.rs").unwrap(),
                r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/metadata.rs").unwrap(),
                r#"
pub struct BundleMetadata {
    pub position_bundle_mint: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let items = completions::completions_with_workspace(
        &document,
        position_after(document.source(), "metadata.position_"),
        Some(&workspace_index),
    )
    .expect("editor-visible split method return completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
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
