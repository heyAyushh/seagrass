use {super::collect_with_workspace, crate::document::ParsedDocument};

/// Impl-method parameters used within the body must not be flagged as
/// unknown members — the method's own typed inputs must be in scope.
#[test]
fn accepts_typed_param_member_access_in_impl_method() {
    let document = ParsedDocument::parse(
        r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub struct BundleManager;

impl BundleManager {
    pub fn process(&self, bundle: PositionBundle) -> Pubkey {
        bundle.position_bundle_mint
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .all(|d| !d.message.contains("position_bundle_mint")),
        "impl-method typed param member access should not be flagged: {diagnostics:#?}"
    );
}

/// A genuinely nonexistent field accessed inside an impl method must still
/// produce a diagnostic — true positives must be preserved.
#[test]
fn reports_unknown_member_in_impl_method_body() {
    let document = ParsedDocument::parse(
        r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub struct BundleManager;

impl BundleManager {
    pub fn process(&self, bundle: PositionBundle) -> Pubkey {
        bundle.nonexistent_field
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("nonexistent_field")),
        "unknown field inside impl method should be flagged: {diagnostics:#?}"
    );
}

/// Context-typed parameters in impl methods must be correctly recognised so
/// that context member accesses are validated (not silently ignored).
#[test]
fn accepts_context_param_accounts_access_in_impl_method() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub struct Processor;

impl Processor {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let _mint = ctx.accounts.position_bundle.position_bundle_mint;
        Ok(())
    }
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics
            .iter()
            .all(|d| !d.message.contains("position_bundle_mint")),
        "known context account field in impl method should not be flagged: {diagnostics:#?}"
    );
}
