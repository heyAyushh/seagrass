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
fn reports_unknown_member_after_wrapper_fallback_methods() {
    let cases = [
        (
            "Option::or",
            "ctx: Context<Run>, maybe_record: Option<SampleRecord>",
            "maybe_record.or(Some(SampleRecord { real_mint: Pubkey::default() })).unwrap()",
        ),
        (
            "Option::or_else",
            "ctx: Context<Run>, maybe_record: Option<SampleRecord>",
            "maybe_record.or_else(|| Some(SampleRecord { real_mint: Pubkey::default() })).unwrap()",
        ),
        (
            "Option::xor",
            "ctx: Context<Run>, maybe_record: Option<SampleRecord>",
            "maybe_record.xor(Some(SampleRecord { real_mint: Pubkey::default() })).unwrap()",
        ),
        (
            "Option::ok_or",
            "ctx: Context<Run>, maybe_record: Option<SampleRecord>",
            "maybe_record.ok_or(()).unwrap()",
        ),
        (
            "Option::ok_or_else",
            "ctx: Context<Run>, maybe_record: Option<SampleRecord>",
            "maybe_record.ok_or_else(|| ()).unwrap()",
        ),
        (
            "Result::or",
            "ctx: Context<Run>, record_result: std::result::Result<SampleRecord, ()>",
            "record_result.or(Ok(SampleRecord { real_mint: Pubkey::default() })).unwrap()",
        ),
        (
            "Result::or_else",
            "ctx: Context<Run>, record_result: std::result::Result<SampleRecord, ()>",
            "record_result.or_else(|_| Ok(SampleRecord { real_mint: Pubkey::default() })).unwrap()",
        ),
        (
            "Result::map_err",
            "ctx: Context<Run>, record_result: std::result::Result<SampleRecord, ()>",
            "record_result.map_err(|_| ()).unwrap()",
        ),
        (
            "Result::inspect_err",
            "ctx: Context<Run>, record_result: std::result::Result<SampleRecord, ()>",
            "record_result.inspect_err(|_| {}).unwrap()",
        ),
    ];

    for (case_name, handler_args, selected_expr) in cases {
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler({handler_args}) -> Result<()> {{
    let selected = {selected_expr};
    selected.real_fake;
    Ok(())
}}

pub struct SampleRecord {{
    pub real_mint: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );

        let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&source));

        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("`selected.real_fake` does not resolve")),
            "{case_name}: missing wrapper fallback diagnostic: {diagnostics:#?}"
        );
    }
}

#[test]
fn reports_unknown_member_after_option_map_or() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.map_or(
        SampleMetadata { real_authority: Pubkey::default() },
        |record| record.metadata(),
    );
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
        "missing Option::map_or output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_result_map_or_else() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.map_or_else(
        |_| SampleMetadata { real_authority: Pubkey::default() },
        |record| record.metadata(),
    );
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
        "missing Result::map_or_else output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn does_not_report_member_for_mismatched_map_or_outputs() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.map_or(
        SampleMetadata { real_authority: Pubkey::default() },
        |record| OtherMetadata { other_authority: Pubkey::default() },
    );
    selected.real_fake;
    Ok(())
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

pub struct SampleMetadata {
    pub real_authority: Pubkey,
}

pub struct OtherMetadata {
    pub other_authority: Pubkey,
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
        "mismatched map_or outputs should not create a false member diagnostic: {diagnostics:#?}"
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

    #[test]
    fn reports_generated_unknown_members_after_option_map_or(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = maybe_record.map_or(
        SampleMetadata {{ {known}: Pubkey::default() }},
        |record| record.metadata(),
    );
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
            "expected generated Option::map_or diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_members_after_option_ok_or(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = maybe_record.ok_or(()).unwrap();
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
            "expected generated Option::ok_or diagnostic: {diagnostics:#?}"
        );
    }
}
