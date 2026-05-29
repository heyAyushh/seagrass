use {
    super::*,
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::Url,
};

mod ranking_tests;
mod value_expression_tests;

#[test]
fn completes_inside_account_attribute() {
    let source = "#[account(init, pa)]\npub state: Account<'info, State>,";
    let document = ParsedDocument::parse_or_empty(source);
    let completions = completions(
        &document,
        Position {
            line: 0,
            character: "#[account(init, pa".len() as u32,
        },
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "payer ="));
}

#[test]
fn completes_inside_multiline_account_attribute() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        pa
    )]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "pa")).unwrap();

    assert!(completions.iter().any(|item| item.label == "payer ="));
}

#[test]
fn resolve_adds_markdown_docs_for_anchor_constraint_completion() {
    let item = CompletionItem {
        label: "payer =".to_string(),
        data: Some(serde_json::json!({ "anchorCompletion": "payer =" })),
        ..CompletionItem::default()
    };

    let resolved = resolve(item);
    let documentation = markdown_documentation(&resolved).expect("markdown docs");

    assert!(documentation.contains("`payer`"));
    assert!(documentation.contains("Family:"));
    assert!(documentation.contains("Value:"));
}

#[test]
fn resolve_adds_docs_for_namespaced_anchor_constraint_completion() {
    let item = CompletionItem {
        label: "seeds::program =".to_string(),
        data: Some(serde_json::json!({ "anchorCompletion": "seeds::program =" })),
        ..CompletionItem::default()
    };

    let resolved = resolve(item);
    let documentation = markdown_documentation(&resolved).expect("markdown docs");

    assert!(documentation.contains("`seeds::program`"));
    assert!(documentation.contains("Family:"));
}

#[test]
fn resolve_preserves_existing_completion_documentation() {
    let item = CompletionItem {
        label: "payer =".to_string(),
        documentation: Some(Documentation::String("client docs".to_string())),
        data: Some(serde_json::json!({ "anchorCompletion": "payer =" })),
        ..CompletionItem::default()
    };

    let resolved = resolve(item);

    assert!(matches!(
        resolved.documentation,
        Some(Documentation::String(ref value)) if value == "client docs"
    ));
}

#[test]
fn completes_inside_account_attribute_after_space_trigger() {
    let source = "#[account(init, )]\npub state: Account<'info, State>,";
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        Position {
            line: 0,
            character: "#[account(init, ".len() as u32,
        },
    )
    .expect("expected account constraint completions after space");

    assert!(items.iter().any(|item| item.label == "payer ="));
}

#[test]
fn account_constraint_completion_data_carries_decorated_field() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, pa)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "#[account(init, pa"))
        .expect("expected account constraint key completions");
    let payer = items
        .iter()
        .find(|item| item.label == "payer =")
        .expect("expected payer completion");

    assert_eq!(
        payer
            .data
            .as_ref()
            .and_then(|data| data.get("accountField"))
            .and_then(|value| value.as_str()),
        Some("state")
    );
}

#[test]
fn completion_gate_stays_quiet_in_normal_rust() {
    let source = r#"
fn main() {
    let result = Result::<(), ()>::Ok(());
    result.un
}
"#;

    assert!(!should_offer_completion(
        source,
        position_after(source, "result.un")
    ));
}

#[test]
fn completion_gate_ignores_anchor_text_in_strings_and_comments() {
    let source = r##"
fn main() {
    let text = "#[account(pa";
    let docs = "Context<Ma";
    // #[account(mu
    /// #[account(se
}
"##;

    for cursor in [
        r##""#[account(pa"##,
        r##""Context<Ma"##,
        "// #[account(mu",
        "/// #[account(se",
    ] {
        assert!(
            !should_offer_completion(source, position_after(source, cursor)),
            "completion gate should ignore Anchor-shaped text at {cursor}"
        );
    }
}

#[test]
fn completion_gate_requires_real_anchor_context_type() {
    let source = r#"
pub struct Context<T> {
    value: T,
}

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}

fn helper() {
    let _value = Context<Ma;
}
"#;

    assert!(!should_offer_completion(
        source,
        position_after(source, "Context<Ma")
    ));
}

#[test]
fn completion_gate_requires_anchor_framework_context_hint() {
    let source = r#"
pub struct Context<T> {
    value: T,
}

pub struct Wrapper {
    accounts: AccountsBag,
}

pub struct AccountsBag {
    count: u64,
}

fn helper(ctx: Context<Ma>, wrapper: Wrapper) {
    let _value = ctx.accounts.co;
}
"#;

    for cursor in ["Context<Ma", "ctx.accounts.co"] {
        assert!(
            !should_offer_completion(source, position_after(source, cursor)),
            "non-Anchor Context-shaped code should stay quiet at {cursor}"
        );
    }
}

#[test]
fn completion_gate_requires_real_anchor_accounts_path() {
    let source = r#"
pub struct Wrapper {
    accounts: AccountsBag,
}

pub struct AccountsBag {
    count: u64,
}

impl Wrapper {
    fn helper(&self) {
        let _value = self.accounts.co;
    }
}
"#;

    assert!(!should_offer_completion(
        source,
        position_after(source, "self.accounts.co")
    ));
}

#[test]
fn cursor_context_carries_instruction_args_and_cpi_sites() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64, name: String) -> Result<()> {
        call_external(ctx)?;
        Ok(())
    }
}

pub fn call_external(ctx: Context<Create>) -> Result<()> {
    let _cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    Ok(())
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [na])]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let cursor = CursorContext::classify_document(&document, position_after(source, "seeds = [na"));
    let CursorContextKind::AccountConstraintValue { prefix } = cursor.kind() else {
        panic!("expected account constraint value context");
    };

    assert_eq!(prefix, "na");
    assert_eq!(
        cursor
            .context()
            .accounts_struct
            .as_ref()
            .map(|accounts| accounts.name.as_str()),
        Some("Create")
    );
    assert_eq!(
        cursor
            .context()
            .account_field
            .as_ref()
            .map(|field| field.name.as_str()),
        Some("state")
    );
    assert_eq!(
        cursor
            .context()
            .enclosing_instruction
            .as_ref()
            .map(|instruction| instruction.name.as_str()),
        Some("initialize")
    );
    assert!(
        cursor
            .context()
            .instruction_arguments
            .iter()
            .any(|argument| argument.name == "name"
                && argument.type_name.as_deref() == Some("String"))
    );
    assert!(cursor.context().cpi_sites.iter().any(|site| {
        site.instruction_name == "call_external" && site.account_name == "token_program"
    }));
}

#[test]
fn completion_gate_wakes_for_typed_anchor_contexts() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let account = ctx.accounts.co
    }
}

#[derive(Accounts)]
#[instruction(na)]
pub struct Run<'info> {
    #[account(init, pa)]
    pub rent: Sysvar<'info, Re
}
"#;

    for cursor in [
        "ctx.accounts.co",
        "Context<Ru",
        "#[instruction(na",
        "#[account(init, pa",
        "pub rent",
        "Sysvar<'info, Re",
    ] {
        assert!(
            should_offer_completion(source, position_after(source, cursor)),
            "expected completion gate for {cursor}"
        );
    }
}

#[test]
fn completion_gate_allows_empty_anchor_slot_wakeups_only_in_anchor_contexts() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let account = ctx.accounts.
    }
}

#[derive(Accounts)]
#[instruction()]
pub struct Run<'info> {
    #[account(init, )]
    pub 
}
"#;

    for cursor in [
        "#[instruction(",
        "#[account(init, ",
        "#[account(init, )]\n    pub ",
        "ctx.accounts.",
        "Context<",
    ] {
        assert!(
            should_offer_completion(source, position_after(source, cursor)),
            "expected completion gate for empty Anchor slot at {cursor}"
        );
    }
}

#[test]
fn completes_constraint_keys_inside_broken_account_attributes() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(in
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "#[account(in"))
        .expect("expected account constraint key completions");

    assert!(items.iter().any(|item| item.label == "init"));
}

#[test]
fn completion_signature_tracks_typed_anchor_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, pa)]
    pub payer: Signer<'info>,
}
"#;

    let signature = completion_signature(source, position_after(source, "#[account(init, pa"))
        .expect("expected completion signature");

    assert_eq!(signature.line, 3);
    assert_eq!(
        signature.kind,
        CompletionSignatureKind::AccountConstraintKey
    );
    assert_eq!(signature.prefix, "pa");
    assert_eq!(
        signature.cache_key(),
        "3:accountConstraintKey:pa".to_string()
    );
}

#[test]
fn completion_signature_tracks_space_trigger_anchor_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, )]
    pub payer: Signer<'info>,
}
"#;

    let signature = completion_signature(source, position_after(source, "#[account(init, "))
        .expect("expected completion signature after space");

    assert_eq!(
        signature.kind,
        CompletionSignatureKind::AccountConstraintKey
    );
    assert_eq!(signature.prefix, "");
}

#[test]
fn completes_context_type_from_local_accounts_struct_after_typing() {
    let source = r#"
#[program]
pub mod demo {
    pub fn make_offer(context: Context<Ma>, id: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}

#[derive(Accounts)]
pub struct TakeOffer<'info> {
    pub taker: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "Context<Ma")).unwrap();

    assert_eq!(completions[0].label, "MakeOffer");
    assert!(!completions.iter().any(|item| item.label == "TakeOffer"));
}

#[test]
fn completes_context_type_from_workspace_accounts_struct_after_typing() {
    let source = r#"
#[program]
pub mod demo {
    pub fn make_offer(context: Context<Ma>, id: u64) -> Result<()> { Ok(()) }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/instructions/make_offer.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#
            .to_string(),
        )],
    );
    let completions = completions_with_workspace(
        &document,
        position_after(source, "Context<Ma"),
        Some(&index),
    )
    .unwrap();

    assert_eq!(completions[0].label, "MakeOffer");
}

#[test]
fn completes_context_type_before_user_types_prefix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn make_offer(context: Context<>, id: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    let completions = completions(&document, position_after(source, "Context<")).unwrap();

    assert!(completions.iter().any(|item| item.label == "MakeOffer"));
}

#[test]
fn filters_account_constraint_completions_by_typed_prefix() {
    let source = "#[account(mu)]\npub state: Account<'info, State>,";
    let document = ParsedDocument::parse_or_empty(source);
    let completions = completions(
        &document,
        Position {
            line: 0,
            character: "#[account(mu".len() as u32,
        },
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "mut"));
    assert!(!completions.iter().any(|item| item.label == "payer ="));
    assert!(!completions.iter().any(|item| item.label == "init account"));
}

#[test]
fn filters_namespaced_account_constraint_completions_by_typed_prefix() {
    let source = "#[account(seeds::)]\npub state: Account<'info, State>,";
    let document = ParsedDocument::parse_or_empty(source);
    let completions = completions(
        &document,
        Position {
            line: 0,
            character: "#[account(seeds::".len() as u32,
        },
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "seeds::program ="));
    assert!(!completions.iter().any(|item| item.label == "payer ="));
    assert!(!completions.iter().any(|item| item.label == "init account"));
}

#[test]
fn completes_payer_value_with_signer_accounts() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = u)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    #[account(signer)]
    pub authority: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 30,
        },
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "user"));
    assert!(!completions.iter().any(|item| item.label == "authority"));
    assert!(!completions.iter().any(|item| item.label == "mint"));
}

#[test]
fn completes_space_value_from_current_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 43,
        },
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "8 + State::INIT_SPACE"));
}

#[test]
fn completes_token_authority_value_with_signer_accounts() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::authority = a)]
    pub token: Account<'info, TokenAccount>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 35,
        },
    )
    .unwrap();

    assert_eq!(completions[0].label, "authority");
}

#[test]
fn completes_mint_authority_with_signer_key_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::authority = p)]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 33,
        },
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "payer.key()"));
}

#[test]
fn completes_seeds_program_with_program_key_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds::program = token)]
    pub metadata: AccountInfo<'info>,
    pub token_metadata_program: Program<'info, Metadata>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 36,
        },
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "token_metadata_program.key()"));
}

#[test]
fn completes_token_program_overrides_with_program_account_and_key() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::token_program = token)]
    pub token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 42,
        },
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "token_program"));
    assert!(completions
        .iter()
        .any(|item| item.label == "token_program.key()"));
}

#[test]
fn completes_mint_decimals_with_instruction_argument() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, _token_decimals: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = _)]
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 8,
            character: 32,
        },
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "_token_decimals"));
}

#[test]
fn does_not_complete_mint_decimals_from_misleading_non_u8_argument() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, token_decimals_label: String, token_decimals: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = t)]
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 8,
            character: 33,
        },
    );

    assert!(completions.is_none());
}

fn position_after(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle") + needle.len();
    let prefix = &source[..offset];
    let line = u32::try_from(prefix.chars().filter(|ch| *ch == '\n').count()).unwrap();
    let line_start = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    Position {
        line,
        character: u32::try_from(prefix[line_start..].chars().count()).unwrap(),
    }
}

fn markdown_documentation(item: &CompletionItem) -> Option<&str> {
    match item.documentation.as_ref()? {
        Documentation::MarkupContent(markup) => Some(markup.value.as_str()),
        Documentation::String(text) => Some(text.as_str()),
    }
}
