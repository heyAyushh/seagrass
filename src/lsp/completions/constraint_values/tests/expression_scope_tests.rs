use {super::*, crate::document::ParsedDocument};

#[test]
fn completes_constraint_expression_same_file_values() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = HEL,
        constraint = validate_
    )]
    pub mint: AccountInfo<'info>,
}

const HELLO_LIMIT: u64 = 1;

fn validate_position() -> bool {
    true
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    let const_items = completions(&document, position_after(source, "constraint = HEL"))
        .expect("const-like expression completions");
    assert!(const_items.iter().any(|item| item.label == "HELLO_LIMIT"));

    let value_items = completions(&document, position_after(source, "constraint = validate_"))
        .expect("function expression completions");
    assert!(value_items
        .iter()
        .any(|item| item.label == "validate_position"));
}

#[test]
fn completes_constraint_expression_const_like_imports() {
    let source = r#"
use crate::checks::EXPECTED_LIMIT;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = EXPECT)]
    pub mint: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "constraint = EXPECT"))
        .expect("imported const-like expression completions");

    assert!(items.iter().any(|item| item.label == "EXPECTED_LIMIT"));
    assert!(!items.iter().any(|item| item.label == "checks"));
}
