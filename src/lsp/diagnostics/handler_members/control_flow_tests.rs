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
fn reports_unknown_member_after_if_expression_infers_type() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = if use_first {
        bundles[0]
    } else {
        bundles.first().unwrap()
    };
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
        "missing if-expression inferred member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_match_expression_infers_type() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle_index: u16, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = match bundle_index {
        0 => bundles[0],
        _ => bundles.first().unwrap(),
    };
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
        "missing match-expression inferred member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_match_arm_pattern_output() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = match maybe_record {
        Some(record) => record.metadata(),
        None => SampleMetadata { real_authority: Pubkey::default() },
    };
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
        "missing match-arm pattern output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_if_let_pattern_output() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = if let Some(record) = maybe_record {
        record.metadata()
    } else {
        SampleMetadata { real_authority: Pubkey::default() }
    };
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
        "missing if-let pattern output diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_match_return_arm() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = match maybe_record {
        Some(record) => record.metadata(),
        None => return Ok(()),
    };
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
        "missing match return arm diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_match_panic_arm() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = match maybe_record {
        Some(record) => record.metadata(),
        None => panic!("missing record"),
    };
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
        "missing match panic arm diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_if_let_return_else() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = if let Some(record) = maybe_record {
        record.metadata()
    } else {
        return Ok(());
    };
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
        "missing if-let return else diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_block_expression_infers_type() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = {
        bundles[0]
    };
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
        "missing block-expression inferred member diagnostic: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unknown_members_after_if_expression_infers_type(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {{
    let selected = if use_first {{
        bundles[0]
    }} else {{
        bundles.first().unwrap()
    }};
    selected.{missing};
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
                .any(|diagnostic| diagnostic.message.contains(&format!("`selected.{missing}` does not resolve"))),
            "expected generated if-expression member diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_members_after_match_pattern_output(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = match maybe_record {{
        Some(record) => record.metadata(),
        None => SampleMetadata {{ {known}: Pubkey::default() }},
    }};
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
            "expected generated match-pattern output diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_members_after_match_return_arm(
        known in generated_ident(),
        missing in generated_ident(),
    ) {
        prop_assume!(known != missing);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = match maybe_record {{
        Some(record) => record.metadata(),
        None => return Ok(()),
    }};
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
            "expected generated match return arm diagnostic: {diagnostics:#?}"
        );
    }
}
