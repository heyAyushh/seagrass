use super::*;

#[test]
fn offers_missing_reference_replacement() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = usr, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );

    assert!(actions.iter().any(|action| action.title.contains("`user`")));
}
#[test]
fn offers_add_missing_ctx_account_field_quickfix() {
    let source = r#"
#[program]
mod demo {
    pub fn create(ctx: Context<Create>) -> Result<()> {
        require!(ctx.accounts.maker.is_signer, ErrorCode::MissingSigner);
        let vault = &mut ctx.accounts.vault;
        vault.lamports();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let maker_diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("account"))
                    .and_then(|value| value.as_str())
                    == Some("maker")
        })
        .expect("expected missing maker diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        maker_diagnostic.range,
        &diagnostics,
    );

    let maker_action = actions
        .iter()
        .find(|action| action.title == "Add `maker` to `Create`")
        .expect("expected add maker quickfix");
    let maker_edit = maker_action
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
    assert_eq!(maker_edit.new_text, "    pub maker: Signer<'info>,\n");

    let vault_action = actions
        .iter()
        .find(|action| action.title == "Add `vault` to `Create`")
        .expect("expected add vault quickfix");
    let vault_edit = vault_action
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
        vault_edit.new_text,
        "    #[account(mut)]\n    pub vault: UncheckedAccount<'info>,\n"
    );
}
#[test]
fn offers_add_missing_constraint_account_field_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("constraint"))
                    .and_then(|value| value.as_str())
                    == Some("payer")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("account"))
                    .and_then(|value| value.as_str())
                    == Some("user")
        })
        .expect("expected missing payer account diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        std::slice::from_ref(diagnostic),
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Add `user` to `Create`")
        .expect("expected add user quickfix");
    let edit = action
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
        edit.new_text,
        "    #[account(mut)]\n    pub user: Signer<'info>,\n"
    );
}
#[test]
fn offers_missing_reference_replacement_for_every_generated_reference_constraint() {
    for (key, value_kind) in constraint_catalog::account_reference_specs() {
        let scenario = missing_reference_action_scenario(key, value_kind);
        let source = generated_missing_reference_source(key, scenario.missing);
        let document = ParsedDocument::parse(&source).unwrap();
        let diagnostics = crate::diagnostics::collect(&document);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
                    && diagnostic
                        .data
                        .as_ref()
                        .and_then(|data| data.get("constraint"))
                        .and_then(|value| value.as_str())
                        == Some(key)
            })
            .unwrap_or_else(|| {
                panic!("expected missing-reference diagnostic for generated constraint `{key}`")
            });
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/lib.rs").unwrap(),
            diagnostic.range,
            std::slice::from_ref(diagnostic),
        );

        assert!(
            actions.iter().any(|action| {
                action
                    .title
                    .contains(&format!("`{}`", scenario.replacement))
            }),
            "generated constraint `{key}` did not offer replacement `{}`; got {:?}",
            scenario.replacement,
            actions
                .iter()
                .map(|action| action.title.as_str())
                .collect::<Vec<_>>()
        );
    }
}
#[test]
fn offers_nested_missing_account_reference_replacement() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.iner.key();
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
    let diagnostics = crate::diagnostics::collect(&document);
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );

    assert!(actions
        .iter()
        .any(|action| action.title.contains("Replace `iner` with `inner`")));
}
#[test]
fn offers_workspace_nested_missing_account_reference_replacement_from_candidates() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.iner.key();
        Ok(())
    }
}

"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = vec![Diagnostic {
        range: Range {
            start: Position {
                line: 4,
                character: 39,
            },
            end: Position {
                line: 4,
                character: 43,
            },
        },
        severity: None,
        code: Some(NumberOrString::String(
            "anchor-missing-account-reference".to_string(),
        )),
        code_description: None,
        source: Some("seagrass".to_string()),
        message:
            "`iner` is used through `ctx.accounts` in `read` but is not declared in `Wrapped`."
                .to_string(),
        related_information: None,
        tags: None,
        data: Some(serde_json::json!({
            "account": "iner",
            "accountsStruct": "Wrapped",
            "instruction": "read",
            "reason": "unknown-ctx-account-field",
            "candidates": ["inner"],
        })),
    }];
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );

    assert!(actions
        .iter()
        .any(|action| action.title.contains("Replace `iner` with `inner`")));
}

#[test]
fn offers_typed_handler_member_replacement_from_candidates() {
    let source = r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        bundle.position_bundle_mnit;
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("unknown-handler-member")
        })
        .unwrap_or_else(|| panic!("missing typed handler member diagnostic: {diagnostics:#?}"));
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        std::slice::from_ref(diagnostic),
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Replace `position_bundle_mnit` with `position_bundle_mint`")
        .unwrap_or_else(|| panic!("missing handler member replacement action: {actions:#?}"));

    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("anchorAction"))
            .and_then(|value| value.as_str()),
        Some("replace-handler-member")
    );
}

#[test]
fn offers_remove_handler_field_call_quickfix() {
    let source = r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        bundle.position_bundle_mint();
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("field-called-as-method")
        })
        .unwrap_or_else(|| panic!("missing field-called-as-method diagnostic: {diagnostics:#?}"));
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        std::slice::from_ref(diagnostic),
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Use `position_bundle_mint` as a field")
        .unwrap_or_else(|| panic!("missing remove field-call action: {actions:#?}"));
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.values().next())
        .and_then(|edits| edits.first())
        .expect("expected remove call edit");

    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("anchorAction"))
            .and_then(|value| value.as_str()),
        Some("remove-handler-field-call")
    );
    let updated = apply_text_edit(source, edit);
    assert!(!updated.contains("position_bundle_mint();"));
    assert!(updated.contains("bundle.position_bundle_mint;"));
}

#[test]
fn offers_unresolved_handler_identifier_replacement_from_scope() {
    let source = r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundel;
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-account-usage")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("reason"))
                    .and_then(|value| value.as_str())
                    == Some("unresolved-handler-identifier")
        })
        .unwrap_or_else(|| panic!("missing unresolved handler identifier: {diagnostics:#?}"));
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        std::slice::from_ref(diagnostic),
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Replace `position_bundel` with `position_bundle`")
        .unwrap_or_else(|| panic!("missing handler identifier replacement action: {actions:#?}"));

    assert_eq!(
        action
            .data
            .as_ref()
            .and_then(|data| data.get("anchorAction"))
            .and_then(|value| value.as_str()),
        Some("replace-handler-identifier")
    );
}
