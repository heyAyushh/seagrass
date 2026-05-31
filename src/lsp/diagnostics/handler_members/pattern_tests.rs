use {
    crate::{
        diagnostics::{self, registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE},
        document::ParsedDocument,
    },
    proptest::prelude::*,
    tower_lsp::lsp_types::NumberOrString,
};

prop_compose! {
    fn generated_ident()(tail in "[a-z][a-z0-9_]{1,8}") -> String {
        format!("sg_{tail}")
    }
}

#[test]
fn reports_unknown_member_on_if_let_account_pattern_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    if let Some(bundle) = ctx.accounts.optional_bundle.as_ref() {
        bundle.asset_fake;
    }
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#,
    ));

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`bundle.asset_fake` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing typed if-let member diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
        ))
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("ownerType"))
            .and_then(|value| value.as_str()),
        Some("PositionBundle")
    );
}

#[test]
fn accepts_known_member_on_if_let_account_pattern_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    if let Some(bundle) = ctx.accounts.optional_bundle.as_ref() {
        bundle.asset_mint;
    }
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#,
    ));

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("asset_mint")),
        "known if-let member should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_on_let_else_account_pattern_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let Some(bundle) = ctx.accounts.optional_bundle.as_ref() else {
        return Ok(());
    };
    bundle.asset_fake;
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#,
    ));

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`bundle.asset_fake` does not resolve")),
        "missing typed let-else member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn does_not_infer_members_from_opaque_tuple_destructuring() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, pair: Pair) -> Result<()> {
    let (bundle, _) = pair;
    bundle.asset_fake;
    Ok(())
}

pub struct Pair(PositionBundle, u64);

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#,
    ));

    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("`bundle.asset_fake` does not resolve")),
        "opaque tuple destructuring should not guess member type: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unknown_members_on_if_let_pattern_bindings(
        binding in generated_ident(),
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(binding != known && binding != missing && known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    if let Some({binding}) = ctx.accounts.optional_bundle.as_ref() {{
        {binding}.{missing};
    }}
    Ok(())
}}

#[account]
pub struct PositionBundle {{
    pub {known}: Pubkey,
}}
"#
        );

        let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&source));

        prop_assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!("`{binding}.{missing}` does not resolve"))),
            "expected generated if-let member diagnostic: {diagnostics:#?}"
        );
    }
}
