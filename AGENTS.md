# Agent Guide

Status: Active

This repository is the Anchor workspace plus the local `anchor-lsp` server and
editor adapters. You should work in the real checkout, preserve unrelated dirty
changes, and verify the user-facing path before reporting success.

## Contents

- [Scope](#scope)
- [Working Rules](#working-rules)
- [Anchor LSP Workflow](#anchor-lsp-workflow)
- [Editor UX Standards](#editor-ux-standards)
- [Versioning And Releases](#versioning-and-releases)
- [Licensing](#licensing)
- [Verification Matrix](#verification-matrix)
- [Maintenance](#maintenance)

## Scope

Use this guide for changes in `/Users/ay/Documents/codes/solana/seagrass`,
especially:

- Anchor Rust crates, CLI, TypeScript packages, docs, and examples.
- `src/`, including the Seagrass language server.
- `editors/vscode` and `editors/zed`.
- Release metadata such as `VERSION`, `CHANGELOG.md`, `CONTRIBUTING.md`, and
  `bump-version.sh`.

## Working Rules

| Rule | What you should do |
| --- | --- |
| Use the real source tree | Confirm `pwd`, inspect the current worktree, and avoid reasoning from stale mirrors. |
| Keep scope tight | Change only files needed for the request. Preserve unrelated dirty files. |
| Do not take toy paths | Build production code, tests, and docs against the existing architecture. |
| Prefer generated or parsed evidence | Use parser/catalog/workspace data before string-only heuristics. |
| Verify the visible path | For editor work, prove diagnostics, completions, hovers, commands, and artifacts through the LSP/editor path, not only helper tests. |
| Document release effects | Update changelog, contribution notes, manifests, and release scripts when behavior or deliverables change. |

Before editing, check:

```sh
pwd
git status --short
```

Use `rg` or `rg --files` for search. Use `apply_patch` for manual file edits.
Do not use destructive git commands unless the user explicitly asks for them.

## Anchor LSP Workflow

`anchor-lsp` owns Anchor-specific diagnostics, completions, hovers, symbols,
navigation, and fixes. Rust-analyzer may run beside it for generic Rust support,
but do not offload Anchor behavior to another server.

When changing LSP behavior:

1. Start from the semantic source: parsed Rust, Anchor syntax, generated
   constraint catalogs, workspace indexes, IDLs, or local build artifacts.
2. Add focused unit tests for the new semantic rule.
3. Add or update `src/editor_ux_parity.rs` when the change affects what
   a user sees in Zed or VS Code.
4. Run the production gate before claiming the change is done.

Run the server locally with:

```sh
cargo run -p anchor-lsp
```

Run the production gate with:

```sh
bun scripts/verify-production.ts
```

## Editor UX Standards

Editor features should be immediate, ranked, and actionable.

| Surface | Standard |
| --- | --- |
| Diagnostics | Prefer specific Anchor/Solana messages over generic Rust parser wording. |
| Completions | Wake on first useful Anchor prefix and delimiter-space contexts, then filter semantically so normal Rust stays quiet. |
| Ranking | Rank inferred semantic matches above broad workspace symbols. |
| Quick fixes | Offer concrete edits when the server has enough evidence. |
| Hovers | Explain Anchor constraints and account semantics from the generated catalog or parsed evidence. |
| Zed | Defaults should populate Problems on open and stay live after edits. |
| VS Code | Avoid duplicate Problems by respecting VS Code diagnostics transport behavior. |

Good editor tests assert the whole user-visible flow together: first completion,
diagnostic payload, evidence, and quickfix title for the same fixture.

Avoid example-specific fixes. If a real project exposes a missed pattern, add a
fixture that survives renaming, reordering, and formatting changes.

## Versioning And Releases

The workspace uses SemVer and Keep a Changelog.

Keep these versions aligned:

- `VERSION`
- `[workspace.package].version` in `Cargo.toml`
- `editors/vscode/package.json`
- `editors/zed/Cargo.toml`
- `editors/zed/extension.toml`

`bun scripts/verify-production.ts` fails when LSP/editor versions drift.
`bump-version.sh` should update local editor manifests that are not handled by
`cargo-release`.

When behavior changes, update `CHANGELOG.md` under `[Unreleased]` with the
smallest accurate scope prefix, such as `lsp:`, `cli:`, `lang:`, `spl:`, `ts:`,
or `client:`.

When contributor workflow changes, update `CONTRIBUTING.md` in the same change.

## Licensing

The LSP overlay and local editor adapters use MIT licensing. Keep the license
metadata aligned across:

- `LICENSE`
- `Cargo.toml`
- `editors/vscode/package.json`
- `editors/zed/Cargo.toml`

Do not silently relicense the parent Anchor workspace. The root `LICENSE` and
root README describe the parent workspace license unless a file explicitly says
otherwise.

## Verification Matrix

| Change type | Minimum verification |
| --- | --- |
| LSP behavior | `cargo test -p anchor-lsp` plus targeted tests. |
| Editor-visible LSP behavior | `cargo test -p anchor-lsp editor_ux_parity -- --nocapture`. |
| LSP production readiness | `bun scripts/verify-production.ts`. |
| VS Code adapter | `cd editors/vscode && bun run check`. |
| Zed adapter | `cd editors/zed && cargo test`. |
| Zed wasm release artifact | `cd editors/zed && cargo build --target wasm32-wasip2 --release`, then copy the release wasm to `extension.wasm`. |
| Root Anchor Rust change | Relevant `cargo test`, `cargo build`, and formatting checks. |
| TypeScript package change | Relevant `yarn build`, `yarn test`, or package-local lint/check command. |

If you cannot run a relevant check, say exactly why and report the residual
risk.

## Maintenance

Audit this file whenever the LSP production gate, editor startup contract,
release process, or repository layout changes. At minimum, refresh it quarterly.

Keep examples concrete and local. If a command or path no longer works in this
checkout, update the guide in the same change that breaks it.

## Changelog

- 2026-05-25: Initial agent guide for Anchor LSP production work, editor UX
  expectations, release version alignment, and verification requirements.
