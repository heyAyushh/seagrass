use super::*;

#[test]
fn offers_has_one_target_replacement() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[account]
pub struct State {
    pub owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("replace-has-one-target")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("owner"))
        .expect("expected has_one replacement quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "owner");
}
#[test]
fn offers_has_one_target_replacement_from_diagnostic_candidates() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let range = Range {
        start: Position {
            line: 3,
            character: 24,
        },
        end: Position {
            line: 3,
            character: 33,
        },
    };
    let diagnostic = Diagnostic {
        range,
        code: Some(NumberOrString::String(
            "anchor-constraint-shape".to_string(),
        )),
        source: Some(diagnostics::SOURCE.to_string()),
        data: Some(serde_json::json!({
            "quickfix": "replace-has-one-target",
            "accountType": "State",
            "missingDataField": "authority",
            "candidates": ["owner"],
        })),
        ..Diagnostic::default()
    };
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        range,
        &[diagnostic],
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("owner"))
        .expect("expected candidate-backed has_one replacement quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "owner");
}
#[test]
fn offers_add_mut_constraint_quickfix() {
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
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-account-usage"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("#[account(mut)]"))
        .expect("expected add mut quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text.trim(), "#[account(mut)]");
}

#[test]
fn offers_scoped_pda_seed_quickfix_for_static_only_pda() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state"], bump)]
    pub state: AccountInfo<'info>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-security-static-pda"))
        .expect("static PDA diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("scoped PDA seed"))
        .expect("expected scoped PDA seed quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);

    assert!(updated.contains("seeds = [authority.key().as_ref(), b\"state\"]"));
}

#[test]
fn add_mut_quickfix_uses_accounts_struct_from_diagnostic() {
    let source = r#"
#[derive(Accounts)]
pub struct Outer<'info> {
    pub inner: Account<'info, Wrong>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = vec![Diagnostic {
            range: Range {
                start: Position {
                    line: 8,
                    character: 8,
                },
                end: Position {
                    line: 8,
                    character: 13,
                },
            },
            severity: None,
            code: Some(NumberOrString::String("anchor-account-usage".to_string())),
            code_description: None,
            source: Some("seagrass".to_string()),
            message: "`inner` is mutated in `update` but its account field in `Wrapped` is missing `#[account(mut)]`."
                .to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "account": "inner",
                "accountsStruct": "Wrapped",
                "instruction": "update",
                "missing": "mut",
                "quickfix": "add-mut-constraint",
            })),
        }];
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );
    let action = actions
        .iter()
        .find(|action| action.title.contains("Add #[account(mut)]"))
        .expect("add mut action");
    let changes = action.edit.as_ref().unwrap().changes.as_ref().unwrap();
    let edit = &changes[&Url::parse("file:///tmp/lib.rs").unwrap()][0];

    assert_eq!(edit.range.start.line, 8);
}
#[test]
fn offers_duplicate_mutable_account_remediation_quickfixes() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub user_a: Account<'info, User>,
    #[account(mut)]
    pub user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-security-duplicate-account"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let distinct_action = actions
        .iter()
        .find(|action| action.title.contains("distinct"))
        .expect("expected distinct key quickfix");
    assert_eq!(distinct_action.is_preferred, Some(true));
    let distinct_edit = distinct_action
        .edit
        .as_ref()
        .unwrap()
        .changes
        .as_ref()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .first()
        .unwrap();
    assert_eq!(
        distinct_edit.new_text,
        ", constraint = user_a.key() != user_b.key()"
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("`dup`"))
        .expect("expected add dup quickfix");
    assert_eq!(action.is_preferred, Some(false));
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, ", dup");
}
#[test]
fn offers_workspace_composite_duplicate_distinct_quickfix() {
    let outer = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Outer<'info> {
    #[account(mut)]
    pub direct: Account<'info, User>,
    pub nested: Nested<'info>,
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/outer.rs").unwrap(),
                outer.source().to_string(),
            ),
            (
                Url::parse("file:///tmp/nested.rs").unwrap(),
                r#"
#[derive(Accounts)]
pub struct Nested<'info> {
    #[account(mut)]
    pub inner: Account<'info, User>,
}
"#
                .to_string(),
            ),
        ],
    );
    let diagnostics = crate::diagnostics::collect_with_workspace(&outer, Some(&index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-security-duplicate-account")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("peerPath"))
                    .and_then(|value| value.as_str())
                    == Some("nested.inner")
        })
        .unwrap();
    let actions = code_actions(
        &outer,
        Url::parse("file:///tmp/outer.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let distinct_action = actions
        .iter()
        .find(|action| action.title.contains("nested.inner"))
        .expect("expected nested distinct key quickfix");
    assert_eq!(distinct_action.is_preferred, Some(true));
    let edit = distinct_action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(
        text_edit.new_text,
        ", constraint = direct.key() != nested.inner.key()"
    );
}
#[test]
fn offers_add_mut_quickfix_for_init_payer() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("add-mut-constraint")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Add #[account(mut)] to `user`"))
        .expect("expected payer mut quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text.trim(), "#[account(mut)]");
}
#[test]
fn offers_executable_constraint_quickfix_for_unchecked_program_account() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub metadata_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn create(ctx: Context<Create>) -> Result<()> {
        let _cpi_ctx = CpiContext::new(ctx.accounts.metadata_program.to_account_info(), ());
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-security-unchecked-account"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("executable"))
        .expect("expected executable constraint quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text.trim(), "#[account(executable)]");
}
#[test]
fn offers_signer_constraint_quickfix_for_unchecked_authority_account() {
    let source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct Create<'info> {
    pub authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn create(ctx: Context<Create>) -> Result<()> {
        let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-security-signer"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    assert!(actions
        .iter()
        .any(|action| action.title.contains("Signer<'info>")));
    let action = actions
        .iter()
        .find(|action| action.title.contains("signer"))
        .expect("expected signer constraint quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text.trim(), "#[account(signer)]");
}
#[test]
fn offers_generated_sysvar_type_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct CheckSysvarAddress<'info> {
    pub rent: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-security-sysvar"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Sysvar<'info, Rent>"))
        .expect("expected typed sysvar quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Sysvar<'info, Rent>");
}
#[test]
fn offers_invalid_sysvar_generic_quickfix_from_field_name() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, i16>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostic = Diagnostic {
        range: Range {
            start: Position {
                line: 3,
                character: 28,
            },
            end: Position {
                line: 3,
                character: 31,
            },
        },
        severity: None,
        code: Some(NumberOrString::String("anchor-syn".to_string())),
        code_description: None,
        source: Some(diagnostics::SOURCE.to_string()),
        message: "invalid sysvar provided".to_string(),
        related_information: None,
        tags: None,
        data: None,
    };
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &[diagnostic],
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("sysvar `Rent`"))
        .expect("expected invalid sysvar replacement quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Rent");
}
