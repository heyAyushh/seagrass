use {
    crate::{
        account_semantics, actions, anchor_errors, anchor_support, code_lens, completions,
        debounce, diagnostics, document,
        document::ParsedDocument,
        document_links, ecosystem, evidence, folding, hover, inlay_hints, navigation,
        program_artifacts, project, query_cache, renaming,
        salsa_db::{LspDatabase, LspSalsaDb},
        selection_ranges, semantic_tokens, server_observability,
        server_types::{
            DiagnosticsTransport, OpenDocument, ParsedOpenDocument, ServerLogEntry, ServerSettings,
        },
        signature_help, solana_project, workspace,
    },
    dashmap::DashMap,
    std::{
        collections::{BTreeMap, BTreeSet, VecDeque},
        fs,
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{Arc, Mutex, RwLock},
        time::{Duration, Instant},
    },
    tower_lsp::{
        jsonrpc::Result,
        lsp_types::{
            request::{
                GotoDeclarationParams, GotoDeclarationResponse, GotoImplementationParams,
                GotoImplementationResponse, GotoTypeDefinitionParams, GotoTypeDefinitionResponse,
            },
            CodeAction, CodeActionKind, CodeActionOptions, CodeActionParams,
            CodeActionProviderCapability, CodeLens, CodeLensOptions, CodeLensParams,
            CompletionContext, CompletionItem, CompletionOptions, CompletionParams,
            CompletionResponse, DeclarationCapability, DiagnosticOptions,
            DiagnosticServerCapabilities, DidChangeConfigurationParams,
            DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
            DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
            DidSaveTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
            DocumentDiagnosticReportResult, DocumentHighlightParams, DocumentLink,
            DocumentLinkOptions, DocumentLinkParams, DocumentSymbolParams, DocumentSymbolResponse,
            ExecuteCommandOptions, ExecuteCommandParams, FoldingRangeParams,
            FoldingRangeProviderCapability, FullDocumentDiagnosticReport, GotoDefinitionParams,
            GotoDefinitionResponse, HoverParams, HoverProviderCapability,
            ImplementationProviderCapability, InitializeParams, InitializeResult, InlayHint,
            InlayHintParams, Location, MessageType, OneOf, PrepareRenameResponse, ReferenceParams,
            RelatedFullDocumentDiagnosticReport, RenameOptions, RenameParams, SaveOptions,
            SelectionRangeParams, SelectionRangeProviderCapability, SemanticTokensParams,
            SemanticTokensRangeParams, SemanticTokensRangeResult, SemanticTokensResult,
            SemanticTokensServerCapabilities, ServerCapabilities, SignatureHelpOptions,
            SignatureHelpParams, SymbolInformation, SymbolKind, TextDocumentPositionParams,
            TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
            TextDocumentSyncSaveOptions, TypeDefinitionProviderCapability, Url,
            WorkDoneProgressOptions, WorkspaceEdit, WorkspaceFoldersServerCapabilities,
            WorkspaceServerCapabilities, WorkspaceSymbolParams,
        },
        Client, LanguageServer, LspService, Server,
    },
};

mod backend_features;
mod diagnostic_pipeline;
mod helpers;
mod navigation_handlers;
mod reports;

use {helpers::*, reports::*};

const STATUS_COMMAND: &str = "seagrass/status";
const ANALYZE_COMMAND: &str = "seagrass/analyze";
const ARTIFACTS_COMMAND: &str = "seagrass/artifacts";
const SEAGRASS_INSTRUCTION_SUMMARY_COMMAND: &str = "seagrass/instructionSummary";
const SEAGRASS_PROGRAM_REPORT_COMMAND: &str = "seagrass/programReport";
const ERROR_COVERAGE_COMMAND: &str = "seagrass/errorCoverage";
const SUPPORT_MATRIX_COMMAND: &str = "seagrass/supportMatrix";
const GENERATOR_PROFILE_COMMAND: &str = "seagrass/generatorProfile";
const LOGS_COMMAND: &str = "seagrass/logs";
const PROJECT_COVERAGE_COMMAND: &str = "seagrass/projectCoverage";
const RECENT_LOG_LIMIT: usize = 200;
const COLD_DIAGNOSTICS_DEBOUNCE_MILLIS: u64 = 700;
const TYPING_DIAGNOSTIC_SUPPRESSION_MILLIS: u64 = 250;
const IDENTIFIER_COMPLETION_TRIGGER_CHARS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_ .<,=";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentUpdateKind {
    Open,
    Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagnosticPublishLane {
    Hot,
    Idle,
    Pull,
    Save,
    Republish,
}

impl DiagnosticPublishLane {
    fn lane(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Idle | Self::Pull | Self::Save | Self::Republish => "cold",
        }
    }

    fn origin(self) -> &'static str {
        match self {
            Self::Hot => "live-hot",
            Self::Idle => "idle-analysis",
            Self::Pull => "pull-analysis",
            Self::Save => "on-save-analysis",
            Self::Republish => "republish-analysis",
        }
    }
}

#[derive(Debug, Clone)]
struct CompletionMemo {
    version: i32,
    signature_cache_key: String,
    response: Option<Vec<CompletionItem>>,
}

#[derive(Debug, Clone)]
struct RecentTypingChange {
    changed_at: Instant,
    range: tower_lsp::lsp_types::Range,
}

#[derive(Debug, Clone)]
struct Backend {
    client: Client,
    documents: DashMap<Url, OpenDocument>,
    workspace_roots: Arc<Mutex<Vec<Url>>>,
    // RwLock allows concurrent read-only LSP handlers (hover, completion, goto, etc.)
    // while serializing the infrequent write operations (workspace refresh).
    workspace_index: Arc<RwLock<workspace::WorkspaceIndex>>,
    settings: Arc<Mutex<ServerSettings>>,
    diagnostics_transport: Arc<Mutex<DiagnosticsTransport>>,
    recent_logs: Arc<Mutex<VecDeque<ServerLogEntry>>>,
    document_debouncers: DashMap<Url, debounce::Debouncer>,
    recent_typing_changes: DashMap<Url, RecentTypingChange>,
    completion_memo: DashMap<Url, CompletionMemo>,
    query_cache: query_cache::QueryCache,
    // Salsa DB (wrapped in Mutex for thread safety - LspSalsaDb uses RefCell internally and is not Sync).
    // Architecture:
    // - ParsedDocument is the raw parsing layer (Tree-sitter + syn::File + symbols); never rewritten.
    // - Salsa tracked queries (document_symbols, constraint_diagnostics) take (Arc<str> source, version) as input.
    //   This completely solves the "clone blocker" - Arc<str> clone is O(1) refcount bump; Salsa handles incrementality
    //   when source text changes (new Arc or different version invalidates dependent queries).
    // - QueryCache kept on top for LSP-specific versioned caching of results (e.g. hover, completion).
    // - Real queries for symbols and Anchor diagnostics now route through Salsa for zero-cost, type-driven caching.
    // Follows Stacc rules: type-driven, ownership (Arc), concurrency (Mutex), zero-cost abstractions.
    salsa_db: Arc<Mutex<LspSalsaDb>>,
}

pub async fn run_stdio() {
    let _hotpath_guard = crate::hotpath::install_guard();
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: DashMap::new(),
        workspace_roots: Arc::new(Mutex::new(Vec::new())),
        workspace_index: Arc::new(RwLock::new(workspace::WorkspaceIndex::default())),
        settings: Arc::new(Mutex::new(ServerSettings::default())),
        diagnostics_transport: Arc::new(Mutex::new(DiagnosticsTransport::Push)),
        recent_logs: Arc::new(Mutex::new(VecDeque::with_capacity(RECENT_LOG_LIMIT))),
        document_debouncers: DashMap::new(),
        recent_typing_changes: DashMap::new(),
        completion_memo: DashMap::new(),
        query_cache: query_cache::QueryCache::new(),
        salsa_db: Arc::new(Mutex::new(LspSalsaDb::default())),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let roots = workspace_roots_from_initialize(&params);

        // Validate workspace roots exist on disk before accepting them.
        // Prevents confusing "file not found" errors later when the LSP tries to index.
        for root in &roots {
            if root.scheme() == "file" {
                if let Ok(path) = root.to_file_path() {
                    if !path.exists() {
                        self.emit_log(
                            MessageType::WARNING,
                            "warn",
                            "invalidWorkspaceRoot",
                            format!("workspace root does not exist: {root}"),
                            serde_json::json!({"root": root.to_string()}),
                        )
                        .await;
                    }
                }
            }
        }

        self.set_workspace_roots(roots);
        let diagnostics_transport =
            diagnostics_transport_from_initialize_options(params.initialization_options.as_ref());
        self.set_diagnostics_transport(diagnostics_transport);
        // Defer workspace indexing to `initialized` — LSP spec requires
        // `initialize` to return fast; scanning the workspace tree during
        // the handshake causes timeout on larger projects.

        Ok(InitializeResult {
            capabilities: server_capabilities(diagnostics_transport),
            server_info: Some(tower_lsp::lsp_types::ServerInfo {
                name: "Seagrass".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn initialized(&self, _: tower_lsp::lsp_types::InitializedParams) {
        self.refresh_workspace_index().await;

        let roots = self.workspace_root_log();
        self.emit_log(
            MessageType::INFO,
            "info",
            "serverInitialized",
            format!("seagrass v{} ready", env!("CARGO_PKG_VERSION")),
            serde_json::json!({"version": env!("CARGO_PKG_VERSION")}),
        )
        .await;
        self.emit_log(
            MessageType::INFO,
            "info",
            "initialized",
            format!("Seagrass initialized; workspace roots: {roots}"),
            serde_json::json!({
                "workspaceRoots": roots,
                "diagnosticsTransport": self.diagnostics_transport().as_str(),
            }),
        )
        .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn shutdown(&self) -> Result<()> {
        let debouncers = self
            .document_debouncers
            .iter()
            .map(|entry| entry.value().clone())
            .collect::<Vec<_>>();
        for debouncer in debouncers {
            let _ = debouncer.flush().await;
        }
        Ok(())
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.handle_document_update(
            params.text_document.uri,
            params.text_document.text,
            Some(params.text_document.version),
            DocumentUpdateKind::Open,
            None,
        )
        .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };

        self.handle_document_update(
            uri,
            change.text,
            Some(version),
            DocumentUpdateKind::Change,
            change.range,
        )
        .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text.or_else(|| {
            self.documents
                .get(&uri)
                .map(|document| document.text.clone())
        });
        let Some(text) = text else {
            return;
        };
        let version = self.current_document_version(&uri);
        self.cancel_scheduled_analysis(&uri).await;
        self.run_full_analysis_and_publish(uri, text, Some(version), DiagnosticPublishLane::Save)
            .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some((_, debouncer)) = self.document_debouncers.remove(&uri) {
            let _ = debouncer.cancel().await;
        }
        self.documents.remove(&uri);
        self.recent_typing_changes.remove(&uri);
        self.completion_memo.remove(&uri);
        self.refresh_workspace_index().await;
        if self.diagnostics_transport().publishes() {
            self.client.publish_diagnostics(uri, Vec::new(), None).await;
        }
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        let added_count = params.event.added.len();
        let removed_count = params.event.removed.len();
        {
            let mut roots = self
                .workspace_roots
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            roots.retain(|root| {
                !params
                    .event
                    .removed
                    .iter()
                    .any(|removed| removed.uri == *root)
            });
            roots.extend(params.event.added.into_iter().map(|folder| folder.uri));
        }
        self.refresh_workspace_index().await;
        let republished_open_documents = self.republish_open_documents().await;

        let roots = self.workspace_root_log();
        self.emit_log(
            MessageType::INFO,
            "info",
            "workspaceRootsChanged",
            format!("workspace roots changed: {roots}"),
            workspace_roots_changed_log_data(
                &roots,
                added_count,
                removed_count,
                republished_open_documents,
            ),
        )
        .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let changed_file_count = params.changes.len();
        self.refresh_workspace_index().await;
        let republished_open_documents = self.republish_open_documents().await;
        self.emit_log(
            MessageType::INFO,
            "info",
            "watchedFilesChanged",
            format!(
                "watched files changed: {changed_file_count}; republished {republished_open_documents} open document(s)"
            ),
            watched_files_changed_log_data(changed_file_count, republished_open_documents),
        )
        .await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        {
            let mut settings = self.settings.lock().unwrap_or_else(|err| err.into_inner());
            settings.apply(params.settings);
        }
        self.record_log(
            "info",
            "configurationChanged",
            "workspace configuration changed",
            serde_json::json!({ "settings": self.settings_snapshot() }),
        );
        self.refresh_workspace_index().await;
        self.republish_open_documents().await;
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        self.with_catch_unwind("document_symbol", move |this| {
            this.document_symbol_impl(params)
        })
        .await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let Some(document) = self.document_for(&params.text_document.uri) else {
            return Ok(None);
        };

        Ok(Some(document_links::document_links(&document)))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.with_catch_unwind("completion", move |this| this.completion_impl(params))
            .await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn completion_resolve(&self, params: CompletionItem) -> Result<CompletionItem> {
        Ok(completions::resolve(params))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn hover(&self, params: HoverParams) -> Result<Option<tower_lsp::lsp_types::Hover>> {
        self.hover_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        self.goto_definition_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn goto_declaration(
        &self,
        params: GotoDeclarationParams,
    ) -> Result<Option<GotoDeclarationResponse>> {
        self.goto_declaration_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn goto_type_definition(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> Result<Option<GotoTypeDefinitionResponse>> {
        self.goto_type_definition_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        self.goto_implementation_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        self.references_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        self.prepare_rename_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        self.rename_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::DocumentHighlight>>> {
        self.document_highlight_impl(params).await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> Result<Option<tower_lsp::lsp_types::CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::CodeActions(range));
        if let Some(query_cache::CacheValue::CodeActions(actions)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(
                (!actions.is_empty()).then_some(actions.into_iter().map(Into::into).collect())
            );
        }

        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };

        let wants_source_action = params
            .context
            .only
            .as_ref()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str().starts_with("source")));
        let diagnostics = self.code_action_diagnostics(
            &uri,
            &document,
            range,
            params.context.diagnostics,
            wants_source_action,
        );
        let actions = actions::code_actions(&document, uri, range, &diagnostics);
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::CodeActions(actions.clone()),
        );
        Ok((!actions.is_empty()).then_some(actions.into_iter().map(Into::into).collect()))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn code_action_resolve(&self, params: CodeAction) -> Result<CodeAction> {
        let Some(uri) = code_action_uri(&params) else {
            return Ok(params);
        };
        let Some(document) = self.document_for(&uri) else {
            return Ok(params);
        };
        let diagnostics = self.collect_diagnostics_for_uri(&uri, &document);
        Ok(actions::resolve(&document, uri, params, &diagnostics))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let workspace_index = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner());

        Ok(Some(code_lens::code_lenses(
            &document,
            &uri,
            &workspace_index,
        )))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        self.with_catch_unwind("diagnostic", move |this| this.diagnostic_impl(params))
            .await
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::SemanticTokens);
        if let Some(query_cache::CacheValue::SemanticTokens(tokens)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(Some(SemanticTokensResult::Tokens(tokens)));
        }

        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let tokens = semantic_tokens::full(&document);
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::SemanticTokens(tokens.clone()),
        );
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::SemanticTokens);
        if let Some(query_cache::CacheValue::SemanticTokens(tokens)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(Some(SemanticTokensRangeResult::Tokens(tokens)));
        }

        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let tokens = semantic_tokens::range(&document, params.range);
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::SemanticTokens(tokens.clone()),
        );
        Ok(Some(SemanticTokensRangeResult::Tokens(tokens)))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::FoldingRange>>> {
        let uri = params.text_document.uri;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::FoldingRanges);
        if let Some(query_cache::CacheValue::FoldingRanges(ranges)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(Some(ranges));
        }

        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let ranges = folding::folding_ranges(&document);
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::FoldingRanges(ranges.clone()),
        );
        Ok(Some(ranges))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn selection_range(
        &self,
        params: SelectionRangeParams,
    ) -> Result<Option<Vec<tower_lsp::lsp_types::SelectionRange>>> {
        let Some(document) = self.document_for(&params.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(selection_ranges::selection_ranges(
            &document,
            &params.positions,
        )))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> Result<Option<tower_lsp::lsp_types::SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        Ok(signature_help::signature_help(&document, position))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        let version = self.current_document_version(&uri);
        let cache_key = (uri.clone(), query_cache::QueryKind::InlayHints(range));
        if let Some(query_cache::CacheValue::InlayHints(hints)) =
            self.query_cache.get(cache_key.clone(), version)
        {
            return Ok(Some(hints));
        }

        let Some(document) = self.document_for(&uri) else {
            return Ok(None);
        };
        let hints = inlay_hints::inlay_hints(&document, range);
        self.query_cache.insert(
            cache_key,
            version,
            query_cache::CacheValue::InlayHints(hints.clone()),
        );
        Ok(Some(hints))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let symbols = self
            .workspace_index
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .workspace_symbols(&params.query);
        Ok((!symbols.is_empty()).then_some(symbols))
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        match params.command.as_str() {
            STATUS_COMMAND => Ok(Some(serde_json::json!(self.status_text()))),
            ANALYZE_COMMAND => Ok(self.analyze_command(params.arguments)),
            ARTIFACTS_COMMAND => Ok(Some(self.artifacts_command(params.arguments))),
            SEAGRASS_INSTRUCTION_SUMMARY_COMMAND => {
                Ok(self.instruction_summary_command(params.arguments))
            }
            SEAGRASS_PROGRAM_REPORT_COMMAND => Ok(Some(self.program_report_command())),
            ERROR_COVERAGE_COMMAND => Ok(Some(anchor_errors::coverage_summary())),
            SUPPORT_MATRIX_COMMAND => Ok(Some(anchor_support::matrix())),
            GENERATOR_PROFILE_COMMAND => Ok(Some(anchor_support::generator_profile())),
            LOGS_COMMAND => Ok(Some(self.recent_logs_snapshot())),
            PROJECT_COVERAGE_COMMAND => Ok(Some(self.project_coverage_command())),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
#[path = "server/tests/mod.rs"]
mod tests;
