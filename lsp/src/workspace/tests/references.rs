use super::*;

#[test]
fn references_can_be_filtered_by_anchor_account_data_container() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        ctx.accounts.other.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
    pub other: Account<'info, Other>,
}

#[account]
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct Other {
    pub count: u64,
}
"#
            .to_string(),
        )],
    );

    let counter_refs = index.references_in_container("count", &[SymbolKind::FIELD], "Counter");
    assert_eq!(counter_refs.len(), 2);
    assert!(counter_refs
        .iter()
        .any(|location| location.range.start.line == 4));
    assert!(counter_refs
        .iter()
        .any(|location| location.range.start.line == 18));
    assert!(!counter_refs
        .iter()
        .any(|location| location.range.start.line == 5));
}

#[test]
fn references_with_kinds_use_parsed_anchor_type_ranges() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<State>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub State: AccountInfo<'info>,
    pub account: Account<'info, State>,
}

#[account]
pub struct State {}
"#
            .to_string(),
        )],
    );

    let references = index.references_with_kinds("State", &[SymbolKind::STRUCT]);
    assert_eq!(references.len(), 3);
    assert!(references
        .iter()
        .any(|location| location.range.start.line == 3));
    assert!(references
        .iter()
        .any(|location| location.range.start.line == 9));
    assert!(references
        .iter()
        .any(|location| location.range.start.line == 13));
    assert!(!references
        .iter()
        .any(|location| location.range.start.line == 8));
}
