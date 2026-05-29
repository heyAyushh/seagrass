use super::*;

#[test]
fn offers_canonical_seed_helper_from_pda_seed_evidence() {
    let source = r#"
#[program]
pub mod demo {
pub fn create(ctx: Context<CreateVault>, name: String) -> Result<()> {
    Ok(())
}
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct CreateVault<'info> {
#[account(seeds = [b"vault", payer.key().as_ref(), name.as_bytes()], bump)]
pub vault: Account<'info, Vault>,
pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/canonical-seeds.rs").unwrap();
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
                == Some(ADD_CANONICAL_SEEDS_STRUCT_ID)
        })
        .expect("canonical seed helper assist");
    assert_eq!(action.title, "Add canonical PDA seeds helper");
    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("helper"))
            .and_then(|value| value.as_str()),
        Some("VaultSeeds")
    );
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("canonical seed helper edit");
    assert!(edit.new_text.contains("pub struct VaultSeeds<'a>"));
    assert!(edit.new_text.contains("pub payer: &'a Pubkey,"));
    assert!(edit.new_text.contains("pub name: &'a str,"));
    assert!(edit
        .new_text
        .contains("pub fn as_seeds(&self) -> [&[u8]; 3]"));
    assert!(edit
        .new_text
        .contains("[b\"vault\", self.payer.as_ref(), self.name.as_bytes()]"));
}

#[test]
fn offers_safer_cpi_program_assists_from_cpi_usage() {
    let source = r#"
#[derive(Accounts)]
pub struct Proxy<'info> {
pub token_program: AccountInfo<'info>,
pub external_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
use super::*;

pub fn proxy(ctx: Context<Proxy>) -> Result<()> {
    let _typed = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    let _external = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/safer-cpi.rs").unwrap();
    let range = document.symbols().accounts_structs["Proxy"].range;
    let actions = code_actions(&document, uri.clone(), range);

    let typed_action = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(USE_TYPED_CPI_PROGRAM_ACCOUNT_ID)
        })
        .expect("typed cpi program assist");
    assert_eq!(typed_action.title, "Use typed CPI program account");
    assert_eq!(
        typed_action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("expectedType"))
            .and_then(|value| value.as_str()),
        Some(TOKEN_PROGRAM_TYPE)
    );
    let typed_edit = typed_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("typed cpi program edit");
    assert_eq!(typed_edit.new_text, TOKEN_PROGRAM_TYPE);

    let executable_action = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_CPI_PROGRAM_EXECUTABLE_CONSTRAINT_ID)
        })
        .expect("executable cpi program assist");
    assert_eq!(
        executable_action
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("field"))
            .and_then(|value| value.as_str()),
        Some("external_program")
    );
    let executable_edit = executable_action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("executable cpi program edit");
    assert_eq!(executable_edit.new_text.trim(), "#[account(executable)]");
}
