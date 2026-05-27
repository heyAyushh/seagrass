use super::*;

#[test]
fn offers_derive_accounts_quickfix() {
    let source = r#"
#[program]
mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

pub struct Initialize {}
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
        .any(|action| action.title.contains("derive(Accounts)")));
}
#[test]
fn offers_create_accounts_struct_quickfix() {
    let source = r#"
#[program]
mod demo {
    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> { Ok(()) }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("no matching `#[derive(Accounts)]` struct")
        })
        .expect("expected missing accounts struct diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Create #[derive(Accounts)] struct MakeOffer")
        .expect("expected create accounts struct quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("#[derive(Accounts)]"));
    assert!(text_edit.new_text.contains("pub struct MakeOffer<'info>"));
}
#[test]
fn create_accounts_struct_quickfix_seeds_fields_from_ctx_accounts_usage() {
    let source = r#"
#[program]
mod demo {
    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> {
        require!(ctx.accounts.maker.is_signer, ErrorCode::MissingSigner);
        let counter = &mut ctx.accounts.counter;
        counter.count += 1;
        let _maker = ctx.accounts.maker.key();
        let _rent = ctx.accounts.rent.key();
        let _system = ctx.accounts.system_program.key();
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("no matching `#[derive(Accounts)]` struct")
        })
        .expect("expected missing accounts struct diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Create #[derive(Accounts)] struct MakeOffer")
        .expect("expected create accounts struct quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit
        .new_text
        .contains("pub counter: UncheckedAccount<'info>,"));
    assert!(text_edit.new_text.contains("#[account(mut)]"));
    assert!(text_edit.new_text.contains("pub maker: Signer<'info>,"));
    assert!(text_edit
        .new_text
        .contains("pub rent: Sysvar<'info, Rent>,"));
    assert!(text_edit
        .new_text
        .contains("pub system_program: Program<'info, System>,"));
}
#[test]
fn create_accounts_struct_quickfix_marks_inferred_cpi_program_executable() {
    let source = r#"
#[program]
mod demo {
    pub fn call_external(ctx: Context<CallExternal>) -> Result<()> {
        let _cpi_ctx = CpiContext::new(ctx.accounts.external.to_account_info(), ());
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("no matching `#[derive(Accounts)]` struct")
        })
        .expect("expected missing accounts struct diagnostic");
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Create #[derive(Accounts)] struct CallExternal")
        .expect("expected create accounts struct quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("#[account(executable)]"));
    assert!(text_edit
        .new_text
        .contains("pub external: UncheckedAccount<'info>,"));
}
#[test]
fn offers_empty_context_type_quickfix() {
    let source = r#"
#[program]
mod demo {
    pub fn make_offer(context: Context<>, id: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("empty `Context<>`"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Context<MakeOffer>"))
        .expect("expected Context<MakeOffer> quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "MakeOffer");
}
#[test]
fn offers_empty_context_type_and_stub_quickfix_when_struct_is_missing() {
    let source = r#"
#[program]
mod demo {
    pub fn make_offer(context: Context<>, id: u64) -> Result<()> { Ok(()) }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("empty `Context<>`"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title == "Use `Context<MakeOffer>` and create Accounts struct")
        .expect("expected combined Context and Accounts struct quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let edits = changes.values().next().unwrap();
    assert!(edits
        .iter()
        .any(|edit| edit.range == diagnostic.range && edit.new_text == "MakeOffer"));
    assert!(edits.iter().any(|edit| {
        edit.new_text.contains("#[derive(Accounts)]")
            && edit.new_text.contains("pub struct MakeOffer<'info>")
    }));
}
