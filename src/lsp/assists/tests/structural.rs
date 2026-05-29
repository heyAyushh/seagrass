use super::*;

#[test]
fn offers_pda_bump_constraint_from_seed_evidence() {
    let source = r#"
#[derive(Accounts)]
pub struct CreateVault<'info> {
#[account(seeds = [payer.key().as_ref()])]
pub vault: Account<'info, Vault>,
pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/pda-bump.rs").unwrap();
    let range = document.symbols().accounts_structs["CreateVault"].fields[0].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_PDA_BUMP_CONSTRAINT_ID)
        })
        .expect("pda bump assist");
    assert_eq!(action.title, "Add Anchor PDA bump constraint");
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("reason"))
            .and_then(|value| value.as_str()),
        Some(PDA_BUMP_REASON)
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("pda bump edit");
    assert_eq!(edit.new_text, ", bump");
}

#[test]
fn offers_mut_constraint_from_mutable_instruction_usage() {
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
    let uri = Url::parse("file:///tmp/mut-assist.rs").unwrap();
    let range = document.symbols().accounts_structs["Update"].fields[0].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_MUT_CONSTRAINT_ID)
        })
        .expect("mut constraint assist");
    assert_eq!(action.title, "Add Anchor mut constraint");
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("instruction"))
            .and_then(|value| value.as_str()),
        Some("update")
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("mut constraint edit");
    assert_eq!(edit.new_text, "#[account(mut)]\n");
}

#[test]
fn offers_instruction_attribute_from_seed_argument_evidence() {
    let source = r#"
#[program]
pub mod demo {
pub fn create(ctx: Context<Create>, name: String, decimals: u8) -> Result<()> {
    Ok(())
}
}

#[derive(Accounts)]
pub struct Create<'info> {
#[account(init, payer = payer, seeds = [name.as_bytes()], bump, mint::decimals = decimals, mint::authority = payer)]
pub mint: Account<'info, Mint>,
#[account(mut)]
pub payer: Signer<'info>,
pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/instruction-args.rs").unwrap();
    let range = document.symbols().accounts_structs["Create"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_INSTRUCTION_ARGS_ATTRIBUTE_ID)
        })
        .expect("instruction args assist");
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("reason"))
            .and_then(|value| value.as_str()),
        Some(INSTRUCTION_ARGS_REASON)
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("instruction attribute edit");
    assert_eq!(
        edit.new_text,
        "#[instruction(decimals: u8, name: String)]\n"
    );
}

#[test]
fn extends_existing_instruction_attribute_with_missing_argument() {
    let source = r#"
#[program]
pub mod demo {
pub fn create(ctx: Context<Create>, existing: u64, name: String) -> Result<()> {
    Ok(())
}
}

#[derive(Accounts)]
#[instruction(existing: u64)]
pub struct Create<'info> {
#[account(seeds = [name.as_bytes()], bump)]
pub vault: Account<'info, Vault>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/extend-instruction-args.rs").unwrap();
    let range = document.symbols().accounts_structs["Create"].selection_range;
    let actions = code_actions(&document, uri.clone(), range);

    let action = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_INSTRUCTION_ARGS_ATTRIBUTE_ID)
        })
        .expect("instruction args assist");
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("instruction attribute edit");
    assert_eq!(edit.new_text, ", name: String");
}
