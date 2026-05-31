use crate::{completions, diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_flags_unresolved_anchor_handler_call_identifier() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        verify_bundel(bundle_index);
        Ok(())
    }
}

fn verify_bundle(bundle_index: u16) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub authority: Signer<'info>,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`verify_bundel` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing unresolved handler call diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unresolved-handler-identifier")
    );
    assert!(diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .is_some_and(|candidates| candidates
            .iter()
            .any(|candidate| candidate.as_str() == Some("verify_bundle"))));
}

#[test]
fn editor_ux_resolves_if_let_pattern_values_in_anchor_handlers() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let maybe_receiver = Some(ctx.accounts.receiver.key());
    if let Some(position_bundle) = maybe_receiver {
        let selected = pos
    }
    Ok(())
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`position_bundle` does not resolve")
        }),
        "if-let pattern value should stay resolved in editor diagnostics: {diagnostics:#?}"
    );

    let items = completions::completions(
        &document,
        super::position_after(source, "let selected = pos"),
    )
    .expect("editor-visible if-let pattern completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("position_bundle")
    );
}

#[test]
fn editor_ux_resolves_typed_if_let_pattern_members() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    if let Some(bundle) = ctx.accounts.optional_bundle.as_ref() {
        bundle.asset_
    }
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`bundle` does not resolve")),
        "typed if-let binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "bundle.asset_"))
        .expect("editor-visible typed if-let member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("asset_mint")
    );
}
