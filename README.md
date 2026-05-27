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

Run agent-friendly JSON diagnostics without an editor:

```sh
cargo run -p seagrass -- diagnostics --json programs/demo/src/lib.rs
```

The JSON output is an array of diagnostics with `file`, `range`, `code`,
`severity`, `topic`, `confidence`, and `message`. The command exits nonzero
when any `ERROR` severity finding is emitted.

## Agent Setup

Use this when an AI coding agent or LSP bridge needs Solana framework semantics.

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
        "diagnostics.transport": "both",
        "diagnostics.coldPath": "idle",
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
live under `seagrass.*`.

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
