use {
    super::*,
    tower_lsp::lsp_types::{Position, Range},
};

#[test]
fn parse_error_expected_semicolon_points_to_unterminated_statement() {
    let source = r#"use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle

    Ok(())
}
"#;

    let err = syn::parse_file(source).unwrap_err();
    let diagnostic = diagnostic_from_parse_error_with_source(err, source);

    assert_eq!(diagnostic.message, "unexpected token, expected `;`");
    assert_eq!(
        diagnostic.range,
        Range {
            start: Position {
                line: 4,
                character: 4
            },
            end: Position {
                line: 4,
                character: 19
            },
        }
    );
}

#[test]
fn parse_error_expected_semicolon_does_not_jump_to_previous_account_attribute() {
    let source = r#"use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[derive(Accounts)]
#[instruction(bundle_index: u16)]
pub struct CloseBundledPosition<'info> {
    #[account(mut)]
    pub bundled_position: Account<'info, Position>,

    #[account(mut)]
    pub position_bundle: Box<Account<'info, PositionBundle>>,

    #[account(
        constraint = position_bundle_token_account.mint == bundled_position.position_mint,
        constraint = position_bundle_token_account.mint == position_bundle.position_bundle_mint,
        constraint = position_bundle_token_account.amount == 1
    )]
    pub position_bundle_token_account: Box<Account<'info, TokenAccount>>,

    pub position_bundle_authority: Signer<'info>,

    #[account(mut)]
    pub receiver: UncheckedAccount<'info>,
}

pub fn handler() -> Result<()> {
    position_bundle = position_bundle  sd ;
    Ok(())
}
"#;

    let err = syn::parse_file(source).unwrap_err();
    let diagnostic = diagnostic_from_parse_error_with_source(err, source);
    let target_line = source
        .lines()
        .position(|line| line.contains("position_bundle = position_bundle"))
        .expect("fixture contains a malformed handler line");
    let line = source.lines().nth(target_line).expect("target line exists");

    assert_eq!(diagnostic.message, "unexpected token, expected `;`");
    assert_eq!(
        diagnostic.range,
        Range {
            start: Position {
                line: target_line as u32,
                character: line.find("sd").expect("bad token start") as u32,
            },
            end: Position {
                line: target_line as u32,
                character: (line.find("sd").expect("bad token start") + "sd".len()) as u32,
            },
        }
    );
}

#[test]
fn parse_error_reports_unresolved_identifier_inside_anchor_handler() {
    let source = r#"use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>, bundle_index: u16) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle = position_bundle  sd ;
    Ok(())
}
"#;

    let err = syn::parse_file(source).unwrap_err();
    let diagnostic = diagnostic_from_parse_error_with_source(err, source);

    assert!(
        diagnostic.message.contains("`sd` does not resolve"),
        "parse error should recover handler-scope semantics: {diagnostic:#?}"
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|reason| reason.as_str()),
        Some("unresolved-handler-identifier")
    );
}

#[test]
fn parse_error_scope_recovery_does_not_bind_current_let_lhs() {
    let source = r#"use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>, bundle_index: u16) -> Result<()> {
    let current = bundle_index  missing_value ;
    Ok(())
}
"#;

    let err = syn::parse_file(source).unwrap_err();
    let diagnostic = diagnostic_from_parse_error_with_source(err, source);
    let candidates = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .expect("expected recovered scope candidates");

    assert!(diagnostic
        .message
        .contains("`missing_value` does not resolve"));
    assert!(candidates
        .iter()
        .any(|candidate| candidate.as_str() == Some("bundle_index")));
    assert!(!candidates
        .iter()
        .any(|candidate| candidate.as_str() == Some("current")));
}

#[test]
fn parse_error_scope_recovery_keeps_file_values_resolved() {
    let source = r#"use anchor_lang::prelude::*;
use crate::util::verify_position_bundle_authority;

const MAX_BUNDLE_INDEX: u16 = 256;

#[program]
pub mod demo {
    use super::*;

    pub fn close(ctx: Context<CloseBundledPosition>, bundle_index: u16) -> Result<()> {
        let mut copied = bundle_index;
        copied = copied  missing_value ;
        Ok(())
    }

    pub fn collect(ctx: Context<CloseBundledPosition>) -> Result<()> {
        Ok(())
    }
}
"#;

    let err = syn::parse_file(source).unwrap_err();
    let diagnostic = diagnostic_from_parse_error_with_source(err, source);
    let candidates = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .expect("expected recovered scope candidates");

    assert!(diagnostic
        .message
        .contains("`missing_value` does not resolve"));
    for expected in [
        "MAX_BUNDLE_INDEX",
        "verify_position_bundle_authority",
        "close",
        "collect",
    ] {
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.as_str() == Some(expected)),
            "missing recovered candidate `{expected}` in {candidates:#?}"
        );
    }
}

#[test]
fn parse_error_scope_recovery_accepts_imported_value_identifier() {
    let source = r#"use anchor_lang::prelude::*;
use crate::util::verify_position_bundle_authority;

pub fn handler(ctx: Context<CloseBundledPosition>, bundle_index: u16) -> Result<()> {
    let mut copied = bundle_index;
    copied = copied  verify_position_bundle_authority ;
    Ok(())
}
"#;

    let err = syn::parse_file(source).unwrap_err();
    let diagnostic = diagnostic_from_parse_error_with_source(err, source);

    assert!(
        !diagnostic
            .message
            .contains("`verify_position_bundle_authority` does not resolve"),
        "imported value should not be reported unresolved during parser recovery: {diagnostic:#?}"
    );
    assert_ne!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|reason| reason.as_str()),
        Some("unresolved-handler-identifier")
    );
}
