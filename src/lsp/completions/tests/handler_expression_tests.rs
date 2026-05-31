use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_after_if_expression_infers_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = if use_first {
        bundles[0]
    } else {
        bundles.first().unwrap()
    };
    selected.real_
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("if-expression inferred member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_after_match_expression_infers_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle_index: u16, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = match bundle_index {
        0 => bundles[0],
        _ => bundles.first().unwrap(),
    };
    selected.real_
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("match-expression inferred member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "match-expression inferred member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_match_arm_pattern_output() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = match maybe_record {
        Some(record) => record.metadata(),
        None => SampleMetadata { real_authority: Pubkey::default() },
    };
    selected.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("match-arm pattern output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "match-arm pattern output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_if_let_pattern_output() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = if let Some(record) = maybe_record {
        record.metadata()
    } else {
        SampleMetadata { real_authority: Pubkey::default() }
    };
    selected.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("if-let pattern output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "if-let pattern output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_match_return_arm() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = match maybe_record {
        Some(record) => record.metadata(),
        None => return Ok(()),
    };
    selected.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("match expression with return arm member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "match return arm should not erase surviving branch type: {items:#?}"
    );
}

#[test]
fn completes_members_after_match_panic_arm() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = match maybe_record {
        Some(record) => record.metadata(),
        None => panic!("missing record"),
    };
    selected.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("match expression with panic arm member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "match panic arm should not erase surviving branch type: {items:#?}"
    );
}

#[test]
fn completes_members_after_if_let_return_else() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = if let Some(record) = maybe_record {
        record.metadata()
    } else {
        return Ok(());
    };
    selected.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("if-let expression with return else member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "if-let return else should not erase surviving branch type: {items:#?}"
    );
}

#[test]
fn completes_members_after_block_expression_infers_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = {
        bundles[0]
    };
    selected.real_
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("block-expression inferred member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "block-expression inferred member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_on_direct_if_expression() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {
    (if use_first { bundles[0] } else { bundles.first().unwrap() }).real_
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, ").real_"))
        .expect("direct if-expression member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "direct if-expression member should complete: {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_if_expression_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {{
    let selected = if use_first {{
        bundles[0]
    }} else {{
        bundles.first().unwrap()
    }};
    selected.real_
}}

pub struct PositionBundle {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "selected.real_"))
            .expect("generated if-expression inferred member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated if-expression inferred member should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_match_pattern_output_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = match maybe_record {{
        Some(record) => record.metadata(),
        None => SampleMetadata {{ {field}: Pubkey::default() }},
    }};
    selected.real_
}}

pub struct SampleRecord {{
    pub source: Pubkey,
}}

impl SampleRecord {{
    pub fn metadata(&self) -> SampleMetadata {{
        SampleMetadata {{ {field}: Pubkey::default() }}
    }}
}}

pub struct SampleMetadata {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "selected.real_"))
            .expect("generated match-pattern output member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated match-pattern output should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_match_return_arm_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = match maybe_record {{
        Some(record) => record.metadata(),
        None => return Ok(()),
    }};
    selected.real_
}}

pub struct SampleRecord {{
    pub source: Pubkey,
}}

impl SampleRecord {{
    pub fn metadata(&self) -> SampleMetadata {{
        SampleMetadata {{ {field}: Pubkey::default() }}
    }}
}}

pub struct SampleMetadata {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "selected.real_"))
            .expect("generated match return arm member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated match return arm should complete; items: {items:#?}"
        );
    }
}
