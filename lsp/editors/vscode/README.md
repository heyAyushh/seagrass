# Seagrass for VS Code

This is the local VS Code client for `seagrass`. It uses Bun for dependency management, typechecking, and building the generated extension entrypoint.

The shared editor surface is defined in [`../UI_CONTRACT.md`](../UI_CONTRACT.md). Command labels, settings semantics, diagnostics transport, and startup logs should match the Zed extension.

## Develop Locally

```sh
bun install
bun run check
code .
```

Then launch `Run Seagrass Extension` from VS Code. The launch task runs `bun run build` before opening the extension development host.

## Capabilities

The client intentionally uses one `vscode-languageclient` instance and lets the server advertise the LSP features. The current server surface includes diagnostics, completion, hover, signature help, code actions, semantic tokens, inlay hints, document/workspace symbols, document links to Anchor docs, definition, references, workspace-backed Anchor rename/prepare-rename, document highlights, selection ranges, folding ranges, workspace folders, watched file changes, and execute commands for status, artifacts, recent logs, error coverage, support matrix, generator profile, and per-document analysis. Artifact reports cover Anchor projects plus deployable Pinocchio and native Solana Cargo programs.

The server advertises identifier and space trigger characters for completion so Anchor-aware suggestions can wake on the first typed prefix and after delimiter spaces. The server-side completion gate still filters normal Rust before returning items.

Diagnostics are Anchor-specific where they inspect Anchor syntax and Accounts semantics, and Solana-aware where they inspect local build artifacts. Current checks include Anchor syntax and Accounts parsing, missing/invalid account references, create/init context inference, structural constraint shape checks for `init`, `seeds`/`bump`, `realloc`, and `close`, plus security-smell checks for unchecked signers, unchecked program accounts, sysvars, token accounts, duplicate mutable account types, static-only PDA seeds, and SBPF artifact/keypair state for Anchor, Pinocchio, and native Solana programs. Live typing uses a hot lane for parser and stable Anchor-structure feedback; full usage, security, and project checks run through the cold path after idle/open/save or pull diagnostics.

The extension contributes these commands:

- `Seagrass: Status`
- `Seagrass: Analyze Document`
- `Seagrass: Artifacts`
- `Seagrass: Recent Logs`
- `Seagrass: Error Coverage`
- `Seagrass: Support Matrix`
- `Seagrass: Generator Profile`
- `Seagrass: Restart Server`
- `Seagrass: Output`

Server process settings are under `seagrass.*`. Changing `serverCommand`, `serverArgs`, `serverCwd`, `serverEnv`, or `diagnostics.transport` restarts the server. Changing analysis settings is sent through `workspace/didChangeConfiguration`.

`seagrass.diagnostics.transport` defaults to `push`. That is intentional for VS Code: enabling both push and pull diagnostics makes VS Code show the same problem twice.

`seagrass.diagnostics.coldPath` defaults to `idle`. Set it to `save` to avoid full cold diagnostics while typing, or `manual` when you only want explicit pull/command-driven full checks.
