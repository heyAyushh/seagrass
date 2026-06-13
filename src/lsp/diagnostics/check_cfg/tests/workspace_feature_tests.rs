use {super::*, tower_lsp::lsp_types::Url};

const MANIFEST_URI: &str = "file:///tmp/program/Cargo.toml";
const INIT_IF_NEEDED_SOURCE: &str = r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
const WORKSPACE_INHERITED_MANIFEST: &str = concat!(
    r#"
[features]
anchor-debug = []

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    r#"
[dependencies]
anchor-lang.workspace = true
"#
);
const DIRECT_FEATURE_MANIFEST: &str = concat!(
    r#"
[features]
anchor-debug = []

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    r#"
[dependencies]
anchor-lang = { version = "0.30.1", features = ["init-if-needed"] }
"#
);

#[test]
fn workspace_inherited_feature_suppresses_diagnostic() {
    let diagnostics = diagnostics_for_manifests(
        WORKSPACE_INHERITED_MANIFEST,
        Some(
            r#"
[workspace]

[workspace.dependencies]
anchor-lang = { version = "0.30.1", features = ["init-if-needed"] }
"#,
        ),
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn workspace_inherited_feature_missing_fires_diagnostic() {
    let diagnostics = diagnostics_for_manifests(
        WORKSPACE_INHERITED_MANIFEST,
        Some(
            r#"
[workspace]

[workspace.dependencies]
anchor-lang = { version = "0.30.1", features = ["idl-build"] }
"#,
        ),
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("init-if-needed"));
}

#[test]
fn workspace_inherited_feature_without_workspace_text_fires_diagnostic() {
    let diagnostics = diagnostics_for_manifests(WORKSPACE_INHERITED_MANIFEST, None);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("init-if-needed"));
}

#[test]
fn direct_feature_still_suppresses_diagnostic() {
    let diagnostics = diagnostics_for_manifests(DIRECT_FEATURE_MANIFEST, None);

    assert!(diagnostics.is_empty());
}

#[test]
fn fixture_workspace_manifest_suppresses_diagnostic() {
    let diagnostics = diagnostics_for_source_and_manifests(
        include_str!("../../../../../tests/fixtures/resolution_fp/program/src/lib.rs"),
        include_str!("../../../../../tests/fixtures/resolution_fp/program/Cargo.toml"),
        Some(include_str!(
            "../../../../../tests/fixtures/resolution_fp/Cargo.toml"
        )),
    );

    assert!(diagnostics.is_empty());
}

fn diagnostics_for_manifests(
    manifest_text: &str,
    workspace_manifest_text: Option<&str>,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    diagnostics_for_source_and_manifests(
        INIT_IF_NEEDED_SOURCE,
        manifest_text,
        workspace_manifest_text,
    )
}

fn diagnostics_for_source_and_manifests(
    source: &str,
    manifest_text: &str,
    workspace_manifest_text: Option<&str>,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let document = ParsedDocument::parse(source).unwrap();
    let manifest_uri = Url::parse(MANIFEST_URI).unwrap();

    super::super::collect(
        &document,
        &manifest_uri,
        manifest_text,
        workspace_manifest_text,
    )
}
