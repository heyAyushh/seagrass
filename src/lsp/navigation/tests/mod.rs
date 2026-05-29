use {super::*, crate::document::ParsedDocument};

#[test]
fn jumps_to_accounts_struct_definition() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        Position {
            line: 3,
            character: 35,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 9);
}

#[test]
fn declaration_uses_anchor_definition_graph() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = declaration_range(
        &document,
        Position {
            line: 3,
            character: 35,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 9);
    assert_eq!(
        declaration_target_kinds(
            &document,
            Position {
                line: 3,
                character: 35,
            }
        )
        .unwrap(),
        vec![SymbolKind::STRUCT]
    );
}

#[test]
fn type_definition_jumps_from_context_reference_to_accounts_struct() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = type_definition_range(
        &document,
        Position {
            line: 3,
            character: 35,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 9);
}

#[test]
fn type_definition_jumps_from_account_field_to_account_data_struct() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info, State>,
}

#[account]
pub struct State {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = type_definition_range(
        &document,
        Position {
            line: 3,
            character: 9,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 7);
}

#[test]
fn type_definition_jumps_from_instruction_body_account_use_to_account_data_struct() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}

#[account]
pub struct Counter {
    pub count: u64,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = type_definition_range(
        &document,
        Position {
            line: 4,
            character: 23,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 16);
}

#[test]
fn implementation_jumps_from_context_reference_to_program_instruction() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }

    pub fn read(ctx: Context<Read>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct Read<'info> {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let ranges = implementation_ranges(
        &document,
        Position {
            line: 3,
            character: 35,
        },
    )
    .unwrap();

    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start.line, 3);
}

#[test]
fn implementation_jumps_from_accounts_struct_to_program_instruction() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let ranges = implementation_ranges(
        &document,
        Position {
            line: 9,
            character: 12,
        },
    )
    .unwrap();

    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start.line, 3);
}

#[test]
fn references_known_anchor_types_on_word_boundaries() {
    let source = r#"
pub struct State {}
pub struct Stateful {}
pub fn use_state(state: State) -> State {
    state
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let refs = references(
        &document,
        Url::parse("file:///tmp/state.rs").unwrap(),
        Position {
            line: 1,
            character: 12,
        },
    )
    .unwrap();

    assert_eq!(refs.len(), 3);
    assert!(refs.iter().all(|location| {
        location.range.end.character - location.range.start.character == "State".len() as u32
    }));
}

#[test]
fn jumps_from_constraint_reference_to_account_field() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        Position {
            line: 3,
            character: 29,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 5);
}

#[test]
fn jumps_from_constraint_associated_const_to_impl_item() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}

impl State {
    const SPACE: usize = 8 + 8;
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(&document, position_of(source, "State::SPACE", "SPACE")).unwrap();

    assert_eq!(range.start, position_of(source, "const SPACE", "SPACE"));
    assert_eq!(
        definition_target_kinds(&document, position_of(source, "State::SPACE", "SPACE")).unwrap(),
        vec![SymbolKind::CONSTANT]
    );
}

#[test]
fn jumps_from_constraint_associated_function_to_impl_item() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = State::dynamic_space())]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}

impl State {
    fn dynamic_space() -> usize {
        8 + 8
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        position_of(source, "State::dynamic_space()", "dynamic_space"),
    )
    .unwrap();

    assert_eq!(
        range.start,
        position_of(source, "fn dynamic_space", "dynamic_space")
    );
    assert_eq!(
        definition_target_kinds(
            &document,
            position_of(source, "State::dynamic_space()", "dynamic_space"),
        )
        .unwrap(),
        vec![SymbolKind::FUNCTION]
    );
}

#[test]
fn jumps_from_generated_init_space_to_account_data_struct() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    #[max_len(32)]
    pub name: String,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        position_of(source, "State::INIT_SPACE", "INIT_SPACE"),
    )
    .unwrap();

    assert_eq!(
        range.start,
        position_of(source, "pub struct State", "State")
    );
    assert_eq!(
        definition_target_kinds(
            &document,
            position_of(source, "State::INIT_SPACE", "INIT_SPACE")
        )
        .unwrap(),
        vec![SymbolKind::CONSTANT]
    );
}

#[test]
fn jumps_from_instruction_body_account_use_to_context_field() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        Position {
            line: 4,
            character: 22,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 12);
}

#[test]
fn jumps_from_instruction_body_account_data_field_to_account_struct_field() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub counter: Account<'info, Counter>,
}

#[account]
pub struct Counter {
    pub count: u64,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        Position {
            line: 4,
            character: 31,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 17);
}

#[test]
fn jumps_from_nested_ctx_account_path_to_nested_accounts_field() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(&document, position_of(source, "wrapper.inner", "inner")).unwrap();

    assert_eq!(range.start.line, 16);
}

#[test]
fn keeps_account_data_fields_distinct_from_nested_account_paths() {
    let source = r#"
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

#[account]
pub struct Counter {
    pub count: u64,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let target = account_field_path_definition_target(
        &document,
        Position {
            line: 4,
            character: 31,
        },
    );

    assert_eq!(target, None);
}

#[test]
fn jumps_from_helper_function_account_data_field_to_account_struct_field() {
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
    let range = definition_range(
        &document,
        Position {
            line: 12,
            character: 35,
        },
    )
    .unwrap();

    assert_eq!(range.start.line, 8);
}

#[test]
fn references_account_data_field_usages_by_account_data_type() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        ctx.accounts.other.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
    pub other: Account<'info, Other>,
}

#[account]
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct Other {
    pub count: u64,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let refs = references(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        Position {
            line: 4,
            character: 31,
        },
    )
    .unwrap();

    assert_eq!(refs.len(), 2);
    assert!(refs.iter().any(|location| location.range.start.line == 4));
    assert!(refs.iter().any(|location| location.range.start.line == 18));
    assert!(!refs.iter().any(|location| location.range.start.line == 5));
}

#[test]
fn recognizes_generic_account_data_type_as_struct_definition_target() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub counter: Account<'info, Counter>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let kinds = definition_target_kinds(
        &document,
        Position {
            line: 3,
            character: 33,
        },
    )
    .unwrap();

    assert_eq!(kinds, vec![SymbolKind::STRUCT]);
}

#[test]
fn allows_workspace_references_for_anchor_types_not_account_fields() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    assert_eq!(
        reference_target_kinds(
            &document,
            Position {
                line: 3,
                character: 35,
            },
        ),
        Some(vec![SymbolKind::STRUCT])
    );
    assert_eq!(
        reference_target_kinds(
            &document,
            Position {
                line: 9,
                character: 31,
            },
        ),
        Some(vec![SymbolKind::STRUCT])
    );
    assert!(!allows_workspace_references(
        &document,
        Position {
            line: 8,
            character: 29,
        },
    ));
}

#[test]
fn highlights_matching_anchor_type_references() {
    let source = r#"
pub struct Initialize {}
pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
    Ok(())
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let highlights = document_highlights(
        &document,
        Position {
            line: 2,
            character: 32,
        },
    )
    .unwrap();

    assert_eq!(highlights.len(), 2);
}

#[test]
fn jumps_from_instruction_argument_constraint_to_handler_argument() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, _token_decimals: u8, token_name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(token_decimals: u8, token_name: String)]
pub struct Create<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = token_decimals,
        seeds = [token_name.as_bytes()],
        bump
    )]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let range = definition_range(
        &document,
        position_of(source, "mint::decimals = token_decimals", "token_decimals"),
    )
    .unwrap();
    assert_eq!(range.start.line, 3);
    assert_eq!(
        range.end.character - range.start.character,
        "_token_decimals".len() as u32
    );

    let refs = references(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        position_of(source, "seeds = [token_name.as_bytes()]", "token_name"),
    )
    .unwrap();
    assert_eq!(refs.len(), 3);
    assert!(refs.iter().any(|location| location.range.start.line == 3));
    assert!(refs.iter().any(|location| location.range.start.line == 9));
    assert!(refs.iter().any(|location| location.range.start.line == 15));
}

fn position_of(source: &str, containing: &str, word: &str) -> Position {
    let containing_offset = source.find(containing).expect("containing text");
    let word_offset = containing_offset + containing.find(word).expect("word in containing");
    let prefix = &source[..word_offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix
        .rsplit('\n')
        .next()
        .map(|line| line.chars().count())
        .unwrap_or_default() as u32;
    Position { line, character }
}
