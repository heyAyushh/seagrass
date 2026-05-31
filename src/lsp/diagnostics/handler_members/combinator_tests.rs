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
fn reports_unknown_member_inside_option_map_closure() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    maybe_record.map(|record| record.real_fake);
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
        "missing Option::map closure input diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_option_map_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.map(|record| record.metadata()).unwrap();
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
        "missing Option::map unwrap output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_option_and_then_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.and_then(|record| Some(record.metadata())).unwrap();
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
        "missing Option::and_then unwrap output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_result_map_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.map(|record| record.metadata()).unwrap();
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
        "missing Result::map unwrap output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_result_and_then_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.and_then(|record| Ok(record.metadata())).unwrap();
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
        "missing Result::and_then unwrap output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_result_ok_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.ok().unwrap();
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
        "missing Result::ok unwrap output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn does_not_report_member_after_invalid_option_ok_unwrap() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.ok().unwrap();
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
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`selected.real_fake`")),
        "Option::ok should not create a false member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn does_not_report_member_after_mismatched_result_and_then_wrapper() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.and_then(|record| Some(record.metadata())).unwrap();
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
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`selected.real_fake`")),
        "mismatched Result::and_then wrapper should not create a false member diagnostic: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unknown_members_after_option_map_unwrap(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = maybe_record.map(|record| record.metadata()).unwrap();
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
            "expected generated Option::map unwrap diagnostic: {diagnostics:#?}"
        );
    }
}
