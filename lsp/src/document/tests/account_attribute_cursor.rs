use super::*;

#[test]
fn parsed_document_keeps_tree_sitter_for_incomplete_rust() {
    let source = "#[account(init, )]\npub state: Account<'info, State>,";
    assert!(ParsedDocument::parse(source).is_err());

    let document = ParsedDocument::parse_or_empty(source);
    let tree_sitter = document.tree_sitter().unwrap();

    assert_eq!(tree_sitter.root_kind(), "source_file");
    assert!(tree_sitter.has_error());
    assert!(
        document.is_in_account_attribute(tower_lsp::lsp_types::Position {
            line: 0,
            character: 16,
        })
    );
}

#[test]
fn account_attribute_cursor_ignores_strings_and_comments() {
    let source = r##"
fn main() {
    let text = "#[account(pa";
    // #[account(mu
    /// #[account(se
}
"##;
    let document = ParsedDocument::parse_or_empty(source);

    for cursor in [r##""#[account(pa"##, "// #[account(mu", "/// #[account(se"] {
        assert!(
            !document.is_in_account_attribute(position_after(source, cursor)),
            "account attribute cursor should ignore Anchor-shaped text at {cursor}"
        );
    }
}

#[test]
fn account_attribute_cursor_reports_field_slot_and_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, seeds = [payer.key().as_ref(), sta)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let cursor = document
        .account_attribute_cursor(position_after(source, "sta"))
        .expect("structured account attribute cursor");

    assert_eq!(cursor.field_name.as_deref(), Some("state"));
    assert_eq!(cursor.slot, AccountAttributeSlot::Value);
    assert_eq!(cursor.constraint_key.as_deref(), Some("seeds"));
    assert_eq!(cursor.prefix, "sta");
    assert!(cursor.in_seed_array);
}

#[test]
fn account_attribute_cursor_recovers_from_unclosed_attribute() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = pay
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let cursor = document
        .account_attribute_cursor(position_after(source, "payer = pay"))
        .expect("structured cursor in broken account attribute");

    assert_eq!(cursor.field_name.as_deref(), Some("state"));
    assert_eq!(cursor.slot, AccountAttributeSlot::Value);
    assert_eq!(cursor.constraint_key.as_deref(), Some("payer"));
    assert_eq!(cursor.prefix, "pay");
}

#[test]
fn account_attribute_cursor_recovers_nested_cfg_attr_seed_array() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[cfg_attr(feature = "pda", account(seeds = [[b"state"], nam))]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let cursor = document
        .account_attribute_cursor(position_after(source, "nam"))
        .expect("nested cfg_attr account cursor");

    assert_eq!(cursor.field_name.as_deref(), Some("state"));
    assert_eq!(cursor.slot, AccountAttributeSlot::Value);
    assert_eq!(cursor.constraint_key.as_deref(), Some("seeds"));
    assert_eq!(cursor.prefix, "nam");
    assert!(cursor.in_seed_array);
}

#[test]
fn document_symbols_fall_back_to_tree_sitter_when_syn_parse_fails() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, s
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let symbols = document_symbols(&document);

    let accounts = symbols
        .iter()
        .find(|symbol| symbol.name == "ReadRent")
        .expect("tree-sitter account symbol");
    assert_eq!(accounts.detail.as_deref(), Some("#[derive(Accounts)]"));
    assert!(accounts
        .children
        .as_ref()
        .is_some_and(|children| children.iter().any(|field| field.name == "rent")));
}

fn position_after(source: &str, needle: &str) -> tower_lsp::lsp_types::Position {
    let offset = source.find(needle).expect("needle") + needle.len();
    let prefix = &source[..offset];
    let line = u32::try_from(prefix.chars().filter(|ch| *ch == '\n').count()).unwrap();
    let line_start = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    tower_lsp::lsp_types::Position {
        line,
        character: u32::try_from(prefix[line_start..].chars().count()).unwrap(),
    }
}
