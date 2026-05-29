use {super::*, crate::document::ParsedDocument};

#[test]
fn hovers_anchor_account_field_from_instruction_body() {
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
    let hover = hover(
        &document,
        Position {
            line: 4,
            character: 22,
        },
    )
    .unwrap();

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("Anchor field in `Update`"));
    assert!(markup.value.contains("Account<Counter>"));
    assert!(markup.value.contains("`increment` mutates"));
}

#[test]
fn hovers_anchor_account_data_field_from_instruction_body() {
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
    let hover = hover(
        &document,
        Position {
            line: 4,
            character: 31,
        },
    )
    .unwrap();

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`Counter.count`"));
    assert!(markup.value.contains("Type: `u64`"));
    assert!(markup.value.contains("`increment` mutates"));
}

#[test]
fn hovers_nested_ctx_account_path_as_anchor_field() {
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
    let markup = hover_markup(&document, position_of(source, "wrapper.inner", "inner"));

    assert!(markup.contains("Anchor field in `Wrapped`"));
    assert!(markup.contains("Type: `Account<Inner>`"));
    assert!(markup.contains("`read` reads"));
}

#[test]
fn hovers_program_instruction_context() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64) -> Result<()> {
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let hover = hover(
        &document,
        Position {
            line: 3,
            character: 12,
        },
    )
    .unwrap();

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("Context<Create>"));
    assert!(markup.value.contains("amount"));
}

#[test]
fn hovers_instruction_argument_constraint_references() {
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
    let markup = hover_markup(
        &document,
        position_of(source, "mint::decimals = token_decimals", "token_decimals"),
    );

    assert!(markup.contains("Anchor instruction argument for `Context<Create>`"));
    assert!(markup.contains("Type: `u8`"));
    assert!(markup.contains("mint::decimals"));
    assert!(markup.contains("Known local references: 3"));

    let markup = hover_markup(
        &document,
        position_of(source, "seeds = [token_name.as_bytes()]", "token_name"),
    );
    assert!(markup.contains("Type: `String`"));
    assert!(markup.contains("seeds"));
}

#[test]
fn hovers_generated_sysvar_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, Rent>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let markup = hover_markup(
        &document,
        Position {
            line: 3,
            character: 29,
        },
    );

    assert!(markup.contains("Sysvar<'info, Rent>"));
    assert!(markup.contains("Anchor sysvar account for `Rent`"));
    assert!(markup.contains("Generated from"));
}

#[test]
fn hovers_workspace_accounts_context_with_field_types() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> {
        Ok(())
    }
}
"#,
    )
    .unwrap();
    let hover = workspace_accounts_context_hover(
        &document,
        Position {
            line: 5,
            character: 37,
        },
        "MakeOffer",
        &[
            WorkspaceContextField {
                name: "maker".to_string(),
                type_name: Some("Signer".to_string()),
                type_display: Some("Signer".to_string()),
            },
            WorkspaceContextField {
                name: "offer".to_string(),
                type_name: Some("Account".to_string()),
                type_display: Some("Account<Offer>".to_string()),
            },
        ],
    )
    .unwrap();

    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    assert!(markup.value.contains("`Context<MakeOffer>`"));
    assert!(markup.value.contains("`maker`: `Signer`"));
    assert!(markup.value.contains("`offer`: `Account<Offer>`"));
}

#[test]
fn account_context_hover_requires_context_generic_or_declaration() {
    let source = r#"
#[derive(Accounts)]
pub struct Instructions<'info> {
    pub payer: Signer<'info>,
}

fn helper() {
    let _items: Vec<Instructions>;
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let hover = hover(
        &document,
        position_of(source, "Vec<Instructions>", "Instructions"),
    );

    assert!(
        hover.is_none(),
        "Vec<T> should not get an Anchor context hover"
    );
}

#[test]
fn hovers_generated_spl_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct UseToken<'info> {
    pub token: Account<'info, TokenAccount>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let markup = hover_markup(
        &document,
        Position {
            line: 3,
            character: 31,
        },
    );

    assert!(markup.contains("Account<'info, TokenAccount>"));
    assert!(markup.contains("Anchor SPL account wrapper"));
    assert!(markup.contains("Generated from"));
}

#[test]
fn hovers_generated_signer_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct NeedsSigner<'info> {
    pub authority: Signer<'info>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let markup = hover_markup(
        &document,
        Position {
            line: 3,
            character: 20,
        },
    );

    assert!(markup.contains("Signer<'info>"));
    assert!(markup.contains("Anchor signer account"));
    assert!(markup.contains("Generated from"));
}

fn hover_markup(document: &ParsedDocument, position: Position) -> String {
    let hover = hover(document, position).unwrap();
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };
    markup.value
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
