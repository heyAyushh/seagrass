use {
    super::strip_markers,
    crate::{actions, completions, diagnostics, document::ParsedDocument},
    tower_lsp::lsp_types::{Diagnostic, NumberOrString, Position, Url},
};

#[test]
fn editor_ux_resolves_handler_struct_literal_fields() {
    let marked = strip_markers(
        r#"
use anchor_lang::prelude::*;

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _complete = PositionBundle {
        position_/*caret:field*/bitmap: [0; 32],
        position_bundle_mint: Pubkey::default(),
    };
    let _typo = PositionBundle {
        position_bundel_mint: Pubkey::default(),
        position_bitmap: [0; 32],
    };
    Ok(())
}
"#,
    );
    let document = ParsedDocument::parse_or_empty(&marked.source);
    let completion_position = *marked.positions.get("field").expect("field marker");
    let completions = completions::completions(&document, completion_position)
        .expect("struct literal field completions");

    assert_eq!(completions[0].label, "position_bitmap");
    assert!(completions.iter().any(|item| {
        item.label == "position_bundle_mint"
            && item
                .data
                .as_ref()
                .and_then(|data| data.get("anchorCompletion"))
                .and_then(|value| value.as_str())
                == Some("handler-struct-literal-field")
    }));

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_data_str(diagnostic, "reason") == Some("unknown-struct-literal-field")
        })
        .unwrap_or_else(|| panic!("missing struct literal field diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            "anchor-missing-account-reference".to_string()
        ))
    );
    assert!(diagnostic.message.contains("position_bundel_mint"));
    assert_eq!(
        diagnostic
            .code_description
            .as_ref()
            .map(|description| description.href.as_str()),
        Some("https://doc.rust-lang.org/reference/expressions/struct-expr.html")
    );

    let uri = Url::parse("file:///editor-struct-literal.rs").unwrap();
    let actions = actions::code_actions(&document, uri, diagnostic.range, &diagnostics);
    assert!(
        actions
            .iter()
            .any(|action| action.title.contains("position_bundle_mint")),
        "missing struct literal field quick fix: {actions:#?}"
    );
}

#[test]
fn editor_ux_semantic_sweep_covers_core_handler_surfaces() {
    let marked = strip_markers(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(bundle_index: u16)]
pub struct Run<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
    #[account(
        constraint = position_bundle.position_/*caret:constraint-member*/,
        constraint = random_constraint_value,
    )]
    pub vault: Account<'info, PositionBundle>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let position_bundle_mint_value = Pubkey::default();
    let bundle: PositionBundle = PositionBundle {
        position_/*caret:literal-field*/bitmap: [0; 32],
        position_bundle_mint: position_/*caret:literal-value*/,
    };
    bundle.position_bundle_m/*caret:member*/;
    random_handler_value;
    let _typo = PositionBundle {
        position_bundel_mint: Pubkey::default(),
        position_bitmap: [0; 32],
    };
    Ok(())
}
"#,
    );
    let document = ParsedDocument::parse_or_empty(&marked.source);

    assert_completion_contains(&document, &marked, "literal-field", "position_bitmap");
    assert_completion_contains(
        &document,
        &marked,
        "literal-value",
        "position_bundle_mint_value",
    );
    assert_completion_contains(&document, &marked, "member", "position_bundle_mint");
    assert_completion_contains(
        &document,
        &marked,
        "constraint-member",
        "position_bundle_mint",
    );

    let diagnostics = diagnostics::collect(&document);
    assert_diagnostic_reason(
        &diagnostics,
        "unresolved-handler-identifier",
        "random_handler_value",
    );
    assert_diagnostic_reason(
        &diagnostics,
        "unknown-struct-literal-field",
        "position_bundel_mint",
    );
    assert_diagnostic_code(
        &diagnostics,
        "anchor-constraint-expression",
        "random_constraint_value",
    );
}

fn assert_completion_contains(
    document: &ParsedDocument,
    marked: &super::MarkedSource,
    marker: &str,
    label: &str,
) {
    let position = *marked.positions.get(marker).expect("caret marker");
    let items = completion_items(document, position);

    assert!(
        items.iter().any(|item| item.label == label),
        "missing completion `{label}` at `{marker}`; got {:?}",
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
    );
}

fn completion_items(
    document: &ParsedDocument,
    position: Position,
) -> Vec<tower_lsp::lsp_types::CompletionItem> {
    completions::completions(document, position).unwrap_or_default()
}

fn assert_diagnostic_reason(diagnostics: &[Diagnostic], reason: &str, message: &str) {
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic_data_str(diagnostic, "reason") == Some(reason)
                && diagnostic.message.contains(message)
        }),
        "missing diagnostic reason `{reason}` containing `{message}`: {diagnostics:#?}"
    );
}

fn assert_diagnostic_code(diagnostics: &[Diagnostic], code: &str, message: &str) {
    assert!(
        diagnostics.iter().any(|diagnostic| {
            matches!(diagnostic.code.as_ref(), Some(NumberOrString::String(value)) if value == code)
                && diagnostic.message.contains(message)
        }),
        "missing diagnostic code `{code}` containing `{message}`: {diagnostics:#?}"
    );
}

fn diagnostic_data_str<'a>(diagnostic: &'a Diagnostic, key: &str) -> Option<&'a str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
}
