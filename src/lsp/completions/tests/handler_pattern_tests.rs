use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_on_if_let_account_pattern_binding() {
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
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
    pub asset_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "bundle.asset_"))
        .expect("if-let account pattern member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"asset_mint"));
    assert!(labels.contains(&"asset_owner"));
}

proptest! {
    #[test]
    fn completes_generated_if_let_account_pattern_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("asset_{field_tail}");
        let binding = format!("bundle_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    if let Some({binding}) = ctx.accounts.optional_bundle.as_ref() {{
        {binding}.asset_
    }}
}}

#[account]
pub struct PositionBundle {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, &format!("{binding}.asset_")))
            .expect("generated if-let account pattern member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated if-let member should complete; items: {items:#?}"
        );
    }
}
