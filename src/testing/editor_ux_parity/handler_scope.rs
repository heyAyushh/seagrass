use crate::{completions, diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_flags_unresolved_anchor_handler_call_identifier() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        verify_bundel(bundle_index);
        Ok(())
    }
}

fn verify_bundle(bundle_index: u16) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub authority: Signer<'info>,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`verify_bundel` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing unresolved handler call diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unresolved-handler-identifier")
    );
    assert!(diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .is_some_and(|candidates| candidates
            .iter()
            .any(|candidate| candidate.as_str() == Some("verify_bundle"))));
}

#[test]
fn editor_ux_resolves_if_let_pattern_values_in_anchor_handlers() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let maybe_receiver = Some(ctx.accounts.receiver.key());
    if let Some(position_bundle) = maybe_receiver {
        let selected = pos
    }
    Ok(())
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`position_bundle` does not resolve")
        }),
        "if-let pattern value should stay resolved in editor diagnostics: {diagnostics:#?}"
    );

    let items = completions::completions(
        &document,
        super::position_after(source, "let selected = pos"),
    )
    .expect("editor-visible if-let pattern completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("position_bundle")
    );
}

#[test]
fn editor_ux_resolves_typed_if_let_pattern_members() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    if let Some(bundle) = ctx.accounts.optional_bundle.as_ref() {
        bundle.asset_
    }
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`bundle` does not resolve")),
        "typed if-let binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "bundle.asset_"))
        .expect("editor-visible typed if-let member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("asset_mint")
    );
}

#[test]
fn editor_ux_resolves_typed_let_else_pattern_members() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub optional_bundle: Option<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let Some(bundle) = ctx.accounts.optional_bundle.as_ref() else {
        return Ok(());
    };
    bundle.asset_
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`bundle` does not resolve")),
        "typed let-else binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "bundle.asset_"))
        .expect("editor-visible typed let-else member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("asset_mint")
    );
}

#[test]
fn editor_ux_resolves_typed_struct_pattern_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let PositionBundle { inner, .. } = bundle;
    inner.real_
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`inner` does not resolve")),
        "typed struct pattern binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "inner.real_"))
        .expect("editor-visible struct pattern member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_typed_function_pattern_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    PositionBundle { inner, .. }: PositionBundle,
) -> Result<()> {
    inner.real_
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`inner` does not resolve")),
        "typed function pattern binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "inner.real_"))
        .expect("editor-visible function pattern member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_typed_closure_pattern_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let check = |PositionBundle { inner, .. }: PositionBundle| {
        inner.real_
    };
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`inner` does not resolve")),
        "typed closure pattern binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "inner.real_"))
        .expect("editor-visible closure pattern member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_typed_for_loop_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<SampleRecord>) -> Result<()> {
    for bundle in bundles.iter() {
        bundle.real_
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
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`bundle` does not resolve")),
        "typed for-loop binding should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "bundle.real_"))
        .expect("editor-visible for-loop member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_indexed_iterable_alias_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let bundle = bundles[0];
    bundle.real_
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
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`bundle` does not resolve")),
        "indexed iterable alias should stay resolved: {diagnostics:#?}"
    );

    let items = completions::completions(&document, super::position_after(source, "bundle.real_"))
        .expect("editor-visible indexed iterable alias member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_indexed_iterable_expression_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    bundles[0].real_
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
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`bundles` does not resolve")),
        "indexed iterable expression receiver should stay resolved: {diagnostics:#?}"
    );

    let items =
        completions::completions(&document, super::position_after(source, "bundles[0].real_"))
            .expect("editor-visible indexed iterable expression member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_assignment_inferred_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let mut selected;
    selected = bundles[0];
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
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`selected` does not resolve")),
        "assignment-inferred binding should stay resolved: {diagnostics:#?}"
    );

    let items =
        completions::completions(&document, super::position_after(source, "selected.real_"))
            .expect("editor-visible assignment-inferred member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_if_expression_inferred_members() {
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
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`selected` does not resolve")),
        "if-expression inferred binding should stay resolved: {diagnostics:#?}"
    );

    let items =
        completions::completions(&document, super::position_after(source, "selected.real_"))
            .expect("editor-visible if-expression inferred member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_iterator_chain_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = bundles.iter().filter(|_| true).next().unwrap();
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
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`selected` does not resolve")),
        "iterator-chain binding should stay resolved: {diagnostics:#?}"
    );

    let items =
        completions::completions(&document, super::position_after(source, "selected.real_"))
            .expect("editor-visible iterator-chain member completions");
    assert_eq!(
        items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_iterator_map_closure_and_output_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, records: Vec<SampleRecord>) -> Result<()> {
    let selected = records.iter().map(|record| record.metadata()).next().unwrap();
    records.iter().map(|record| record.real_).count();
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
    let closure_items =
        completions::completions(&document, super::position_after(source, "record.real_"))
            .expect("editor-visible iterator map closure member completions");
    assert_eq!(
        closure_items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );

    let output_items =
        completions::completions(&document, super::position_after(source, "selected.real_"))
            .expect("editor-visible iterator map output member completions");
    assert_eq!(
        output_items.first().map(|item| item.label.as_str()),
        Some("real_authority")
    );
}

#[test]
fn editor_ux_resolves_option_result_combinator_members() {
    let closure_source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
    maybe_record.map(|record| record.real_);
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
    let closure_document = ParsedDocument::parse_or_empty(closure_source);
    let closure_items = completions::completions(
        &closure_document,
        super::position_after(closure_source, "record.real_"),
    )
    .expect("editor-visible Option::map closure member completions");
    assert_eq!(
        closure_items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );

    let mapped_source = r#"
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
    let mapped_document = ParsedDocument::parse_or_empty(mapped_source);
    let mapped_items = completions::completions(
        &mapped_document,
        super::position_after(mapped_source, "selected.real_"),
    )
    .expect("editor-visible Option::and_then output member completions");
    assert_eq!(
        mapped_items.first().map(|item| item.label.as_str()),
        Some("real_authority")
    );

    let recovered_source = r#"
use anchor_lang::prelude::*;

pub fn handler(
    ctx: Context<Run>,
    record_result: std::result::Result<SampleRecord, ()>,
) -> Result<()> {
    let recovered = record_result.ok().unwrap();
    recovered.real_
}

pub struct SampleRecord {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let recovered_document = ParsedDocument::parse_or_empty(recovered_source);
    let recovered_items = completions::completions(
        &recovered_document,
        super::position_after(recovered_source, "recovered.real_"),
    )
    .expect("editor-visible Result::ok output member completions");
    assert_eq!(
        recovered_items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}

#[test]
fn editor_ux_resolves_option_result_pattern_members() {
    let option_source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, maybe_record: Option<SampleRecord>) -> Result<()> {
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
    let option_document = ParsedDocument::parse_or_empty(option_source);
    let option_items = completions::completions(
        &option_document,
        super::position_after(option_source, "record.real_"),
    )
    .expect("editor-visible Option pattern member completions");
    assert_eq!(
        option_items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );

    let result_source = r#"
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
    let result_document = ParsedDocument::parse_or_empty(result_source);
    let result_items = completions::completions(
        &result_document,
        super::position_after(result_source, "record.real_"),
    )
    .expect("editor-visible Result Ok pattern member completions");
    assert_eq!(
        result_items.first().map(|item| item.label.as_str()),
        Some("real_mint")
    );
}
