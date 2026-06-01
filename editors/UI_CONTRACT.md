# Seagrass Editor UI Contract

Seagrass should feel like the same tool in every editor. Keep the adapter thin, keep the words consistent, and let the server own Anchor intelligence.

## Product Surface

- Product name: `Seagrass`
- Server id: `seagrass`
- Output/log channel label: `Seagrass`
- Visible status label: `Seagrass`
- Role: Anchor-specific diagnostics, completions, hovers, symbols, and fixes for Solana programs using Anchor, plus local artifact evidence for Anchor, Pinocchio, and native Solana programs.
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
- `/seagrass-coverage`
- `/seagrass-artifacts`
- `/seagrass-feedback`

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
sync: full
diagnostics: <push|pull|both>
workspaces: <comma-separated roots|none>
features: <short comma-separated feature list>
```

Each adapter should pick the default diagnostics transport that gives the best native editor UX without duplicating Problems entries. VS Code defaults to `push` because VS Code can duplicate pull and publish diagnostics. Zed also defaults to `push` so each Seagrass diagnostic has one native Problems transport.

Completion should wake on the first typed identifier character and on space in Anchor-aware contexts. The server advertises those trigger characters and then applies its own semantic gate, so normal Rust stays quiet while Anchor prefixes and delimiter-space flows do not wait for editor minimum-word heuristics.

Quick fixes may carry snippet tabstops when an editor advertises
`experimental.snippetTextEdit`. VS Code advertises this capability and applies
the server's raw snippet text. Editors that do not advertise it receive the same
materialized edit text without tabstops.

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
- `diagnostics.security.arbitraryCpi`
- `diagnostics.security.instructionDataBounds`
- `diagnostics.security.pdaSeedCollision`
- `security.strictNative.enabled`
- `diagnostics.experimental.enabled`
- `diagnostics.transport`
- `diagnostics.coldPath`
- `feedback.url`
- `editor.client`
- `editor.inlineValues.enabled`
- `telemetry.completion.enabled`
- `telemetry.diagnostics.enabled`
- `inlayHints.enabled`
- `workspaceIndex.enabled`
- `trace.server`

Security family settings accept `off`, `warn`, `error`, or `hint`. `agent.mode`
defaults to `false`; when enabled, the server fills unset analysis settings with
agent-friendly defaults: every security family at `warn`, cold-path diagnostics
at `idle`, security and experimental diagnostics enabled, strict native security
enabled, and server tracing enabled. Explicit user settings always win.

Server launch settings are editor-specific because each editor models binaries differently. The behavior should still resolve in this order when possible:

1. Explicit editor binary setting.
2. `seagrass` on `PATH`.
3. Local `cargo run` fallback for development.

## Manifest Watching

The server dynamically registers `workspace/didChangeWatchedFiles` during `initialized` for `**/Cargo.toml`, `**/Anchor.toml`, and `**/Seagrass.toml`. Manifest-driven rules (e.g. `anchor-check-cfg`) re-evaluate as soon as the client reports a save, without requiring a follow-up edit to a Rust source file. Registration is gated on `workspace.didChangeWatchedFiles.dynamicRegistration` from `ClientCapabilities`; clients that don't advertise it are tolerated — Seagrass falls back to refreshing manifests on the next text-document change.

## Visual Tone

Keep editor-facing text compact, content-first, and concrete. Prefer labels over explanations in command names. Put operational detail in logs and README files, not popups.
