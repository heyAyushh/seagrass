use super::*;

#[test]
fn offers_missing_mint_decimals_constraint_quickfix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, token_decimals: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::authority = payer)]
    pub mint: Account<'info, Mint>,
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
                    == Some("mint::decimals")
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
        .find(|action| action.title.contains("mint::decimals = token_decimals"))
        .expect("expected missing mint decimals quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit
        .new_text
        .contains("mint::decimals = token_decimals"));
}
#[test]
fn offers_missing_instruction_argument_quickfix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = _token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
                && diagnostic
                    .data
                    .as_ref()
                    .is_some_and(|data| data["quickfix"] == "add-instruction-argument")
        })
        .expect("expected missing instruction argument diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("_token_decimals: u8"))
        .expect("expected missing instruction argument quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[instruction(_token_decimals: u8)]"));
}
#[test]
fn appends_missing_instruction_argument_to_existing_attribute() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, token_name: String, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(token_name: String)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = _token_decimals, seeds = [token_name.as_bytes()], mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
                && diagnostic.message.contains("_token_decimals")
        })
        .expect("expected missing instruction argument diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("_token_decimals: u8"))
        .expect("expected missing instruction argument quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[instruction(token_name: String, _token_decimals: u8)]"));
}
#[test]
fn replaces_out_of_order_instruction_argument_quickfix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("instruction-attribute-order")
        })
        .expect("expected instruction attribute order diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("decimals: u8"))
        .expect("expected instruction argument replacement quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[instruction(decimals: u8)]"));
}
#[test]
fn replaces_mistyped_instruction_argument_type_quickfix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(decimals: u64)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("instruction-attribute-type")
        })
        .expect("expected instruction attribute type diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("decimals: u8"))
        .expect("expected instruction argument type quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[instruction(decimals: u8)]"));
}
#[test]
fn removes_extra_instruction_argument_quickfix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(decimals: u8, name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("extra-instruction-attribute-argument")
        })
        .expect("expected extra instruction attribute diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Remove `name` from #[instruction]")
        .expect("expected remove extra instruction argument quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[instruction(decimals: u8)]"));
    assert!(!updated.contains("name: String"));
}
#[test]
fn removes_single_extra_instruction_argument_attribute_line() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("extra-instruction-attribute-argument")
        })
        .expect("expected extra instruction attribute diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Remove `name` from #[instruction]")
        .expect("expected remove single instruction argument quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(!updated.contains("#[instruction"));
    assert!(updated.contains("#[derive(Accounts)]\npub struct Create"));
}
#[test]
fn missing_mint_decimals_quickfix_ignores_misleading_non_u8_argument() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, token_decimals_label: String, token_decimals: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::authority = payer)]
    pub mint: Account<'info, Mint>,
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
                    == Some("mint::decimals")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    assert!(actions
        .iter()
        .any(|action| action.title.contains("mint::decimals = 0")));
    assert!(!actions.iter().any(|action| action
        .title
        .contains("mint::decimals = token_decimals_label")));
}
