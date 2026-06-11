use {super::*, tower_lsp::lsp_types::NumberOrString};

mod workspace_feature_tests;

fn collect(
    document: &ParsedDocument,
    manifest_uri: &Url,
    manifest_text: &str,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    super::collect(document, manifest_uri, manifest_text, None)
}

#[test]
fn reports_accounts_derive_when_manifest_lacks_anchor_debug() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    pub user: Signer<'info>,
}
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
default = []

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(matches!(
        diagnostics[0].code.as_ref(),
        Some(NumberOrString::String(code)) if code == "anchor-check-cfg"
    ));
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|value| value.as_str()),
        Some(ADD_ANCHOR_DEBUG_FEATURE_QUICKFIX)
    );
}

#[test]
fn reports_program_attribute_when_manifest_lacks_anchor_debug() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
default = []

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("#[program]"));
    assert_eq!(diagnostics[0].range.start.line, 1);
}

#[test]
fn reports_each_anchor_macro_site_when_manifest_lacks_anchor_debug() {
    let document = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {}

#[derive(Accounts)]
pub struct Initialize<'info> {}
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
default = []

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("#[program]"));
    assert!(diagnostics[1].message.contains("derives `Accounts`"));
}

#[test]
fn ignores_manifest_that_declares_anchor_debug() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Initialize<'info> {}
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
anchor-debug = []

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn reports_solana_target_os_check_cfg_when_manifest_lacks_lint() {
    let document = ParsedDocument::parse(
        r#"
declare_id!("Demo111111111111111111111111111111111111");
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
anchor-debug = []
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("target_os"));
    assert!(diagnostics[0]
        .message
        .contains("add `cfg(target_os, values(\"solana\"))`"));
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|value| value.as_str()),
        Some(ADD_SOLANA_TARGET_OS_CHECK_CFG_QUICKFIX)
    );
}

#[test]
fn accepts_workspace_solana_target_os_check_cfg_lint() {
    let document = ParsedDocument::parse(
        r#"
declare_id!("Demo111111111111111111111111111111111111");
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[workspace]

[workspace.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn accepts_package_solana_target_os_check_cfg_lint() {
    let document = ParsedDocument::parse(
        r#"
declare_id!("Demo111111111111111111111111111111111111");
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[package]
name = "demo"

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn reports_init_if_needed_when_anchor_lang_feature_is_missing() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
anchor-debug = []

[dependencies]
anchor-lang = "1.0.0"

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("init-if-needed"));
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|value| value.as_str()),
        Some(ADD_INIT_IF_NEEDED_FEATURE_QUICKFIX)
    );
}

#[test]
fn accepts_init_if_needed_when_anchor_lang_feature_is_present() {
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer)]
    pub state: Account<'info, State>,
}
"#,
    )
    .unwrap();
    let diagnostics = collect(
        &document,
        &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
        r#"
[features]
anchor-debug = []

[dependencies]
anchor-lang = { version = "1.0.0", features = ["init-if-needed"] }

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn inserts_anchor_debug_under_existing_features_header() {
    let edit = anchor_debug_feature_edit(
        r#"
[features]
default = []

[dependencies]
anchor-lang = "0.31"
"#,
    )
    .unwrap();

    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.new_text, "anchor-debug = []\n");
}

#[test]
fn appends_features_section_when_missing() {
    let edit = anchor_debug_feature_edit("[package]\nname = \"demo\"\n").unwrap();

    assert_eq!(edit.range.start.line, 2);
    assert_eq!(edit.range.start.character, 0);
    assert_eq!(edit.new_text, "\n[features]\nanchor-debug = []\n");
}

#[test]
fn appends_workspace_solana_target_os_check_cfg_lint() {
    let edit = solana_target_os_check_cfg_edit("[workspace]\nmembers = []\n").unwrap();

    assert!(edit
        .new_text
        .contains("[workspace.lints.rust.unexpected_cfgs]"));
    assert!(edit
        .new_text
        .contains("'cfg(target_os, values(\"solana\"))'"));
}

#[test]
fn appends_package_solana_target_os_check_cfg_lint() {
    let edit = solana_target_os_check_cfg_edit("[package]\nname = \"demo\"\n").unwrap();

    assert!(edit
        .new_text
        .contains("[package.lints.rust.unexpected_cfgs]"));
    assert!(edit
        .new_text
        .contains("'cfg(target_os, values(\"solana\"))'"));
}

#[test]
fn extends_existing_solana_check_cfg_lint_array() {
    let edit = solana_target_os_check_cfg_edit(
        r#"
[workspace.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(feature, values("anchor-debug"))',
]
"#,
    )
    .unwrap();

    assert_eq!(edit.range.start.line, 5);
    assert_eq!(edit.new_text, "    'cfg(target_os, values(\"solana\"))',\n");
}

#[test]
fn skips_solana_target_os_check_cfg_edit_when_lint_exists() {
    let edit = solana_target_os_check_cfg_edit(
        r#"
[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
    );

    assert!(edit.is_none());
}

#[test]
fn converts_anchor_lang_version_to_feature_table() {
    let edit = anchor_lang_feature_edit(
        r#"[dependencies]
anchor-lang = "1.0.0"
"#,
        "init-if-needed",
    )
    .unwrap();

    assert_eq!(edit.range.start.line, 1);
    assert_eq!(
        edit.new_text,
        "anchor-lang = { version = \"1.0.0\", features = [\"init-if-needed\"] }\n"
    );
}

#[test]
fn inserts_anchor_lang_feature_into_inline_table() {
    let edit = anchor_lang_feature_edit(
        r#"[dependencies]
anchor-lang = { version = "1.0.0", features = ["idl-build"] }
"#,
        "init-if-needed",
    )
    .unwrap();

    assert_eq!(
        edit.new_text,
        "anchor-lang = { version = \"1.0.0\", features = [\"idl-build\", \"init-if-needed\"] }\n"
    );
}
