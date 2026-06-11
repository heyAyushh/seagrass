use super::*;

#[test]
fn offers_anchor_debug_feature_quickfix_for_manifest() {
    let root = std::env::temp_dir().join(format!("seagrass-action-test-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("Cargo.toml");
    std::fs::write(&manifest_path, "[features]\ndefault = []\n").unwrap();
    let manifest_uri = Url::from_file_path(&manifest_path).unwrap();
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Initialize<'info> {}
"#,
    )
    .unwrap();
    let diagnostics =
        crate::diagnostics::check_cfg::collect(&document, &manifest_uri, "[features]\n", None);
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("anchor-debug"))
        .expect("expected anchor-debug quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.get(&manifest_uri).unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "anchor-debug = []\n");

    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn offers_init_if_needed_feature_quickfix_for_manifest() {
    let root = std::env::temp_dir().join(format!(
        "seagrass-init-needed-action-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("Cargo.toml");
    std::fs::write(
        &manifest_path,
        "[features]\nanchor-debug = []\n\n[dependencies]\nanchor-lang = \"1.0.0\"\n",
    )
    .unwrap();
    let manifest_uri = Url::from_file_path(&manifest_path).unwrap();
    let document = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#,
    )
    .unwrap();
    let diagnostics = crate::diagnostics::check_cfg::collect(
        &document,
        &manifest_uri,
        &std::fs::read_to_string(&manifest_path).unwrap(),
        None,
    );
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("init-if-needed"))
        .expect("expected init-if-needed quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.get(&manifest_uri).unwrap().first().unwrap();
    assert!(text_edit
        .new_text
        .contains("features = [\"init-if-needed\"]"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn offers_solana_target_os_check_cfg_quickfix_for_manifest() {
    let root = std::env::temp_dir().join(format!(
        "seagrass-solana-cfg-action-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("Cargo.toml");
    std::fs::write(&manifest_path, "[workspace]\nmembers = []\n").unwrap();
    let manifest_uri = Url::from_file_path(&manifest_path).unwrap();
    let document = ParsedDocument::parse(
        r#"
declare_id!("Demo111111111111111111111111111111111111");
"#,
    )
    .unwrap();
    let diagnostics = crate::diagnostics::check_cfg::collect(
        &document,
        &manifest_uri,
        &std::fs::read_to_string(&manifest_path).unwrap(),
        None,
    );
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostics[0].range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("target_os"))
        .expect("expected target_os quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.get(&manifest_uri).unwrap().first().unwrap();
    assert!(text_edit
        .new_text
        .contains("[workspace.lints.rust.unexpected_cfgs]"));
    assert!(text_edit
        .new_text
        .contains("'cfg(target_os, values(\"solana\"))'"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn offers_sync_declare_id_quickfix() {
    let source = r#"declare_id!("Declared111111111111111111111111111111111");"#;
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::parse("file:///tmp/demo/programs/my-program/src/lib.rs").unwrap();
    let diagnostics = crate::diagnostics::project_identity::collect(
        &document,
        &uri,
        &Url::parse("file:///tmp/demo/Anchor.toml").unwrap(),
        r#"
[programs.localnet]
my_program = "Expected111111111111111111111111111111111"
"#,
    );
    let actions = code_actions(&document, uri, diagnostics[0].range, &diagnostics);

    let action = actions
        .iter()
        .find(|action| action.title.contains("declare_id"))
        .expect("expected declare_id sync quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(
        text_edit.new_text,
        "Expected111111111111111111111111111111111"
    );
}
