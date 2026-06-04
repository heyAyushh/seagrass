use {super::*, crate::document::ParsedDocument};

const ERROR_SOURCE: &str = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority @ )]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[error_code]
pub enum CustomError {
    #[msg("not authorized")]
    Unauthorized,
    InvalidAmount,
}
"#;

#[test]
fn completes_error_variants_after_at() {
    let document = ParsedDocument::parse(ERROR_SOURCE).unwrap();
    let items = completions(
        &document,
        position_after(ERROR_SOURCE, "has_one = authority @ "),
    )
    .expect("error annotation completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"CustomError::Unauthorized"), "{labels:?}");
    assert!(labels.contains(&"CustomError::InvalidAmount"), "{labels:?}");
}

#[test]
fn filters_error_variants_by_typed_path_prefix() {
    let source = ERROR_SOURCE.replace("@ )", "@ CustomError::Inv)");
    let document = ParsedDocument::parse(&source).unwrap();
    let items = completions(&document, position_after(&source, "@ CustomError::Inv"))
        .expect("prefix-filtered error annotation completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(labels, vec!["CustomError::InvalidAmount"]);
}

#[test]
fn does_not_offer_error_variants_in_value_position() {
    let document = ParsedDocument::parse(ERROR_SOURCE).unwrap();
    // Before the `@`, in the constraint value slot, error variants must not appear.
    let items = completions(&document, position_after(ERROR_SOURCE, "has_one = "));
    let has_error = items
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|item| item.label.starts_with("CustomError::"));
    assert!(!has_error, "error variants leaked into value position");
}
