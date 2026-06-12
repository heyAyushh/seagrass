use super::*;
use tower_lsp::lsp_types::SymbolKind;

mod account_attribute_cursor;
mod associated_values;
mod generated_constraints;
mod imports;

mod account_usage;
#[test]
fn parsed_document_captures_anchor_symbols() {
    let source = r#"
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const PUBKEY_CONST: Pubkey = pubkey!("4LVUJzLugULF1PemZ1StknKJEEtJM6rJZaGijpNqCouG");

#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[account]
pub struct State {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let symbols = document.symbols();

    assert!(symbols
        .instructions
        .iter()
        .any(|ix| ix.name == "initialize"));
    assert!(symbols
        .context_references
        .iter()
        .any(|reference| reference.name == "Initialize"));
    assert!(symbols.instructions.iter().any(|instruction| {
        instruction.name == "initialize"
            && instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == "Initialize")
    }));
    assert!(symbols.accounts_structs.contains_key("Initialize"));
    assert!(symbols.account_data_structs.contains_key("State"));
    assert!(symbols
        .constants
        .iter()
        .any(|constant| constant.name == "PUBKEY_CONST"));
    assert_eq!(
        symbols
            .declared_program_id
            .as_ref()
            .map(|declared| declared.value.as_str()),
        Some("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS")
    );
    assert!(document.tree_sitter().is_some());
}

#[test]
fn parsed_document_captures_anchor_helper_functions() {
    let source = r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub offer: Account<'info, Offer>,
}

#[account]
pub struct Offer {
    pub token_b_wanted_amount: u64,
}

pub fn save_offer(context: Context<MakeOffer>, amount: u64) -> Result<()> {
    context.accounts.offer.token_b_wanted_amount = amount;
    Ok(())
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let helper = document
        .symbols()
        .functions
        .iter()
        .find(|function| function.name == "save_offer")
        .unwrap();

    assert_eq!(
        helper.context.as_ref().map(|context| context.name.as_str()),
        Some("MakeOffer")
    );
    assert!(helper
        .arguments
        .iter()
        .any(|argument| argument.name == "amount"));
    assert!(helper
        .account_data_field_usages
        .iter()
        .any(|usage| usage.account == "offer" && usage.field == "token_b_wanted_amount"));
    assert!(document
        .symbols()
        .context_references
        .iter()
        .any(|reference| reference.name == "MakeOffer"));
}

#[test]
fn function_calls_include_method_calls() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.vault.validate()?;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let initialize = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "initialize")
        .unwrap();

    assert!(initialize
        .function_calls
        .iter()
        .any(|call| call.name == "validate"));
}

#[test]
fn function_calls_include_calls_inside_closures() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        [1_u64].iter().for_each(|_| validate_authority(&ctx));
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let initialize = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "initialize")
        .unwrap();

    assert!(initialize
        .function_calls
        .iter()
        .any(|call| call.name == "validate_authority"));
}

#[test]
fn function_calls_dedupe_repeated_callsites() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        validate_authority(&ctx);
        validate_authority(&ctx);
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let initialize = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "initialize")
        .unwrap();
    let ranges = initialize
        .function_calls
        .iter()
        .filter(|call| call.name == "validate_authority")
        .map(|call| call.range)
        .collect::<Vec<_>>();

    assert_eq!(ranges.len(), 2);
    assert_ne!(ranges[0], ranges[1]);
}

#[test]
fn parsed_document_captures_field_type_and_constraint_ranges() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(signer)]
    authority: AccountInfo<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let create = document.symbols().accounts_structs.get("Create").unwrap();
    let authority = create
        .fields
        .iter()
        .find(|field| field.name == "authority")
        .unwrap();

    assert_eq!(authority.type_name.as_deref(), Some("AccountInfo"));
    assert_eq!(authority.type_range.unwrap().start.line, 4);
    assert_eq!(authority.account_constraints.len(), 1);
    assert_eq!(authority.account_constraints[0].range.start.line, 3);
    assert!(authority.account_constraints[0].text.contains("signer"));
}

#[test]
fn parsed_document_unwraps_optional_account_field_types() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Option<Account<'info, State>>,
    pub system_program: Option<Program<'info, System>>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let create = document.symbols().accounts_structs.get("Create").unwrap();
    let state = create
        .fields
        .iter()
        .find(|field| field.name == "state")
        .unwrap();
    let system_program = create
        .fields
        .iter()
        .find(|field| field.name == "system_program")
        .unwrap();

    assert!(state.is_optional);
    assert_eq!(state.type_name.as_deref(), Some("Account"));
    assert!(state.generic_type_names.iter().any(|name| name == "State"));
    assert!(system_program.is_optional);
    assert_eq!(system_program.type_name.as_deref(), Some("Program"));
    assert!(system_program
        .generic_type_names
        .iter()
        .any(|name| name == "System"));
}

#[test]
fn parsed_document_unwraps_boxed_account_field_types() {
    let source = r#"
#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
    pub maybe_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let close = document.symbols().accounts_structs.get("Close").unwrap();
    let position_bundle = close
        .fields
        .iter()
        .find(|field| field.name == "position_bundle")
        .unwrap();
    let maybe_mint = close
        .fields
        .iter()
        .find(|field| field.name == "maybe_mint")
        .unwrap();

    assert_eq!(position_bundle.type_name.as_deref(), Some("Account"));
    assert!(position_bundle
        .generic_type_names
        .iter()
        .any(|name| name == "PositionBundle"));
    assert!(maybe_mint.is_optional);
    assert_eq!(maybe_mint.type_name.as_deref(), Some("InterfaceAccount"));
    assert!(maybe_mint
        .generic_type_names
        .iter()
        .any(|name| name == "Mint"));
}

#[test]
fn parsed_document_captures_instruction_arguments() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, _token_decimals: u8, name: String) -> Result<()> {
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let initialize = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "initialize")
        .unwrap();

    assert_eq!(initialize.arguments.len(), 2);
    assert!(initialize.arguments.iter().any(|argument| {
        argument.name == "_token_decimals" && argument.type_name.as_deref() == Some("u8")
    }));
    assert!(initialize
        .arguments
        .iter()
        .all(|argument| argument.name != "ctx"));
}

#[test]
fn parsed_document_captures_accounts_instruction_attribute_arguments() {
    let source = r#"
#[derive(Accounts)]
#[instruction(decimals: u8, name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let create = document.symbols().accounts_structs.get("Create").unwrap();

    assert_eq!(create.instruction_arguments.len(), 2);
    assert_eq!(create.instruction_arguments[0].name, "decimals");
    assert_eq!(
        create.instruction_arguments[0].type_name.as_deref(),
        Some("u8")
    );
    assert_eq!(create.instruction_arguments[0].range.start.line, 2);
    assert_eq!(create.instruction_arguments[1].name, "name");
    assert_eq!(
        create.instruction_arguments[1].type_name.as_deref(),
        Some("String")
    );
}

#[test]
fn parsed_document_captures_parser_backed_pda_projection() {
    let source = r#"
#[derive(Accounts)]
pub struct UsePda<'info> {
    #[account(seeds = pda_seeds(user.key().as_ref()), bump = state.bump, seeds::program = other_program.key())]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub other_program: Program<'info, Other>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let accounts = document.symbols().accounts_structs.get("UsePda").unwrap();
    let state = accounts
        .fields
        .iter()
        .find(|field| field.name == "state")
        .unwrap();
    let pda = state.pda_constraint.as_ref().expect("pda projection");

    assert_eq!(
        pda.seeds,
        PdaSeeds::Expr("pda_seeds(user.key().as_ref())".to_string())
    );
    assert_eq!(pda.bump, PdaBump::Explicit("state.bump".to_string()));
    assert_eq!(pda.program_seed.as_deref(), Some("other_program.key()"));
}

#[test]
fn document_symbols_include_instruction_arguments() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64) -> Result<()> {
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let symbols = document_symbols(&document);
    let program = symbols.iter().find(|symbol| symbol.name == "demo").unwrap();
    let initialize = program
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|symbol| symbol.name == "initialize")
        .unwrap();

    assert!(initialize
        .children
        .as_ref()
        .unwrap()
        .iter()
        .any(|symbol| symbol.name == "amount" && symbol.detail.as_deref() == Some("u64")));
}

#[test]
fn document_symbols_include_account_constraint_children() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let symbols = document_symbols(&document);
    let create = symbols
        .iter()
        .find(|symbol| symbol.name == "Create")
        .unwrap();
    let state = create
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|symbol| symbol.name == "state")
        .unwrap();
    let constraints = state.children.as_ref().unwrap();

    for key in ["init", "payer", "space"] {
        assert!(
            constraints
                .iter()
                .any(|symbol| symbol.name == key && symbol.kind == SymbolKind::PROPERTY),
            "missing constraint symbol {key}: {constraints:?}"
        );
    }
}

#[test]
fn document_symbols_include_declare_id() {
    let source = r#"declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");"#;
    let document = ParsedDocument::parse(source).unwrap();
    let symbols = document_symbols(&document);

    assert!(symbols.iter().any(|symbol| {
        symbol.name == "declare_id!"
            && symbol.kind == SymbolKind::CONSTANT
            && symbol
                .detail
                .as_deref()
                .is_some_and(|detail| detail.starts_with("Fg6PaFpo"))
    }));
}
