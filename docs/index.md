# Seagrass Documentation

Seagrass is codebase intelligence for Solana programs. It ships as a standalone
language server and CLI that owns Anchor, Pinocchio, native Solana, artifact,
security, and release-proof semantics while rust-analyzer owns generic Rust.

## Start Here

- [Agent and automation commands](agents.md) covers `seagrass diagnostics`,
  SARIF, stdin mode, and LSP `workspace/executeCommand` payloads.
- [Product framing](product-framing.md) defines the shipped static intelligence
  layer and the evidence requirements for runtime compute/traffic claims.
- [Lint catalog](lints/README.md) describes every diagnostic topic, false-positive
  boundary, suppression form, and documentation-link contract.
- [Diagnostic architecture](diagnostic-architecture.md) explains how Seagrass
  routes parsed evidence into editor diagnostics and fixes.
- [Quality kit](quality-kit.md) records the release proof expectations for
  fuzz, property, protocol, and production checks.

## Build The Searchable Site

```sh
mkdocs build --strict
```

The MkDocs search plugin indexes Markdown pages under `docs/`. Regenerate the
HTML lint topic index separately with:

```sh
bun scripts/build-lint-docs-index.ts
```
