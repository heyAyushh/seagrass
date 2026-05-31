use crate::{completions, diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_resolves_constructor_wrapped_pattern_members() {
    let completion_source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let maybe_record = Some(SampleRecord { real_mint: Pubkey::default() });
    if let Some(record) = maybe_record {
        record.real_
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
"#;
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "record.real_"),
    )
    .expect("editor-visible constructor wrapper member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );

    let diagnostic_source = completion_source.replace("record.real_", "record.real_fake;");
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&diagnostic_source));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`record.real_fake` does not resolve")),
        "editor diagnostics should flag unknown constructor-wrapped members: {diagnostics:#?}"
    );
}

#[test]
fn editor_ux_resolves_wrapper_control_flow_members() {
    let completion_source = r#"
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
    Ok(())
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "record.real_"),
    )
    .expect("editor-visible wrapper control-flow member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );

    let diagnostic_source = completion_source.replace("record.real_", "record.real_fake;");
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&diagnostic_source));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`record.real_fake` does not resolve")),
        "editor diagnostics should flag unknown wrapper control-flow members: {diagnostics:#?}"
    );
}
