# Seagrass LSP

AI-agent native multi-framework LSP for Solana Rust programs. Seagrass provides
Anchor-aware diagnostics, completions, hovers, navigation, quick fixes, local
artifact evidence, and structured CLI output for Anchor, Pinocchio, and native
Solana projects.

Seagrass is a standalone stdio LSP server. It complements rust-analyzer for
generic Rust and owns the Solana framework layer.

Unofficial. Not affiliated with Coral or the Anchor project. Compatible with
the `otter-sec/anchor` Anchor line used by this workspace.

## Install The Server

From the repository root:

```sh
cargo build -p seagrass
```

Install the launch entrypoint:

```sh
cargo install --path crates/seagrass
```

Run from source:

```sh
cargo run -p seagrass
```

Run automation-ready JSON diagnostics without an editor:

```sh
cargo run -p seagrass -- diagnostics programs/demo/src/lib.rs --json
```

Or pipe a single Rust source file through stdin while preserving the source path
for workspace context:

```sh
cat programs/demo/src/lib.rs | cargo run -p seagrass -- diagnostics --stdin --stdin-path programs/demo/src/lib.rs --json
```

The JSON output is an array of diagnostics with `file`, `range`, `code`,
`severity`, `topic`, `confidence`, and `message`. The command exits `1` when
any `ERROR` severity finding is emitted and exits `2` for usage or input
errors. Run `cargo run -p seagrass -- diagnostics --help` for copy-pasteable
examples.

## Assistant And Automation Setup

Use this when Claude Code, OpenCode, Cursor, Codex, Aider, CI, or an LSP bridge
needs Solana framework semantics. In this repo, "headless" only means "without
an editor UI": stdin/stdout, JSON diagnostics, and LSP `workspace/executeCommand`
payloads. It is not a separate Seagrass mode.

### Claude Code

Claude Code supports `SKILL.md` folders. Symlink the bundled skills once into
your global Claude skills directory:

```bash
mkdir -p ~/.claude/skills
for skill in skills/seagrass-*; do
  ln -sfn "$(pwd)/$skill" "$HOME/.claude/skills/$(basename $skill)"
done
```

For project-local skills, use `.claude/skills` instead of `~/.claude/skills`.
Then ask for workflows such as "lint my program", "explain
seagrass/security.owner-check", "suppress this", or "audit my Anchor code for
production".

See `skills/README.md` for the full catalog.

### OpenCode

No OpenCode package is required for the repo-level workflow. The tracked
template lives under `editors/opencode/` so editor-specific files do not clutter
the repository root. To activate it for a local checkout:

```bash
ln -sfn editors/opencode/opencode.json opencode.json
```

That root `opencode.json` routes OpenCode to:

- `AGENTS.md` for repository rules
- `docs/agents.md` for CLI and LSP command payloads
- `skills/README.md` for the optional Claude-style skill catalog

Start OpenCode from the repository root so it can read the activated
`opencode.json`.

### Cursor

Cursor currently recommends project rules in `.cursor/rules/*.mdc`, while still
supporting `AGENTS.md` as the simple fallback. The tracked Seagrass Cursor rule
lives under `editors/cursor/`; activate it locally with:

```bash
mkdir -p .cursor/rules
ln -sfn ../../editors/cursor/rules/seagrass.mdc .cursor/rules/seagrass.mdc
```

The rule is scoped to Rust and Solana manifest files. It tells Cursor to run
`seagrass diagnostics --json` and to prefer the structured LSP reports in
`docs/agents.md` before editing Anchor account logic.

### Codex, Aider, CI, And Low-Level LSP Bridges

1. Start the server over stdio:

   ```json
   {
     "command": "cargo",
     "args": ["run", "-p", "seagrass", "--quiet"]
   }
   ```

2. For structured snapshots, use the execute-command examples in
   `docs/agents.md`.

3. Before trusting editor-visible behavior, run:

   ```sh
   bun scripts/verify-production.ts
   ```

## Zed Setup

Build the dev extension artifact:

```sh
rustup target add wasm32-wasip2
cd editors/zed
cargo build --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/seagrass_zed.wasm extension.wasm
```

In Zed, run `zed: extensions`, choose `Install Dev Extension`, and select
`editors/zed`.

Example settings for a local source run:

```json
{
  "lsp": {
    "seagrass": {
      "binary": {
        "path": "cargo",
        "arguments": ["run", "-p", "seagrass", "--quiet"]
      },
      "settings": {
        "agent.mode": false,
        "diagnostics.transport": "push",
        "diagnostics.coldPath": "idle",
        "diagnostics.security.ownerChecks": "warn",
        "diagnostics.security.typeCosplay": "warn",
        "workspaceIndex.enabled": true
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

See `editors/zed/README.md` for logging and full settings.

## VS Code Setup

```sh
cd editors/vscode
bun install
bun run check
code .
```

Run the `Run Seagrass Extension` launch configuration. Server process settings
live under `seagrass.*`. VS Code advertises Seagrass snippet quick-fix support,
so account-field fixes can include tabstops while other editors receive the
same materialized edits.

## Smoke Test

Open `fixtures/broken.rs` in the editor. You should see:

```text
payer must be provided when initializing an account
```

Then confirm:

- completions wake inside `#[account(...)]`
- hover on `init` shows Anchor constraint docs
- the lightbulb offers a starter quick fix for `payer` and `space`

## Production Gate

Run this before publishing, handing off, or trusting editor-visible behavior:

```sh
bun scripts/verify-production.ts
```

It checks formatting, editor UX parity, full LSP tests, protocol smoke, shared
editor UI contract, diagnostic rule hygiene, VS Code, Zed, version alignment,
generated support, and checked-in Zed wasm freshness.

## Generated Support

Anchor support catalogs are checked in under `src/generated`. Normal builds
do not scrape parent Anchor sources. Regenerate explicitly:

```sh
bun scripts/regen-support.ts --anchor-path . --family v1
```

See `SUPPORT_GENERATOR.md` for the source list, v1 policy, and v2 preview
workflow.

## Release Packages

Create a signed LSP release tag after `VERSION` and every LSP/editor manifest
match the intended version:

```sh
git tag -s v1.0.2
git push origin v1.0.2
```

The release workflow builds and attaches:

- `seagrass-<version>-<target>.tar.gz`
- `seagrass-zed-<version>.tar.gz`
- `seagrass-vscode-<version>.vsix`
- matching `.sha256` files

Pre-release tags such as `v1.0.2-rc.1` create GitHub pre-releases.

## License

The Seagrass LSP overlay and local editor adapters are MIT licensed. The parent
Anchor workspace keeps its top-level license unless a file explicitly says
otherwise. See `NOTICE` for Apache-2.0 attribution covering generated
Anchor-derived support catalogs.
