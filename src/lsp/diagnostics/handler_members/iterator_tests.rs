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
fn reports_unknown_member_after_iterator_next_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    let selected = bundles.iter().next().unwrap();
    selected.real_fake;
    Ok(())
}

pub struct SampleRecord {
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
        "missing iterator next member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_iterator_adapter_chain() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    let selected = bundles.iter().filter(|_| true).nth(0).unwrap();
    selected.real_fake;
    Ok(())
}

pub struct SampleRecord {
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
        "missing iterator adapter chain member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_iterator_find_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    let selected = bundles.iter().find(|_| true).unwrap();
    selected.real_fake;
    Ok(())
}

pub struct SampleRecord {
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
        "missing iterator find member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_inside_iterator_map_closure() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    records.iter().map(|record| record.real_fake).count();
    Ok(())
}

pub struct SampleRecord {
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
            .contains("`record.real_fake` does not resolve")),
        "missing iterator map closure input diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_inside_iterator_filter_closure() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    records.iter().filter(|record| record.real_fake == Pubkey::default()).count();
    Ok(())
}

pub struct SampleRecord {
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
            .contains("`record.real_fake` does not resolve")),
        "missing iterator filter closure input diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_iterator_map_next_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    let selected = records.iter().map(|record| record.metadata()).next().unwrap();
    selected.real_fake;
    Ok(())
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

impl SampleRecord {
    pub fn metadata(&self) -> SampleMetadata {
        SampleMetadata { real_authority: Pubkey::default() }
    }
}

pub struct SampleMetadata {
    pub real_authority: Pubkey,
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
        "missing iterator map output diagnostic: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unknown_members_after_iterator_adapter_chain(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {{
    let selected = bundles.iter().filter(|_| true).next().unwrap();
    selected.{missing};
    Ok(())
}}

pub struct SampleRecord {{
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
                .any(|diagnostic| diagnostic.message.contains(&format!("`selected.{missing}` does not resolve"))),
            "expected generated iterator-chain member diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_members_after_iterator_map_output(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {{
    let selected = records.iter().map(|record| record.metadata()).next().unwrap();
    selected.{missing};
    Ok(())
}}

pub struct SampleRecord {{
    pub source: Pubkey,
}}

impl SampleRecord {{
    pub fn metadata(&self) -> SampleMetadata {{
        SampleMetadata {{ {known}: Pubkey::default() }}
    }}
}}

pub struct SampleMetadata {{
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
                .any(|diagnostic| diagnostic.message.contains(&format!("`selected.{missing}` does not resolve"))),
            "expected generated iterator map output diagnostic: {diagnostics:#?}"
        );
    }
}
