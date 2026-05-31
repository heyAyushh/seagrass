use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_on_option_if_let_pattern_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    if let Some(record) = maybe_record {
        record.real_
    }
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
        .expect("Option if-let pattern member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_after_option_constructor_local_pattern() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let maybe_record = Some(SampleRecord { real_mint: Pubkey::default() });
    if let Some(record) = maybe_record {
        record.real_
    }
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
        .expect("Option constructor local pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Option constructor local should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_direct_option_constructor_pattern() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    if let Some(record) = Some(SampleRecord { real_mint: Pubkey::default() }) {
        record.real_
    }
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
        .expect("direct Option constructor pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "direct Option constructor pattern should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_qualified_option_constructor_pattern() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    if let Some(record) = std::option::Option::Some(SampleRecord { real_mint: Pubkey::default() }) {
        record.real_
    }
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
        .expect("qualified Option constructor pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "qualified Option constructor should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_option_constructor_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let record = Some(SampleRecord { real_mint: Pubkey::default() }).unwrap();
    record.real_
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
        .expect("Option constructor unwrap member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Option constructor unwrap should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_result_constructor_local_match() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let record_result = Ok(SampleRecord { real_mint: Pubkey::default() });
    match record_result {
        Ok(record) => record.real_,
        _ => return Ok(()),
    }
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
        .expect("Result constructor local match member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Result constructor local should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_option_if_else_some_none_pattern() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_record: bool) -> Result<()> {
    let maybe_record = if use_record {
        Some(SampleRecord { real_mint: Pubkey::default() })
    } else {
        None
    };
    if let Some(record) = maybe_record {
        record.real_
    }
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
        .expect("Option Some/None if-expression pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Option Some/None if-expression should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_option_match_some_none_pattern() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_record: bool) -> Result<()> {
    let maybe_record = match use_record {
        true => Some(SampleRecord { real_mint: Pubkey::default() }),
        false => None,
    };
    if let Some(record) = maybe_record {
        record.real_
    }
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
        .expect("Option Some/None match-expression pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Option Some/None match-expression should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_after_result_if_else_ok_err_unwrap() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_record: bool) -> Result<()> {
    let selected = (if use_record {
        Ok(SampleRecord { real_mint: Pubkey::default() })
    } else {
        Err(())
    }).unwrap();
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
        .expect("Result Ok/Err if-expression unwrap member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Result Ok/Err if-expression unwrap should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_on_option_let_else_pattern_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let Some(record) = maybe_record else {
        return Ok(());
    };
    record.real_
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
        .expect("Option let-else pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Option let-else binding should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_on_result_match_ok_pattern_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    match record_result {
        Ok(record) => record.real_,
        _ => return Ok(()),
    }
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
        .expect("Result match Ok pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "Result Ok binding should complete inner members: {items:#?}"
    );
}

#[test]
fn completes_members_on_option_while_let_pop_binding() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, mut records: Vec<SampleRecord>) -> Result<()> {
    while let Some(record) = records.pop() {
        record.real_
    }
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
        .expect("Option while-let pop pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "while-let pop binding should complete inner members: {items:#?}"
    );
}

#[test]
fn does_not_infer_members_for_mismatched_wrapper_pattern() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    if let Ok(record) = maybe_record {
        record.real_
    }
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
    let items = completions(&document, position_after(source, "record.real_")).unwrap_or_default();

    assert!(
        items.iter().all(|item| item.label != "real_mint"),
        "mismatched wrapper pattern should not infer Option inner members: {items:#?}"
    );
}

#[test]
fn does_not_infer_members_for_arbitrary_some_constructor_path() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let maybe_record = custom::Some(SampleRecord { real_mint: Pubkey::default() });
    if let Some(record) = maybe_record {
        record.real_
    }
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
    let items = completions(&document, position_after(source, "record.real_")).unwrap_or_default();

    assert!(
        items.iter().all(|item| item.label != "real_mint"),
        "arbitrary namespaced Some path should not infer Option inner members: {items:#?}"
    );
}

#[test]
fn does_not_infer_members_for_mismatched_some_branch_types() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_record: bool) -> Result<()> {
    let maybe_record = if use_record {
        Some(SampleRecord { real_mint: Pubkey::default() })
    } else {
        Some(OtherRecord { other_mint: Pubkey::default() })
    };
    if let Some(record) = maybe_record {
        record.real_
    }
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

pub struct OtherRecord {
    pub other_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "record.real_")).unwrap_or_default();

    assert!(
        items.iter().all(|item| item.label != "real_mint"),
        "mismatched Some branch types should not infer a single inner type: {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_option_if_let_pattern_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let binding = format!("record_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {{
    if let Some({binding}) = maybe_record {{
        {binding}.real_
    }}
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
        let items = completions(&document, position_after(&source, &format!("{binding}.real_")))
            .expect("generated Option if-let pattern member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated Option if-let member should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_option_constructor_local_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let binding = format!("record_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let maybe_record = Some(SampleRecord {{ {field}: Pubkey::default() }});
    if let Some({binding}) = maybe_record {{
        {binding}.real_
    }}
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
        let items = completions(&document, position_after(&source, &format!("{binding}.real_")))
            .expect("generated Option constructor local member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated Option constructor local member should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_option_some_none_branch_members(
        field_tail in rust_identifier(),
        binding_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let binding = format!("record_{binding_tail}");
        prop_assume!(field != binding);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_record: bool) -> Result<()> {{
    let maybe_record = if use_record {{
        Some(SampleRecord {{ {field}: Pubkey::default() }})
    }} else {{
        None
    }};
    if let Some({binding}) = maybe_record {{
        {binding}.real_
    }}
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
        let items = completions(&document, position_after(&source, &format!("{binding}.real_")))
            .expect("generated Option Some/None branch member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated Option Some/None branch member should complete; items: {items:#?}"
        );
    }
}
