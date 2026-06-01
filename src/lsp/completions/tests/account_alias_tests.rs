use {
    super::position_after,
    crate::lsp::completions::{proptest_support::rust_identifier, should_offer_completion},
    proptest::prelude::*,
};

#[test]
fn completion_gate_wakes_for_local_account_alias_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>) -> Result<()> {
    let account = &mut ctx.accounts.data;
    account.
}
"#;

    assert!(should_offer_completion(
        source,
        position_after(source, "account.")
    ));
}

#[test]
fn completion_gate_wakes_for_single_letter_alias_after_fn_substring() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>) -> Result<()> {
    let afn = &mut ctx.accounts;
    let a = &mut afn.data;
    a.
}
"#;

    assert!(should_offer_completion(
        source,
        position_after(source, "    a.")
    ));
}

#[test]
fn completion_gate_wakes_for_short_alias_from_numbered_accounts_field() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>) -> Result<()> {
    let afn = &mut ctx.accounts;
    let a = &mut afn.a0;
    a.
}
"#;

    assert!(should_offer_completion(
        source,
        position_after(source, "    a.")
    ));
}

#[test]
fn completion_gate_wakes_for_indexed_member_expression() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    bundles[0].real_
}
"#;

    assert!(should_offer_completion(
        source,
        position_after(source, "bundles[0].real_")
    ));
}

proptest! {
    #[test]
    fn completion_gate_wakes_for_generated_direct_account_alias_members(
        alias in rust_identifier(),
        account in rust_identifier(),
        member_prefix in "[a-z_]{0,8}",
        mutable in any::<bool>(),
        reference in any::<bool>(),
    ) {
        prop_assume!(alias != "ctx" && alias != account);
        let mutability = if mutable { "mut " } else { "" };
        let reference = if reference { "&" } else { "" };
        let source = format!(
            "use anchor_lang::prelude::*;\n\npub fn run(ctx: Context<Run>) -> Result<()> {{\n    let {alias} = {reference}{mutability}ctx.accounts.{account};\n    {alias}.{member_prefix}\n}}\n"
        );
        let completion_line = format!("    {alias}.{member_prefix}");

        prop_assert!(should_offer_completion(
            &source,
            position_after(&source, &completion_line)
        ));
    }

    #[test]
    fn completion_gate_wakes_for_generated_intermediate_account_alias_members(
        accounts_alias in rust_identifier(),
        account_alias in rust_identifier(),
        account in rust_identifier(),
        member_prefix in "[a-z_]{0,8}",
    ) {
        prop_assume!(accounts_alias != account_alias);
        prop_assume!(accounts_alias != "ctx" && account_alias != "ctx");
        prop_assume!(account_alias != account);
        let source = format!(
            "use anchor_lang::prelude::*;\n\npub fn run(ctx: Context<Run>) -> Result<()> {{\n    let {accounts_alias} = &mut ctx.accounts;\n    let {account_alias} = &mut {accounts_alias}.{account};\n    {account_alias}.{member_prefix}\n}}\n"
        );
        let completion_line = format!("    {account_alias}.{member_prefix}");

        prop_assert!(should_offer_completion(
            &source,
            position_after(&source, &completion_line)
        ));
    }
}
