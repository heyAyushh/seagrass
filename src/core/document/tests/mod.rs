use super::*;
use {crate::constraint_catalog, tower_lsp::lsp_types::SymbolKind};

mod account_attribute_cursor;

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
fn parsed_document_captures_ctx_account_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        let authority = ctx.accounts.authority.key();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "counter" && usage.mutable));
    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "authority" && !usage.mutable));
    assert!(increment
        .account_data_field_usages
        .iter()
        .any(|usage| { usage.account == "counter" && usage.field == "count" && usage.mutable }));
}

#[test]
fn parsed_document_captures_ctx_accounts_alias_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let accounts = &mut ctx.accounts;
        accounts.counter.count += 1;
        let authority = accounts.authority.key();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "counter" && usage.mutable));
    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "authority" && !usage.mutable));
    assert!(increment
        .account_data_field_usages
        .iter()
        .any(|usage| { usage.account == "counter" && usage.field == "count" && usage.mutable }));
    assert!(increment.account_path_usages.iter().any(|usage| {
        usage
            .segments
            .iter()
            .map(|segment| segment.name.as_str())
            .collect::<Vec<_>>()
            == ["counter", "count"]
    }));
}

#[test]
fn parsed_document_captures_account_field_alias_data_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count += 1;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment
        .account_data_field_usages
        .iter()
        .any(|usage| usage.account == "counter" && usage.field == "count" && usage.mutable));
}

#[test]
fn parsed_document_captures_composite_account_alias_paths() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let wrapper = &ctx.accounts.wrapper;
        wrapper.inner.fake;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read")
        .unwrap();

    assert!(read.account_path_usages.iter().any(|usage| {
        usage
            .segments
            .iter()
            .map(|segment| segment.name.as_str())
            .collect::<Vec<_>>()
            == ["wrapper", "inner", "fake"]
    }));
}

#[test]
fn parsed_document_captures_nested_ctx_account_usage() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read_token(ctx: Context<ReadToken>) -> Result<()> {
        let data = ctx.accounts.vaultish.data.borrow();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read_token = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read_token")
        .unwrap();

    assert!(read_token
        .account_usages
        .iter()
        .any(|usage| usage.name == "vaultish"));
    assert!(read_token
        .account_data_field_usages
        .iter()
        .any(|usage| usage.account == "vaultish" && usage.field == "data"));
}

#[test]
fn parsed_document_captures_full_ctx_account_paths() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read_nested(ctx: Context<ReadNested>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read_nested = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read_nested")
        .unwrap();
    let usage = read_nested
        .account_path_usages
        .iter()
        .find(|usage| {
            usage
                .segments
                .iter()
                .map(|segment| segment.name.as_str())
                .eq(["wrapper", "inner"])
        })
        .expect("nested account path");

    assert!(!usage.mutable);
    assert_eq!(usage.segments[0].range.start.line, 4);
    assert_eq!(usage.segments[1].range.start.line, 4);
}

#[test]
fn parsed_document_captures_cpi_program_usage() {
    let source = r#"
use anchor_lang::solana_program::instruction::Instruction;

#[program]
pub mod demo {
    pub fn call_external(ctx: Context<CallExternal>) -> Result<()> {
        let ix = Instruction {
            program_id: ctx.accounts.metadata_program.key(),
            accounts: vec![],
            data: vec![],
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
        let signed_cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.associated_token_program.to_account_info(),
            (),
            signer_seeds,
        );
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let call_external = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "call_external")
        .unwrap();

    assert!(call_external
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == "metadata_program"));
    assert!(call_external
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == "token_program"));
    assert!(call_external
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == "associated_token_program"));
}

#[test]
fn parsed_document_captures_account_meta_signer_usage() {
    let source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[program]
pub mod demo {
    pub fn call_external(ctx: Context<CallExternal>) -> Result<()> {
        let metas = vec![
            AccountMeta::new(ctx.accounts.authority.key(), true),
            AccountMeta {
                pubkey: ctx.accounts.readonly_authority.key(),
                is_signer: true,
                is_writable: false,
            },
        ];
        require!(ctx.accounts.checked_authority.is_signer, ErrorCode::MissingSigner);
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let call_external = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "call_external")
        .unwrap();

    assert!(call_external
        .signer_usages
        .iter()
        .any(|usage| usage.name == "authority"));
    assert!(call_external
        .signer_usages
        .iter()
        .any(|usage| usage.name == "readonly_authority"));
    assert!(call_external
        .signer_checks
        .iter()
        .any(|usage| usage.name == "checked_authority"));
}

#[test]
fn parsed_document_captures_account_key_comparisons() {
    let source = r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        if ctx.accounts.user_a.key() == ctx.accounts.user_b.key() {
            return err!(ErrorCode::DuplicateAccount);
        }
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let update = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "update")
        .unwrap();

    assert!(update
        .account_key_comparisons
        .iter()
        .any(|comparison| comparison.matches("user_a", "user_b")));
    assert!(update.account_usages.iter().all(|usage| !usage.mutable));
}

#[test]
fn parsed_document_captures_token_account_unpack_usage() {
    let source = r#"
use spl_token::state::Account as SplTokenAccount;

#[program]
pub mod demo {
    pub fn read_token(ctx: Context<ReadToken>) -> Result<()> {
        let token = SplTokenAccount::unpack(&ctx.accounts.vaultish.data.borrow())?;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read_token = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read_token")
        .unwrap();

    assert!(read_token
        .token_account_unpack_usages
        .iter()
        .any(|usage| usage.name == "vaultish"));
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
fn document_symbols_include_every_generated_account_constraint_child() {
    let constraints = constraint_catalog::CONSTRAINTS
        .iter()
        .map(sample_constraint_fragment)
        .collect::<Vec<_>>()
        .join(",\n        ");
    let source = format!(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account(
        {constraints}
    )]
    pub state: Account<'info, State>,
}}
"#
    );

    let document = ParsedDocument::parse_or_empty(&source);
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

    for spec in constraint_catalog::CONSTRAINTS {
        let key = constraint_catalog::key(spec.label);
        let expected_occurrences = constraint_catalog::CONSTRAINTS
            .iter()
            .filter(|candidate| constraint_catalog::key(candidate.label) == key)
            .count();
        let actual_occurrences = constraints
            .iter()
            .filter(|symbol| symbol.name == key && symbol.kind == SymbolKind::PROPERTY)
            .count();

        assert!(
            actual_occurrences >= expected_occurrences,
            "missing document symbol child for generated constraint `{key}`: {constraints:?}"
        );
        assert!(
            constraints.iter().any(|symbol| {
                symbol.name == key
                    && symbol.detail.as_deref() == Some(&format!("{:?} constraint", spec.family))
            }),
            "missing document symbol metadata for generated constraint `{key}`: {constraints:?}"
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

fn sample_constraint_fragment(spec: &constraint_catalog::ConstraintSpec) -> String {
    let key = constraint_catalog::key(spec.label);
    match spec.value_kind {
        constraint_catalog::ConstraintValueKind::None => key.to_string(),
        constraint_catalog::ConstraintValueKind::AnyExpression => format!("{key} = expr"),
        constraint_catalog::ConstraintValueKind::AccountReference => {
            format!("{key} = account")
        }
        constraint_catalog::ConstraintValueKind::SignerReference => format!("{key} = signer"),
        constraint_catalog::ConstraintValueKind::ProgramReference => {
            format!("{key} = program")
        }
        constraint_catalog::ConstraintValueKind::InstructionArgument => {
            format!("{key} = arg")
        }
        constraint_catalog::ConstraintValueKind::Keyword => format!("{key} = skip"),
        constraint_catalog::ConstraintValueKind::Boolean => format!("{key} = true"),
        constraint_catalog::ConstraintValueKind::Space => format!("{key} = 8"),
        constraint_catalog::ConstraintValueKind::Seeds => {
            format!("{key} = [b\"state\"]")
        }
    }
}
