use {
    crate::{assists, document::ParsedDocument},
    tower_lsp::lsp_types::{CodeAction, Url},
};

#[test]
fn editor_ux_proactive_assist_adds_system_program_without_diagnostic() {
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
    let uri = Url::parse("file:///editor-ux-proactive-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["Create"].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add Anchor system program account",
        "add-system-program-field",
        "pub system_program: Program<'info, System>,",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_token_program_without_diagnostic() {
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
    let uri = Url::parse("file:///editor-ux-token-program-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateToken"].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add Anchor token program account",
        "add-token-program-field",
        "pub token_program: Program<'info, Token>,",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_associated_token_program_without_diagnostic() {
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
    let uri = Url::parse("file:///editor-ux-associated-token-program-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateAta"].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add Anchor associated token program account",
        "add-associated-token-program-field",
        "pub associated_token_program: Program<'info, AssociatedToken>,",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_pda_bump_without_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateVault<'info> {
    #[account(seeds = [payer.key().as_ref()])]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///editor-ux-pda-bump-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateVault"].fields[0].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add Anchor PDA bump constraint",
        "add-pda-bump-constraint",
        ", bump",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_mut_constraint_without_diagnostic() {
    let source = r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.vault.count = ctx.accounts.vault.count.checked_add(1).unwrap();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub vault: Account<'info, Vault>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///editor-ux-mut-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["Update"].fields[0].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add Anchor mut constraint",
        "add-mut-constraint",
        "#[account(mut)]",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_instruction_attribute_without_diagnostic() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [name.as_bytes()], bump)]
    pub vault: Account<'info, Vault>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///editor-ux-instruction-args-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["Create"].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add Anchor instruction arguments attribute",
        "add-instruction-args-attribute",
        "#[instruction(name: String)]",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_canonical_seed_helper_without_diagnostic() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Create<'info> {
    #[account(seeds = [b"vault", payer.key().as_ref(), name.as_bytes()], bump)]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///editor-ux-canonical-seeds-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["Create"].fields[0].selection_range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Add canonical PDA seeds helper",
        "add-canonical-seeds-struct",
        "pub struct VaultSeeds<'a>",
    );
}

#[test]
fn editor_ux_proactive_assist_adds_cpi_safety_without_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct Proxy<'info> {
    pub token_program: AccountInfo<'info>,
    pub metadata_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn proxy(ctx: Context<Proxy>) -> Result<()> {
        let _token = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
        let _metadata = CpiContext::new(ctx.accounts.metadata_program.to_account_info(), ());
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///editor-ux-cpi-safety-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["Proxy"].range;
    let actions = assists::code_actions(&document, uri.clone(), range);

    assert_editor_assist(
        &actions,
        &uri,
        "Use typed CPI program account",
        "use-typed-cpi-program-account",
        "Program<'info, Token>",
    );
    assert_editor_assist(
        &actions,
        &uri,
        "Add executable CPI program constraint",
        "add-cpi-program-executable-constraint",
        "#[account(executable)]",
    );
}

fn assert_editor_assist(
    actions: &[CodeAction],
    uri: &Url,
    title: &str,
    assist_id: &str,
    expected_edit_text: &str,
) {
    let action = actions
        .iter()
        .find(|action| action.title == title)
        .unwrap_or_else(|| panic!("missing {title}; actions: {actions:#?}"));
    assert!(
        action.diagnostics.as_ref().is_none_or(Vec::is_empty),
        "proactive assist should not require an attached diagnostic: {action:#?}"
    );
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("seagrassAssist"))
            .and_then(|value| value.as_str()),
        Some(assist_id)
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(uri))
        .and_then(|edits| edits.first())
        .expect("assist field edit");
    assert!(edit.new_text.contains(expected_edit_text));
}
