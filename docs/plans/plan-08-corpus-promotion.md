# Plan 08 — Corpus Promotion: Triage the External Corpus and Flip Discovery → Hard Gate

## Context

The corpus mechanism has two tiers, both already built:

1. **Committed corpus** (`fixtures/corpus/`) — 3 programs (`blueshift_anchor_escrow`,
   `fp_family_a_token_interface`, `fp_family_b_composite_payer`), hard zero-ERROR
   gate on every test run via `all_corpus_programs_have_no_error_diagnostics()`
   (`src/lsp/diagnostics/tests/corpus.rs:222`). New subdirectories are auto-discovered;
   no test registration needed.
2. **External corpus** (`corpus/manifest.toml`, 18 programs pinned at exact SHAs —
   8 active mainnet protocols incl. orca-whirlpools, kamino-lending, drift,
   marginfi-v2, squads-v4; 4 stale; 6 educational) — fetched by
   `scripts/fetch-corpus.sh` (blobless sparse clone), run nightly by
   `.github/workflows/corpus.yml` with `CORPUS_ENABLED=1`. The test `external_corpus()`
   (`src/lsp/diagnostics/tests/corpus.rs:281`) runs in **discovery mode**: it prints
   ERROR counts but always passes (comment at line 371: "Discovery mode: always pass.
   Flip to assert_no_errors_in_program after triage").

This plan is that triage and flip. It is mostly *judgment work on real findings*,
not new infrastructure.

### The compile-truth doctrine (what makes triage tractable)

ERROR severity is reserved for syntactically provable claims (registry invariant:
`Provability::Syntactic → ERROR`, `src/lsp/diagnostics/registry.rs:275`). Every
external-corpus program is a deployed or published protocol whose pinned source
compiles. Therefore:

> **Any ERROR-severity diagnostic on an external-corpus program is a false positive
> by definition** — unless the *fetched tree itself* is not the compiling artifact
> (sparse-checkout dropped a module, `cfg`-gated code, generated code absent).

So triage has exactly two buckets: (1) FP → fix as a general rule, (2) broken
fetched tree → excluded with a documented reason. There is no "real bug found in
Drift" bucket at ERROR level; WARNINGs and HINTs are out of scope for the gate.

### Current state (verified file paths)

| File | Role |
|---|---|
| `src/lsp/diagnostics/tests/corpus.rs` | All corpus tests. Key items: `committed_corpus_root()` :19, `external_programs_dir()` :25 (`corpus/programs/`, gitignored), `find_program_src_dirs()` :52, `ReadFailurePolicy` :115 (`Panic`/`Skip`), `assert_no_errors_in_program()` :126, `scan_program()` :160 (builds `WorkspaceIndex`, collects via `collect_with_workspace`, filters `DiagnosticSeverity::ERROR`), committed gate :222, blueshift guard :247, `external_corpus()` :281 (discovery mode), oversized-file smoke test :374 |
| `corpus/manifest.toml` | `[[program]]` entries: `name`, `repo`, `sha` (40-char), optional `subpath`, `license` |
| `scripts/fetch-corpus.sh` | Manifest-driven blobless sparse clone; `--force` to re-clone; requires git ≥2.36, python3 ≥3.11 |
| `.github/workflows/corpus.yml` | Nightly (02:17 UTC) + manual + push-to-main-on-manifest-change; caches `corpus/programs/` keyed on manifest hash; runs `CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture` |
| `fixtures/corpus/README.md` | Vendoring instructions: copy only `src/**/*.rs`, program `Cargo.toml`, workspace `Anchor.toml` |
| `docs/diagnostics-strategy.md` | Lines 63–68: golden corpus + compile-truth rationale |

### Architecture doctrine (must hold in every step)

1. Corpus failures are fixed as **general rules, never special-cased** (README
   invariant 3). An FP found on Drift is fixed for the *pattern*, and the regression
   fixture is a minimal reproduction, not Drift's source.
2. Silence over guess: when a fix would require whole-program evidence the engine
   doesn't have, the diagnostic is suppressed/demoted per the registry, not patched
   around.
3. License discipline: **only MIT or Apache-2.0 sources may be vendored into
   `fixtures/corpus/`**. `CC-BY-NC-4.0` and `unlicensed` manifest entries
   (solana-programs-list, the unlicensed submodules) may be *fetched and scanned*
   but never committed. Check the `license` field in the manifest before copying
   anything.
4. Severity derives from provability; FP fixes must not bypass the registry.

## Non-Goals

- No gating on WARNING/HINT severity. The compile-truth argument only covers ERROR.
- No `cargo check`/`anchor build` subprocess in the test. Compile-truth is
  established by *provenance* (pinned mainnet source), not by compiling in CI.
- No new diagnostics. This plan only removes lies.
- No expansion of the manifest itself (adding new protocols is routine maintenance,
  not this plan) — with the one exception of Step 4's committed vendoring.

## Steps

### Step 0 — Baseline

```bash
cargo test 2>&1 | tail -3      # 0 failed required (1545 at plan time)
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
git status                      # clean tree
```

### Step 1 — Materialize and capture the baseline report

```bash
scripts/fetch-corpus.sh
CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture 2>&1 | tee /tmp/corpus-baseline.txt
```

Capture: total programs scanned, total ERROR diagnostics, total oversized-file
skips. The per-error lines include file path and diagnostic message (the scan
collects `(PathBuf, String)` pairs).

### Step 2 — Triage table

Create `docs/plans/plan-08-triage.md`. One row per distinct (diagnostic topic ×
syntactic pattern) — *not* per occurrence. Columns:

| topic | pattern (1-line) | example file | count | bucket | disposition |

Buckets:
- **FP-family** — engine claims something provably wrong about compiling code.
  Disposition: named family (`fp_family_c_…`, `fp_family_d_…`), fixed in Step 3.
- **Broken-tree** — the fetched source is not the compiling artifact: sparse
  checkout dropped a referenced module, `cfg(feature=…)`-gated code with the
  feature's deps absent, build-generated code (`include!` of OUT_DIR). Disposition:
  exclusion entry in Step 5 with the reason copied into the table.

If a finding doesn't fit either bucket cleanly, it is by definition an FP-family
finding whose generalization isn't understood yet — keep investigating; do not
create a third bucket.

### Step 3 — Fix FP families as general rules

For each FP-family row, in its own commit:

1. Write the minimal reproduction first: `fixtures/corpus/fp_family_<x>_<slug>/`
   following the exact layout of `fp_family_b_composite_payer/`
   (`Anchor.toml` + `programs/<name>/Cargo.toml` + `programs/<name>/src/lib.rs`,
   ~40 lines). The committed gate at corpus.rs:222 picks it up automatically and
   fails — red first.
2. Fix the rule generally. Honor the overlap rules: resolution claims go through
   workspace facts (plan-04), recognition through the capability registry (plan-05),
   account-shape reasoning through the semantic model where already ported (plan-06).
   Do not re-introduce path-string matching or name heuristics.
3. Preserve true positives: every fix commit must show at least one existing
   true-positive test still passing for the touched rule (name it in the commit body).

### Step 4 — Vendor 3–5 permissively-licensed real programs into the committed gate

The committed corpus is what runs on *every* `cargo test`, not just nightly. Grow it
from 3 to 6–8 programs, chosen for idiom coverage rather than size:

- From `program-examples` (MIT): one CPI-heavy program and one PDA/realloc program.
- One Apache-2.0/MIT SPL program small enough to commit (check `license` in manifest;
  `mpl-token-metadata` is large — prefer a smaller one or a single-crate subpath).
- One Pinocchio or native program **only if** a permissively-licensed one exists
  (the manifest's pinocchio entries are `unlicensed` submodules — those cannot be
  vendored; find an MIT alternative or skip, noting it in the triage doc).

Per program: copy only `src/**/*.rs`, program `Cargo.toml`, workspace `Anchor.toml`
(per `fixtures/corpus/README.md`); record upstream repo + SHA + license in that
README. Auto-discovery means no test changes. If a newly vendored program trips the
gate, that is a Step-2 finding — triage it, don't drop the program.

### Step 5 — Flip the external gate

In `src/lsp/diagnostics/tests/corpus.rs`:

1. Add an exclusion table near the top:

```rust
/// Fetched trees that are not the compiling artifact (sparse checkout, cfg-gated,
/// or generated code). Every entry needs a reason; an empty reason is a test failure.
/// Exclusions are path prefixes relative to corpus/programs/.
const EXTERNAL_CORPUS_EXCLUSIONS: &[(&str, &str)] = &[
    // ("drift/programs/drift/src/generated", "include!-ed OUT_DIR code absent from sparse tree"),
];
```

2. Rewrite `external_corpus()` (line 281): after scanning, filter errors whose path
   matches an exclusion prefix, then **assert the remainder is empty**, printing the
   same per-error detail on failure that discovery mode printed. Keep
   `ReadFailurePolicy::Skip` and the oversized-file accounting (skips are reported
   in the summary, not asserted — file-size limits are an engine setting, not a
   corpus property).
3. Add a companion test `external_corpus_exclusions_have_reasons()` asserting every
   exclusion has a non-empty reason, and (cheaply) that the excluded path prefix
   exists when `CORPUS_ENABLED=1` — a stale exclusion should be deleted, not rot.
4. Delete the "Discovery mode: always pass" comment block.

### Step 6 — Keep CI honest

`.github/workflows/corpus.yml` needs no structural change — the flipped test now
fails the nightly run on regression. Two small hardening edits:

- Add `pull_request` trigger filtered to `paths: [src/lsp/diagnostics/**, src/core/**, crates/seagrass-framework/**]`
  so diagnostic-engine PRs run the external gate before merge (cache makes this
  cheap; fetch is skipped when the manifest hash matches).
- On failure, upload the scan output as a workflow artifact so triage doesn't
  require a local re-run.

### Step 7 — Document

- `docs/diagnostics-strategy.md`: update the corpus status — external corpus is now
  a hard gate; record date and the count of FP families fixed.
- `fixtures/corpus/README.md`: list the newly vendored programs with repo/SHA/license.
- `docs/plans/plan-08-triage.md` stays as the permanent triage record.

## Acceptance Criteria

1. `cargo test` — 0 failed; committed corpus has ≥6 programs
   (`ls fixtures/corpus/ | wc -l` ≥ 6).
2. `cargo clippy --all-targets -- -D warnings` — clean.
3. `CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture` — **passes as a hard
   assertion** (not discovery), with the exclusion table either empty or every entry
   reasoned.
4. `grep -n "Discovery mode" src/lsp/diagnostics/tests/corpus.rs` — no matches.
5. Every FP family from triage has a `fixtures/corpus/fp_family_*` reproduction and
   a one-commit general fix referencing a preserved true-positive test.
6. `git log --oneline` shows one commit per FP family (no mixed fixes).
7. No vendored program in `fixtures/corpus/` lacks an MIT/Apache-2.0 upstream license
   recorded in `fixtures/corpus/README.md`.

## Risks / Edge Cases

- **Triage volume unknown.** Nightly discovery counts are the sizing signal — read
  the latest workflow run's output before estimating. If a single family dominates
  (likely: one rule, many protocols), the fix list is short even if the count is large.
- **Sparse-checkout artifacts masquerading as FPs.** A missing module makes
  `mod foo;` unresolvable and can cascade. Before declaring an FP, check whether the
  referenced path exists in the fetched tree (`ls` the sibling files). Cascades from
  a genuinely absent file are Broken-tree, not FP.
- **Manifest SHA drift.** Upstream force-pushes can orphan pinned SHAs and break
  fetch. Fetch failures are infra, not corpus findings; fix the manifest entry in a
  standalone commit.
- **Stale entries** (openbook-v2, mango-v4, tensor-amm, spl-token-swap) use older
  Anchor versions — FPs found only there may be version-specific idioms. Still fix
  generally (the engine doesn't know the Anchor version), but note the version in
  the triage row.
- **Over-suppression temptation.** The fastest way to make the gate green is to
  demote rules wholesale. The registry invariant (severity from provability) is the
  guard: a demotion needs a provability argument, not a corpus-pressure argument.
  When in doubt, prefer a narrower claim that stays ERROR over a blanket demotion.
- **CI cache poisoning.** The cache key is the manifest hash with no restore-keys —
  correct as-is; do not add restore-keys (a stale tree under a new manifest would
  scan the wrong SHAs).
