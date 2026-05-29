use super::*;

#[test]
fn system_program_field_provider_returns_assist_from_evidence() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
#[account(mut)]
pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/system-provider.rs").unwrap();
    let context = AssistContext {
        document: &document,
        uri: &uri,
        range: None,
    };

    let assists = SystemProgramFieldProvider.assists(&context);
    let assist = assists
        .iter()
        .find(|assist| assist.id.0 == ADD_SYSTEM_PROGRAM_FIELD_ID)
        .expect("system program provider assist");

    assert_eq!(assist.title, "Add Anchor system program account");
    assert_eq!(assist.data["field"], SYSTEM_PROGRAM_FIELD_NAME);
}

#[test]
fn program_field_provider_returns_token_program_assist_from_evidence() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateToken<'info> {
#[account(init, payer = payer, token::mint = mint, token::authority = payer)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/program-provider.rs").unwrap();
    let context = AssistContext {
        document: &document,
        uri: &uri,
        range: None,
    };

    let assists = ProgramFieldProvider.assists(&context);
    let assist = assists
        .iter()
        .find(|assist| assist.id.0 == ADD_TOKEN_PROGRAM_FIELD_ID)
        .expect("token program provider assist");

    assert_eq!(assist.title, "Add Anchor token program account");
    assert_eq!(assist.data["field"], TOKEN_PROGRAM_FIELD_NAME);
}

#[test]
fn offers_system_program_field_for_init_context_without_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
#[account(mut)]
pub payer: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let range = document.symbols().accounts_structs["Create"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| action.title == "Add Anchor system program account")
        .expect("system program assist");
    assert!(action.diagnostics.is_none());
    assert_eq!(action.kind, Some(CodeActionKind::REFACTOR));
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("seagrassAssist"))
            .and_then(|value| value.as_str()),
        Some(ADD_SYSTEM_PROGRAM_FIELD_ID)
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("system program text edit");
    assert_eq!(
        edit.new_text,
        "pub system_program: Program<'info, System>,\n"
    );
}

#[test]
fn skips_context_with_existing_system_program() {
    let titles = assist_titles(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor system program account"));
}

#[test]
fn skips_context_without_payer_candidate() {
    let titles = assist_titles(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
pub vault: Account<'info, Vault>,
pub payer: AccountInfo<'info>,
}
"#,
    );

    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor system program account"));
}

#[test]
fn skips_context_without_init_constraint() {
    let titles = assist_titles(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
pub vault: Account<'info, Vault>,
#[account(mut)]
pub payer: Signer<'info>,
}
"#,
    );

    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor system program account"));
}

#[test]
fn offers_token_program_field_for_token_init_context_without_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateToken<'info> {
#[account(init, payer = payer, token::mint = mint, token::authority = payer)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#;

    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/token.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateToken"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| action.title == "Add Anchor token program account")
        .expect("token program assist");
    assert!(action.diagnostics.is_none());
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("seagrassAssist"))
            .and_then(|value| value.as_str()),
        Some(ADD_TOKEN_PROGRAM_FIELD_ID)
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("token program text edit");
    assert_eq!(edit.new_text, "pub token_program: Program<'info, Token>,\n");
}

#[test]
fn offers_associated_token_program_field_for_associated_token_init_context_without_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateAta<'info> {
#[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub token_program: Program<'info, Token>,
pub system_program: Program<'info, System>,
}
"#;

    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/ata.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateAta"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| action.title == "Add Anchor associated token program account")
        .expect("associated token program assist");
    assert!(action.diagnostics.is_none());
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("seagrassAssist"))
            .and_then(|value| value.as_str()),
        Some(ADD_ASSOCIATED_TOKEN_PROGRAM_FIELD_ID)
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("associated token program text edit");
    assert_eq!(
        edit.new_text,
        "pub associated_token_program: Program<'info, AssociatedToken>,\n"
    );
}

#[test]
fn offers_token_program_field_for_associated_token_init_context_without_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateAta<'info> {
#[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#;

    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/ata-token-program.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateAta"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| action.title == "Add Anchor token program account")
        .expect("token program assist");
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("token program text edit");
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("reason"))
            .and_then(|value| value.as_str()),
        Some(TOKEN_PROGRAM_REASON)
    );
    assert_eq!(edit.new_text, "pub token_program: Program<'info, Token>,\n");
}

#[test]
fn offers_interface_token_program_field_for_interface_account_init() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateToken<'info> {
#[account(init, payer = payer, token::mint = mint, token::authority = payer)]
pub token: InterfaceAccount<'info, TokenAccount>,
pub mint: InterfaceAccount<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#;

    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/interface-token.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateToken"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| action.title == "Add Anchor token program account")
        .expect("token interface program assist");
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("token interface program text edit");
    assert_eq!(
        edit.new_text,
        "pub token_program: Interface<'info, TokenInterface>,\n"
    );
}

#[test]
fn offers_named_token_program_field_from_constraint_reference() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateToken<'info> {
#[account(init, payer = payer, token::mint = mint, token::authority = payer, token::token_program = payment_token_program)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#;

    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/named-token-program.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateToken"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| action.title == "Add Anchor token program account")
        .expect("named token program assist");
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("named token program text edit");
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("field"))
            .and_then(|value| value.as_str()),
        Some("payment_token_program")
    );
    assert_eq!(
        edit.new_text,
        "pub payment_token_program: Program<'info, Token>,\n"
    );
}

#[test]
fn skips_companion_program_fields_without_init_constraint() {
    let titles = assist_titles(
        r#"
#[derive(Accounts)]
pub struct CreateAta<'info> {
#[account(associated_token::mint = mint, associated_token::authority = payer)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor token program account"));
    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor associated token program account"));
}

#[test]
fn skips_existing_companion_program_fields() {
    let titles = assist_titles(
        r#"
#[derive(Accounts)]
pub struct CreateAta<'info> {
#[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
pub token: Account<'info, TokenAccount>,
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub token_program: Program<'info, Token>,
pub associated_token_program: Program<'info, AssociatedToken>,
pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor token program account"));
    assert!(!titles
        .iter()
        .any(|title| title == "Add Anchor associated token program account"));
}
