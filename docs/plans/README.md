# Execution Plans

Self-contained plans for zero-context executors. Each plan carries its own context,
doctrine, file paths (verified at write time), steps, and acceptance criteria — read
nothing else to execute one, except this index for ordering.

## Execution order

| # | Plan | Depends on | Why this position |
|---|---|---|---|
| 1 | [plan-01-fp-families](plan-01-fp-families.md) | — | Kills the 38 known FPs found on real mainnet programs; everything else assumes a non-lying baseline |
| 2 | [plan-02-truth-source-audit](plan-02-truth-source-audit.md) | plan-01 | Prunes/demotes heuristic lints; adds `TruthSource` enforcement to the registry |
| 3 | [plan-03-resolution-delegation](plan-03-resolution-delegation.md) | — (parallel-safe with 2) | Glob-import open-world guards + workspace-inherited feature detection |
| 4 | [plan-04-semantic-graph](plan-04-semantic-graph.md) | 1–3 recommended | Staged migration to the framework-agnostic semantic model; long-running |
| 5 | [plan-05-distribution](plan-05-distribution.md) | **hard gate: 1 and 2 merged** | Widening install base before FP fixes multiplies exposure to known FPs |

## Overlap resolution (binding)

- **plan-02 steps 4–5 duplicate plan-01 steps 1–2** (FP families A and B, approached
  slightly differently). Execute plan-01's version. When executing plan-02, treat its
  steps 4–5 as **verify-only**: confirm the regression tests from plan-01 exist and pass,
  then skip the code changes. If plan-01 was not executed first, follow plan-02's version
  and mark plan-01's steps 1–2 verify-only instead. Never apply both.
- **plan-04 Stage 3** re-ports the family-B fix as a model query. That is intentional
  (proof-of-concept port of an already-correct rule), not a duplicate fix.

## Invariants for every plan

1. Start from a green, committed baseline (`cargo test` + `cargo clippy --all-targets -- -D warnings`).
   Commit between plans — never let two plans' changes interleave in one tree.
2. Three truth sources: single-file syntax, pinned catalog (anchor-syn @ 4addac5 /
   solana-program =2.2.1), or toolchain oracle. No in-house whole-program Rust resolution.
3. Silence over guess. Corpus failures are fixed as general rules, never special-cased.
4. Severity derives from provability (registry invariant); do not bypass it.
