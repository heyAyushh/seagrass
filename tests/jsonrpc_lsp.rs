use {
    serde_json::{json, Value},
    std::{
        fs::{create_dir_all, remove_dir_all, write},
        io::{Read, Write},
        path::{Path, PathBuf},
        process::{Child, ChildStdin, Command, Stdio},
        sync::mpsc::{self, Receiver},
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    },
};

const LSP_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const SERVER_EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const FORMAT_SOURCE: &str = "pub fn formatting_smoke(){let value=1;}\n";
const SUPPRESSED_ANCHOR_SOURCE: &str = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    // seagrass-ignore
    #[account(init)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
const UNSUPPRESSED_ANCHOR_SOURCE: &str = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
const SUPPRESSED_PARSE_PAUSE_SOURCE: &str = r#"
pub fn handler() -> Result<()> {
    // seagrass-ignore
    position_bundle.(bundle_index)?;
    Ok(())
}
"#;
const PINOCCHIO_SOURCE: &str = r#"
use pinocchio::{account_info::AccountInfo, ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let _state = unsafe { load::<State>(account.borrow_data_unchecked())? };
    Ok(())
}
"#;
const PINOCCHIO_CPI_SOURCE: &str = r#"
use pinocchio::{
    account_info::AccountInfo,
    cpi::invoke,
    instruction::{InstructionAccount, InstructionView},
    pubkey::Pubkey,
    ProgramResult,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let instruction_accounts = [InstructionAccount::writable_signer(account.key())];
    let instruction = InstructionView {
        program_id,
        accounts: &instruction_accounts,
        data: &[],
    };
    invoke(&instruction, &[account])?;
    Ok(())
}
"#;
const NATIVE_SOURCE: &str = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let data = account.try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
const NATIVE_CPI_SOURCE: &str = r#"
use {
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let metas = vec![AccountMeta::new(*account.key, true)];
    let instruction = Instruction {
        program_id: *program_id,
        accounts: metas,
        data: vec![],
    };
    invoke(&instruction, accounts)?;
    Ok(())
}
"#;

#[test]
fn jsonrpc_lsp_formats_and_reports_framework_diagnostics() {
    let workspace = TestWorkspace::new("jsonrpc-lsp");
    let mut client = LspClient::spawn();
    let initialize = client.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": file_uri(workspace.path()),
            "workspaceFolders": [
                { "uri": file_uri(workspace.path()), "name": "jsonrpc-lsp" }
            ],
            "initializationOptions": {
                "seagrass": {
                    "diagnostics": { "transport": "pull" }
                }
            },
            "capabilities": {
                "workspace": { "workspaceFolders": true },
                "textDocument": {
                    "diagnostic": { "dynamicRegistration": false }
                }
            }
        }),
    );

    assert!(
        initialize
            .pointer("/capabilities/documentFormattingProvider")
            .is_some(),
        "document formatting provider was not advertised: {initialize}"
    );
    assert!(
        initialize
            .pointer("/capabilities/diagnosticProvider")
            .is_some(),
        "pull diagnostic provider was not advertised: {initialize}"
    );

    client.notify("initialized", json!({}));

    let formatting_uri = workspace.file_uri("formatting.rs");
    client.open_rust_document(&formatting_uri, FORMAT_SOURCE);
    let formatting_edits = client.request(
        "textDocument/formatting",
        json!({
            "textDocument": { "uri": formatting_uri },
            "options": { "tabSize": 4, "insertSpaces": true }
        }),
    );
    let formatted_text = formatting_edits
        .get(0)
        .and_then(|edit| edit.get("newText"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        formatted_text.contains("pub fn formatting_smoke()")
            && formatted_text.contains("let value = 1;"),
        "formatting response did not include rustfmt output: {formatting_edits}"
    );

    let pinocchio_uri = workspace.file_uri("pinocchio.rs");
    client.open_rust_document(&pinocchio_uri, PINOCCHIO_SOURCE);
    let pinocchio_diagnostics = client.pull_diagnostics(&pinocchio_uri);
    assert_diagnostic_attack(&pinocchio_diagnostics, "owner-checks", Some("pinocchio"));
    assert_diagnostic_attack(&pinocchio_diagnostics, "type-cosplay", Some("pinocchio"));
    let pinocchio_owner_diagnostic =
        diagnostic_with_attack(&pinocchio_diagnostics, "owner-checks", Some("pinocchio"));
    let pinocchio_owner_actions = client.code_actions(&pinocchio_uri, &pinocchio_owner_diagnostic);
    assert_code_action_edit_contains(
        &pinocchio_owner_actions,
        "Insert owner guard",
        ".is_owned_by(&crate::ID)",
    );

    let pinocchio_cpi_uri = workspace.file_uri("pinocchio_cpi.rs");
    client.open_rust_document(&pinocchio_cpi_uri, PINOCCHIO_CPI_SOURCE);
    let pinocchio_cpi_diagnostics = client.pull_diagnostics(&pinocchio_cpi_uri);
    assert_diagnostic_attack(
        &pinocchio_cpi_diagnostics,
        "signer-authorization",
        Some("pinocchio"),
    );
    assert_diagnostic_attack(
        &pinocchio_cpi_diagnostics,
        "arbitrary-cpi",
        Some("pinocchio"),
    );
    assert_diagnostic_attack(
        &pinocchio_cpi_diagnostics,
        "writable-account",
        Some("pinocchio"),
    );
    let pinocchio_signer_diagnostic = diagnostic_with_attack(
        &pinocchio_cpi_diagnostics,
        "signer-authorization",
        Some("pinocchio"),
    );
    let pinocchio_signer_actions =
        client.code_actions(&pinocchio_cpi_uri, &pinocchio_signer_diagnostic);
    assert_code_action_edit_contains(
        &pinocchio_signer_actions,
        "Insert signer guard",
        "!account.is_signer()",
    );
    let pinocchio_writable_diagnostic = diagnostic_with_attack(
        &pinocchio_cpi_diagnostics,
        "writable-account",
        Some("pinocchio"),
    );
    let pinocchio_writable_actions =
        client.code_actions(&pinocchio_cpi_uri, &pinocchio_writable_diagnostic);
    assert_code_action_edit_contains(
        &pinocchio_writable_actions,
        "Insert writable guard",
        "!account.is_writable()",
    );
    let pinocchio_cpi_diagnostic = diagnostic_with_attack(
        &pinocchio_cpi_diagnostics,
        "arbitrary-cpi",
        Some("pinocchio"),
    );
    let pinocchio_cpi_actions = client.code_actions(&pinocchio_cpi_uri, &pinocchio_cpi_diagnostic);
    assert_code_action_edit_contains(
        &pinocchio_cpi_actions,
        "Insert CPI program-id guard",
        "program_id != &crate::ID",
    );

    let native_uri = workspace.file_uri("native.rs");
    client.open_rust_document(&native_uri, NATIVE_SOURCE);
    let native_diagnostics = client.pull_diagnostics(&native_uri);
    assert_diagnostic_attack(&native_diagnostics, "owner-checks", Some("native-solana"));
    assert_diagnostic_attack(&native_diagnostics, "type-cosplay", Some("native-solana"));
    let native_type_diagnostic =
        diagnostic_with_attack(&native_diagnostics, "type-cosplay", Some("native-solana"));
    let native_type_actions = client.code_actions(&native_uri, &native_type_diagnostic);
    assert_code_action_edit_contains(
        &native_type_actions,
        "Insert discriminator guard",
        "State::DISCRIMINATOR.as_ref()",
    );

    let native_cpi_uri = workspace.file_uri("native_cpi.rs");
    client.open_rust_document(&native_cpi_uri, NATIVE_CPI_SOURCE);
    let native_cpi_diagnostics = client.pull_diagnostics(&native_cpi_uri);
    assert_diagnostic_attack(
        &native_cpi_diagnostics,
        "signer-authorization",
        Some("native-solana"),
    );
    assert_diagnostic_attack(
        &native_cpi_diagnostics,
        "arbitrary-cpi",
        Some("native-solana"),
    );
    assert_diagnostic_attack(
        &native_cpi_diagnostics,
        "writable-account",
        Some("native-solana"),
    );
    let native_signer_diagnostic = diagnostic_with_attack(
        &native_cpi_diagnostics,
        "signer-authorization",
        Some("native-solana"),
    );
    let native_signer_actions = client.code_actions(&native_cpi_uri, &native_signer_diagnostic);
    assert_code_action_edit_contains(
        &native_signer_actions,
        "Insert signer guard",
        "!account.is_signer",
    );
    let native_writable_diagnostic = diagnostic_with_attack(
        &native_cpi_diagnostics,
        "writable-account",
        Some("native-solana"),
    );
    let native_writable_actions = client.code_actions(&native_cpi_uri, &native_writable_diagnostic);
    assert_code_action_edit_contains(
        &native_writable_actions,
        "Insert writable guard",
        "!account.is_writable",
    );
    let native_cpi_diagnostic = diagnostic_with_attack(
        &native_cpi_diagnostics,
        "arbitrary-cpi",
        Some("native-solana"),
    );
    let native_cpi_actions = client.code_actions(&native_cpi_uri, &native_cpi_diagnostic);
    assert_code_action_edit_contains(
        &native_cpi_actions,
        "Insert CPI program-id guard",
        "program_id != &crate::ID",
    );

    client.shutdown();
}

#[test]
fn jsonrpc_suppression_applies_to_push_diagnostics() {
    let workspace = TestWorkspace::new("jsonrpc-push-suppression");
    let mut client = LspClient::spawn();
    let initialize = client.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": file_uri(workspace.path()),
            "workspaceFolders": [
                { "uri": file_uri(workspace.path()), "name": "jsonrpc-push-suppression" }
            ],
            "capabilities": {}
        }),
    );
    assert!(
        initialize
            .pointer("/capabilities/diagnosticProvider")
            .is_none(),
        "push transport should not advertise pull diagnostics: {initialize}"
    );
    client.notify("initialized", json!({}));

    let uri = workspace.file_uri("suppressed_push.rs");
    workspace.write_file("suppressed_push.rs", SUPPRESSED_ANCHOR_SOURCE);
    client.open_rust_document(&uri, SUPPRESSED_ANCHOR_SOURCE);
    let published = client.read_notification("textDocument/publishDiagnostics", |message| {
        message.pointer("/params/uri").and_then(Value::as_str) == Some(uri.as_str())
    });

    assert_empty_published_diagnostics(&published);
    client.shutdown();
}

#[test]
fn jsonrpc_suppression_applies_to_pull_parse_pause_and_seagrass_toml() {
    let workspace = TestWorkspace::new("jsonrpc-pull-suppression");
    let mut client = LspClient::spawn();
    client.request(
        "initialize",
        json!({
            "processId": null,
            "rootUri": file_uri(workspace.path()),
            "workspaceFolders": [
                { "uri": file_uri(workspace.path()), "name": "jsonrpc-pull-suppression" }
            ],
            "initializationOptions": {
                "seagrass": {
                    "diagnostics": { "transport": "pull" }
                }
            },
            "capabilities": {
                "textDocument": {
                    "diagnostic": { "dynamicRegistration": false }
                }
            }
        }),
    );
    client.notify("initialized", json!({}));

    let ignored_uri = workspace.file_uri("suppressed_pull.rs");
    workspace.write_file("suppressed_pull.rs", SUPPRESSED_ANCHOR_SOURCE);
    client.open_rust_document(&ignored_uri, SUPPRESSED_ANCHOR_SOURCE);
    assert_empty_pull_diagnostics(&client.pull_diagnostics(&ignored_uri));

    workspace.write_file("suppressed_pull.rs", SUPPRESSED_PARSE_PAUSE_SOURCE);
    client.change_rust_document(&ignored_uri, 2, SUPPRESSED_PARSE_PAUSE_SOURCE);
    assert_empty_pull_diagnostics(&client.pull_diagnostics(&ignored_uri));

    workspace.write_file("suppressed_pull.rs", SUPPRESSED_ANCHOR_SOURCE);
    client.change_rust_document(&ignored_uri, 3, SUPPRESSED_ANCHOR_SOURCE);
    assert_empty_pull_diagnostics(&client.pull_diagnostics(&ignored_uri));

    workspace.write_file(
        "Seagrass.toml",
        r#"
[lints]
allow = [
    "seagrass/anchor.constraint.shape",
    "seagrass/anchor.init.missing-payer",
    "seagrass/anchor.init.missing-space",
]
"#,
    );
    let toml_uri = workspace.file_uri("suppressed_by_toml.rs");
    workspace.write_file("suppressed_by_toml.rs", UNSUPPRESSED_ANCHOR_SOURCE);
    client.open_rust_document(&toml_uri, UNSUPPRESSED_ANCHOR_SOURCE);
    assert_empty_pull_diagnostics(&client.pull_diagnostics(&toml_uri));

    client.shutdown();
}

fn assert_empty_pull_diagnostics(diagnostics: &Value) {
    let items = diagnostics
        .get("items")
        .and_then(Value::as_array)
        .expect("diagnostic report should contain items");
    assert!(
        items.is_empty(),
        "suppressed pull diagnostics should be empty: {diagnostics}"
    );
}

fn assert_empty_published_diagnostics(notification: &Value) {
    let diagnostics = notification
        .pointer("/params/diagnostics")
        .and_then(Value::as_array)
        .expect("publishDiagnostics should contain diagnostics");
    assert!(
        diagnostics.is_empty(),
        "suppressed published diagnostics should be empty: {notification}"
    );
}

fn assert_diagnostic_attack(diagnostics: &Value, attack: &str, program_kind: Option<&str>) {
    let found = diagnostic_with_attack(diagnostics, attack, program_kind);
    assert!(
        found.is_object(),
        "missing {attack} diagnostic: {diagnostics}"
    );
}

fn diagnostic_with_attack(diagnostics: &Value, attack: &str, program_kind: Option<&str>) -> Value {
    let items = diagnostics
        .get("items")
        .and_then(Value::as_array)
        .expect("diagnostic report should contain items");
    items
        .iter()
        .find(|item| {
            item.get("code").and_then(Value::as_str) == Some("solana-code-quality")
                && item.pointer("/data/attack").and_then(Value::as_str) == Some(attack)
                && program_kind.is_none_or(|kind| {
                    item.pointer("/data/programKind").and_then(Value::as_str) == Some(kind)
                })
        })
        .cloned()
        .unwrap_or(Value::Null)
}

fn assert_code_action_edit_contains(actions: &Value, title: &str, expected_text: &str) {
    let found = actions.as_array().is_some_and(|items| {
        items.iter().any(|action| {
            action.get("title").and_then(Value::as_str) == Some(title)
                && action
                    .get("edit")
                    .is_some_and(|edit| value_contains_string(edit, expected_text))
        })
    });
    assert!(
        found,
        "missing {title} code action edit containing {expected_text}: {actions}"
    );
}

fn value_contains_string(value: &Value, expected_text: &str) -> bool {
    match value {
        Value::String(text) => text.contains(expected_text),
        Value::Array(items) => items
            .iter()
            .any(|item| value_contains_string(item, expected_text)),
        Value::Object(entries) => entries
            .values()
            .any(|entry| value_contains_string(entry, expected_text)),
        _ => false,
    }
}

struct LspClient {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    next_id: u64,
    shutdown: bool,
}

impl LspClient {
    fn spawn() -> Self {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut child = Command::new("cargo")
            .args(["run", "-p", "seagrass-cli", "--quiet"])
            .current_dir(repo_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("seagrass LSP process should start");
        let stdin = child.stdin.take().expect("server stdin should be piped");
        let stdout = child.stdout.take().expect("server stdout should be piped");
        let (sender, messages) = mpsc::channel();
        thread::spawn(move || {
            let mut stdout = stdout;
            while let Ok(message) = read_lsp_message(&mut stdout) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            messages,
            next_id: 1,
            shutdown: false,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }));
        self.read_response(id, method)
    }

    fn request_without_params(&mut self, method: &str) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method
        }));
        self.read_response(id, method)
    }

    fn read_response(&self, id: u64, method: &str) -> Value {
        let deadline = Instant::now() + LSP_REQUEST_TIMEOUT;
        loop {
            let now = Instant::now();
            assert!(now < deadline, "timed out waiting for {method}");
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(now))
                .unwrap_or_else(|error| panic!("timed out waiting for {method}: {error}"));
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                panic!("LSP request {method} failed: {error}");
            }
            return message.get("result").cloned().unwrap_or(Value::Null);
        }
    }

    fn read_notification(&self, method: &str, predicate: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + LSP_REQUEST_TIMEOUT;
        loop {
            let now = Instant::now();
            assert!(now < deadline, "timed out waiting for {method}");
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(now))
                .unwrap_or_else(|error| panic!("timed out waiting for {method}: {error}"));
            if message.get("method").and_then(Value::as_str) == Some(method) && predicate(&message)
            {
                return message;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }));
    }

    fn notify_without_params(&mut self, method: &str) {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": method
        }));
    }

    fn open_rust_document(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": text
                }
            }),
        );
    }

    fn change_rust_document(&mut self, uri: &str, version: i32, text: &str) {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri,
                    "version": version
                },
                "contentChanges": [
                    { "text": text }
                ]
            }),
        );
    }

    fn pull_diagnostics(&mut self, uri: &str) -> Value {
        self.request(
            "textDocument/diagnostic",
            json!({
                "textDocument": { "uri": uri },
                "identifier": "seagrass",
                "previousResultId": null
            }),
        )
    }

    fn code_actions(&mut self, uri: &str, diagnostic: &Value) -> Value {
        self.request(
            "textDocument/codeAction",
            json!({
                "textDocument": { "uri": uri },
                "range": diagnostic.get("range").cloned().unwrap_or(Value::Null),
                "context": { "diagnostics": [diagnostic.clone()] }
            }),
        )
    }

    fn shutdown(&mut self) {
        let _ = self.request_without_params("shutdown");
        self.notify_without_params("exit");
        let deadline = Instant::now() + SERVER_EXIT_TIMEOUT;
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().expect("server wait should succeed") {
                assert!(status.success(), "server exited with {status}");
                self.shutdown = true;
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.shutdown = true;
    }

    fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
            .expect("LSP message should be written");
        self.stdin.flush().expect("LSP message should flush");
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        if !self.shutdown {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("seagrass-{label}-{nonce}"));
        create_dir_all(&root).expect("test workspace should be created");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn file_uri(&self, name: &str) -> String {
        file_uri(&self.root.join(name))
    }

    fn write_file(&self, name: &str, text: &str) {
        write(self.root.join(name), text).expect("test workspace file should be written");
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        if self.root.exists() {
            remove_dir_all(&self.root).expect("test workspace should be removed");
        }
    }
}

fn read_lsp_message(stdout: &mut impl Read) -> std::io::Result<Value> {
    let mut header = Vec::new();
    let mut byte = [0_u8; 1];
    while !header.ends_with(b"\r\n\r\n") {
        stdout.read_exact(&mut byte)?;
        header.push(byte[0]);
    }
    let header = String::from_utf8_lossy(&header);
    let content_length = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| std::io::Error::other("missing LSP Content-Length"))?;
    let mut body = vec![0_u8; content_length];
    stdout.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace(' ', "%20"))
}
