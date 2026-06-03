# Seagrass Cursor Rule

This directory contains the project rule Cursor can load for Seagrass-aware
Solana Rust editing.

Activate it from the repository root:

```sh
mkdir -p .cursor/rules
ln -sfn ../../editors/cursor/rules/seagrass.mdc .cursor/rules/seagrass.mdc
```

The rule tells Cursor to prefer `seagrass diagnostics --json` and the structured
LSP command payloads in `docs/agents.md` before editing Anchor account logic.
