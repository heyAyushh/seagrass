use super::*;

#[test]
fn proposed_assists_returns_structured_agent_payload() {
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
    let proposals = proposed_assists(&document, &uri);
    let proposal = proposals
        .iter()
        .find(|proposal| proposal["id"] == ADD_SYSTEM_PROGRAM_FIELD_ID)
        .expect("system program proposal");

    assert_eq!(proposal["kind"], "refactor");
    assert_eq!(proposal["applicability"], "machineApplicable");
    assert_eq!(proposal["hasEdit"], true);
    assert_eq!(
        proposal["evidence"]["accountsStruct"],
        serde_json::json!("Create")
    );
    assert_eq!(
        proposal["evidence"]["field"],
        serde_json::json!(SYSTEM_PROGRAM_FIELD_NAME)
    );
    assert_eq!(
        proposal["evidence"]["reason"],
        serde_json::json!("init-like account constraints require the System program account")
    );
    assert_eq!(
        proposal["edit"]["changes"][uri.as_str()][0]["newText"],
        serde_json::json!("pub system_program: Program<'info, System>,\n")
    );
}

#[test]
fn proposed_assists_returns_companion_program_payloads() {
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
    let uri = Url::parse("file:///tmp/ata-agent.rs").unwrap();
    let proposals = proposed_assists(&document, &uri);

    let token_program = proposals
        .iter()
        .find(|proposal| proposal["id"] == ADD_TOKEN_PROGRAM_FIELD_ID)
        .expect("token program proposal");
    assert_eq!(token_program["kind"], "refactor");
    assert_eq!(token_program["applicability"], "machineApplicable");
    assert_eq!(
        token_program["evidence"]["field"],
        serde_json::json!(TOKEN_PROGRAM_FIELD_NAME)
    );
    assert_eq!(
        token_program["evidence"]["reason"],
        serde_json::json!(TOKEN_PROGRAM_REASON)
    );
    assert_eq!(
        token_program["edit"]["changes"][uri.as_str()][0]["newText"],
        serde_json::json!("pub token_program: Program<'info, Token>,\n")
    );

    let associated_token_program = proposals
        .iter()
        .find(|proposal| proposal["id"] == ADD_ASSOCIATED_TOKEN_PROGRAM_FIELD_ID)
        .expect("associated token program proposal");
    assert_eq!(associated_token_program["kind"], "refactor");
    assert_eq!(
        associated_token_program["applicability"],
        "machineApplicable"
    );
    assert_eq!(
        associated_token_program["evidence"]["field"],
        serde_json::json!(ASSOCIATED_TOKEN_PROGRAM_FIELD_NAME)
    );
    assert_eq!(
        associated_token_program["evidence"]["reason"],
        serde_json::json!(ASSOCIATED_TOKEN_PROGRAM_REASON)
    );
    assert_eq!(
        associated_token_program["edit"]["changes"][uri.as_str()][0]["newText"],
        serde_json::json!("pub associated_token_program: Program<'info, AssociatedToken>,\n")
    );
}

#[test]
fn proposed_assists_returns_structural_assist_payloads() {
    let source = r#"
#[program]
pub mod demo {
pub fn update(ctx: Context<Update>, name: String) -> Result<()> {
    ctx.accounts.vault.count = ctx.accounts.vault.count.checked_add(1).unwrap();
    Ok(())
}
}

#[derive(Accounts)]
pub struct Update<'info> {
#[account(seeds = [name.as_bytes()])]
pub vault: Account<'info, Vault>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/structural-agent.rs").unwrap();
    let proposals = proposed_assists(&document, &uri);

    let pda_bump = proposals
        .iter()
        .find(|proposal| proposal["id"] == ADD_PDA_BUMP_CONSTRAINT_ID)
        .expect("pda bump proposal");
    assert_eq!(pda_bump["evidence"]["constraint"], BUMP_CONSTRAINT_NAME);
    assert_eq!(
        pda_bump["edit"]["changes"][uri.as_str()][0]["newText"],
        serde_json::json!(", bump")
    );

    let mut_constraint = proposals
        .iter()
        .find(|proposal| proposal["id"] == ADD_MUT_CONSTRAINT_ID)
        .expect("mut constraint proposal");
    assert_eq!(mut_constraint["evidence"]["instruction"], "update");
    assert_eq!(
        mut_constraint["edit"]["changes"][uri.as_str()][0]["newText"],
        serde_json::json!(", mut")
    );

    let instruction_args = proposals
        .iter()
        .find(|proposal| proposal["id"] == ADD_INSTRUCTION_ARGS_ATTRIBUTE_ID)
        .expect("instruction args proposal");
    assert_eq!(
        instruction_args["evidence"]["arguments"][0]["name"],
        serde_json::json!("name")
    );
    assert_eq!(
        instruction_args["edit"]["changes"][uri.as_str()][0]["newText"],
        serde_json::json!("#[instruction(name: String)]\n")
    );
}

#[test]
fn realistic_fixture_offers_canonical_seed_and_cpi_safety_assists() {
    let source = r#"
#[program]
pub mod market {
use super::*;

pub fn settle_position(ctx: Context<SettlePosition>, position_name: String) -> Result<()> {
    let _token_cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    let _metadata_cpi = CpiContext::new(ctx.accounts.metadata_program.to_account_info(), ());
    ctx.accounts.position.lamports = ctx.accounts.position.lamports.checked_add(1).unwrap();
    Ok(())
}
}

#[derive(Accounts)]
#[instruction(position_name: String)]
pub struct SettlePosition<'info> {
#[account(seeds = [b"position", market.key().as_ref(), authority.key().as_ref(), position_name.as_bytes()])]
pub position: Account<'info, Position>,
pub market: AccountInfo<'info>,
pub authority: Signer<'info>,
pub token_program: AccountInfo<'info>,
pub metadata_program: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/realistic-proactive.rs").unwrap();
    let range = document.symbols().accounts_structs["SettlePosition"].range;
    let actions = code_actions(&document, uri.clone(), range);

    let canonical = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_CANONICAL_SEEDS_STRUCT_ID)
        })
        .expect("canonical seeds assist");
    let canonical_edit = canonical
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.get(&uri))
        .and_then(|edits| edits.first())
        .expect("canonical helper text edit");
    assert!(canonical_edit
        .new_text
        .contains("pub struct PositionSeeds<'a>"));
    assert!(canonical_edit.new_text.contains("pub market: &'a Pubkey,"));
    assert!(canonical_edit
        .new_text
        .contains("pub authority: &'a Pubkey,"));
    assert!(canonical_edit
        .new_text
        .contains("pub position_name: &'a str,"));

    let typed_cpi = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(USE_TYPED_CPI_PROGRAM_ACCOUNT_ID)
        })
        .expect("typed CPI program assist");
    assert_eq!(
        typed_cpi
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("field"))
            .and_then(|value| value.as_str()),
        Some(TOKEN_PROGRAM_FIELD_NAME)
    );

    let executable_cpi = actions
        .iter()
        .find(|action| {
            action
                .data
                .as_ref()
                .and_then(|data| data.get("seagrassAssist"))
                .and_then(|value| value.as_str())
                == Some(ADD_CPI_PROGRAM_EXECUTABLE_CONSTRAINT_ID)
        })
        .expect("executable CPI program assist");
    assert_eq!(
        executable_cpi
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|evidence| evidence.get("field"))
            .and_then(|value| value.as_str()),
        Some("metadata_program")
    );
}
