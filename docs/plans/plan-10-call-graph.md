# Plan 10 — Call Graph (Rung c): Sound Cross-Function Reasoning

## Context

`docs/diagnostics-strategy.md` names the inter-procedural call graph as "the single
highest-leverage rung — it unlocks the largest set of moat diagnostics and the
soundest cross-file reasoning at once." Today every security lint reasons within
one function body: a signer check inside a helper called by the handler is
invisible (FP risk), and a CPI inside a helper reachable from a handler is
unanalyzable (missed-finding risk). Both directions need the same artifact.

Most of the substrate already exists:

- Every function's outgoing calls are already captured at parse time:
  `InstructionSymbol.function_calls: Vec<FunctionCall>` (`src/core/document/mod.rs`
  ~:397), recorded by `record_function_call()`
  (`src/core/document/account_usage.rs:204–222`, called from `visit_expr_call` :338).
- The workspace index already persists them: `IndexedFunction.calls: Vec<String>`
  and `IndexedFunctionEntry.calls` (`src/core/workspace/indexing.rs:120–153`),
  extracted by `indexed_functions()` (:176–224) for both instructions and free
  functions.
- What's missing is purely: (1) method-call capture, (2) a graph with reachability
  queries over those edges, (3) consumers.

This plan builds the graph, applies it **defense-first** (suppress FPs when a
reachable helper performs the check), and lands exactly **one** offense diagnostic
as proof (CPI-in-reachable-helper), leaving the rest of the moat tier
(stale-after-CPI cross-fn, oracle staleness, authority reachability, signer
propagation through CPIs) to later plans that consume the same graph.

### Current state (verified file paths)

| File | Role |
|---|---|
| `src/core/document/account_usage.rs` | `record_function_call()` :204–222 — records last path segment of `ExprCall` callee; **does NOT record `ExprMethodCall`** (gap, Step 1) |
| `src/core/document/mod.rs` | `InstructionSymbol { function_calls, signer_checks, … }` ~:391–406 |
| `src/core/workspace/indexing.rs` | `IndexedFunction`/`IndexedFunctionEntry` :120–153 (`calls`, `signer_checks`, `cpi_program_usages`, `is_program_instruction`, `context_name`); `indexed_functions()` :176–224 |
| `src/core/workspace/mod.rs` | `WorkspaceIndex` — `functions_by_name` map ~:58; query API (`accounts_struct`, `resolve_account_field_path`, …); **no call-reachability query** (gap, Step 3) |
| `src/lsp/diagnostics/lint.rs` | `Region` :13–24 (`InstructionBody` vs `HelperFnBody`); region classification :279–307 |
| `src/lsp/diagnostics/security/mod.rs` | `collect_with_workspace()` :28–39; `SignerAuthorizationVisitor` SCOPE = `[AccountsStructField]`; `has_manual_signer_check(document, workspace_index, …)` — workspace-aware but not reachability-aware |
| `src/lsp/diagnostics/security/tests/core_tests.rs` | `ignores_unchecked_cpi_program_in_unreachable_split_helper` — documents today's blanket helper silence (Step 5 splits this into reachable/unreachable) |
| `crates/seagrass-framework/src/semantic/nodes.rs` | `SemanticModel` :4–9; `Instruction { signer_checks, cpi_calls, … }` :23–31; `Check`/`CheckKind` :111–124; `PopulatedFields` flags |
| `crates/seagrass-framework/src/semantic/queries.rs` | `missing_signer_diagnostics()` :12–32 — collects runtime checks from direct instruction bodies only |
| `src/anchor/extractor/instruction.rs` | `extract_instruction()` :9–55 — checks from `InstructionSymbol.signer_checks` (direct body only) |

### Architecture doctrine (must hold in every step)

1. **Edges are name-based and incomplete by construction** (no type-driven
   resolution, no trait dispatch, no macro expansion). The asymmetric usage rule
   that keeps this sound:
   - **Defense (suppression)**: an edge *may* suppress a diagnostic. Missing edges
     → missed suppression → residual FP (caught by the corpus gate). Name-collision
     edges → over-suppression → silence. Both failure modes are silence, never a
     wrong claim.
   - **Offense (new findings)**: a claim may only be made when the supporting call
     edge positively exists *and* the callee is unambiguous (exactly one candidate
     in the workspace for that name). Ambiguity → no claim.
2. Reachability is bounded: depth cap (const, e.g. 8) + visited set. Hitting the
   cap during a defense query → treat everything beyond as "might check" → suppress
   (silence over guess). Hitting it during an offense query → no claim.
3. The graph lives in the **workspace layer**, not the per-file semantic model.
   Semantic queries accept it as an optional parameter; `None` reproduces today's
   behavior exactly (the framework crate stays workspace-agnostic).
4. Severity derives from provability. The offense diagnostic reuses
   `SecurityCpiProgram` (Provability::Heuristic → WARNING ceiling); nothing new at
   ERROR.

## Non-Goals

- No dataflow/taint (rung d). "Reachable helper performs a signer check on account
  X" is name-and-shape evidence, not value flow.
- No cross-crate edges (calls into dependencies are absent from the index — they
  simply have no candidates and resolve per doctrine 1).
- No call-graph-powered ports of the remaining moat diagnostics (stale-after-CPI
  cross-fn, oracle staleness, authority reachability) — each is its own plan
  consuming this graph.
- No closure-call edges in v1 (a closure called as `f()` records nothing; document
  as known incompleteness — defense-safe per doctrine 1).
- No UI surface (no "show call graph" feature).

## Steps

### Step 0 — Baseline

```bash
cargo test 2>&1 | tail -3
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
```

Sequencing: execute after plan-08 (the flipped external-corpus gate is the FP
backstop this plan's suppression changes rely on). Plan-09 is disjoint
(hover/lens/actions) — parallel-safe.

### Step 1 — Close the capture gap: method calls

`record_function_call()` (`account_usage.rs:204`) only fires for `ExprCall` with a
path callee. Add `ExprMethodCall` capture: in the visitor's `visit_expr_method_call`
(add the override if absent), record `method.method` ident + span as a
`FunctionCall`. Method names land in the same name-keyed edge space as free
functions — acceptable per doctrine 1 (collisions only over-suppress).

Also confirm calls inside nested blocks/closures within a function body are
visited (syn's default recursion covers them once the visitor doesn't return
early) — add a test pinning it.

Tests (existing document test module): `function_calls_include_method_calls`,
`function_calls_include_calls_inside_closures`,
`function_calls_dedupe_repeated_callsites` (dedupe by name+range already exists
:213–218 — pin it).

### Step 2 — The graph (`src/core/workspace/call_graph.rs`, NEW)

```rust
/// Name-keyed call graph over the workspace's indexed functions.
/// Edges are syntactic (callee name), deliberately incomplete; see doctrine
/// in docs/plans/plan-10-call-graph.md — defense may use any edge, offense
/// requires unambiguous candidates.
pub struct CallGraph {
    /// function name → callee names (deduped).
    edges: HashMap<String, Vec<String>>,
    /// function name → number of distinct definitions in the workspace.
    definition_counts: HashMap<String, usize>,
}

pub const MAX_REACHABILITY_DEPTH: usize = 8;

impl CallGraph {
    pub fn build(functions: &[IndexedFunctionEntry]) -> Self;
    /// All function names reachable from `from` (BFS, visited set, depth cap).
    /// `truncated` = depth cap hit (callers apply doctrine 2).
    pub fn reachable_from(&self, from: &str) -> Reachability;
    pub fn is_unambiguous(&self, name: &str) -> bool;  // exactly 1 definition
}

pub struct Reachability {
    pub names: HashSet<String>,
    pub truncated: bool,
}
```

Built once per index build/update inside `WorkspaceIndex` (store as a field;
rebuild where `functions_by_name` is rebuilt — find the update path and hook the
same place; if indexing is incremental per-file, rebuilding the graph from the
already-held `IndexedFunctionEntry` list on each change is acceptable v1 cost —
it's a name-map fold, not a parse).

Tests: linear chain A→B→C; diamond; cycle terminates; depth cap sets `truncated`;
duplicate definitions counted; unknown name → empty reachability.

### Step 3 — Workspace query API (`src/core/workspace/mod.rs`)

```rust
/// Entries for every function reachable from `from` (excluding `from` itself).
pub fn reachable_function_entries(&self, from: &str) -> (Vec<&IndexedFunctionEntry>, /*truncated*/ bool);
```

Resolution from names → entries goes through the existing `functions_by_name` map;
multiple same-name entries are ALL returned (union — defense semantics; offense
callers must check `call_graph().is_unambiguous` themselves).

### Step 4 — Defense: propagate checks through reachable helpers

Two consumers, same shape:

1. **`has_manual_signer_check`** (`src/lsp/diagnostics/security/mod.rs`): today it
   consults the workspace for runtime checks in functions tied to the context.
   Extend: for each instruction whose `context_name` matches the accounts struct
   under analysis, also union the `signer_checks` of every entry in
   `reachable_function_entries(instruction.name)` **filtered to helpers that can
   see the accounts** — v1 filter: the helper's `context_name` is the same
   `Context<C>` (the dominant Anchor split-module pattern, e.g. blueshift's
   make/take/refund delegating to module helpers), OR the helper has any
   `signer_checks` naming the exact field under analysis. `truncated == true` →
   suppress (doctrine 2).
2. **Semantic query** (`crates/seagrass-framework/src/semantic/queries.rs`
   `missing_signer_diagnostics` :12): add a parallel entry point
   `missing_signer_diagnostics_with_reachability(model, reachable_checks:
   &HashMap<String /*instruction*/, HashSet<String /*account names checked*/>>)`
   so the framework crate stays workspace-free (doctrine 3); the main crate builds
   the map from the graph and calls it. The existing signature stays and delegates
   with an empty map.

Regression fixture FIRST (red→green): a two-file fixture — handler in
`#[program]` calling `validate_authority(&ctx)` defined in another module which
performs `ctx.accounts.authority.is_signer` — assert today's FP fires, then the
fix suppresses it. Add the mirror true-positive: helper exists but is NOT called
by the handler → diagnostic stays.

### Step 5 — Offense POC: CPI in a *reachable* helper

Today `arbitrary_cpi_program_diagnostics` is silent for all helpers
(`ignores_unchecked_cpi_program_in_unreachable_split_helper` pins this). Split the
silence using reachability:

- For each instruction, walk `reachable_function_entries(instruction.name)`; for
  entries with non-empty `cpi_program_usages` where the used account belongs to
  the instruction's `Context<C>` accounts struct and no program-check evidence
  exists for it anywhere on the instruction's reachable set → emit the existing
  `SecurityCpiProgram` diagnostic at the helper's CPI site, with
  `relatedInformation` pointing at the calling instruction.
- Offense gating per doctrine 1: every hop on the path must be unambiguous
  (`is_unambiguous`); `truncated` → no claim. Confidence stays Heuristic; severity
  WARNING via the registry — do not bypass.

Tests: rename the pinned test's scenario into two —
`ignores_unchecked_cpi_program_in_unreachable_split_helper` (unchanged behavior:
helper not called from any instruction) and
`reports_unchecked_cpi_program_in_reachable_split_helper` (helper called from the
handler → WARNING at the CPI site). Plus: ambiguous helper name (two definitions)
→ silent; program-check in a sibling reachable helper → silent.

### Step 6 — Corpus proof

```bash
cargo test corpus
scripts/fetch-corpus.sh && CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture
```

The defense change can only remove diagnostics, but the offense POC adds WARNINGs —
scan the external-corpus WARNING delta for the new topic on real protocols and
spot-check ~10 by hand. A WARNING storm on mainnet code = the reachability filter
is too loose; tighten the account-visibility filter (Step 4's filter) before
shipping, do not demote.

### Step 7 — Document

- `docs/diagnostics-strategy.md`: mark rung (c) as landed-for-defense + 1 offense
  POC; list the moat diagnostics now unblocked and their dependency on
  `CallGraph`/`reachable_function_entries`.
- `docs/plans/plan-06-distance-audit.md`: update the `security` row (cross-fn
  query target now has its substrate).
- Module-level doc comment in `call_graph.rs` carrying the doctrine-1 asymmetry
  rule (it is the load-bearing invariant for every future consumer).

## Acceptance Criteria

1. `cargo test` — 0 failed; count strictly greater than baseline.
2. `cargo clippy --all-targets -- -D warnings` — clean.
3. `cargo test call_graph` — build/reachability/cycle/depth/ambiguity tests pass.
4. The Step-4 fixture: FP suppressed when the checking helper is reachable;
   diagnostic preserved when it is not called.
5. Both Step-5 tests pass: unreachable helper silent, reachable helper WARNING;
   ambiguous-name and checked-elsewhere cases silent.
6. `cargo test corpus` green; external corpus run shows no new ERROR and a
   hand-reviewed WARNING delta recorded in the commit body.
7. `grep -n "MAX_REACHABILITY_DEPTH" src/core/workspace/call_graph.rs` — bounded
   reachability exists; no unbounded walk anywhere (`rg "reachable" --type rust`
   audit).
8. Framework crate has no workspace dependency
   (`grep -rn "WorkspaceIndex" crates/seagrass-framework/src/` — no matches).

## Risks / Edge Cases

- **Name collisions over-suppress (defense).** Two functions named `validate` —
  a check in either suppresses for both. Accepted: failure mode is silence.
  The corpus WARNING review (Step 6) is where systematic over-suppression would
  surface as "this lint never fires anymore" — compare true-positive test counts
  before/after.
- **Macro-generated calls are invisible** (e.g. handlers invoked via macro
  dispatch). Edges missing → defense misses suppression → residual FP family for
  macro-heavy codebases. Known limitation; corpus gate measures its real size.
- **Method-call edges are receiver-blind.** `ctx.accounts.validate()` and
  `foo.validate()` produce the same edge. Same over-suppression analysis as name
  collisions; offense is protected by the unambiguity rule.
- **Index rebuild cost.** Graph rebuild on every file change is O(functions ×
  mean-calls) name-map folding. If profiling shows it matters, make it lazy
  (build on first query after invalidation) — do not pre-optimize.
- **Trait/impl methods named like instructions** can create spurious
  instruction→helper edges. The `is_program_instruction` flag on entries lets
  consumers exclude instruction-to-instruction edges where that matters (the
  offense POC should: an instruction is not a "helper").
- **Don't grow offense scope mid-plan.** The temptation is to port stale-after-CPI
  here "while we're in there." Each moat diagnostic gets its own plan with its own
  corpus review; this plan's offense budget is exactly one POC.
