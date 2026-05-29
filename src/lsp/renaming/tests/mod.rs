use {
    super::*,
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
};

#[test]
fn renames_account_data_field_declaration_and_semantic_usages() {
    let source = r#"
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
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let edit = rename_with_workspace(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        Position {
            line: 4,
            character: 31,
        },
        "total",
        None,
        |_| None,
    )
    .unwrap();
    let edits = edit
        .changes
        .unwrap()
        .remove(&Url::parse("file:///tmp/lib.rs").unwrap())
        .unwrap();

    assert_eq!(edits.len(), 2);
    assert!(edits.iter().any(|edit| edit.range.start.line == 4));
    assert!(edits.iter().any(|edit| edit.range.start.line == 18));
    assert!(!edits.iter().any(|edit| edit.range.start.line == 5));
    assert!(edits.iter().all(|edit| edit.new_text == "total"));
}

#[test]
fn renames_nested_account_field_declaration_and_semantic_usage() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let edit = rename_with_workspace(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        position_of(source, "wrapper.inner", "inner"),
        "account",
        None,
        |_| None,
    )
    .unwrap();
    let edits = edit
        .changes
        .unwrap()
        .remove(&Url::parse("file:///tmp/lib.rs").unwrap())
        .unwrap();

    assert_eq!(edits.len(), 2);
    assert!(edits.iter().any(|edit| edit.range.start.line == 4));
    assert!(edits.iter().any(|edit| edit.range.start.line == 16));
    assert!(edits.iter().all(|edit| edit.new_text == "account"));
}

#[test]
fn prepares_anchor_type_rename() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let response = prepare_rename(
        &document,
        Position {
            line: 3,
            character: 35,
        },
        None,
    )
    .unwrap();

    let PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } = response else {
        panic!("expected range with placeholder");
    };
    assert_eq!(placeholder, "Initialize");
}

#[test]
fn renames_account_data_field_across_workspace_index() {
    let program_uri = Url::parse("file:///tmp/program.rs").unwrap();
    let types_uri = Url::parse("file:///tmp/types.rs").unwrap();
    let program_source = r#"
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
"#;
    let types_source = r#"
#[account]
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct Other {
    pub count: u64,
}
"#;
    let document = ParsedDocument::parse(program_source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (program_uri.clone(), program_source.to_string()),
            (types_uri.clone(), types_source.to_string()),
        ],
    );

    let edit = rename_with_workspace(
        &document,
        program_uri.clone(),
        Position {
            line: 4,
            character: 31,
        },
        "total",
        Some(&index),
        |_| None,
    )
    .unwrap();
    let changes = edit.changes.unwrap();
    let program_edits = changes.get(&program_uri).unwrap();
    let types_edits = changes.get(&types_uri).unwrap();

    assert_eq!(program_edits.len(), 1);
    assert_eq!(program_edits[0].range.start.line, 4);
    assert_eq!(types_edits.len(), 1);
    assert_eq!(types_edits[0].range.start.line, 3);
    assert_eq!(program_edits[0].new_text, "total");
    assert_eq!(types_edits[0].new_text, "total");
}

#[test]
fn renames_nested_account_field_across_workspace_index() {
    let program_uri = Url::parse("file:///tmp/program.rs").unwrap();
    let accounts_uri = Url::parse("file:///tmp/accounts.rs").unwrap();
    let program_source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let document = ParsedDocument::parse(program_source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (program_uri.clone(), program_source.to_string()),
            (accounts_uri.clone(), accounts_source.to_string()),
        ],
    );

    let edit = rename_with_workspace(
        &document,
        program_uri.clone(),
        position_of(program_source, "wrapper.inner", "inner"),
        "account",
        Some(&index),
        |_| None,
    )
    .unwrap();
    let changes = edit.changes.unwrap();
    let program_edits = changes.get(&program_uri).unwrap();
    let accounts_edits = changes.get(&accounts_uri).unwrap();

    assert_eq!(program_edits.len(), 1);
    assert_eq!(program_edits[0].range.start.line, 4);
    assert_eq!(accounts_edits.len(), 1);
    assert_eq!(accounts_edits[0].range.start.line, 8);
    assert_eq!(program_edits[0].new_text, "account");
    assert_eq!(accounts_edits[0].new_text, "account");
}

#[test]
fn prepares_nested_account_field_rename_from_workspace_index() {
    let program_uri = Url::parse("file:///tmp/program.rs").unwrap();
    let accounts_uri = Url::parse("file:///tmp/accounts.rs").unwrap();
    let program_source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let document = ParsedDocument::parse(program_source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (program_uri, program_source.to_string()),
            (accounts_uri, accounts_source.to_string()),
        ],
    );
    let response = prepare_rename(
        &document,
        position_of(program_source, "wrapper.inner", "inner"),
        Some(&index),
    )
    .unwrap();

    let PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. } = response else {
        panic!("expected range with placeholder");
    };
    assert_eq!(placeholder, "inner");
}

#[test]
fn renames_instruction_argument_declaration_attribute_and_constraint_references() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, _token_decimals: u8, token_name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(token_decimals: u8, token_name: String)]
pub struct Create<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = token_decimals,
        seeds = [token_name.as_bytes()],
        bump
    )]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let edit = rename_with_workspace(
        &document,
        uri.clone(),
        position_of(source, "_token_decimals", "_token_decimals"),
        "decimals",
        None,
        |_| None,
    )
    .unwrap();
    let edits = edit.changes.unwrap().remove(&uri).unwrap();

    assert_eq!(edits.len(), 3);
    assert!(edits.iter().any(|edit| edit.range.start.line == 3));
    assert!(edits.iter().any(|edit| edit.range.start.line == 9));
    assert!(edits.iter().any(|edit| edit.range.start.line == 14));
    assert!(edits.iter().all(|edit| edit.new_text == "decimals"));

    let edit = rename_with_workspace(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        position_of(source, "seeds = [token_name.as_bytes()]", "token_name"),
        "seed_name",
        None,
        |_| None,
    )
    .unwrap();
    let edits = edit
        .changes
        .unwrap()
        .remove(&Url::parse("file:///tmp/lib.rs").unwrap())
        .unwrap();

    assert_eq!(edits.len(), 3);
    assert!(edits.iter().any(|edit| edit.range.start.line == 3));
    assert!(edits.iter().any(|edit| edit.range.start.line == 9));
    assert!(edits.iter().any(|edit| edit.range.start.line == 15));
    assert!(edits.iter().all(|edit| edit.new_text == "seed_name"));
}

#[test]
fn renames_associated_value_declaration_and_constraint_references() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::SPACE)]
    pub state: Account<'info, State>,
    #[account(init, payer = user, space = State::SPACE)]
    pub second_state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {}

impl State {
    const SPACE: usize = 16;
}
"#;
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let edit = rename_with_workspace(
        &document,
        uri.clone(),
        position_of(source, "State::SPACE", "SPACE"),
        "ACCOUNT_SPACE",
        None,
        |_| None,
    )
    .unwrap();
    let edits = edit.changes.unwrap().remove(&uri).unwrap();

    assert_eq!(edits.len(), 3);
    assert!(edits.iter().any(|edit| edit.range.start.line == 3));
    assert!(edits.iter().any(|edit| edit.range.start.line == 5));
    assert!(edits.iter().any(|edit| edit.range.start.line == 14));
    assert!(edits.iter().all(|edit| edit.new_text == "ACCOUNT_SPACE"));
}

#[test]
fn rejects_generated_init_space_rename() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    #[max_len(32)]
    pub name: String,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    assert!(prepare_rename(
        &document,
        position_of(source, "State::INIT_SPACE", "INIT_SPACE"),
        None,
    )
    .is_none());
}

#[test]
fn renames_instruction_argument_across_workspace_index() {
    let program_uri = Url::parse("file:///tmp/program.rs").unwrap();
    let accounts_uri = Url::parse("file:///tmp/accounts.rs").unwrap();
    let program_source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, decimals: u8) -> Result<()> {
        Ok(())
    }
}
"#;
    let accounts_source = r#"
#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = decimals)]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(program_source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (program_uri.clone(), program_source.to_string()),
            (accounts_uri.clone(), accounts_source.to_string()),
        ],
    );
    let edit = rename_with_workspace(
        &document,
        program_uri.clone(),
        position_of(program_source, "decimals: u8", "decimals"),
        "mint_decimals",
        Some(&index),
        |_| None,
    )
    .unwrap();
    let changes = edit.changes.unwrap();
    let program_edits = changes.get(&program_uri).unwrap();
    let accounts_edits = changes.get(&accounts_uri).unwrap();

    assert_eq!(program_edits.len(), 1);
    assert_eq!(program_edits[0].range.start.line, 3);
    assert_eq!(accounts_edits.len(), 1);
    assert!(accounts_edits.iter().any(|edit| edit.range.start.line == 2));
    assert!(program_edits
        .iter()
        .chain(accounts_edits.iter())
        .all(|edit| edit.new_text == "mint_decimals"));
}

#[test]
fn rejects_invalid_new_names() {
    let source = r#"
#[account]
pub struct Counter {
    pub count: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    assert!(rename_with_workspace(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        Position {
            line: 3,
            character: 9,
        },
        "not-valid",
        None,
        |_| None,
    )
    .is_none());
}

fn position_of(source: &str, containing: &str, word: &str) -> Position {
    let containing_offset = source.find(containing).expect("containing text");
    let word_offset = containing_offset + containing.find(word).expect("word in containing");
    let prefix = &source[..word_offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix
        .rsplit('\n')
        .next()
        .map(|line| line.chars().count())
        .unwrap_or_default() as u32;
    Position { line, character }
}
