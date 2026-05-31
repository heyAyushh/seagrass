use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_inside_option_map_closure() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    maybe_record.map(|record| record.real_);
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
        .expect("Option::map closure input member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_after_option_map_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.map(|record| record.metadata()).unwrap();
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
        .expect("Option::map unwrap output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "Option::map unwrap output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_option_and_then_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.and_then(|record| Some(record.metadata())).unwrap();
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
        .expect("Option::and_then unwrap output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "Option::and_then unwrap output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_result_map_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.map(|record| record.metadata()).unwrap();
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
        .expect("Result::map unwrap output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "Result::map unwrap output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_result_and_then_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.and_then(|record| Ok(record.metadata())).unwrap();
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
        .expect("Result::and_then unwrap output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "Result::and_then unwrap output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_result_ok_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.ok().unwrap();
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
        .expect("Result::ok unwrap member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Result::ok unwrap output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_option_map_or() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.map_or(
        SampleMetadata { real_authority: Pubkey::default() },
        |record| record.metadata(),
    );
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
        .expect("Option::map_or output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "Option::map_or output should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_result_map_or_else() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.map_or_else(
        |_| SampleMetadata { real_authority: Pubkey::default() },
        |record| record.metadata(),
    );
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
        .expect("Result::map_or_else output member completions");

    assert!(
        items.iter().any(|item| item.label == "real_authority"),
        "Result::map_or_else output should complete: {items:#?}"
    );
}

#[test]
fn does_not_complete_members_for_mismatched_map_or_outputs() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.map_or(
        SampleMetadata { real_authority: Pubkey::default() },
        |record| OtherMetadata { other_authority: Pubkey::default() },
    );
    selected.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items =
        completions(&document, position_after(source, "selected.real_")).unwrap_or_default();

    assert!(
        items.iter().all(|item| item.label != "real_authority"),
        "mismatched map_or outputs should not infer a single output type: {items:#?}"
    );
}

#[test]
fn does_not_complete_members_after_invalid_option_ok_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.ok().unwrap();
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
    let items =
        completions(&document, position_after(source, "selected.real_")).unwrap_or_default();

    assert!(
        items.iter().all(|item| item.label != "real_mint"),
        "Option::ok is not a standard wrapper transition; items: {items:#?}"
    );
}

#[test]
fn does_not_complete_members_after_mismatched_result_and_then_wrapper() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let selected = record_result.and_then(|record| Some(record.metadata())).unwrap();
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
    let items =
        completions(&document, position_after(source, "selected.real_")).unwrap_or_default();

    assert!(
        items.iter().all(|item| item.label != "real_authority"),
        "Result::and_then must return Result, not Option; items: {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_option_map_unwrap_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = maybe_record.map(|record| record.metadata()).unwrap();
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
            .expect("generated Option::map unwrap output member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated Option::map unwrap output should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_option_map_or_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    let selected = maybe_record.map_or(
        SampleMetadata {{ {field}: Pubkey::default() }},
        |record| record.metadata(),
    );
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
            .expect("generated Option::map_or output member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated Option::map_or output should complete; items: {items:#?}"
        );
    }
}
