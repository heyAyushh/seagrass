use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_after_iterator_next_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    let selected = bundles.iter().next().unwrap();
    selected.real_
}

pub struct SampleRecord {
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
        .expect("iterator next member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_after_iterator_adapter_chain() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    let selected = bundles.iter().filter(|_| true).nth(0).unwrap();
    selected.real_
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("iterator adapter chain member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "iterator adapter chain member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_on_direct_iterator_find_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    bundles.iter().find(|_| true).unwrap().real_
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(
        &document,
        position_after(source, "find(|_| true).unwrap().real_"),
    )
    .expect("direct iterator find member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "direct iterator find member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_inside_iterator_map_closure() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    records.iter().map(|record| record.real_).count();
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "record.real_"))
        .expect("iterator map closure input member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_inside_iterator_filter_closure() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    records.iter().filter(|record| record.real_ == Pubkey::default()).count();
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "record.real_"))
        .expect("iterator filter closure input member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "iterator filter closure input member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_iterator_map_next_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    let selected = records.iter().map(|record| record.metadata()).next().unwrap();
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
        .expect("iterator map output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "iterator map output member should complete: {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_iterator_adapter_chain_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {{
    let selected = bundles.iter().filter(|_| true).next().unwrap();
    selected.real_
}}

pub struct SampleRecord {{
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
            .expect("generated iterator-chain member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated iterator-chain member should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_iterator_map_output_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {{
    let selected = records.iter().map(|record| record.metadata()).next().unwrap();
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
            .expect("generated iterator map output member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated iterator map output member should complete; items: {items:#?}"
        );
    }
}
