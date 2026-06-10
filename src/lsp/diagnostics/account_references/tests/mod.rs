use {
    super::*,
    crate::constraint_catalog,
    crate::diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE,
    tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString},
};

#[test]
fn reports_missing_payer_account_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
}
"#,
    );

    let diagnostic = missing_reference_diagnostic(&diagnostics);
    // AnchorMissingAccountReference is WholeProgram provability, so it defaults to WARNING.
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(diagnostic.range.start.line, 3);
    assert_eq!(
        diagnostic.range.end.character - diagnostic.range.start.character,
        "user".len() as u32
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("constraint"))
            .and_then(|constraint| constraint.as_str()),
        Some("payer")
    );
    let related = diagnostic
        .related_information
        .as_ref()
        .expect("expected related information for missing reference");
    assert!(related
        .iter()
        .any(|info| info.message.contains("Accounts struct being checked")));
}

#[test]
fn accepts_declared_payer_account_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| !has_missing_reference_code(diagnostic)));
}

#[test]
fn missing_reference_related_information_lists_candidate_fields() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = usr, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub authority: Signer<'info>,
}
"#,
    );

    let diagnostic = missing_reference_diagnostic(&diagnostics);
    let messages = diagnostic
        .related_information
        .as_ref()
        .expect("expected related information")
        .iter()
        .map(|info| info.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("candidate account field `user`")),
        "expected candidate field related information; got {messages:?}"
    );
}

#[test]
fn missing_reference_data_includes_ranked_candidates() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = usr, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
"#,
    );

    let diagnostic = missing_reference_diagnostic(&diagnostics);
    let candidates = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|candidates| candidates.as_array())
        .expect("expected ranked candidates in diagnostic data")
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();

    assert_eq!(candidates.first().copied(), Some("user"));
    assert!(candidates.contains(&"authority"));
}

#[test]
fn reports_missing_token_authority_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::authority = authority)]
    pub token: Account<'info, TokenAccount>,
}
"#,
    );

    let diagnostic = missing_reference_diagnostic(&diagnostics);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("constraint"))
            .and_then(|constraint| constraint.as_str()),
        Some("token::authority")
    );
}

#[test]
fn reports_missing_references_for_every_generated_account_reference_constraint() {
    let constraints = constraint_catalog::account_reference_specs()
        .map(|(key, _)| format!("{key} = missing_account"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account({constraints})]
    pub state: Account<'info, State>,
}}
"#,
    );
    let diagnostics = diagnostics_for(&source);
    let reported = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| {
                Some((
                    data.get("constraint")?.as_str()?.to_string(),
                    data.get("account")?.as_str()?.to_string(),
                ))
            })
        })
        .collect::<std::collections::BTreeSet<_>>();

    for (key, _) in constraint_catalog::account_reference_specs() {
        assert!(
            reported.contains(&(key.to_string(), "missing_account".to_string())),
            "missing diagnostic coverage for generated account reference constraint `{key}`"
        );
    }
}

#[test]
fn reports_missing_instruction_arguments_for_every_generated_instruction_argument_constraint() {
    let constraints = constraint_catalog::instruction_argument_keys()
        .map(|key| format!("{key} = missing_arg"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"
#[program]
pub mod demo {{
    pub fn initialize(ctx: Context<Create>) -> Result<()> {{
        Ok(())
    }}
}}

#[derive(Accounts)]
pub struct Create<'info> {{
    #[account({constraints})]
    pub state: Account<'info, State>,
}}
"#,
    );
    let diagnostics = diagnostics_for(&source);
    let reported = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| {
                Some((
                    data.get("constraint")?.as_str()?.to_string(),
                    data.get("argument")?.as_str()?.to_string(),
                ))
            })
        })
        .collect::<std::collections::BTreeSet<_>>();

    for key in constraint_catalog::instruction_argument_keys() {
        assert!(
            reported.contains(&(key.to_string(), "missing_arg".to_string())),
            "missing diagnostic coverage for generated instruction argument constraint `{key}`"
        );
    }
}

#[test]
fn reports_missing_seeds_program_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"metadata"], bump, seeds::program = token_metadata_program.key())]
    pub metadata: AccountInfo<'info>,
}
"#,
    );

    let diagnostic = missing_reference_diagnostic(&diagnostics);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("constraint"))
            .and_then(|constraint| constraint.as_str()),
        Some("seeds::program")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|account| account.as_str()),
        Some("token_metadata_program")
    );
}

#[test]
fn accepts_declared_seeds_program_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"metadata"], bump, seeds::program = token_metadata_program.key())]
    pub metadata: AccountInfo<'info>,
    pub token_metadata_program: Program<'info, Metadata>,
}
"#,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| !has_missing_reference_code(diagnostic)));
}

#[test]
fn accepts_non_account_seeds_program_expressions() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"metadata"], bump, seeds::program = crate::ID)]
    pub metadata: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CreateWithCall<'info> {
    #[account(seeds = [b"metadata"], bump, seeds::program = System::id())]
    pub metadata: AccountInfo<'info>,
}
"#,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| !has_missing_reference_code(diagnostic)));
}

#[test]
fn accepts_const_pubkey_seeds_program_expression() {
    let diagnostics = diagnostics_for(
        r#"
const PUBKEY_CONST: Pubkey = pubkey!("4LVUJzLugULF1PemZ1StknKJEEtJM6rJZaGijpNqCouG");

#[derive(Accounts)]
pub struct PubkeyConst<'info> {
    #[account(seeds = [], seeds::program = PUBKEY_CONST, bump)]
    pub acc: UncheckedAccount<'info>,
}
"#,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| !has_missing_reference_code(diagnostic)));
}

#[test]
fn accepts_instruction_argument_seeds_program_expression() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, metadata_program_id: Pubkey) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"metadata"], bump, seeds::program = metadata_program_id)]
    pub metadata: AccountInfo<'info>,
}
"#,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| !has_missing_reference_code(diagnostic)));
}

#[test]
fn accepts_non_argument_mint_decimals_expressions() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = get_lp_mint_decimal(token_a_mint.decimals, token_b_mint.decimals), mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
        )
    }));
}

#[test]
fn reports_missing_mint_decimals_instruction_argument() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = _token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
        ) && diagnostic.message.contains("_token_decimals")
    }));
}

#[test]
fn missing_instruction_argument_related_information_points_to_handler_argument() {
    let diagnostics = diagnostics_for(
        r#"
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
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
            ) && diagnostic.message.contains("_token_decimals")
        })
        .expect("expected missing instruction argument diagnostic");
    let related = diagnostic
        .related_information
        .as_ref()
        .expect("expected related information for missing instruction argument");
    assert!(related.iter().any(|info| info
        .message
        .contains("handler `initialize` declares `_token_decimals`")));
}

#[test]
fn accepts_declared_mint_decimals_instruction_argument() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(_token_decimals: u8)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = _token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
        )
    }));
}

#[test]
fn accepts_instruction_attribute_alias_for_unused_handler_argument() {
    let diagnostics = diagnostics_for(
        r#"
#[program]
pub mod demo {
    pub fn create_token(ctx: Context<Create>, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(token_decimals: u8)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
        )
    }));
}

#[test]
fn accepts_workspace_instruction_argument_for_split_accounts_file() {
    let accounts_uri =
        tower_lsp::lsp_types::Url::parse("file:///tmp/instructions/create.rs").unwrap();
    let program_uri = tower_lsp::lsp_types::Url::parse("file:///tmp/lib.rs").unwrap();
    let accounts_source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(token_decimals: u8)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let program_source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod split_demo {
    use super::*;

    pub fn create(ctx: Context<Create>, token_decimals: u8) -> Result<()> {
        Ok(())
    }
}
"#;
    let accounts_document = ParsedDocument::parse(accounts_source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (accounts_uri, accounts_source.to_string()),
            (program_uri, program_source.to_string()),
        ],
    );
    let diagnostics = collect_with_workspace(&accounts_document, Some(&workspace_index));

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
        )
    }));
}

#[test]
fn reports_missing_workspace_instruction_argument_for_split_accounts_file() {
    let accounts_uri =
        tower_lsp::lsp_types::Url::parse("file:///tmp/instructions/create.rs").unwrap();
    let program_uri = tower_lsp::lsp_types::Url::parse("file:///tmp/lib.rs").unwrap();
    let accounts_source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let program_source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod split_demo {
    use super::*;

    pub fn create(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}
"#;
    let accounts_document = ParsedDocument::parse(accounts_source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (accounts_uri, accounts_source.to_string()),
            (program_uri, program_source.to_string()),
        ],
    );
    let diagnostics = collect_with_workspace(&accounts_document, Some(&workspace_index));

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-missing-instruction-argument"
        ) && diagnostic.message.contains("token_decimals")
    }));
}

#[test]
fn reports_missing_has_one_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
}
"#,
    );

    assert!(diagnostics.iter().any(has_missing_reference_code));
}

#[test]
fn reports_missing_close_recipient_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct CloseState<'info> {
    #[account(mut, close = receiver)]
    pub state: Account<'info, State>,
}
"#,
    );

    let diagnostic = missing_reference_diagnostic(&diagnostics);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("constraint"))
            .and_then(|constraint| constraint.as_str()),
        Some("close")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|account| account.as_str()),
        Some("receiver")
    );
}

#[test]
fn accepts_declared_close_recipient_reference() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct CloseState<'info> {
    #[account(mut, close = receiver)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub receiver: SystemAccount<'info>,
}
"#,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| !has_missing_reference_code(diagnostic)));
}

fn diagnostics_for(source: &str) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse(source).unwrap();
    collect(&document)
}

fn missing_reference_diagnostic(diagnostics: &[Diagnostic]) -> &Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| has_missing_reference_code(diagnostic))
        .unwrap_or_else(|| panic!("expected missing account reference; got {diagnostics:?}"))
}

fn has_missing_reference_code(diagnostic: &Diagnostic) -> bool {
    matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
    )
}
