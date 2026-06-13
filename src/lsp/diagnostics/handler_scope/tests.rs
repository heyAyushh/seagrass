use {
    super::collect,
    crate::{diagnostics::registry::ANCHOR_ACCOUNT_USAGE_CODE, document::ParsedDocument},
    proptest::prelude::*,
    tower_lsp::lsp_types::NumberOrString,
};

prop_compose! {
    fn generated_ident()(tail in "[a-z0-9_]{1,10}") -> String {
        format!("sg_{tail}")
    }
}

#[test]
fn reports_unresolved_handler_identifier() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundle = position_bundle = sd;
        Ok(())
    }
}
"#,
        )
        .unwrap(),
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`sd` does not resolve"))
        .unwrap_or_else(|| panic!("missing unresolved identifier diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_ACCOUNT_USAGE_CODE.to_string()
        ))
    );
    assert!(diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .is_some_and(|candidates| candidates
            .iter()
            .any(|candidate| candidate.as_str() == Some("position_bundle"))));
}

#[test]
fn accepts_arguments_locals_and_imported_helpers() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use crate::util::verify_position_bundle_authority;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        let copied_index = bundle_index;
        verify_position_bundle_authority(copied_index)?;
        let nested = |value| {
            let local = value;
            local
        };
        nested(copied_index);
        Ok(())
    }
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.is_empty(),
        "bound handler identifiers should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn reports_unresolved_identifier_in_context_helper() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle = missing_value;
    Ok(())
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`missing_value` does not resolve")),
        "missing context helper identifier diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unresolved_handler_call_identifier() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let copied_index = bundle_index;
        verify_bundel(copied_index);
        Ok(())
    }
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`verify_bundel` does not resolve")),
        "missing unresolved handler call diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn accepts_imported_and_local_handler_call_identifiers() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use crate::util::verify_position_bundle_authority;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let local_helper = |value| value;
        let copied_index = local_helper(bundle_index);
        verify_position_bundle_authority(copied_index)?;
        Ok(())
    }
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.is_empty(),
        "resolved handler calls should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn ignores_non_anchor_rust_functions() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close() -> Result<()> {
    missing_value;
    Ok(())
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.is_empty(),
        "non-Anchor helper should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn accepts_impl_method_parameters_on_accounts_struct() {
    // Regression: helper methods on an `#[derive(Accounts)]` struct declare
    // their own parameters, which must not be reported as unresolved.
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: Signer<'info>,
}

impl<'info> Make<'info> {
    fn populate_escrow(&mut self, seed: u64, amount: u64, bump: u8) -> Result<()> {
        self.escrow.set_inner(Escrow {
            seed,
            receive: amount,
            bump,
        });
        Ok(())
    }
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.is_empty(),
        "impl-method parameters should resolve in their own scope: {diagnostics:#?}"
    );
}

#[test]
fn reports_unresolved_identifier_in_impl_method() {
    // True positives inside impl methods must still fire even though the
    // method's own parameters are now scoped.
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: Signer<'info>,
}

impl<'info> Make<'info> {
    fn populate_escrow(&mut self, seed: u64) -> Result<()> {
        let scoped = seed;
        scoped = missing_value;
        Ok(())
    }
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`missing_value` does not resolve")),
        "unresolved identifier in impl method should still be reported: {diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`seed` does not resolve")),
        "impl-method parameter should stay quiet: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn reports_generated_unresolved_handler_identifiers(
        missing in generated_ident(),
        local in generated_ident(),
        argument in generated_ident(),
    ) {
        prop_assume!(missing != local && missing != argument);
        prop_assume!(local != "ctx" && argument != "ctx" && missing != "ctx");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {{
    pub fn close(ctx: Context<Close>, {argument}: u16) -> Result<()> {{
        let {local} = {argument};
        {local};
        {missing};
        Ok(())
    }}
}}
"#
        );

        let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

        prop_assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!("`{missing}` does not resolve"))),
            "expected generated unresolved handler identifier diagnostic, got {diagnostics:#?}"
        );
        prop_assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(&format!("`{local}` does not resolve"))
                    && !diagnostic.message.contains(&format!("`{argument}` does not resolve"))),
            "bound generated identifiers should stay quiet: {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unresolved_handler_call_identifiers(
        missing in generated_ident(),
        local in generated_ident(),
        argument in generated_ident(),
    ) {
        prop_assume!(missing != local && missing != argument);
        prop_assume!(local != "ctx" && argument != "ctx" && missing != "ctx");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {{
    pub fn close(ctx: Context<Close>, {argument}: u16) -> Result<()> {{
        let {local} = {argument};
        {missing}({local});
        Ok(())
    }}
}}
"#
        );

        let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

        prop_assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&format!("`{missing}` does not resolve"))),
            "expected generated unresolved handler call diagnostic, got {diagnostics:#?}"
        );
        prop_assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(&format!("`{local}` does not resolve"))
                    && !diagnostic.message.contains(&format!("`{argument}` does not resolve"))),
            "bound generated identifiers should stay quiet: {diagnostics:#?}"
        );
    }

    #[test]
    fn recovers_generated_parse_error_identifiers(
        missing in generated_ident(),
        local in generated_ident(),
        argument in generated_ident(),
    ) {
        prop_assume!(missing != local && missing != argument);
        prop_assume!(local != "ctx" && argument != "ctx" && missing != "ctx");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, {argument}: u16) -> Result<()> {{
    let {local} = {argument};
    {local} = {local}  {missing} ;
    Ok(())
}}
"#
        );

        let err = syn::parse_file(&source).unwrap_err();
        let diagnostic =
            crate::diagnostics::diagnostic_from_parse_error_with_source(err, &source);

        prop_assert!(
            diagnostic.message.contains(&format!("`{missing}` does not resolve")),
            "expected parse-error semantic recovery for `{missing}`, got {diagnostic:#?}"
        );
        prop_assert_eq!(
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|reason| reason.as_str()),
            Some("unresolved-handler-identifier")
        );
    }
}
