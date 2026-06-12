use {super::*, crate::document::ParsedDocument, crate::workspace::WorkspaceIndex};

#[test]
fn offers_init_placeholder_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init)]
pub state: Account<'info, State>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        Range {
            start: Position {
                line: 3,
                character: 14,
            },
            end: Position {
                line: 3,
                character: 18,
            },
        },
        &[],
    );

    assert_eq!(actions.len(), 1);
    let edit = actions[0].edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("payer = payer"));
    assert!(text_edit.new_text.contains("space = 8 + State::INIT_SPACE"));
}

#[test]
fn init_placeholder_quickfix_ignores_init_substrings() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(initializer = true)]
pub state: Account<'info, State>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        Range {
            start: Position {
                line: 3,
                character: 14,
            },
            end: Position {
                line: 3,
                character: 18,
            },
        },
        &[],
    );

    assert!(actions.is_empty());
}

#[test]
fn init_placeholder_quickfix_ignores_payer_and_space_in_constraint_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, constraint = payer.key() != Pubkey::default(), constraint = has_space())]
pub state: Account<'info, State>,
pub payer: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        Range {
            start: Position {
                line: 3,
                character: 14,
            },
            end: Position {
                line: 3,
                character: 18,
            },
        },
        &[],
    );

    assert_eq!(actions.len(), 1);
    let edit = actions[0].edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("payer = payer"));
    assert!(text_edit.new_text.contains("space = 8 + State::INIT_SPACE"));
}

#[test]
fn init_placeholder_quickfix_accepts_spaced_existing_assignments() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = wallet)]
pub state: Account<'info, State>,
pub wallet: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        Range {
            start: Position {
                line: 3,
                character: 14,
            },
            end: Position {
                line: 3,
                character: 18,
            },
        },
        &[],
    );

    assert_eq!(actions.len(), 1);
    let edit = actions[0].edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(!text_edit.new_text.contains("payer = payer"));
    assert!(text_edit.new_text.contains("space = 8 + State::INIT_SPACE"));
}

#[test]
fn offers_derive_init_space_same_file() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
pub payer: Signer<'info>,
}

#[account]
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct Vault {
pub authority: Pubkey,
}
"#;
    let uri = Url::parse("file:///tmp/create.rs").unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        uri,
        cursor_range(source, "Vault::INIT_SPACE"),
        &[],
    );
    let action = actions
        .iter()
        .find(|action| action.title.contains("derive(InitSpace)"))
        .unwrap();
    let edit = first_edit(action);

    assert_eq!(edit.new_text, ", InitSpace");
}

#[test]
fn offers_derive_init_space_cross_file_on_defining_uri() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
pub payer: Signer<'info>,
}
"#;
    let vault_source = r#"
#[account]
pub struct Vault {
pub authority: Pubkey,
}
"#;
    let accounts_uri = Url::parse("file:///tmp/accounts.rs").unwrap();
    let vault_uri = Url::parse("file:///tmp/vault.rs").unwrap();
    let document = ParsedDocument::parse(accounts_source).unwrap();
    let workspace = WorkspaceIndex::build(
        &[],
        [
            (accounts_uri.clone(), accounts_source.to_string()),
            (vault_uri.clone(), vault_source.to_string()),
        ],
    );
    let actions = code_actions_with_workspace(
        &document,
        accounts_uri,
        cursor_range(accounts_source, "Vault::INIT_SPACE"),
        &[],
        Some(&workspace),
    );
    let action = actions
        .iter()
        .find(|action| action.title.contains("derive(InitSpace)"))
        .unwrap();
    let changes = action.edit.as_ref().unwrap().changes.as_ref().unwrap();

    assert!(changes.contains_key(&vault_uri));
    assert_eq!(changes[&vault_uri][0].new_text, "#[derive(InitSpace)]\n");
}

#[test]
fn derive_init_space_adds_max_len_stubs_for_variable_fields() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
pub payer: Signer<'info>,
}

#[account]
pub struct Vault {
pub name: String,
pub values: Vec<u64>,
}
"#;
    let uri = Url::parse("file:///tmp/create.rs").unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        uri,
        cursor_range(source, "Vault::INIT_SPACE"),
        &[],
    );
    let action = actions
        .iter()
        .find(|action| action.title.contains("max_len stubs"))
        .unwrap();
    let edits = action
        .edit
        .as_ref()
        .unwrap()
        .changes
        .as_ref()
        .unwrap()
        .values()
        .next()
        .unwrap();

    assert_eq!(
        edits
            .iter()
            .filter(|edit| edit.new_text.contains("#[max_len(/* TODO */)]"))
            .count(),
        2
    );
}

#[test]
fn derive_init_space_ignores_already_derived_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
pub payer: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
pub authority: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        cursor_range(source, "Vault::INIT_SPACE"),
        &[],
    );

    assert!(!actions
        .iter()
        .any(|action| action.title.contains("derive(InitSpace)")));
}

#[test]
fn derive_init_space_ignores_external_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + External::INIT_SPACE)]
pub vault: Account<'info, External>,
pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        cursor_range(source, "External::INIT_SPACE"),
        &[],
    );

    assert!(actions.is_empty());
}

fn cursor_range(source: &str, needle: &str) -> Range {
    let position = position_of(source, needle);
    Range {
        start: position,
        end: position,
    }
}

fn position_of(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).unwrap();
    let prefix = &source[..offset];
    Position {
        line: prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
        character: prefix
            .rsplit('\n')
            .next()
            .map(|line| line.chars().count())
            .unwrap_or_default() as u32,
    }
}

fn first_edit(action: &CodeAction) -> &TextEdit {
    action
        .edit
        .as_ref()
        .unwrap()
        .changes
        .as_ref()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .first()
        .unwrap()
}
