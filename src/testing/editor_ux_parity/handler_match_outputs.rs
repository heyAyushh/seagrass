use crate::{completions, diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_resolves_match_arm_pattern_outputs() {
    let completion_source = r#"
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
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "selected.real_"),
    )
    .expect("editor-visible match-arm pattern output completions");
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
        "editor diagnostics should flag unknown match-arm pattern output members: {diagnostics:#?}"
    );
}

#[test]
fn editor_ux_resolves_if_let_pattern_outputs() {
    let completion_source = r#"
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
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "selected.real_"),
    )
    .expect("editor-visible if-let pattern output completions");
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
        "editor diagnostics should flag unknown if-let pattern output members: {diagnostics:#?}"
    );
}

#[test]
fn editor_ux_resolves_match_outputs_with_return_arm() {
    let completion_source = r#"
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
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "selected.real_"),
    )
    .expect("editor-visible match return arm completions");
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
        "editor diagnostics should flag unknown match return arm members: {diagnostics:#?}"
    );
}

#[test]
fn editor_ux_resolves_block_local_outputs() {
    let completion_source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let selected = {
        let metadata = SampleMetadata { real_authority: Pubkey::default() };
        metadata
    };
    selected.real_
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
    .expect("editor-visible block-local output completions");
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
        "editor diagnostics should flag unknown block-local output members: {diagnostics:#?}"
    );
}
