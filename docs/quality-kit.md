# Quality Kit

Status: Active

Seagrass uses four complementary checks: regression tests, hotpath replay,
libFuzzer, and property tests.

## Regression Tests

Run the LSP suite:

```sh
cargo test -p seagrass
```

Editor-visible behavior should also touch `editor_ux_parity` when diagnostics,
completions, hovers, code actions, ranking, or execute commands change.

## Hotpath Replay

Replay protocol sessions and enforce latency budgets:

```sh
bun lsp/scripts/perf-replay.ts
bun lsp/scripts/hotpath-replay-session.ts --fixture lsp/fixtures/broken.rs
```

The CI hotpath job blocks regressions against the latency budgets enforced by
`lsp/scripts/perf-replay.ts`: hover p99 < 50 ms, completion p99 < 100 ms,
diagnostics p99 < 150 ms, workspace scan p99 < 2 s.

## libFuzzer

Nightly fuzzing runs the LSP fuzz targets and uploads crash/corpus artifacts.
The long workflow runs target shards sequentially and uploads
`fuzz-readiness-<sha>.json`, which is the machine-readable source for release
readiness evidence.
Local runs require nightly Rust:

```sh
cd lsp/fuzz
cargo +nightly fuzz run fuzz_document_parse
cargo +nightly fuzz run fuzz_anchor_attr
cargo +nightly fuzz run fuzz_manifest_parse
```

Treat parser panics on malformed source as security-relevant until triaged.

## proptest

Property tests live in the relevant Rust crate and use the workspace `proptest`
dependency.

```sh
PROPTEST_CASES=10000 cargo test -p seagrass proptest
bun lsp/scripts/verify-proptest.ts
```

Prefer invariants that survive formatting, renaming, and input order changes.

## Release Evidence

Release readiness also requires evidence that cannot be produced by unit tests:
a clean 24h fuzz run and external review signoff. Generate the machine-readable
objects with:

```sh
bun lsp/scripts/fuzz-readiness.ts --help
bun lsp/scripts/import-fuzz-artifacts.ts --help
bun lsp/scripts/review-readiness.ts --help
bun lsp/scripts/apply-release-readiness.ts --help
```

The release checker binds fuzz evidence to the checked-in fuzz workflow matrix
and shard count, requires a concrete GitHub Actions run URL with a positive run
id, and binds review proof to concrete pull, issue, discussion, and CHANGELOG.md
URLs under the current `origin` GitHub repo. Stale matrix claims, stale shard
claims, placeholder run ids, generic review pages, and unrelated review links
fail before release. The import command safely stages downloaded fuzz artifacts;
the apply command merges the generated fuzz and review artifacts only after the
strict checker passes. PR guardrails run the release-evidence test suite so
those checks cannot drift before the tag workflow runs.

See `lsp/docs/10-10-completion-audit.md` for the current requirement-by-
requirement evidence map.
