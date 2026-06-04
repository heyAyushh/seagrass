# Audit Reconciliation - 2026-06-04

Status: Active

This note records the corrected audit state after re-checking the Seagrass
repository. It separates refuted claims from confirmed findings, and it records
which confirmed findings were resolved by the current cleanup.

## Refuted Claims

These earlier claims were wrong:

- "No clippy/fmt in the PR gate" is false. `.github/workflows/pr.yaml` runs
  `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`, and
  `bun scripts/protocol-smoke.ts` on every PR/push.
- "protocol-smoke only runs on release" is false. It runs on every PR.
- "AGENTS.md / guard script hardcode /Users/ay/..." is false. Both use
  `git rev-parse --show-toplevel`.
- "Zed cp path is wrong" is false. `../../target/` is correct for a workspace
  member.
- "CHANGELOG skips 1.0.0/1.0.1" is false. Both entries exist.
- "cursor/opencode have no README" is false. Both have README files.
- "SECURITY.md fallback is a dead noreply email" is false. The fallback is to
  open a public issue asking maintainers to enable a private reporting channel.
- "TextDocumentSyncKind::FULL plus synchronous syn parse on every keystroke" is
  false. Seagrass advertises incremental sync and parses on a blocking worker
  thread.
- "references is single-file only" is false. Cross-file references use the
  workspace index, with single-file behavior as a fallback.
- "didChange drops all but the last change" is false. Range changes are applied
  in order.
- "release-plz has workflow-level write" is false. Workflow-level permissions
  are `contents: read`; write permissions are job-scoped.
- "anchor-syn pinned by 8-char rev" is false. The dependency is pinned by a
  full 40-character SHA.
- "unbounded reads on Cargo.toml/Anchor.toml/project files" is no longer true
  after the read-limit hardening work.

## Confirmed Findings

These findings were real when audited:

1. Watched-file updates triggered a full workspace re-walk through
   `WorkspaceIndex::build`, using `params.changes` only for logging.
2. A Salsa-backed `AnchorAnalysis` computation was built, memoized, and then not
   consumed by the root LSP path.
3. Four generated catalog files under
   `crates/seagrass-anchor-v1/src/generated/` were dead duplicates; only
   `anchor_support_generated.rs` was included.
4. Framework crates existed but the root LSP did not call the Pinocchio/native
   crates.
5. Pinocchio/native support was narrower than the README implied.
6. Formatting capability was not exposed.
7. Test coverage still has structural gaps: coverage tooling is absent.

## Resolved In Current Cleanup

The following confirmed findings have been addressed:

1. Watched-file invalidation now applies file-granular workspace-index updates
   where possible instead of rebuilding every root for every file event.
2. The dead Salsa diagnostics computation and unused `AnchorAnalysis` surface
   were removed instead of left memoized and unread.
3. Dead duplicate generated catalog files under
   `crates/seagrass-anchor-v1/src/generated/` were removed.
4. The root LSP now calls framework crates for native Solana and Pinocchio
   diagnostics through `src/lsp/diagnostics/code_quality/mod.rs`.
5. Native Solana and Pinocchio raw-account, signer, and CPI diagnostics now live
   behind framework-owned crate APIs:
   - `crates/seagrass-native/src/lib.rs`
   - `crates/seagrass-pinocchio/src/lib.rs`
   - `crates/seagrass-framework/src/native_rules/`
6. Shared framework helpers for diagnostic payloads, ranges, and lint-region
   filtering now live in `crates/seagrass-framework/src/`.
7. Framework-level tests and editor parity coverage were added for native and
   Pinocchio security diagnostics.
8. The diagnostic audit checker now accepts crate-owned provider paths and scans
   framework diagnostics for emitted topics and quickfix tags.
9. Document formatting is exposed through the standard LSP formatting provider
   and backed by `rustfmt` for open editor buffers.
10. Protocol smoke now verifies document formatting through JSON-RPC.
11. Fuzzing includes semantic diagnostic collection in addition to parse-focused
    targets.
12. PR guardrails use the same proptest depth as release/property workflows.
13. CI Bun installs now verify pinned npm package integrity before installation.
14. Rust source collection now has explicit depth and file-count traversal
    budgets.
15. Native Solana and Pinocchio applicable parity is documented in
    `docs/framework-parity.md` and enforced through a black-box JSON-RPC LSP
    integration test that covers formatting, framework diagnostic metadata, and
    framework-correct quick fixes for raw-account, signer, writable-account, and
    CPI program-id findings.

## Remaining Backlog

These items are still real and should not be described as complete:

1. Native Solana and Pinocchio do not implement Anchor-only constraint,
   Accounts, IDL, or generated type surfaces; those are documented as
   non-applicable rather than parity gaps.
2. Test infrastructure still needs coverage tooling and broader JSON-RPC
   end-to-end cases beyond the protocol smoke path.
3. Runtime demos need execution proof before they are described as complete:
   add a runtime evidence ingestion demo backed by real artifacts such as
   Trident coverage, LiteSVM/Mollusk traces, or simulation logs. Preflight has a
   checked-in CLI fixture under `fixtures/preflight/anchor-errors.json`.

## Partial Or Nuanced Findings

- Parse errors now explicitly report that semantic diagnostics are paused until
  the Rust file parses again. This is a visible degraded-state choice, not a
  hidden failure.
- Anchor v2 detection was a narrow real gap. It now classifies real
  `anchor-lang` / `anchor-spl` 2.x dependencies as Anchor v2 preview.
- Lint-doc boilerplate count needs a fresh dedicated audit before quoting a
  number.
- README references to a missing VS Code launch config were confirmed and
  cleaned up in the root README.

## Current Assessment

The first audit overstated CI, docs onboarding, sync protocol, and reference
navigation issues. The verified backlog is now narrower: finish non-Anchor
framework parity beyond the moved native security diagnostics, grow
end-to-end/fuzz/coverage infrastructure, harden Bun installation, and cap source
traversal depth.

## Verification Notes

The framework wiring and moved rules were checked with:

- `cargo test -p seagrass-framework -p seagrass-pinocchio -p seagrass-native`
- `cargo test -p seagrass code_quality::tests::native_ -- --nocapture`
- `cargo test -p seagrass editor_ux_surfaces_pinocchio_native_security_metadata -- --nocapture`
- `cargo clippy -p seagrass-framework -p seagrass-pinocchio -p seagrass-native --all-targets -- -D warnings`
- `cargo clippy -p seagrass --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `bun scripts/check-source-size.ts`
- `bun scripts/check-diagnostic-audit.ts`
- `bun scripts/check-lint-catalog.ts`
