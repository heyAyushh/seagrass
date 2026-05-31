use {
    crate::{diagnostics, document::ParsedDocument},
    proptest::prelude::*,
};

prop_compose! {
    fn generated_ident()(tail in "[a-z][a-z0-9_]{1,8}") -> String {
        format!("sg_{tail}")
    }
}

#[test]
fn reports_unknown_member_after_assignment_infers_type() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let mut selected;
    selected = bundles[0];
    selected.real_fake;
    Ok(())
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#,
    ));

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "missing assignment-inferred member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_unwrapped_assignment_infers_type() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let mut selected;
    selected = bundles.first().unwrap();
    selected.real_fake;
    Ok(())
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#,
    ));

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "missing unwrapped assignment-inferred member diagnostic: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unknown_members_after_assignment_infers_type(
        binding in generated_ident(),
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(binding != known && binding != missing && known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {{
    let mut {binding};
    {binding} = bundles[0];
    {binding}.{missing};
    Ok(())
}}

pub struct PositionBundle {{
    pub {known}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );

        let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&source));

        prop_assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!("`{binding}.{missing}` does not resolve"))),
            "expected generated assignment-inferred member diagnostic: {diagnostics:#?}"
        );
    }
}
