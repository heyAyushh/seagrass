use super::*;

mod capability_registry;

#[test]
fn reports_program_handler_missing_return_type_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) {
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("should return `Result<()>`"))
        .expect("expected program handler return diagnostic");

    assert!(diagnostic.message.contains("`initialize`"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == "expected a return type"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("anchor-program-handler-return")
    );
}

#[test]
fn reports_program_handler_invalid_generic_return_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<'info> {
        Ok(())
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("type return argument"))
        .expect("expected program handler return argument diagnostic");

    assert!(diagnostic.message.contains("`initialize`"));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("expected generic return type to be a type")));
}

#[test]
fn reports_multiple_fallback_handlers_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn fallback_one(data: &[u8]) -> Result<()> {
        Ok(())
    }

    pub fn fallback_two(data: &[u8]) -> Result<()> {
        Ok(())
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("more than one fallback handler")
        })
        .expect("expected fallback handler diagnostic");

    assert!(diagnostic.message.contains("`fallback_one`"));
    assert!(diagnostic.message.contains("`fallback_two`"));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("More than one fallback function found")));
}

#[test]
fn reports_external_program_module_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo;
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("must contain inline handlers"))
        .expect("expected inline module diagnostic");

    assert!(diagnostic.message.contains("`demo`"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("program content not provided")));
}

#[test]
fn enriches_invalid_sysvar_with_generated_field_hint() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, i16>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Sysvar<'info, Rent>"))
        .expect("expected generated sysvar hint");

    assert!(diagnostic.message.contains("`i16`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|value| value.as_str()),
        Some("replace-invalid-sysvar")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("generatedFrom"))
            .and_then(|value| value.as_str()),
        Some("lang/syn/src/parser/accounts/mod.rs")
    );
}

#[test]
fn keeps_completed_lowercase_sysvar_diagnostic() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, s>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("use `Sysvar<'info, Rent>`")));
}

#[test]
fn reports_unknown_sysvar_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct ReadUnknown<'info> {
    pub something: Sysvar<'info, Bogus>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("not an Anchor sysvar type"))
        .expect("expected generic invalid sysvar diagnostic");

    assert!(diagnostic.message.contains("`Bogus`"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("invalid sysvar provided")));
    assert!(diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .is_some_and(|candidates| candidates.iter().any(|value| value == "Rent")));
}

#[test]
fn suppresses_transient_lowercase_sysvar_prefix_until_type_is_closed() {
    let source = "    pub rent: Sysvar<'info, s";
    let range = Range {
        start: Position {
            line: 0,
            character: "    pub rent: Sysvar<'info, ".len() as u32,
        },
        end: Position {
            line: 0,
            character: "    pub rent: Sysvar<'info, s".len() as u32,
        },
    };

    assert!(is_transient_incomplete_sysvar(source, "s", range));
}

#[test]
fn does_not_suppress_completed_lowercase_sysvar_prefix() {
    let source = "    pub rent: Sysvar<'info, s>,";
    let range = Range {
        start: Position {
            line: 0,
            character: "    pub rent: Sysvar<'info, ".len() as u32,
        },
        end: Position {
            line: 0,
            character: "    pub rent: Sysvar<'info, s".len() as u32,
        },
    };

    assert!(!is_transient_incomplete_sysvar(source, "s", range));
}

#[test]
fn reports_unresolved_placeholder_generic_in_interface_account() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, M>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: InterfaceAccount<'info, TokenAccount>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected unresolved generic account diagnostic");

    assert!(diagnostic
        .message
        .contains("`M` is unresolved in `InterfaceAccount<'info, M>`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|value| value.as_str()),
        Some("replace-account-type")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("InterfaceAccount<'info, Mint>")
    );
}

#[test]
fn accepts_import_alias_for_interface_account_generic() {
    let document = ParsedDocument::parse(
        r#"
use anchor_spl::{
    token_2022::Token2022,
    token_interface::{
        Mint as MintAccount,
    },
};

#[derive(Accounts)]
pub struct ChangeMode<'info> {
    #[account(mut, mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, MintAccount>,
    pub token_program: Program<'info, Token2022>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);

    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        }),
        "import alias for a known Anchor account type must not be reported as unresolved"
    );
}

#[test]
fn reports_lowercase_unresolved_placeholder_generic_with_field_evidence() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, a>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: InterfaceAccount<'info, TokenAccount>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected lowercase unresolved generic account diagnostic");

    assert!(diagnostic
        .message
        .contains("`a` is unresolved in `InterfaceAccount<'info, a>`"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("InterfaceAccount<'info, Mint>")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|value| value.as_str()),
        Some("account-reference")
    );
}

#[test]
fn reports_empty_unresolved_placeholder_generic_with_actionable_message() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, >,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: InterfaceAccount<'info, TokenAccount>,
}
"#,
    );

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected empty unresolved generic account diagnostic");

    assert!(diagnostic.message.contains(
        "Missing account data type for `token_mint_b`; use `InterfaceAccount<'info, Mint>`."
    ));
    assert!(!diagnostic.message.contains("`` is unresolved"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("bracket arguments must be")));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("current"))
            .and_then(|value| value.as_str()),
        Some("InterfaceAccount<'info, >")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("InterfaceAccount<'info, Mint>")
    );
}

#[test]
fn reports_missing_wrapper_generic_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info>,
    pub rent: Sysvar<'info>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("Missing account data type for `state`; use `Account<'info, AccountType>`")));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("Missing account data type for `rent`; use `Sysvar<'info, Rent>`")));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("bracket arguments must be")));
}

#[test]
fn reports_missing_lifetime_before_account_generic() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<State>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("without the `'info` lifetime first")
        })
        .expect("expected missing lifetime diagnostic");

    assert!(diagnostic.message.contains("Account<'info, State>"));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("first bracket argument must be a lifetime")));
}

#[test]
fn reports_qualified_account_wrapper_without_raw_parser_message() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: anchor_lang::prelude::Account<'info, State>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("uses qualified `Account`"))
        .expect("expected qualified wrapper diagnostic");

    assert!(diagnostic.message.contains("Account<'info, State>"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("segmented paths")));
}

#[test]
fn reports_unresolved_mint_generic_from_mint_constraint_evidence() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct CreateMint<'info> {
    #[account(init, payer = payer, mint::decimals = decimals, mint::authority = payer)]
    pub token_mint: InterfaceAccount<'info, a>,
    pub payer: Signer<'info>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected constraint-backed unresolved mint generic diagnostic");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("InterfaceAccount<'info, Mint>")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|value| value.as_str()),
        Some("constraint")
    );
}

#[test]
fn reports_unresolved_token_account_generic_from_token_constraint_evidence() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct CreateVault<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub vault: Account<'info, a>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected constraint-backed unresolved token generic diagnostic");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("Account<'info, TokenAccount>")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|value| value.as_str()),
        Some("constraint")
    );
}

#[test]
fn reports_unresolved_token_account_generic_from_associated_token_evidence() {
    let document = ParsedDocument::parse(
            r#"
#[derive(Accounts)]
pub struct CreateAta<'info> {
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
    pub ata: Account<'info, a>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
        )
        .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected associated-token-backed unresolved generic diagnostic");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("Account<'info, TokenAccount>")
    );
}

#[test]
fn reports_unresolved_token_account_generic_from_custom_token_account_properties() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    pub position: Account<'info, Position>,
    #[account(
        constraint = position_token_account.mint == position.position_mint,
        constraint = position_token_account.amount == 1,
    )]
    pub position_token_account: InterfaceAccount<'info, a>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-generic-account-type")
        })
        .expect("expected custom-constraint-backed unresolved token generic diagnostic");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("expected"))
            .and_then(|value| value.as_str()),
        Some("InterfaceAccount<'info, TokenAccount>")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("evidence"))
            .and_then(|value| value.as_str()),
        Some("token-account-property")
    );
}

#[test]
fn accepts_final_expression_constraint_without_trailing_comma() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
#[instruction(bundle_index: u16)]
pub struct CloseBundledPosition<'info> {
    /// !no-warn
    #[account(
        mut,
        close = receiver,
        seeds = [
            b"bundled_position".as_ref(),
            position_bundle.position_bundle_mint.key().as_ref(),
            bundle_index.to_string().as_bytes()
        ],
        bump,
    )]
    pub bundled_position: Account<'info, Position>,
    #[account(mut)]
    pub position_bundle: Box<Account<'info, PositionBundle>>,
    #[account(
        constraint = position_bundle_token_account.mint == bundled_position.position_mint,
        constraint = position_bundle_token_account.mint == position_bundle.position_bundle_mint,
        constraint = position_bundle_token_account.amount == 1
    )]
    pub position_bundle_token_account: Box<Account<'info, TokenAccount>>,
    pub receiver: UncheckedAccount<'info>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);

    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("unexpected token")
                || diagnostic.message.contains("expected `;`")
        }),
        "final expression constraint should parse without a trailing comma: {diagnostics:#?}"
    );
}

#[test]
fn does_not_report_unresolved_generic_when_type_param_is_declared() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct GenericContext<'info, M> {
    pub token_mint_b: InterfaceAccount<'info, M>,
}
"#,
    )
    .unwrap();

    let diagnostics = collect(&document);

    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str())
            == Some("unresolved-generic-account-type")
    }));
}

#[test]
fn does_not_report_workspace_known_account_generic() {
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/lib.rs").unwrap();
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct UseExternal<'info> {
    pub position: InterfaceAccount<'info, ExternalPosition>,
}
"#,
    )
    .unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
pub struct ExternalPosition {
    pub bump: u8,
}
"#
            .to_string(),
        )],
    );

    let diagnostics = collect_with_workspace(&document, Some(&workspace_index));

    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str())
            == Some("unresolved-generic-account-type")
    }));
}
