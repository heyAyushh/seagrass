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

#[test]
fn completes_members_on_let_else_account_pattern_binding() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let Some(bundle) = ctx.accounts.optional_bundle.as_ref() else {
        return Ok(());
    };
    bundle.asset_
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
    pub asset_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "bundle.asset_"))
        .expect("let-else account pattern member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"asset_mint"));
    assert!(labels.contains(&"asset_owner"));
}

#[test]
fn completes_members_on_struct_pattern_field_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let PositionBundle { inner, .. } = bundle;
    inner.real_
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "inner.real_"))
        .expect("struct pattern member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_on_function_struct_pattern_field_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    PositionBundle { inner, .. }: PositionBundle,
) -> Result<()> {
    inner.real_
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "inner.real_"))
        .expect("function struct pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "function struct pattern field should complete: {items:#?}"
    );
}

#[test]
fn completes_members_on_typed_closure_pattern_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let check = |PositionBundle { inner, .. }: PositionBundle| {
        inner.real_
    };
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "inner.real_"))
        .expect("typed closure pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "typed closure pattern field should complete: {items:#?}"
    );
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

proptest! {
    #[test]
    fn completes_generated_closure_pattern_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let binding = format!("inner_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let check = |PositionBundle {{ {binding}, .. }}: PositionBundle| {{
        {binding}.real_
    }};
}}

pub struct PositionBundle {{
    pub {binding}: InnerBundle,
}}

pub struct InnerBundle {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, &format!("{binding}.real_")))
            .expect("generated closure pattern member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated closure pattern member should complete; items: {items:#?}"
        );
    }
}

proptest! {
    #[test]
    fn completes_generated_struct_pattern_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let binding = format!("inner_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {{
    let PositionBundle {{ {binding}, .. }} = bundle;
    {binding}.real_
}}

pub struct PositionBundle {{
    pub {binding}: InnerBundle,
}}

pub struct InnerBundle {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, &format!("{binding}.real_")))
            .expect("generated struct pattern member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated struct pattern member should complete; items: {items:#?}"
        );
    }
}

proptest! {
    #[test]
    fn completes_generated_function_pattern_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let binding = format!("inner_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    PositionBundle {{ {binding}, .. }}: PositionBundle,
) -> Result<()> {{
    {binding}.real_
}}

pub struct PositionBundle {{
    pub {binding}: InnerBundle,
}}

pub struct InnerBundle {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, &format!("{binding}.real_")))
            .expect("generated function pattern member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated function pattern member should complete; items: {items:#?}"
        );
    }
}
