# Seagrass for Zed

This is a local Zed language-server extension for `seagrass`. It registers the server for Rust files and runs standalone by default.

The shared editor surface is defined in [`../UI_CONTRACT.md`](../UI_CONTRACT.md). Zed and VS Code should use the same product name, settings semantics, and command labels wherever the editor exposes them. Diagnostics transport defaults are editor-specific when the native Problems behavior differs.

## Install As A Dev Extension

1. Install the Wasm target once:

   ```sh
   rustup target add wasm32-wasip2
   ```

2. In Zed, run `zed: extensions`, choose `Install Dev Extension`, and select this directory:

   ```text
   editors/zed
   ```

3. Open an Anchor or Solana project.

## Server Command Resolution

The extension starts `seagrass` in this order:

1. `lsp.seagrass.binary` from Zed settings.
2. `seagrass` from `PATH`.
3. `cargo run --manifest-path Cargo.toml --quiet` when the opened worktree is the `crates/seagrass` CLI package.
4. `cargo run --manifest-path <this checkout>/crates/seagrass/Cargo.toml --quiet` as the default dev-extension fallback for any other Anchor project.

The default dev-extension fallback should work on this machine after installing the extension from this checkout. For another server checkout, point Zed at that checkout's server package:

```json
{
  "lsp": {
    "seagrass": {
      "binary": {
        "path": "cargo",
        "arguments": [
          "run",
          "--manifest-path",
          "<seagrass-checkout>/crates/seagrass/Cargo.toml",
          "--quiet"
        ]
      },
      "settings": {
        "agent.mode": false,
        "diagnostics.security.enabled": true,
        "diagnostics.security.ownerChecks": "warn",
        "diagnostics.security.typeCosplay": "warn",
        "diagnostics.security.accountClosing": "warn",
        "diagnostics.security.initialization": "warn",
        "diagnostics.security.staleCpiReload": "warn",
        "diagnostics.security.signerAuthorization": "warn",
        "diagnostics.security.writableAccounts": "warn",
        "diagnostics.security.arbitraryCpi": "warn",
        "diagnostics.security.instructionDataBounds": "warn",
        "diagnostics.security.pdaSeedCollision": "warn",
        "security.strictNative.enabled": true,
        "diagnostics.experimental.enabled": true,
        "diagnostics.coldPath": "idle",
        "diagnostics.transport": "push",
        "editor.client": "zed",
        "editor.inlineValues.enabled": true,
        "telemetry.completion.enabled": true,
        "telemetry.diagnostics.enabled": true,
        "inlayHints.enabled": true,
        "workspaceIndex.enabled": true,
        "trace.server": false
      }
    }
  },
  "languages": {
    "Rust": {
      "language_servers": ["seagrass", "!rust-analyzer", "..."]
    }
  }
}
```

Use `!rust-analyzer` when you want Seagrass without rust-analyzer while preserving any other Zed Rust server slots. In Zed, `"..."` means "also start the remaining registered language servers" for that language; the explicit `!rust-analyzer` entry keeps that server disabled.

As an alternative to the `binary` setting, set `SEAGRASS_MANIFEST_PATH` in the shell environment used to launch Zed:

```sh
export SEAGRASS_MANIFEST_PATH=<seagrass-checkout>/crates/seagrass/Cargo.toml
```

## Capabilities

The extension registers `seagrass` with Rust's `rust` language id and advertises quick-fix/source code-action kinds to Zed. The server still owns the real LSP capability negotiation: diagnostics, completions, hovers, signature help, semantic tokens, code actions, document/workspace symbols, document links to Anchor docs, definition, references, workspace-backed Anchor rename/prepare-rename, highlights, selection ranges, folding ranges, watched files, workspace folders, and execute commands for status/artifacts/recent logs/error coverage/support matrix/generator profile. Artifact reports include Anchor projects plus deployable Pinocchio and native Solana Cargo programs, and native/Pinocchio security diagnostics include account-scoped owner/type/signer/writable validation plus expression-scoped CPI program-id checks.

Zed settings under `lsp.seagrass.settings` are passed through to `workspace/didChangeConfiguration`. The extension also sends `diagnostics.transport` during initialization. Zed defaults to `push` so each Seagrass diagnostic has one editor transport. Use `pull` only if your Zed build needs pull-based Problems population. Mixed push-and-pull transport is intentionally rejected because Zed can display duplicate diagnostics for the same range.

Completions are expected to wake on the first typed Anchor prefix and after delimiter spaces such as `#[account(init, `, not after Zed's generic minimum-word heuristic. The server advertises identifier and space trigger characters, then filters requests semantically so normal Rust spaces stay quiet.

Zed currently receives materialized quick-fix edits. `zed_extension_api` 0.7.0 does not expose a way for this adapter to advertise `experimental.snippetTextEdit` or to apply snippet WorkspaceEdits, so snippet tabstops remain a VS Code path for now.

`diagnostics.coldPath` defaults to `idle`: hot parser, Anchor-structure, and shallow handler-scope diagnostics stay live while full usage, security, and project diagnostics wait for a typing pause. Use `save` to run full diagnostics on open/save only, or `manual` to leave full checks to explicit pull/command-driven flows.

`agent.mode` defaults to `false`. When enabled, Seagrass fills unset settings with assistant and automation defaults: security and experimental diagnostics on, strict native security on, all nine security families at `warn`, `diagnostics.coldPath` at `idle`, and `trace.server` on. Set any specific key beside it to override that preset.

`telemetry.completion.enabled` and `telemetry.diagnostics.enabled` control local editor observability only. Seagrass does not upload editor events or recent logs. Use `trace.server: true` when a user needs verbose local debugging for a support report.

During `initialized`, Seagrass dynamically registers `workspace/didChangeWatchedFiles` for `**/Cargo.toml`, `**/Anchor.toml`, and `**/Seagrass.toml`. When Zed's filesystem watcher reports a change to one of those manifests, Seagrass refreshes the workspace index and republishes diagnostics for every open document, so manifest-driven rules like `anchor-check-cfg` clear immediately after you save the manifest — no need to edit the Rust source again. Registration is gated on the client advertising `workspace.didChangeWatchedFiles.dynamicRegistration`; on clients that don't (e.g. Claude Code's built-in LSP tool), Seagrass silently skips the registration and falls back to refreshing on the next text-document edit.

## Logs And Support

Zed shows extension and language-server process output in `Zed.log`; run `zed: open log` from the command palette. For live foreground debugging, launch Zed from a terminal with `zed --foreground`.

When Zed starts the server, the resolved command should match the shared startup contract:

```text
Seagrass
server: <command> <args>
cwd: <worktree or configured command cwd>
sync: incremental
diagnostics: push
workspaces: <opened worktree>
features: diagnostics, completion, hover, symbols, fixes, logs
```

The server also exposes a bounded in-memory log snapshot over LSP. The snapshot is local to the running language-server process and is intended for support/debugging, not analytics export:

```json
{ "command": "seagrass/logs", "arguments": [] }
```

The same execute-command surface exposes `seagrass/status`, `seagrass/artifacts`, `seagrass/logs`, `seagrass/feedback`, `seagrass/errorCoverage`, `seagrass/supportMatrix`, `seagrass/generatorProfile`, and `seagrass/analyze`. Zed's adapter stays thin, so these commands are implemented once in the server and remain available to other LSP clients and agent harnesses.

Assistant slash commands are registered as `/seagrass-status`, `/seagrass-analyze`, `/seagrass-coverage`, `/seagrass-artifacts`, `/seagrass-program-report`, `/seagrass-error-coverage`, `/seagrass-support-matrix`, `/seagrass-generator-profile`, `/seagrass-logs`, and `/seagrass-feedback`. Zed 0.7.0 does not expose a direct Assistant-to-running-LSP execute-command bridge, so the adapter sends a one-shot `workspace/executeCommand` request to the resolved Seagrass binary and returns the server JSON. If the binary or cargo fallback is unavailable, the slash command returns a "Start the Seagrass server first" message instead of failing silently.

`seagrass/feedback` returns the bundled feedback URL from the server. The Zed extension does not expose a native command for it because `zed_extension_api` 0.7.0 cannot open external URLs from the wasm extension; use `/seagrass-feedback` to print the URL in Assistant.

`seagrass/projectCoverage` returns the current workspace settings, open-document diagnostic counts, attack-family counts, and support-matrix coverage. This is the quick way to check which security families are active in the editor without reading logs.
