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
fn reports_unknown_member_from_question_mark_method_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        let metadata = bundle.metadata()?;
        metadata.s;
        Ok(())
    }
}

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}
"#,
    );

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`metadata.s` does not resolve"))
        .unwrap_or_else(|| panic!("missing method-return member diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
        ))
    );
    assert!(diagnostic
        .message
        .contains("`BundleMetadata` has no field `s`"));
}

#[test]
fn does_not_treat_unwrapped_result_method_as_account_data() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        let metadata = bundle.metadata();
        metadata.asset_mint;
        Ok(())
    }
}

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}
"#,
    );

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`metadata.asset_mint` does not resolve")
        }),
        "Result<T> method returns should not be flattened into T: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_from_workspace_method_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        let metadata = bundle.metadata()?;
        metadata.s;
        Ok(())
    }
}
"#,
    );
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/bundle.rs").unwrap(),
                r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/metadata.rs").unwrap(),
                r#"
pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&document, Some(&index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`metadata.s` does not resolve"))
        .unwrap_or_else(|| {
            panic!("missing workspace method-return member diagnostic: {diagnostics:#?}")
        });

    assert!(diagnostic
        .message
        .contains("`BundleMetadata` has no field `s`"));
}

proptest! {
    #[test]
    fn reports_generated_unknown_method_return_members(
        local in "sg[a-z0-9_]{0,8}",
        method in "sg[a-z0-9_]{0,8}",
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        returned in "[A-Z][A-Za-z0-9_]{1,10}",
        missing in "sg[a-z0-9_]{0,8}",
        known in "sg[a-z0-9_]{0,8}",
    ) {
        prop_assume!(local != method);
        prop_assume!(local != missing);
        prop_assume!(local != known);
        prop_assume!(method != missing);
        prop_assume!(method != known);
        prop_assume!(missing != known);
        prop_assume!(owner != returned);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {{
    pub fn close(ctx: Context<Close>, {local}: {owner}) -> Result<()> {{
        let returned = {local}.{method}()?;
        returned.{missing};
        Ok(())
    }}
}}

pub struct {owner} {{
    pub value: Pubkey,
}}

impl {owner} {{
    pub fn {method}(&self) -> Result<{returned}> {{
        unreachable!()
    }}
}}

pub struct {returned} {{
    pub {known}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let diagnostics = collect_with_workspace(&document, None);
        let expected = format!("`returned.{missing}` does not resolve");

        prop_assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains(&expected)),
            "missing generated method-return diagnostic {expected}: {diagnostics:#?}"
        );
    }
}
