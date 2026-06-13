# Seagrass Editor UI Contract

Seagrass should feel like the same tool in every editor. Keep the adapter thin, keep the words consistent, and let the server own Anchor intelligence.

## Product Surface

- Product name: `Seagrass`
- Server id: `seagrass`
- Output/log channel label: `Seagrass`
- Visible status label: `Seagrass`
- Role: Anchor-first diagnostics, completions, hovers, symbols, and fixes for
  Solana programs using Anchor, plus native/Pinocchio security invariants and
  local artifact evidence for Anchor, Pinocchio, and native Solana programs.
  Rust document formatting is server-owned and exposed through the standard LSP
  formatting provider. Native Solana and Pinocchio parity is stable applicable
  parity: shared Solana diagnostics and artifact evidence use the same LSP
  surface, with account-scoped owner/type/signer/writable checks and
  expression-scoped CPI program-id checks, while Anchor-only
  constraint/account/IDL/type semantics remain non-applicable.
- Relationship to Rust tooling: Seagrass runs standalone and owns Anchor semantics. Other Rust servers are optional.

## Commands

Use these labels anywhere an editor exposes commands:

- `Seagrass: Status`
- `Seagrass: Analyze Document`
- `Seagrass: Artifacts`
- `Seagrass: Send Feedback`
- `Seagrass: Recent Logs`
- `Seagrass: Error Coverage`
- `Seagrass: Support Matrix`
- `Seagrass: Generator Profile`
- `Seagrass: Restart Server`
- `Seagrass: Output`

The server-side execute-command ids stay stable:

- `seagrass/status`
- `seagrass/analyze`
- `seagrass/artifacts`
- `seagrass/programReport`
- `seagrass/feedback`
- `seagrass/logs`
- `seagrass/errorCoverage`
- `seagrass/supportMatrix`
- `seagrass/generatorProfile`
- `seagrass/projectCoverage`

The bundled feedback destination lives in `editors/feedback.toml`. Editor
adapters may expose native commands or slash commands, but they should request
`seagrass/feedback` from the server instead of hardcoding the URL.

Zed Assistant slash commands mirror the server-owned command surface:

- `/seagrass-status`
- `/seagrass-analyze`
- `/seagrass-coverage`
- `/seagrass-artifacts`
- `/seagrass-program-report`
- `/seagrass-error-coverage`
- `/seagrass-support-matrix`
- `/seagrass-generator-profile`
- `/seagrass-logs`
- `/seagrass-feedback`

Vim and Neovim packages expose the same command surface with editor-native
command names:

- `:SeagrassInfo`
- `:SeagrassStatus`
- `:SeagrassAnalyze`
- `:SeagrassArtifacts`
- `:SeagrassProgramReport`
- `:SeagrassErrorCoverage`
- `:SeagrassSupportMatrix`
- `:SeagrassGeneratorProfile`
- `:SeagrassLogs`
- `:SeagrassProjectCoverage`
- `:SeagrassFeedback`
- `:SeagrassRestart`
- `:SeagrassDiagnostics`
- `:SeagrassAnalyzeCli`

`:SeagrassDiagnostics` is the Vim-family quickfix fallback for users who want
an explicit scan or whose LSP transport is not attached. It must call
`seagrass diagnostics <path> --json` and translate LSP zero-based ranges into
editor one-based quickfix locations. `:SeagrassAnalyzeCli` prints
`seagrass analyze <path> --json` into a scratch buffer.

Classic POSIX `vi` has no LSP or plugin surface to reach parity with. Document
it as CLI-only and point users whose `vi` is Vim or Neovim to the real
Vim-family packages.

## Status Surface

When an editor exposes a persistent status surface, use `Seagrass` as the label and keep it diagnostic-aware for the active file:

- starting: `Seagrass` with a busy indicator
- ready and clean: `Seagrass` with a success indicator
- active-file errors: `Seagrass <error-count>` with an error indicator
- active-file warnings: `Seagrass <warning-count>` with a warning indicator
- stopped: `Seagrass` with a stopped indicator

The status action should open `Seagrass: Status` where the editor supports commands.

## Startup Summary

Every adapter that can write startup output should use this shape:

```text
Seagrass
server: <command> <args>
cwd: <working directory>
sync: incremental
diagnostics: <push|pull|both>
workspaces: <comma-separated roots|none>
features: <short comma-separated feature list>
```

Each adapter should pick the default diagnostics transport that gives the best native editor UX without duplicating Problems entries. VS Code defaults to `push` because VS Code can duplicate pull and publish diagnostics. Zed also defaults to `push` so each Seagrass diagnostic has one native Problems transport.
The Vim package defaults to `push` through vim-lsp or CoC and identifies the
client as `vim`.

Completion should wake on the first typed identifier character and on space in Anchor-aware contexts. The server advertises those trigger characters and then applies its own semantic gate, so normal Rust stays quiet while Anchor prefixes and delimiter-space flows do not wait for editor minimum-word heuristics.
Zed adapters should also provide native code labels for Seagrass completions and symbols when the extension API exposes label hooks; these labels are presentation-only and must not replace the server-owned completion, hover, symbol, or navigation payloads.

Quick fixes may carry snippet tabstops when an editor advertises
`experimental.snippetTextEdit`. VS Code advertises this capability and applies
the server's raw snippet text. Editors that do not advertise it receive the same
materialized edit text without tabstops.
The Vim package relies on vim-lsp for completion, hover, definition, formatting,
code actions, symbols, and diagnostics; Seagrass-specific report commands use
`workspace/executeCommand` through vim-lsp when available and identify the
client as `vim`. The Neovim package uses the built-in LSP client, identifies the
client as `nvim`, prefers `vim.lsp.config`/`vim.lsp.enable`, and falls back to
`nvim-lspconfig` on older installs.

Formatting should use the server's `textDocument/formatting` response. Adapters
should not shell out to `rustfmt` independently for Seagrass-managed documents.

Framework parity is documented in `docs/framework-parity.md`. Editor adapters
should not describe native Solana or Pinocchio as having Anchor constraint
completions, hovers, or IDL/type artifact checks.

## Settings

VS Code uses `seagrass.*`. Zed uses `lsp.seagrass.settings.*`. Keep these settings equivalent:

- `agent.mode`
- `diagnostics.security.enabled`
- `diagnostics.security.ownerChecks`
- `diagnostics.security.typeCosplay`
- `diagnostics.security.accountClosing`
- `diagnostics.security.initialization`
- `diagnostics.security.staleCpiReload`
- `diagnostics.security.signerAuthorization`
- `diagnostics.security.writableAccounts`
- `diagnostics.security.arbitraryCpi`
- `diagnostics.security.instructionDataBounds`
- `diagnostics.security.pdaSeedCollision`
- `security.strictNative.enabled`
- `diagnostics.artifacts.enabled`
- `diagnostics.experimental.enabled`
- `diagnostics.transport`
- `diagnostics.coldPath`
- `feedback.url`
- `editor.client`
- `editor.inlineValues.enabled`
- `telemetry.completion.enabled`
- `telemetry.diagnostics.enabled`
- `workspaceIndex.enabled`
- `trace.server`

Security family settings accept `off`, `warn`, `error`, or `hint`. `agent.mode`
defaults to `false`; when enabled, the server fills unset analysis settings with
assistant and automation defaults: every security family at `warn`, cold-path
diagnostics at `idle`, security and experimental diagnostics enabled, strict
native security enabled, and server tracing enabled. Explicit user settings
always win.

Server launch settings are editor-specific because each editor models binaries differently. The behavior should still resolve in this order when possible:

1. Explicit editor binary setting.
2. `seagrass` on `PATH`.
3. Local `cargo run` fallback for development.

## Silent Surfaces

Silent control surfaces must have one canon and one loud guard:

- Server settings canon: `src/runtime/server_types/mod.rs::RECOGNIZED_SETTING_KEYS`.
  The generated `editors/recognized-settings.json`,
  `bun scripts/check-recognized-settings.ts`, and
  `bun editors/check-ui-contract.ts` keep VS Code, Vim/CoC, and this contract
  aligned with what the server actually parses.
- Suppression choke point: `src/server/helpers.rs::syntax_diagnostics_for_document`
  for parse-pause diagnostics plus `src/lsp/diagnostics/engine.rs` for semantic
  diagnostics. `tests/jsonrpc_lsp.rs` proves suppression through push, pull,
  parse-pause, and real `Seagrass.toml` discovery paths.
- `Seagrass.toml` key canon:
  `src/lsp/diagnostics/suppression.rs::SEAGRASS_TOML_KEY_PATHS`.
  `bun scripts/check-seagrass-toml-contract.ts` rejects docs, skills, or editor
  examples that show unparsed config keys.
- Cargo project suppression canon:
  `src/lsp/diagnostics/suppression.rs` parses
  `[package.metadata.seagrass] suppress = true` and
  `[workspace.metadata.seagrass] suppress = true` from
  `DiagnosticInput.manifest`/`workspace_manifest` in the same suppression choke
  point.
- Release artifact canon: `.github/workflows/release.yaml`.
  `bun scripts/check-release-parity.ts` checks installer target names against the
  release matrix, and `.github/workflows/install-smoke.yaml` proves build-time
  and release-time install paths on macOS, Linux, and Windows.

## Manifest Watching

The server dynamically registers `workspace/didChangeWatchedFiles` during `initialized` for `**/Cargo.toml`, `**/Anchor.toml`, and `**/Seagrass.toml`. Manifest-driven rules (e.g. `anchor-check-cfg`) re-evaluate as soon as the client reports a save, without requiring a follow-up edit to a Rust source file. Registration is gated on `workspace.didChangeWatchedFiles.dynamicRegistration` from `ClientCapabilities`; clients that don't advertise it are tolerated — Seagrass falls back to refreshing manifests on the next text-document change.

## Visual Tone

Keep editor-facing text compact, content-first, and concrete. Prefer labels over explanations in command names. Put operational detail in logs and README files, not popups.
