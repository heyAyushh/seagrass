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
fn reports_unknown_member_on_option_if_let_pattern_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    if let Some(record) = maybe_record {
        record.real_fake;
    }
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
        "missing Option if-let member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_on_option_let_else_pattern_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let Some(record) = maybe_record else {
        return Ok(());
    };
    record.real_fake;
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
        "missing Option let-else member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_on_result_match_ok_pattern_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    match record_result {
        Ok(record) => record.real_fake,
        _ => return Ok(()),
    };
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
        "missing Result Ok match member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_on_option_while_let_pop_binding() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, mut records: Vec<SampleRecord>) -> Result<()> {
    while let Some(record) = records.pop() {
        record.real_fake;
    }
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
        "missing while-let pop member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn does_not_report_member_for_mismatched_wrapper_pattern() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    if let Ok(record) = maybe_record {
        record.real_fake;
    }
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
            .all(|diagnostic| !diagnostic.message.contains("`record.real_fake`")),
        "mismatched wrapper pattern should not create a false member diagnostic: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unknown_members_on_option_if_let_patterns(
        binding in generated_ident(),
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(binding != known && binding != missing && known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    if let Some({binding}) = maybe_record {{
        {binding}.{missing};
    }}
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
                .any(|diagnostic| diagnostic.message.contains(&format!("`{binding}.{missing}` does not resolve"))),
            "expected generated Option if-let member diagnostic: {diagnostics:#?}"
        );
    }
}
