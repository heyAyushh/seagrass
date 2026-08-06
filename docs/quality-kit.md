# Quality Kit

Status: Active

Seagrass uses four complementary checks: regression tests, hotpath replay,
libFuzzer, and property tests.

Checkout builds use Rust `1.89.0`, pinned in `rust-toolchain.toml`. Nightly Rust
is reserved for libFuzzer targets because `cargo-fuzz` runs those targets on the
nightly toolchain.

## Regression Tests

Run the LSP suite:

```sh
cargo test -p seagrass
```

Editor-visible behavior should also touch `editor_ux_parity` when diagnostics,
completions, hovers, code actions, ranking, or execute commands change.

The Anchor preflight CLI fixture should keep detecting every preflight-covered
error without claiming runtime telemetry:

```sh
seagrass preflight fixtures/preflight/anchor-errors.json --json
```

## Hotpath Replay

Replay protocol sessions and enforce latency budgets:

```sh
bun scripts/perf-replay.ts
bun scripts/hotpath-replay-session.ts
```

The CI hotpath job blocks regressions against the latency budgets enforced by
`scripts/perf-replay.ts`: hover p99 < 50 ms, completion p99 < 100 ms,
diagnostics p99 < 150 ms, workspace scan p99 < 2 s.

## libFuzzer

Nightly fuzzing runs the LSP fuzz targets and uploads crash/corpus artifacts.
The long workflow runs target shards sequentially and uploads
`fuzz-readiness-<sha>.json`, which is the machine-readable source for release
readiness evidence.
Local runs require nightly Rust for both installing `cargo-fuzz` and executing
targets. The install step must not use the repository `1.89.0` pin because
`cargo-fuzz`'s locked dependency tree currently requires rustc `1.91+`.
`scripts/run-fuzz.sh` defaults to `-s none` because current nightly ASAN builds
fail to link with undefined `__sancov_gen_*` symbols (`rust-fuzz/cargo-fuzz#404`);
libFuzzer coverage instrumentation still runs.

PR CI runs a **Fuzz build smoke** job that installs `cargo-fuzz`, builds
`fuzz_document_parse`, and runs it for 15 seconds so install/MSRV and ASAN link
regressions fail before merge. The long scheduled workflow is still the source of
release-readiness hours; retrigger it from Actions → Seagrass Fuzz → Run workflow
(or `gh workflow run fuzz.yaml --ref <branch>`).

```sh
bash scripts/install-cargo-fuzz.sh --check-toolchain
bash scripts/install-cargo-fuzz.sh
bash scripts/run-fuzz.sh build fuzz_document_parse
bash scripts/run-fuzz.sh run fuzz_document_parse
bash scripts/run-fuzz.sh run fuzz_anchor_attr
bash scripts/run-fuzz.sh run fuzz_anchor_preflight
bash scripts/run-fuzz.sh run fuzz_manifest_parse
bash scripts/run-fuzz.sh run fuzz_semantic_diagnostics
```

Treat parser panics on malformed source as security-relevant until triaged.

## proptest

Property tests live in the relevant Rust crate and use the workspace `proptest`
dependency.

```sh
PROPTEST_CASES=10000 cargo test -p seagrass proptest
bun scripts/verify-proptest.ts
```

Prefer invariants that survive formatting, renaming, and input order changes.

## Release Evidence

Release readiness also requires evidence that cannot be produced by unit tests:
a clean 32 aggregate fuzz-hour run and external review signoff. Generate the machine-readable
objects with:

```sh
bun scripts/fuzz-readiness.ts --help
bun scripts/import-fuzz-artifacts.ts --help
bun scripts/review-readiness.ts --help
bun scripts/apply-release-readiness.ts --help
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

See `docs/release-readiness.md` for the current release-proof requirements and
the guardrails that keep the workflow evidence concrete.
