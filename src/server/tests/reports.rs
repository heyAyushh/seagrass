use super::*;

#[test]
fn analysis_uri_from_args_accepts_string_and_object_arguments() {
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();

    assert_eq!(
        analysis_uri_from_args(&[serde_json::json!(uri.as_str())]),
        Some(uri.clone())
    );
    assert_eq!(
        analysis_uri_from_args(&[serde_json::json!({ "uri": uri.as_str() })]),
        Some(uri)
    );
    assert!(analysis_uri_from_args(&[serde_json::json!({ "path": "/tmp/lib.rs" })]).is_none());
    assert_eq!(
        analysis_instruction_from_args(&[serde_json::json!({
            "uri": "file:///workspace/programs/demo/src/lib.rs",
            "instruction": "create"
        })]),
        Some("create")
    );
    assert_eq!(
        analysis_instruction_from_args(&[serde_json::json!({
            "uri": "file:///workspace/programs/demo/src/lib.rs",
            "function": "initialize"
        })]),
        Some("initialize")
    );
    assert_eq!(
        analysis_context_from_args(&[serde_json::json!({
            "uri": "file:///workspace/programs/demo/src/lib.rs",
            "context": "Create"
        })]),
        Some("Create")
    );
}

#[test]
fn instruction_summary_exports_agent_facing_shape() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64) -> Result<()> {
        let _payer = ctx.accounts.payer.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}
"#,
    )
    .unwrap();

    let summary = instruction_summary(&document, &[], "initialize", None);

    assert_eq!(summary["name"], "initialize");
    assert_eq!(summary["context"], "Create");
    assert_eq!(summary["args"][0]["name"], "amount");
    assert!(
        summary["accounts"]
            .as_array()
            .is_some_and(|accounts| accounts.iter().any(|account| {
                account["name"] == "payer"
                    && account["ty"] == "Signer"
                    && account["mutability"] == true
                    && account["signer"] == true
            })),
        "{summary}"
    );
    assert_eq!(summary["errorsReturned"], serde_json::json!([]));
}

#[test]
fn analysis_report_exposes_agent_friendly_symbols_evidence_and_diagnostics() {
    let source = r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod demo {
    use super::*;

    pub fn create(ctx: Context<Create>) -> Result<()> {
        ctx.accounts.missing.key();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
}
"#;
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let report = analysis_report(
        &uri,
        &document,
        &diagnostics,
        None,
        Some("create"),
        None,
        None,
    );

    assert_eq!(report["uri"], uri.as_str());
    assert_eq!(report["project"], serde_json::Value::Null);
    assert_eq!(report["focus"]["kind"], "anchor.programInstruction");
    assert_eq!(report["focus"]["found"], true);
    assert_eq!(report["focus"]["instruction"], "create");
    assert_eq!(report["focus"]["context"]["name"], "Create");
    assert!(report["focus"]["contextFields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| {
            field["name"] == "user"
                && field["type"] == "Signer"
                && field["source"] == "localDocument"
        }));
    assert!(report["focus"]["accountUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| usage["name"] == "missing"));
    assert!(report["focus"]["resolvedAccountUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| usage["name"] == "missing" && usage["declared"] == false));
    assert!(report["focus"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "anchor-missing-account-reference"));
    assert_eq!(
        report["anchorSupport"]["generatorProfile"]["generationMode"],
        "source-driven"
    );
    assert!(report["evidence"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instruction| instruction["name"] == "create" && instruction["context"] == "Create"));
    assert!(report["documentSymbols"]
        .as_array()
        .unwrap()
        .iter()
        .any(|symbol| symbol["name"] == "demo"));
    assert!(report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["code"] == "anchor-missing-account-reference"));

    let context_report = analysis_report(
        &uri,
        &document,
        &diagnostics,
        None,
        None,
        Some("Create"),
        None,
    );
    assert_eq!(context_report["focus"]["kind"], "anchor.accountsContext");
    assert_eq!(context_report["focus"]["found"], true);
    assert_eq!(context_report["focus"]["context"], "Create");
    assert!(context_report["focus"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instruction| instruction["name"] == "create"));
    assert!(context_report["focus"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["name"] == "user" && field["type"] == "Signer"));
}

#[test]
fn accounts_context_focus_uses_workspace_instructions_for_split_files() {
    let accounts_uri = Url::parse("file:///workspace/programs/demo/src/accounts.rs").unwrap();
    let handler_uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let handler_source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        let user = ctx.accounts.user.key();
        let inner = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(accounts_source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let workspace_index = workspace::WorkspaceIndex::build(
        &[],
        [
            (accounts_uri.clone(), accounts_source.to_string()),
            (handler_uri.clone(), handler_source.to_string()),
        ],
    );
    let report = analysis_report(
        &accounts_uri,
        &document,
        &diagnostics,
        None,
        None,
        Some("Create"),
        Some(&workspace_index),
    );

    assert_eq!(report["focus"]["kind"], "anchor.accountsContext");
    assert_eq!(report["focus"]["context"], "Create");
    assert!(report["focus"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|instruction| {
            instruction["name"] == "initialize" && instruction["uri"] == handler_uri.as_str()
        }));
}

#[test]
fn instruction_focus_uses_workspace_context_fields_for_split_files() {
    let accounts_uri = Url::parse("file:///workspace/programs/demo/src/accounts.rs").unwrap();
    let handler_uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let handler_source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        let user = ctx.accounts.user.key();
        let inner = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(handler_source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let workspace_index = workspace::WorkspaceIndex::build(
        &[],
        [
            (accounts_uri, accounts_source.to_string()),
            (handler_uri.clone(), handler_source.to_string()),
        ],
    );
    let report = analysis_report(
        &handler_uri,
        &document,
        &diagnostics,
        None,
        Some("initialize"),
        None,
        Some(&workspace_index),
    );

    assert_eq!(report["focus"]["kind"], "anchor.programInstruction");
    assert_eq!(report["focus"]["instruction"], "initialize");
    assert!(report["focus"]["contextFields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| {
            field["name"] == "user"
                && field["type"] == "Signer"
                && field["source"] == "workspaceIndex"
        }));
    assert!(report["focus"]["resolvedAccountUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| {
            usage["name"] == "user"
                && usage["declared"] == true
                && usage["type"] == "Signer"
                && usage["source"] == "workspaceIndex"
        }));
    assert!(report["focus"]["resolvedAccountPathUsages"]
        .as_array()
        .unwrap()
        .iter()
        .any(|usage| {
            usage["path"] == "wrapper.inner"
                && usage["segments"].as_array().unwrap().iter().any(|segment| {
                    segment["name"] == "inner"
                        && segment["declared"] == true
                        && segment["container"] == "Wrapped"
                        && segment["type"] == "Account<Inner>"
                        && segment["source"] == "workspaceIndex"
                })
        }));
}
