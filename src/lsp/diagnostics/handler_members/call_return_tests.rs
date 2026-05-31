use {
    super::collect_with_workspace,
    crate::{
        diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    proptest::prelude::*,
    tower_lsp::lsp_types::{NumberOrString, Url},
};

#[test]
fn reports_unknown_member_from_question_mark_result_helper() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let bundle = load_bundle()?;
        bundle.s;
        Ok(())
    }
}

fn load_bundle() -> Result<PositionBundle> {
    unreachable!()
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`bundle.s` does not resolve"))
        .unwrap_or_else(|| panic!("missing helper-return member diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
        ))
    );
    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}

#[test]
fn does_not_treat_unwrapped_result_as_account_data() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let bundle = load_bundle();
        bundle.position_bundle_mint;
        Ok(())
    }
}

fn load_bundle() -> Result<PositionBundle> {
    unreachable!()
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    );

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`bundle.position_bundle_mint` does not resolve")
        }),
        "Result<T> without `?` should not be flattened into T: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_from_workspace_helper_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let bundle = load_bundle()?;
        bundle.s;
        Ok(())
    }
}
"#,
    );
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/helpers.rs").unwrap(),
                r#"
use anchor_lang::prelude::*;

pub fn load_bundle() -> Result<PositionBundle> {
    unreachable!()
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&document, Some(&index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`bundle.s` does not resolve"))
        .unwrap_or_else(|| {
            panic!("missing workspace helper-return member diagnostic: {diagnostics:#?}")
        });

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}

#[test]
fn reports_unknown_member_from_associated_function_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let metadata = BundleMetadata::new()?;
        metadata.s;
        Ok(())
    }
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}

impl BundleMetadata {
    pub fn new() -> Result<Self> {
        unreachable!()
    }
}
"#,
    );

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`metadata.s` does not resolve"))
        .unwrap_or_else(|| {
            panic!("missing associated-function return diagnostic: {diagnostics:#?}")
        });

    assert!(diagnostic
        .message
        .contains("`BundleMetadata` has no field `s`"));
}

#[test]
fn reports_unknown_member_from_workspace_associated_function_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let metadata = BundleMetadata::new()?;
        metadata.s;
        Ok(())
    }
}
"#,
    );
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/metadata.rs").unwrap(),
            r#"
use anchor_lang::prelude::*;

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}

impl BundleMetadata {
    pub fn new() -> Result<Self> {
        unreachable!()
    }
}
"#
            .to_string(),
        )],
    );

    let diagnostics = collect_with_workspace(&document, Some(&index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`metadata.s` does not resolve"))
        .unwrap_or_else(|| {
            panic!("missing workspace associated-function return diagnostic: {diagnostics:#?}")
        });

    assert!(diagnostic
        .message
        .contains("`BundleMetadata` has no field `s`"));
}

proptest! {
    #[test]
    fn reports_generated_unknown_helper_return_members(
        local in "sg[a-z0-9_]{0,8}",
        helper in "sg[a-z0-9_]{0,8}",
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        missing in "sg[a-z0-9_]{0,8}",
        known in "sg[a-z0-9_]{0,8}",
    ) {
        prop_assume!(local != helper);
        prop_assume!(local != missing);
        prop_assume!(local != known);
        prop_assume!(helper != missing);
        prop_assume!(helper != known);
        prop_assume!(missing != known);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {{
    pub fn close(ctx: Context<Close>) -> Result<()> {{
        let {local} = {helper}()?;
        {local}.{missing};
        Ok(())
    }}
}}

fn {helper}() -> Result<{owner}> {{
    unreachable!()
}}

pub struct {owner} {{
    pub {known}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let diagnostics = collect_with_workspace(&document, None);
        let expected = format!("`{local}.{missing}` does not resolve");

        prop_assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains(&expected)),
            "missing generated helper-return diagnostic {expected}: {diagnostics:#?}"
        );
    }
}
