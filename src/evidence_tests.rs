use super::*;

#[test]
fn evidence_graph_captures_accounts_instructions_and_constraints() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, bump: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, seeds = [b"state"], bump = bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    )
    .unwrap();

    let graph = EvidenceGraph::from_document(&document);
    let create = graph
        .account_sets()
        .iter()
        .find(|accounts| accounts.accounts.name == "Create")
        .unwrap();

    assert!(create.has_account("user"));
    assert!(create.has_instruction_argument("bump"));
    assert!(create.has_payer_candidate());
    assert!(create.has_system_program());
    assert!(create.init_like_instruction().is_some());

    let state = create
        .fields()
        .iter()
        .find(|field| field.field.name == "state")
        .unwrap();
    let constraint = &state.constraints()[0];
    assert!(constraint.has_flag_or_key("init"));
    assert!(constraint.has_key("payer"));
    assert_eq!(constraint.instruction_argument_references()[0].name, "bump");
    assert!(constraint.seeds_are_static_only());
}

#[test]
fn evidence_graph_classifies_pda_seed_expressions() {
    let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, name: String, id: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", user.key().as_ref(), name.as_bytes(), id.to_le_bytes().as_ref(), crate::ID.as_ref()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
        )
        .unwrap();

    let graph = EvidenceGraph::from_document(&document);
    let create = graph
        .account_sets()
        .iter()
        .find(|accounts| accounts.accounts.name == "Create")
        .unwrap();
    let state = create
        .fields()
        .iter()
        .find(|field| field.field.name == "state")
        .unwrap();
    let seeds = state.constraints()[0].seed_expressions(create);

    assert_eq!(seeds[0].kind, SeedExpressionKind::StaticBytes);
    assert_eq!(seeds[1].kind, SeedExpressionKind::AccountKey);
    assert_eq!(seeds[2].kind, SeedExpressionKind::InstructionArgument);
    assert_eq!(seeds[3].kind, SeedExpressionKind::InstructionArgument);
    assert_eq!(seeds[4].kind, SeedExpressionKind::Expression);
}

#[test]
fn evidence_summary_uses_parser_backed_pda_projection() {
    let document = ParsedDocument::parse(
            r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = pda_seeds(user.key().as_ref()), bump = state_bump, seeds::program = other_program.key())]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub other_program: Program<'info, Other>,
}
"#,
        )
        .unwrap();

    let summary = summary(&document);
    let constraint = &summary["accounts"][0]["fields"][0]["constraints"][0];

    assert_eq!(constraint["pdaParserBacked"], true);
    assert_eq!(constraint["bump"], "Explicit(\"state_bump\")");
    assert_eq!(constraint["seedsProgram"], "other_program.key()");
    assert_eq!(
        constraint["seeds"][0]["expression"],
        "pda_seeds(user.key().as_ref())"
    );
    assert!(summary["syntaxOverlay"]
        .as_array()
        .unwrap()
        .iter()
        .any(|capture| capture["kind"] == "AccountsStruct" && capture["name"] == "Create"));
    assert!(summary["syntaxOverlay"]
        .as_array()
        .unwrap()
        .iter()
        .any(|capture| capture["kind"] == "AccountField" && capture["name"] == "state"));
    assert!(summary["syntaxOverlay"]
        .as_array()
        .unwrap()
        .iter()
        .any(|capture| capture["kind"] == "AccountConstraintKey"
            && capture["name"] == "seeds::program"));
}

#[test]
fn constraint_keys_do_not_match_namespaced_suffixes() {
    let constraint = AccountConstraint {
        text: "account(mut,realloc=8,realloc::payer=user,realloc::zero=false)".to_string(),
        range: Range::default(),
        pda: None,
    };
    let evidence = ConstraintEvidence::new(&constraint);

    assert!(evidence.has_flag_or_key("mut"));
    assert!(evidence.has_key("realloc"));
    assert!(evidence.has_key("realloc::zero"));
    assert!(!evidence.has_key("zero"));
    assert!(!evidence.has_flag_or_key("zero"));
}

#[test]
fn evidence_graph_links_ctx_account_usages_to_fields() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#,
    )
    .unwrap();

    let graph = EvidenceGraph::from_document(&document);
    let update = graph
        .account_sets()
        .iter()
        .find(|accounts| accounts.accounts.name == "Update")
        .unwrap();
    let counter = update
        .fields()
        .iter()
        .find(|field| field.field.name == "counter")
        .unwrap();

    assert!(counter
        .used_by_instructions()
        .iter()
        .any(|usage| usage.instruction == "increment" && usage.mutable));
}
