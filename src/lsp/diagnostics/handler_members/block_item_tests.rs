use {super::collect_with_workspace, crate::document::ParsedDocument};

#[test]
fn accepts_block_item_const_member_declared_after_use() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        LOCAL_BUNDLE.position_bundle_mint;

        const LOCAL_BUNDLE: PositionBundle = PositionBundle {
            position_bundle_mint: Pubkey::default(),
            position_bitmap: [0; 32],
        };

        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("position_bundle_mint")),
        "block item const type should resolve before declaration: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_block_item_const_member() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        LOCAL_BUNDLE.fake_member;

        const LOCAL_BUNDLE: PositionBundle = PositionBundle {
            position_bundle_mint: Pubkey::default(),
            position_bitmap: [0; 32],
        };

        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("fake_member")),
        "unknown block item const member should be flagged: {diagnostics:#?}"
    );
}
