use crate::{completions, diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_resolves_wrapper_map_or_outputs() {
    let completion_source = r#"
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
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "selected.real_"),
    )
    .expect("editor-visible wrapper map_or output completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_authority")
    );

    let diagnostic_source = completion_source.replace("selected.real_", "selected.real_fake;");
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&diagnostic_source));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "editor diagnostics should flag unknown wrapper map_or output members: {diagnostics:#?}"
    );
}

#[test]
fn editor_ux_resolves_wrapper_fallback_conversions() {
    let completion_source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    let selected = maybe_record.ok_or(()).unwrap();
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
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "selected.real_"),
    )
    .expect("editor-visible wrapper fallback completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );

    let diagnostic_source = completion_source.replace("selected.real_", "selected.real_fake;");
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&diagnostic_source));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "editor diagnostics should flag unknown wrapper fallback members: {diagnostics:#?}"
    );
}
