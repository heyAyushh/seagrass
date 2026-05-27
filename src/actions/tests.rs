use {
    super::{
        code_actions, code_actions_unfiltered, common::diagnostic_code, common::parser_rule_kind,
        cursor_dependent_code_actions, rank_and_filter_for_cursor, resolve,
    },
    crate::{
        constraint_catalog,
        diagnostics::{self, ANCHOR_MISSING_INIT_CONSTRAINT_CODE},
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{Diagnostic, NumberOrString, Position, Range, TextEdit, Url},
};

#[path = "tests/account_refs.rs"]
mod account_refs;
#[path = "tests/account_types.rs"]
mod account_types;
#[path = "tests/context.rs"]
mod context;
#[path = "tests/features_project.rs"]
mod features_project;
#[path = "tests/generated_constraints.rs"]
mod generated_constraints;
#[path = "tests/instructions.rs"]
mod instructions;
#[path = "tests/missing_init.rs"]
mod missing_init;
#[path = "tests/security_mut.rs"]
mod security_mut;
#[path = "tests/token_constraints.rs"]
mod token_constraints;

fn apply_text_edit(source: &str, edit: &TextEdit) -> String {
    let start = byte_offset(source, edit.range.start);
    let end = byte_offset(source, edit.range.end);
    let mut updated = String::new();
    updated.push_str(&source[..start]);
    updated.push_str(&edit.new_text);
    updated.push_str(&source[end..]);
    updated
}

fn byte_offset(source: &str, position: Position) -> usize {
    let target_line = usize::try_from(position.line).unwrap();
    let target_character = usize::try_from(position.character).unwrap();
    let mut offset = 0usize;
    for (idx, line) in source.split_inclusive('\n').enumerate() {
        if idx == target_line {
            let line = line.strip_suffix('\n').unwrap_or(line);
            return offset
                + line
                    .char_indices()
                    .nth(target_character)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
        }
        offset += line.len();
    }
    source.len()
}

#[test]
fn cursor_range_filters_unrelated_quickfix_actions() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub vault: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = vec![
        missing_account_reference_diagnostic(2, "usr", "user"),
        missing_account_reference_diagnostic(4, "vualt", "vault"),
    ];
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        point_range(diagnostics[1].range.start),
        &diagnostics,
    );

    assert!(actions
        .iter()
        .any(|action| action.title == "Replace `vualt` with `vault`"));
    assert!(!actions
        .iter()
        .any(|action| action.title == "Replace `usr` with `user`"));
}

#[test]
fn cursor_distance_ranks_nearest_quickfix_actions() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub vault: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = vec![
        missing_account_reference_diagnostic(2, "usr", "user"),
        missing_account_reference_diagnostic(4, "vualt", "vault"),
    ];
    let cursor_near_vault = Range {
        start: Position {
            line: 5,
            character: 0,
        },
        end: Position {
            line: 5,
            character: 0,
        },
    };
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        cursor_near_vault,
        &diagnostics,
    );

    assert_eq!(
        actions.first().map(|action| action.title.as_str()),
        Some("Replace `vualt` with `vault`")
    );
}

#[test]
fn split_build_matches_wrapper_output_for_targeted_cursor() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub vault: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = vec![
        missing_account_reference_diagnostic(2, "usr", "user"),
        missing_account_reference_diagnostic(4, "vualt", "vault"),
    ];
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let cursor = point_range(diagnostics[1].range.start);

    let wrapper = code_actions(&document, uri.clone(), cursor, &diagnostics);

    // The wrapper composes the same three steps the cached LSP handler unrolls.
    let mut split = code_actions_unfiltered(&document, uri.clone(), &diagnostics);
    split.extend(cursor_dependent_code_actions(
        &document,
        uri,
        cursor,
        &diagnostics,
    ));
    let split = rank_and_filter_for_cursor(split, cursor);

    let wrapper_titles: Vec<&str> = wrapper.iter().map(|a| a.title.as_str()).collect();
    let split_titles: Vec<&str> = split.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(wrapper_titles, split_titles);
}

#[test]
fn unfiltered_build_is_a_superset_of_cursor_filtered_actions() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub vault: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = vec![
        missing_account_reference_diagnostic(2, "usr", "user"),
        missing_account_reference_diagnostic(4, "vualt", "vault"),
    ];
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let cursor = point_range(diagnostics[1].range.start);

    let cursor_filtered = code_actions(&document, uri.clone(), cursor, &diagnostics);
    let unfiltered = code_actions_unfiltered(&document, uri, &diagnostics);

    assert!(
        unfiltered.len() >= cursor_filtered.len(),
        "unfiltered ({}) should not be smaller than cursor-filtered ({})",
        unfiltered.len(),
        cursor_filtered.len()
    );
    let unfiltered_titles: Vec<&str> = unfiltered.iter().map(|a| a.title.as_str()).collect();
    assert!(unfiltered_titles.contains(&"Replace `vualt` with `vault`"));
    assert!(unfiltered_titles.contains(&"Replace `usr` with `user`"));
}

fn missing_account_reference_diagnostic(line: u32, missing: &str, replacement: &str) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position { line, character: 4 },
            end: Position {
                line,
                character: 4 + u32::try_from(missing.len()).unwrap(),
            },
        },
        source: Some(diagnostics::SOURCE.to_string()),
        code: Some(NumberOrString::String(
            "anchor-missing-account-reference".to_string(),
        )),
        data: Some(serde_json::json!({
            "account": missing,
            "accountsStruct": "Create",
            "candidates": [replacement],
        })),
        ..Diagnostic::default()
    }
}

fn point_range(position: Position) -> Range {
    Range {
        start: position,
        end: position,
    }
}

struct MissingReferenceActionScenario {
    missing: &'static str,
    replacement: &'static str,
}

fn missing_reference_action_scenario(
    key: &str,
    value_kind: constraint_catalog::ConstraintValueKind,
) -> MissingReferenceActionScenario {
    match value_kind {
        constraint_catalog::ConstraintValueKind::AccountReference => match key {
            "token::mint" | "associated_token::mint" => MissingReferenceActionScenario {
                missing: "mnt",
                replacement: "mint",
            },
            _ => MissingReferenceActionScenario {
                missing: "recipent",
                replacement: "recipient",
            },
        },
        constraint_catalog::ConstraintValueKind::SignerReference => {
            MissingReferenceActionScenario {
                missing: "authorty",
                replacement: "authority",
            }
        }
        constraint_catalog::ConstraintValueKind::ProgramReference => {
            if key.ends_with("token_program") {
                MissingReferenceActionScenario {
                    missing: "tokn_program",
                    replacement: "token_program",
                }
            } else if key == "seeds::program" {
                MissingReferenceActionScenario {
                    missing: "metadata_programm",
                    replacement: "metadata_program",
                }
            } else {
                MissingReferenceActionScenario {
                    missing: "hook_programm",
                    replacement: "hook_program",
                }
            }
        }
        _ => unreachable!("only generated reference constraints are passed here"),
    }
}

fn generated_missing_reference_source(key: &str, missing: &str) -> String {
    format!(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account({key} = {missing})]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub authority: Signer<'info>,
    pub recipient: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub metadata_program: Program<'info, Metadata>,
    pub hook_program: Program<'info, HookProgram>,
}}

#[account]
pub struct State {{
    pub recipient: Pubkey,
}}
"#
    )
}
