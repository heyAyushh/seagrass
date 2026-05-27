# 10/10 Completion Audit

Status: Active
Date: 2026-05-27

This audit maps every release-gate requirement to current evidence. It is
intentionally strict: local implementation work can pass while the release
remains blocked on external proof artifacts.

## Evidence Summary

| requirement | current evidence | status |
| --- | --- | --- |
| Diagnostics fire only on proven regions | `cargo test -p seagrass` includes region and false-positive fixtures; `ignores_deref_in_account_attribute` and `editor_ux_does_not_flag_account_attribute_deref_as_arithmetic` cover the motivating two-field Whirlpools token-owner attribute pattern with zero diagnostics; `bun scripts/check-diagnostic-audit.ts` verifies all 107 audit rows reference real source paths and concrete topics, every auditable diagnostics/completions/hover/actions provider source file has an audit row, and AST/region/confidence/risk/fixture cells use concrete structured evidence instead of bare prose. For `fixed:` substring-risk rows, fixture cells must name existing negative Rust regression tests. | satisfied locally |
| Diagnostic metadata includes source, code, confidence, topic, applicability | `diagnostics::tests::every_diagnostic_kind_carries_metadata_axes`; `bun scripts/check-diagnostic-topics.ts`; `bun test scripts/check-diagnostic-topics.test.ts` verifies manifest sorting, duplicate rejection, drift detection, production-source topic extraction, test-file exclusion, and schema/checker alignment. | satisfied locally |
| No raw-source diagnostic rule regressions | `bun scripts/check-rule-hygiene.ts` passes with no legacy findings. | satisfied locally |
| Duplicate diagnostics fold by span/topic with related information | `cargo test -p seagrass` covers arbitration and related-information enrichment. | satisfied locally |
| Public lint catalog covers every emitted topic | `bun scripts/check-lint-catalog.ts` reports 40 documented topics; `bun test scripts/check-lint-catalog.test.ts` verifies the checker rejects stale boilerplate and requires line, file, item, and workspace suppression forms. | satisfied locally |
| IDE latency budgets | `bun scripts/perf-replay.ts` passes and enforces hover < 50 ms, completion < 100 ms, diagnostics < 150 ms, and workspace scan < 2 s p99 budgets against `target/seagrass-hotpath-report.json`. | satisfied locally |
| Workspace scale N=200 | `bun scripts/scale-benchmark.ts --programs 200 --samples 3 --report target/seagrass-scale-200.json` passed with p99 cold start 1765.9 ms and p99 RSS 26.5 MB. | satisfied locally |
| Property tests | `bun scripts/verify-proptest.ts` passed with the CI default `PROPTEST_CASES=10000`. | satisfied locally |
| Research citations | `bun scripts/check-research-citations.ts` rejects mutable GitHub and googlesource source citations in `docs/lint-patterns-research.md`; upstream code references are pinned to commit SHAs. | satisfied locally |
| Release evidence schema rejects weak proof | `bun test scripts/check-release-readiness.test.ts scripts/apply-release-readiness.test.ts scripts/import-fuzz-artifacts.test.ts scripts/release-workflow.test.ts scripts/review-readiness.test.ts` passes; checker/importer rejects stale fuzz matrices, stale shard counts, missing shard archives, unsafe tar members, short fuzz windows, stale corpus hashes, missing signoffs, unrelated repo URLs, rejects placeholder workflow run ids, rejects generic review proof URLs, mutable changelog proof URLs, invalid merged artifacts, release packages missing readiness proof, release packages missing replay inputs, unchecked release assets, mutable VSIX release metadata URLs, unpinned workflow actions, and PR guardrails missing release-evidence tests. Checked-in pending evidence is tested to use explicit `pending` sentinels and zero fuzz hours. | satisfied locally |
| 24h clean fuzz run | Requires real `fuzz-readiness-<sha>.json` from `.github/workflows/lsp-fuzz.yaml`. Checked-in evidence is intentionally pending. | external blocker |
| External review signoff | Requires paid review signoff or 30-day community audit evidence with disposition. Checked-in evidence is intentionally pending. | external blocker |

## Commands Last Run

```sh
bun scripts/check-source-size.ts
bun scripts/check-rule-hygiene.ts
bun scripts/check-diagnostic-topics.ts
bun test scripts/check-diagnostic-topics.test.ts
bun scripts/check-diagnostic-audit.ts
bun test scripts/check-diagnostic-audit.test.ts
bun scripts/check-research-citations.ts
bun test scripts/check-research-citations.test.ts
bun scripts/check-lint-catalog.ts
bun test scripts/check-lint-catalog.test.ts
bun scripts/check-release-readiness.ts --allow-pending --version "$(tr -d '[:space:]' < VERSION)"
bun scripts/verify-proptest.ts
bun scripts/scale-benchmark.ts --programs 10 --samples 3 --report target/seagrass-scale-10.json
bun scripts/scale-benchmark.ts --programs 50 --samples 3 --report target/seagrass-scale-50.json
bun scripts/scale-benchmark.ts --programs 200 --samples 3 --report target/seagrass-scale-200.json
bun scripts/verify-production.ts
```

## Release Blockers

Do not mark `docs/release-readiness.json` final until both artifacts exist:

- `fuzzCleanRun.status = "passed"` from a real 24h clean fuzz workflow run.
- `externalReview.status = "signed-off"` with at least one signoff URL and
  findings-disposition URL under the current GitHub repo.
