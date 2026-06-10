use super::*;

#[test]
fn offers_system_program_type_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("system-program-type")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Program<'info, System>"))
        .expect("expected system_program type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Program<'info, System>");
}
#[test]
fn offers_system_program_type_quickfix_for_realloc() {
    let source = r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8 + State::INIT_SPACE, realloc::payer = user, realloc::zero = false)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic.message.contains("uses `realloc`")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("system-program-type")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Program<'info, System>"))
        .expect("expected realloc system_program type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Program<'info, System>");
}
#[test]
fn offers_missing_system_program_field_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8 + State::INIT_SPACE, realloc::payer = user, realloc::zero = false)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("system_program")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Add `system_program`"))
        .expect("expected missing system_program quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(
        text_edit.new_text,
        "    pub system_program: Program<'info, System>,\n"
    );
}
#[test]
fn offers_missing_token_program_field_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("token_program")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Add `token_program`"))
        .expect("expected missing token_program quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(
        text_edit.new_text,
        "    pub token_program: Program<'info, Token>,\n"
    );
}
#[test]
fn missing_token_program_quickfix_uses_token_interface_for_interface_account() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("token_program")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Add `token_program`"))
        .expect("expected missing token_program quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(
        text_edit.new_text,
        "    pub token_program: Interface<'info, TokenInterface>,\n"
    );
}
#[test]
fn offers_program_field_type_quickfix_for_token_program() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, System>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("program-field-type")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Program<'info, Token>"))
        .expect("expected token_program type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Program<'info, Token>");
}
#[test]
fn token_program_type_quickfix_uses_token_interface_for_interface_account() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);

    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic_code(diagnostic) != Some("anchor-constraint-shape")
                || diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    != Some("program-field-type")
        }),
        "Program<Token> is a valid token program for InterfaceAccount token init"
    );
}
#[test]
fn interface_token_mint_reference_quickfix_replaces_mint_wrapper() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer, token::token_program = token_program)]
    pub token: InterfaceAccount<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("replace-account-type")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("InterfaceAccount<'info, Mint>"))
        .expect("expected interface mint wrapper quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "InterfaceAccount<'info, Mint>");
}
#[test]
fn unresolved_placeholder_generic_quickfix_replaces_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, M>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: InterfaceAccount<'info, TokenAccount>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-syn")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("replace-account-type")
        })
        .expect("expected unresolved placeholder generic quickfix");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("InterfaceAccount<'info, Mint>"))
        .expect("expected placeholder generic account type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "InterfaceAccount<'info, Mint>");
}
#[test]
fn token_constraint_type_quickfix_replaces_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::mint = mint, token::authority = user)]
    pub vault: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic.message.contains("token account constraints")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Account<'info, TokenAccount>"))
        .expect("expected token account type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Account<'info, TokenAccount>");
}
#[test]
fn cpi_program_quickfix_replaces_account_type() {
    let source = r#"
use anchor_lang::solana_program::{instruction::Instruction, program::invoke};

#[derive(Accounts)]
pub struct Cpi<'info> {
    pub source: AccountInfo<'info>,
    pub metadata_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        let ix = Instruction {
            program_id: ctx.accounts.metadata_program.key(),
            accounts: vec![],
            data: vec![],
        };
        invoke(&ix, &[ctx.accounts.source.clone(), ctx.accounts.metadata_program.clone()])
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-security-cpi-program")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("executable-cpi-program")
        })
        .expect("expected cpi program diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("executable"))
        .expect("expected executable cpi program quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text.trim(), "#[account(executable)]");
}
#[test]
fn mint_constraint_type_quickfix_replaces_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = decimals, mint::authority = user)]
    pub mint: AccountInfo<'info>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic.message.contains("mint constraints")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Account<'info, Mint>"))
        .expect("expected mint account type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Account<'info, Mint>");
}
