use {super::collect, crate::document::ParsedDocument, proptest::prelude::*};

prop_compose! {
    fn generated_ident()(tail in "[a-z][a-z0-9_]{1,8}") -> String {
        format!("sg_{tail}")
    }
}

#[test]
fn accepts_if_let_pattern_binding_in_then_block() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let maybe_bundle = Some(ctx.accounts.position_bundle.key());
    if let Some(position_bundle) = maybe_bundle {
        position_bundle;
    }
    Ok(())
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("`position_bundle` does not resolve")),
        "if-let binding should be in scope for then block: {diagnostics:#?}"
    );
}

#[test]
fn reports_unresolved_if_let_scrutinee_before_declaring_pattern() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    if let Some(position_bundle) = missing_bundle {
        position_bundle;
    }
    Ok(())
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`missing_bundle` does not resolve")),
        "if-let scrutinee should still resolve before pattern binding: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("`position_bundle` does not resolve")),
        "if-let pattern binding should resolve inside then block: {diagnostics:#?}"
    );
}

#[test]
fn accepts_while_let_and_match_arm_pattern_bindings() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let mut keys = vec![ctx.accounts.position_bundle.key()];
    while let Some(position_bundle) = keys.pop() {
        position_bundle;
    }
    let maybe_bundle = Some(1u64);
    match maybe_bundle {
        Some(position_bundle) if position_bundle > 0 => position_bundle,
        _ => 0,
    };
    Ok(())
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`position_bundle` does not resolve")
        }),
        "while-let and match arm pattern bindings should resolve: {diagnostics:#?}"
    );
}

#[test]
fn accepts_block_item_values_declared_after_use() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
    require!(bundle_index < LOCAL_LIMIT, ErrorCode::BadBundle);
    local_helper();

    const LOCAL_LIMIT: u16 = 64;
    fn local_helper() {}

    Ok(())
}

pub enum ErrorCode {
    BadBundle,
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`LOCAL_LIMIT` does not resolve")
                && !diagnostic
                    .message
                    .contains("`local_helper` does not resolve")
        }),
        "block item values should resolve throughout their block: {diagnostics:#?}"
    );
}

proptest! {
    #[test]
    fn resolves_generated_if_let_and_match_pattern_bindings(
        if_binding in generated_ident(),
        match_binding in generated_ident(),
        local in generated_ident(),
    ) {
        prop_assume!(if_binding != match_binding && if_binding != local && match_binding != local);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {{
    let {local} = Some(ctx.accounts.position_bundle.key());
    if let Some({if_binding}) = {local} {{
        {if_binding};
    }}
    match {local} {{
        Some({match_binding}) => {match_binding},
        _ => ctx.accounts.position_bundle.key(),
    }};
    Ok(())
}}
"#
        );

        let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

        prop_assert!(
            diagnostics.iter().all(|diagnostic| {
                !diagnostic.message.contains(&format!("`{if_binding}` does not resolve"))
                    && !diagnostic.message.contains(&format!("`{match_binding}` does not resolve"))
                    && !diagnostic.message.contains(&format!("`{local}` does not resolve"))
            }),
            "generated pattern bindings should resolve: {diagnostics:#?}"
        );
    }
}
