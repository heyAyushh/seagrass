# Plan 02 — Debt Remediation: Live Bugs + Uncovered Rot from the Debt Inventory

## Context

Source: `docs/plans/debt-inventory.md` (2026-06-10 three-way scan). This plan
executes ONLY the items with no owner plan. Items owned elsewhere are out of
scope here: wrapper-type list collapse (plan-06), crate-detection lists
(plan-05), SPL type strings (plan-01/06), raw diagnostic-code strings and
confidence round-trip (plan-03).

**Repo:** `/Users/ay/Documents/codes/solana/seagrass`. Baseline: run
`cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`;
both must be green and the tree committed before starting (if plan-01 work is
in flight, land it first — Part A touches `src/lsp/diagnostics/`).

Doctrine (binding): three truth sources (single-file syntax / generated catalog /
toolchain); silence over guess; general fixes only, never special-cases; severity
derives from provability — do not bypass the registry.

## Non-Goals

- No new diagnostics, no severity changes, no constraint-catalog content changes
  beyond regeneration.
- No wrapper-type list consolidation (plan-06) and no crate-name registry
  (plan-05).
- No CI restructuring beyond the one drift-gate job/test in Part B.

---

## Part A — P0.1: Emitter topic mismatch defeats arbitration dedup (live bug)

**Facts:** `src/lsp/diagnostics/handler_members.rs:33` and
`src/lsp/diagnostics/handler_struct_literals.rs:26` declare
`TOPIC = "seagrass/anchor.account.usage"` while emitting diagnostics whose
registry kind is `AnchorMissingAccountReference`, whose `topic()` in
`src/lsp/diagnostics/registry.rs` is `"seagrass/anchor.constraint.account-reference"`.
Arbitration (`src/lsp/diagnostics/arbitration.rs`,
`prefer_highest_confidence_by_topic`) groups by `(range, topic)` — the mismatch
prevents dedup of overlapping diagnostics, showing doubles to users.

**Steps:**
1. Read both files. Determine the diagnostic kind each actually emits (check the
   code constant used when constructing the Diagnostic). If the emitted kind is
   `AnchorMissingAccountReference`, replace the local `TOPIC` literal with
   `AnchorDiagnosticKind::AnchorMissingAccountReference.topic()` (or the registry
   constant it returns). If investigation shows the emitter intentionally uses a
   different kind, align topic to THAT kind's registry topic instead — the
   invariant is emitter-topic == registry-topic-of-emitted-kind, not any specific
   string.
2. Search for other drifted emitters:
   `rg -n 'TOPIC\s*[:=]' src/ crates/ -g '*.rs'` — for every hardcoded topic
   literal, verify it equals the registry topic of the kind emitted alongside it.
   Fix all mismatches the same way (reference, not literal).
3. Add an enforcement test (in `src/lsp/diagnostics/tests/mod.rs` or the
   arbitration test module): construct one diagnostic from each emitter module
   that exposes a TOPIC (minimal fixtures), and assert
   `diagnostic.topic == kind.topic()` for the kind identified by its code. If
   per-emitter construction is impractical, the minimum viable gate is: no `TOPIC`
   constant in `src/lsp/diagnostics/` may contain a literal string — they must be
   initialized from registry methods (enforce via a grep-style test over source,
   mirroring existing hygiene checks in `scripts/check-rule-hygiene.ts` if a
   source-scan test already exists there — check first).
4. Regression test for the user-visible symptom: a fixture where
   handler_members and constraint_expressions both flag the same unresolved
   reference at the same span must yield ONE diagnostic after
   `arbitration::arbitrate`, not two.

**Acceptance:** new tests pass; `rg -n '"seagrass/anchor.account.usage"' src/lsp/diagnostics/handler_members.rs src/lsp/diagnostics/handler_struct_literals.rs`
returns nothing; full suite green.

---

## Part B — P0.2: Generated catalogs drifted across crates (live bug)

**Facts:** three copies of generated catalogs exist:
- `src/anchor/generated/` (main)
- `crates/seagrass-anchor-v1/src/generated/`
- `crates/seagrass-anchor-v2-preview/src/generated/`

Verified drift: v2-preview `constraint_catalog_generated.rs` has 40 specs vs
main's 43 (missing `extensions::group_pointer::authority`,
`extensions::group_pointer::group_address`, `extensions::pausable::authority`);
v2-preview `anchor_error_catalog_generated.rs` lacks
`ConstraintMintPausableExtension` (2043) and `ConstraintMintPausableAuthority`
(2044). No gate compares the copies. The generator is `scripts/regen-support.ts`
(verify name with `ls scripts/ | grep -i regen`).

**Steps:**
1. Inspect the generator to learn whether the v2-preview catalog is generated
   from a DIFFERENT anchor source (intentional divergence — v2 may genuinely not
   have those constraints) or from the same pinned anchor-syn (drift = stale
   generation). Read the generator header comments and `scripts/anchor-source.ts`.
   THIS DETERMINES THE FIX:
   - Same source intended → re-run the generator for all three targets, commit
     the regenerated files, and the gate asserts byte-equality of catalogs across
     crates.
   - Different sources intended → the gate asserts each catalog matches a fresh
     generation from ITS OWN pinned source (no cross-crate equality), and the
     "drift" is reclassified as correct. Document whichever is true in the
     generated-file header.
2. Add the gate so this cannot recur silently:
   - Preferred: extend the existing parity check script
     (`scripts/check-lint-catalog.ts` — verify it exists:
     `ls scripts/ | grep -i catalog`) to regenerate to a temp dir and diff
     against all committed copies, failing on any difference.
   - Wire into CI wherever that script already runs (check
     `.github/workflows/pr.yaml` for the existing invocation).
3. If step 1 found genuine staleness, also check `git log --follow` on the
   v2-preview catalog to note when it diverged, and record one line in
   `docs/diagnostics-strategy.md` under a `## Catalog drift incident` note —
   the corpus/keystone work depends on these catalogs being trustworthy.

**Acceptance:** drift gate fails when any catalog copy is hand-edited (verify by
mutating one byte locally, running the check, reverting); all copies
regenerated/justified; full suite green.

---

## Part C — P1.4: Sysvar names derived from the runtime catalog

**Facts:** `src/solana/runtime_catalog.rs` exports `SYSVARS` (verified, parity
tested). Three sites hardcode sysvar names independently:
1. `src/lsp/diagnostics/constraint_expressions/resolution.rs:15` —
   `BUILTIN_ASSOCIATED_PATH_ROOTS` contains `"Clock"`, `"Rent"` literals among
   genuinely-builtin roots (`Option`, `Vec`, `Pubkey`, ...).
2. `src/lsp/diagnostics/anchor_syn/mod.rs:394` — example string
   `"Sysvar<'info, Clock>"`; and `:553` — message text naming `Clock`/`Rent`.
3. `src/lsp/completions/constraint_values/program_ids.rs:72` —
   `SYSVAR_ID_NAMES = ["rent", "clock", "instructions"]`.

**Steps:**
1. In `resolution.rs`: split the constant — keep true Rust builtins as the
   literal list; for sysvar type names, check membership against
   `runtime_catalog::SYSVARS` (type-ident field — read the struct definition in
   runtime_catalog.rs for the exact field name) at the same call sites. Result:
   a newly added sysvar in the catalog is automatically resolvable.
2. In `anchor_syn/mod.rs`: derive the example sysvar from the catalog (first
   non-deprecated entry) for both the shape example and the message text; keep
   wording otherwise identical so existing message-matching tests still pass —
   run `cargo test anchor_syn` and update only asserted strings that contain the
   derived name.
3. In `program_ids.rs`: keep `SYSVAR_ID_NAMES` as a curated *ordering/subset*
   ONLY if completions intentionally offer a subset (read the surrounding code
   and comment); otherwise derive from `SYSVARS`. If curation is intentional,
   add a test asserting every curated name exists in the catalog (subset
   invariant), so a renamed/removed sysvar breaks the build.

**Acceptance:** `rg -n '"Clock"|"Rent"' src/lsp/diagnostics/constraint_expressions/ src/lsp/diagnostics/anchor_syn/`
shows only catalog-derived or test occurrences; new subset-invariant test passes;
suite green.

---

## Part D — P1.5: Accept≠offer parity completion

**Facts:** `src/lsp/completions/constraint_values/program_ids.rs:34-65`
(`NON_SYSVAR_ADDRESSES`) offers `system_program::ID`, `token::ID`,
`token_2022::ID`, `associated_token::ID`, `mpl_token_metadata::ID` with five
hardcoded base58 addresses. The parity test
(`src/lsp/completions/constraint_values/tests/parity_tests.rs:17-30`) covers only
`token::ID`, `mpl_token_metadata::ID`, `crate::ID`. The diagnostic side
(`constraint_expressions/resolution.rs`) accepts an offered path only when the
import exists.

**Steps:**
1. Extend `parity_tests.rs`: one parity case per `NON_SYSVAR_ADDRESSES` entry —
   completion offers it AND the diagnostic accepts it in a fixture where the
   relevant import is present; plus the inverse guard documenting intended
   behavior when the import is absent (decide from existing test conventions:
   if completion is offered without the import, the diagnostic must also accept
   or the completion must be import-gated — pick whichever the existing
   `token::ID` test establishes, and make all entries consistent with it).
2. Address-truth check for the five base58 literals: assert each equals the
   real `ID` from the corresponding crate if that crate is already a
   (dev-)dependency (`rg 'anchor-spl|mpl-token-metadata' Cargo.toml crates/*/Cargo.toml`).
   For crates not in the tree, add a comment with the authoritative source URL
   and date verified — do NOT add new runtime deps for this.

**Acceptance:** every `NON_SYSVAR_ADDRESSES` entry appears in a parity test
(`grep` each path literal in parity_tests.rs); suite green.

---

## Part E — P1.7: Centralize macro-emitted idiom names

**Facts (sites verified by scan):**
- `"DISCRIMINATOR" | "discriminator"` — `src/lsp/diagnostics/security/raw_account.rs:523,527`
- `"CLOSED_ACCOUNT_DISCRIMINATOR"` — `src/lsp/diagnostics/code_quality/manual_close.rs:119`
- `"INIT_SPACE"` — `src/core/document/associated_values.rs:9` AND
  `src/lsp/completions/constraint_values/associated_values.rs:10` (independent
  duplicates; the completions copy also defines `DISCRIMINATOR_ASSOCIATED_CONST`)
- `"declare_id"` — `src/core/document/mod.rs:212`, `src/core/document/symbols.rs:16`,
  `src/solana/project/mod.rs:257`
- CPI builder names `CpiContext::new/new_with_signer`, `invoke` —
  `src/lsp/diagnostics/code_quality/stale_cpi.rs:124-126`,
  `src/core/document/account_usage.rs:496-500`
- `"reload"` — `stale_cpi.rs:87`
- deserialize method lists — `raw_account.rs:502-514`
- token unpack methods + `"StateWithExtensions"` — `account_usage.rs:532,539`

**Steps:**
1. Create `src/anchor/idioms.rs` (NEW): named `pub const`s / const slices for
   every literal above, each with a doc comment stating which anchor-lang /
   solana-program / borsh version pins it and what emits it. Module doc states:
   "Facts about what pinned macro/library versions emit. Review on every Anchor
   version bump alongside the generated catalogs."
2. Mechanically replace each call site with the named constant. Zero behavior
   change — byte-identical strings.
3. Where two sites defined the same concept independently (INIT_SPACE,
   DISCRIMINATOR), both now import the single constant; delete the local copies.
4. Add `declare_program!` alongside `declare_id!` ONLY as a constant
   (`PROGRAM_DECLARATION_MACROS = ["declare_id", "declare_program"]`) — wiring it
   into detection is a behavior change; do it only where the surrounding logic
   is a pure "is this a program declaration" check (the `src/core/document/`
   sites), NOT in framework classification (`project/mod.rs`) which has FP
   implications — leave that site consuming only `declare_id` and note it.
5. Add a hygiene test mirroring the existing source-scan checks: the literal
   strings may not appear outside `idioms.rs` (scan `src/` excluding the module
   and tests).

**Acceptance:** `rg -n '"INIT_SPACE"|"CLOSED_ACCOUNT_DISCRIMINATOR"' src/ --glob '!src/anchor/idioms.rs' --glob '!*test*'`
returns nothing; suite green; clippy clean.

---

## Part F — P2 quick structural fixes (bundled, small)

1. **Anchor.toml hand parser** — `src/core/project/mod.rs:29-91`
   (`parse_anchor_toml`) is a line scanner. Replace internals with real TOML
   parsing via the already-present `cargo_toml`/`toml` dependency chain
   (`rg '^toml|cargo_toml' Cargo.lock` to confirm what's available). Keep the
   function signature and all existing tests; add one test with a same-line
   inline table and one with reordered sections (cases line scanners miss).
2. **IDL helper duplication** — `idl_program_name`/`idl_address` exist in BOTH
   `src/solana/program_artifacts/mod.rs:503-528` and
   `src/solana/ecosystem/mod.rs:475-506`. Merge into one helper (place in
   whichever module both can import without a cycle; check `use` graphs) and
   delete the copy.
3. **Artifact path set** — centralize the `target/deploy`, `target/idl`,
   `target/types` literals from `program_artifacts/mod.rs:265-282,119-127`,
   `src/lsp/diagnostics/artifacts.rs:402-409`, and
   `src/core/definition_bridge/paths.rs:38` into one constants module
   (`src/solana/artifact_paths.rs`, NEW) so an Anchor layout change is a
   one-file fix. Pure mechanical move, zero behavior change.

**Acceptance:** suite green; clippy clean; `rg -n '"target/idl"' src/ --glob '!src/solana/artifact_paths.rs'`
returns nothing.

---

## Final validation

```bash
cargo test --workspace 2>&1 | tail -3        # 0 failed
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
bun run scripts/check-diagnostic-topics.ts   # still green
bun run scripts/check-diagnostic-audit.ts    # still green
```

Commit per part (six focused commits), conventional format.

## Risks

- Part A step 2 may surface MORE drifted topics than the two known — fix all;
  do not scope-limit to the two.
- Part B's outcome depends on whether v2-preview divergence is intentional;
  do not force equality if the sources genuinely differ.
- Part E's hygiene test must exclude generated files and corpus fixtures.
- Anything here colliding with in-flight plan-01 work: plan-01 lands first.
