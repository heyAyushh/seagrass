use super::*;

#[test]
fn parses_incomplete_rust_with_errors() {
    let source = "fn incomplete(";
    let syntax = RustSyntax::parse(source).unwrap();
    assert!(syntax.has_error());
}

#[test]
fn finds_account_attribute_in_incomplete_rust() {
    let source = "#[account(init, )]\npub state: Account<'info, State>,";
    let syntax = RustSyntax::parse(source).unwrap();

    assert!(syntax.account_attribute_at_position(
        source,
        Position {
            line: 0,
            character: 16,
        }
    ));
}

#[test]
fn recovers_account_attribute_cursor_from_nested_cfg_attr_seed_array() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[cfg_attr(feature = "pda", account(seeds = [[b"state"], na))]
    pub state: Account<'info, State>,
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let cursor = syntax
        .account_attribute_cursor_at_position(source, position_after(source, "na"))
        .expect("tree-sitter account cursor in nested cfg_attr");

    assert_eq!(cursor.slot, crate::document::AccountAttributeSlot::Value);
    assert_eq!(cursor.constraint_key.as_deref(), Some("seeds"));
    assert_eq!(cursor.prefix, "na");
    assert!(cursor.in_seed_array);
}

#[test]
fn recovers_context_prefix_from_unclosed_context_generic() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn create(ctx: Context<Cr
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let context = syntax
        .context_type_prefix_at_position(source, position_after(source, "Context<Cr"))
        .expect("tree-sitter context generic prefix");

    assert_eq!(context.prefix, "Cr");
}

#[test]
fn recovers_context_prefix_with_anchor_v2_preview_hint() {
    let source = r#"
use anchor_lang_v2::prelude::*;

#[program]
pub mod demo {
    pub fn create(ctx: Context<Cr
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let context = syntax
        .context_type_prefix_at_position(source, position_after(source, "Context<Cr"))
        .expect("tree-sitter context generic prefix");

    assert_eq!(context.prefix, "Cr");
}

#[test]
fn finds_accounts_struct_for_incomplete_field_type_argument() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, s
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();

    assert!(syntax.accounts_struct_at_position(
        source,
        Position {
            line: 3,
            character: 28,
        }
    ));
}

#[test]
fn finds_account_field_context_for_incomplete_generic_argument() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, s
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let field = syntax
        .account_field_at_position(
            source,
            Position {
                line: 3,
                character: 28,
            },
        )
        .expect("field syntax context");

    assert_eq!(field.name.as_deref(), Some("rent"));
    assert!(field
        .type_text
        .as_deref()
        .is_some_and(|text| text.starts_with("Sysvar")));
}

#[test]
fn ignores_non_accounts_structs() {
    let source = r#"
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, s
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();

    assert!(!syntax.accounts_struct_at_position(
        source,
        Position {
            line: 2,
            character: 28,
        }
    ));
}

#[test]
fn extracts_anchor_symbols_from_incomplete_rust() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub rent: Sysvar<'info, s
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let symbols = syntax.anchor_document_symbols(source);

    let program = symbols
        .iter()
        .find(|symbol| symbol.name == "demo")
        .expect("program symbol");
    assert!(program
        .children
        .as_ref()
        .is_some_and(|children| children.iter().any(|symbol| symbol.name == "initialize")));

    let accounts = symbols
        .iter()
        .find(|symbol| symbol.name == "Create")
        .expect("accounts symbol");
    assert_eq!(accounts.detail.as_deref(), Some("#[derive(Accounts)]"));
    assert!(accounts
        .children
        .as_ref()
        .is_some_and(|children| children.iter().any(|symbol| symbol.name == "rent")));

    assert!(symbols.iter().any(|symbol| {
        symbol.name == "State" && symbol.detail.as_deref() == Some("#[account]")
    }));
}

#[test]
fn query_overlay_classifies_anchor_rust_nodes_in_incomplete_source() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = user,
        seeds::program = other_program.key(),
        space = 8 + State::INIT_SPACE
    )]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub rent: Sysvar<'info, R
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let captures = syntax.anchor_query_captures(source);

    assert!(captures.iter().any(|capture| {
        capture.kind == AnchorQueryKind::ProgramModule && capture.name.as_deref() == Some("demo")
    }));
    assert!(captures.iter().any(|capture| {
        capture.kind == AnchorQueryKind::ProgramInstruction
            && capture.name.as_deref() == Some("initialize")
    }));
    assert!(captures.iter().any(|capture| {
        capture.kind == AnchorQueryKind::AccountsStruct && capture.name.as_deref() == Some("Create")
    }));
    assert!(captures.iter().any(|capture| {
        capture.kind == AnchorQueryKind::AccountField && capture.name.as_deref() == Some("rent")
    }));
    assert!(captures.iter().any(|capture| {
        capture.kind == AnchorQueryKind::AccountDataStruct
            && capture.name.as_deref() == Some("State")
    }));
    assert!(captures
        .iter()
        .any(|capture| capture.kind == AnchorQueryKind::AccountAttribute));
    for key in ["init", "payer", "seeds::program", "space"] {
        assert!(
            captures.iter().any(|capture| {
                capture.kind == AnchorQueryKind::AccountConstraintKey
                    && capture.name.as_deref() == Some(key)
            }),
            "missing overlay constraint key capture for `{key}`: {captures:?}"
        );
    }
}

#[test]
fn extracts_known_type_references_from_incomplete_rust() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub account: Account<'info, State
}

#[account]
pub struct State {}
"#;
    let syntax = RustSyntax::parse(source).unwrap();
    let known_types = ["Create".to_string(), "State".to_string()]
        .into_iter()
        .collect::<HashSet<_>>();

    let references = syntax.anchor_type_references(source, &known_types);

    assert!(references
        .iter()
        .any(|reference| reference.name == "Create" && reference.range.start.line == 3));
    assert!(references
        .iter()
        .any(|reference| reference.name == "Create" && reference.range.start.line == 9));
    assert!(references
        .iter()
        .any(|reference| reference.name == "State" && reference.range.start.line == 10));
    assert!(references
        .iter()
        .any(|reference| reference.name == "State" && reference.range.start.line == 14));
}

#[test]
fn supports_incremental_reparsing() {
    let original = "fn hello() { println!(\"old\"); }";
    let edited = "fn hello() { println!(\"new\"); }";

    let old_syntax = RustSyntax::parse(original).unwrap();
    let new_syntax = RustSyntax::parse_edited(edited, Some(&old_syntax.tree)).unwrap();

    assert_eq!(old_syntax.root_kind(), new_syntax.root_kind());
    assert!(!new_syntax.has_error());
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
