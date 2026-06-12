# Execution Plans

Self-contained plans for zero-context executors. Each plan carries its own context,
doctrine, file paths (verified at write time), steps, and acceptance criteria — read
nothing else to execute one, except this index for ordering.

## Execution order (numerical = execution; run 01 → 11)

| # | Plan | Depends on | Why this position |
|---|---|---|---|
| 01 | [plan-01-fp-families](plan-01-fp-families.md) | — (in flight; see its RESUME STATE) | Kills the 38 known FPs found on real mainnet programs; everything else assumes a non-lying baseline |
| 02 | [plan-02-debt-remediation](plan-02-debt-remediation.md) | plan-01 | Two P0 live bugs (topic-mismatch dedup break, catalog drift) + uncovered rot from [debt-inventory](debt-inventory.md) |
| 03 | [plan-03-truth-source-audit](plan-03-truth-source-audit.md) | plan-01 | Prunes/demotes heuristic lints; adds `TruthSource` enforcement to the registry |
| 04 | [plan-04-resolution-delegation](plan-04-resolution-delegation.md) | — (parallel-safe with 03) | Glob-import open-world guards + workspace-inherited feature detection |
| 05 | [plan-05-capability-registry](plan-05-capability-registry.md) | plan-04 (manifest plumbing) | Crate-agnostic recognition: solves the solana-program crate split, Pinocchio, and future frameworks in one mechanism |
| 06 | [plan-06-semantic-graph](plan-06-semantic-graph.md) | 01–05 recommended | Staged migration to the framework-agnostic semantic model; consumes 05's registry; long-running |
| 07 | [plan-07-distribution](plan-07-distribution.md) | **hard gate: 01 and 03 merged** | Widening install base before FP fixes multiplies exposure to known FPs |
| 08 | [plan-08-corpus-promotion](plan-08-corpus-promotion.md) | 01–06 merged | Triage the 18-program external corpus, fix the FP families it surfaces, flip discovery mode → hard gate; the backstop every later plan leans on |
| 09 | [plan-09-space-calculator](plan-09-space-calculator.md) | — (parallel-safe with 08/10; touches hover/lens/actions only) | `space =` size engine + hover/lens + checked-arithmetic and InitSpace quick fixes — daily-use wins while the big rocks land |
| 10 | [plan-10-call-graph](plan-10-call-graph.md) | plan-06 (model); **plan-08 strongly recommended first** (its gate is the FP backstop) | Rung (c): name-keyed call graph, defense check-propagation through reachable helpers, one offense POC (CPI-in-reachable-helper); unblocks the moat tier |
| 11 | [plan-11-silent-surfaces](plan-11-silent-surfaces.md) | — (parallel-safe; touches settings/CI/scripts, not diagnostics) | Generalizes the broken-`seagrass-ignore` lesson (d5e1b2e): parity guards for every silent control surface (settings keys, suppression paths, Seagrass.toml, release artifacts) + install smoke matrix on 3 OSes |

[debt-inventory.md](debt-inventory.md) is the findings register behind plan-02;
it also assigns P1 items to plans 03/05/06 — those plans absorb them at
execution time.

## Overlap resolution (binding)

- **plan-03 steps 4–5 duplicate plan-01 steps 1–2** (FP families A and B, approached
  slightly differently). Execute plan-01's version. When executing plan-03, treat its
  steps 4–5 as **verify-only**: confirm the regression tests from plan-01 exist and pass,
  then skip the code changes. If plan-01 was not executed first, follow plan-03's version
  and mark plan-01's steps 1–2 verify-only instead. Never apply both.
- **plan-06 Stage 3** re-ports the family-B fix as a model query. That is intentional
  (proof-of-concept port of an already-correct rule), not a duplicate fix.
- **plan-10 Step 5 modifies a test that plan-08 may also touch**
  (`ignores_unchecked_cpi_program_in_unreachable_split_helper`): plan-10 splits it
  into reachable/unreachable variants. If plan-08's triage renames or moves it,
  plan-10 follows the new name — the scenario, not the identifier, is binding.

## Invariants for every plan

1. Start from a green, committed baseline (`cargo test` + `cargo clippy --all-targets -- -D warnings`).
   Commit between plans — never let two plans' changes interleave in one tree.
2. Three truth sources: single-file syntax, pinned catalog (anchor-syn @ 4addac5 /
   solana-program =2.2.1), or toolchain oracle. No in-house whole-program Rust resolution.
3. Silence over guess. Corpus failures are fixed as general rules, never special-cased.
4. Severity derives from provability (registry invariant); do not bypass it.
