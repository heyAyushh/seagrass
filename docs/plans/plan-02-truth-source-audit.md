# Plan 02 — Truth-Source Audit of All Diagnostics + Prune Shallow Heuristics

## Context

**Repo:** `/Users/ay/Documents/codes/solana/seagrass`  
**Language:** Rust. LSP server (tower-lsp + syn/tree-sitter + salsa 0.26) for Solana programs.  
**Baseline:** `cargo test` must be green before starting. The working tree has uncommitted changes. **First action: verify `cargo test` is green; if green, commit the baseline before any edit.**

### Architecture doctrine (non-negotiable; every step must obey)

1. **Three truth sources** — every diagnostic claim must be backed by exactly one of:
   - **(A) Single-file syntax** — parse + structural check on one file, no cross-file resolution
   - **(B) Pinned catalog** — anchor-syn fork pinned at rev `4addac5` / `solana-program = 2.2.1`; generated files are `src/anchor/generated/`.  Catalog entries are the authority on valid Anchor types/constraints.
   - **(C) Toolchain oracle** — `cargo metadata` / `cargo check` output; used for manifest/artifact checks
   - **(NONE)** — diagnostic has no backing from any of the above; must be cut, demoted, or re-based

2. **Provability ceiling** (already landed in `src/lsp/diagnostics/registry.rs`):
   `Syntactic` → ERROR allowed; `WholeProgram` / `Heuristic` → WARNING max; enforced by `severity_is_bounded_by_provability` test.

3. **No whole-program Rust name/type resolution** — seagrass cannot resolve types across crate boundaries. Claims requiring that resolution are unproven.

4. **Corpus-fix rule** — a corpus failure is fixed as a general rule in the model/extractor, never by special-casing the triggering program.

5. **Silence, never a guess** — if a query cannot determine provability from (A)/(B)/(C), it must emit nothing.

6. **Decision criterion for NONE-backed lints:** "Can a well-informed Solana engineer write valid, compiling code that this diagnostic flags as wrong?" If yes and the claim cannot be proven from (A)/(B)/(C), the lint must be **cut** (remove entirely) or **demoted to HINT** (severity `HINT`, new `Provability::Speculative` tier, opt-in only). HINT is never shown by default; it requires explicit user configuration.

### Current state of relevant files (verified to exist)

| File | Role |
|---|---|
| `src/lsp/diagnostics/registry.rs` | `AnchorDiagnosticKind` (31 variants), `Provability` enum, `severity_is_bounded_by_provability` + `all_variants_are_covered_by_all` tests |
| `src/lsp/diagnostics/code_quality/mod.rs` | `UncheckedArithmeticVisitor` (BALANCE_TERMS name-match), `pda_seed_collision_diagnostics` (trie), `NonCanonicalPdaBumpVisitor` |
| `src/lsp/diagnostics/security/duplicates.rs` | `DuplicateMutableAccountVisitor` (type-name string equality) |
| `src/lsp/diagnostics/security/mod.rs` | `SecuritySysvar` (sysvar_type_for_field lookup), `SecuritySigner`, `SecurityCpiProgram`, `SecurityTokenAccount`, `SecurityOwnerCheck`, `SecurityTypeCosplay` |
| `src/lsp/diagnostics/constraint_shape/token.rs` | `token_program_init_diagnostics`, `is_token_program_field` — FP family A lives here |
| `src/lsp/diagnostics/constraint_shape/pda.rs` | `static_pda_seed_diagnostic` (`SecurityStaticPda`), seed-shape checks |
| `src/lsp/diagnostics/constraint_shape/program_account.rs` | `SecurityUncheckedAccount` |
| `src/lsp/diagnostics/spl_semantics.rs` | `AnchorSplTokenInterface` |
| `src/lsp/diagnostics/ecosystem.rs` | `SolanaIdlArtifact`, `SolanaProgramMetadata`, `SolanaTestHarness`, `SolanaSurfpoolWorkspace` |
| `src/anchor/types/mod.rs` | `sysvar_type_for_field` — queries `FIELD_COMPLETIONS` from pinned generated catalog |
| `src/anchor/generated/anchor_field_completions_generated.rs` | Pinned catalog — sysvar names, account types (truth source B) |
| `src/lsp/diagnostics/security/tests/core_tests.rs` | Tests: SecuritySigner, SecuritySysvar, SecurityCpiProgram |
| `src/lsp/diagnostics/security/tests/duplicate_tests.rs` | Tests: SecurityDuplicateAccount |
| `src/lsp/diagnostics/code_quality/tests/arithmetic.rs` | Tests: unchecked-arithmetic |
| `src/lsp/diagnostics/code_quality/tests/pda.rs` | Tests: pda-seed-collision, non-canonical-bump |
| `src/lsp/diagnostics/constraint_shape/tests/pda_tests.rs` | Tests: SecurityStaticPda, AnchorConstraintShape/seeds |
| `src/lsp/diagnostics/constraint_shape/tests/token_tests.rs` | Tests: token program shape (FP family A) |
| `docs/diagnostics-strategy.md` | Strategy doc — must record dispositions |

**NEW files this plan creates:**
- `docs/plans/plan-02-truth-source-audit.md` — this file
- No new source files required; changes are edits to existing files listed above

### Known corpus FP families

**FP family A (~22 cases):** `AnchorConstraintShape` rejects `Program<'info, Token2022>` / `Program<'info, Token>` where the field is `InterfaceAccount`. Both types satisfy the `TokenInterface` relation and compile. Location: `src/lsp/diagnostics/constraint_shape/token.rs`, `is_token_program_field`. Current code at line ~350 already handles `InterfaceAccount` fields with `Interface<'info, TokenInterface>`, but the diagnostic still fires for the non-`InterfaceAccount` path (when `initialized.type_name() != Some("InterfaceAccount")`). The general fix: derive the valid token-program type set from the catalog (truth source B), not from an inline list.

**FP family B (~16 cases):** `AnchorMissingAccountReference` fires when `payer = trade.taker` references a field in a *nested* `#[derive(Accounts)]` sub-struct (composite accounts). The rule resolves names only within the flat field list of the immediate struct. Location: `src/lsp/diagnostics/account_references/mod.rs`, `accounts.has_account(reference.name)`. The general fix: traverse the `AccountsStruct` graph through nested `Accounts` fields before concluding a reference is missing.

---

## Non-Goals

- No whole-program Rust name/type resolution (forbidden by doctrine).
- No changes to `Syntactic` or `WholeProgram` provability tiers — only `Heuristic` variants are audited for cuts.
- No new diagnostic variants in this plan.
- No corpus-specific special-casing (allowlists of program names, type aliases, crate names).
- No changes to LSP completion logic.
- Do not touch `Cargo.lock` beyond what Rust compilation requires.

---

## Audit Table

Build this table to drive decisions. Every row is a `AnchorDiagnosticKind` variant. Column **TruthSource** = A, B, C, or NONE.

| Variant | Current Provability | TruthSource | Decision |
|---|---|---|---|
| AnchorSyn | Syntactic | A | KEEP |
| AnchorInitConstraints | Syntactic | B | KEEP |
| AnchorConstraintShape | Syntactic | B | KEEP (+ FP-A fix in step 4) |
| AnchorContextAccounts | WholeProgram | A | KEEP |
| AnchorMissingAccountReference | WholeProgram | A | KEEP (+ FP-B fix in step 5) |
| AnchorMissingInstructionArgument | WholeProgram | A | KEEP |
| AnchorMissingInitConstraint | WholeProgram | A+B | KEEP |
| AnchorConstraintExpression | WholeProgram | A | KEEP |
| AnchorAccountUsage | WholeProgram | A | KEEP |
| AnchorProjectId | WholeProgram | C | KEEP |
| AnchorCheckCfg | Heuristic | C | KEEP — checks `[features]` table in Cargo.toml; manifest evidence qualifies as (C) |
| AnchorSbfArtifact | Heuristic | C | KEEP — file existence check; qualifies as (C) |
| AnchorProgramKeypair | Heuristic | C | KEEP — file existence check; qualifies as (C) |
| AnchorIdlArtifact | Heuristic | C | KEEP — file existence check |
| AnchorTypesArtifact | Heuristic | C | KEEP — file existence check |
| SolanaIdlArtifact | Heuristic | C | KEEP — file existence check |
| SolanaProgramMetadata | Heuristic | C | KEEP — file existence check |
| SolanaTestHarness | Heuristic | C | KEEP — file existence check |
| SolanaSurfpoolWorkspace | Heuristic | C | KEEP — file existence check |
| AnchorSplTokenInterface | Heuristic | B | KEEP — catalog-backed type check |
| SecuritySigner | Heuristic | A | KEEP — name-based only within unchecked account + no signer constraint; see analysis in step 3 |
| SecurityTokenAccount | Heuristic | NONE | CUT — see step 3 |
| SecurityCpiProgram | Heuristic | A | KEEP — limited to AccountInfo/UncheckedAccount used as CPI program with no address/executable constraint; see step 3 |
| SecuritySysvar | Heuristic | B | KEEP — sysvar names from pinned catalog (`anchor_field_completions_generated.rs`) |
| SecurityDuplicateAccount | Heuristic | NONE | CUT — see step 3 |
| SecurityUncheckedAccount | Heuristic | A | KEEP — fires only when AccountInfo/UncheckedAccount field is used as CPI program without constraint; provable from single file |
| SecurityStaticPda | Heuristic | NONE | DEMOTE to Speculative — see step 3 |
| SecurityOwnerCheck | Heuristic | A | KEEP with narrowing — tracks let-binding aliases in raw_account.rs; see step 3 |
| SecurityTypeCosplay | Heuristic | A | KEEP with narrowing — fires only on AccountInfo/UncheckedAccount lacking discriminator check; see step 3 |
| SolanaCodeQuality | Heuristic | MIXED | Sub-rule split — see step 3 |
| PdaSeedResolution | Heuristic | A | KEEP — parses seeds from single-file syntax |

---

## Steps

### Step 0 — Baseline verification and commit

**File:** none (shell only)

```
cargo test 2>&1 | tail -5
```

If any tests fail, STOP. Fix failing tests before proceeding. Do not edit any production code until the baseline is green.

If green, commit the working tree:
```
git add -A
git commit -m "chore: baseline before truth-source audit (plan-02)"
```

Then confirm with `cargo test` + `cargo clippy --all-targets -- -D warnings` both pass on the committed baseline.

### Step 1 — Extend `Provability` with `Speculative` tier

**File:** `src/lsp/diagnostics/registry.rs`

Add a fourth variant to `Provability`:

```rust
/// Pattern-based speculation: may surface real issues but has a known non-trivial FP rate.
/// Must default to HINT severity (not shown unless user opts in).
/// Never ERROR, never WARNING by default.
Speculative,
```

Extend `default_severity()`:
- `Speculative` → `DiagnosticSeverity::HINT`

Extend `severity_is_bounded_by_provability` test to assert `Speculative` variants default to `HINT` (not WARNING, not ERROR).

**Doctrine check:** `Provability` is the mechanism that enforces truth-source ceilings. A new tier below `Heuristic` allows explicit opt-in surfacing of low-confidence speculations without polluting the default view.

### Step 2 — Add `fn truth_source(self) -> TruthSource` to registry

**File:** `src/lsp/diagnostics/registry.rs`

Add enum (just above `impl AnchorDiagnosticKind`):

```rust
/// Which of the three doctrine-approved evidence sources backs this diagnostic's claims.
/// Every variant must declare exactly one. Compound sources (e.g., A+B) use the weakest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruthSource {
    /// (A) Single-file syntax — parse + structural checks, no cross-file resolution.
    SingleFileSyntax,
    /// (B) Pinned catalog — anchor-syn fork at rev 4addac5 / solana-program =2.2.1.
    PinnedCatalog,
    /// (C) Toolchain oracle — cargo metadata / cargo check / file system state.
    ToolchainOracle,
    /// Backed by two or more sources; conservative = weakest applies.
    Compound,
    /// No approved truth source. Variant must be Heuristic or Speculative provability.
    None,
}
```

Add method (exhaustive match, no wildcard):

```rust
pub fn truth_source(self) -> TruthSource {
    match self {
        // (A) Single-file syntax
        Self::AnchorSyn
        | Self::AnchorConstraintShape
        | Self::AnchorInitConstraints
        | Self::AnchorContextAccounts
        | Self::AnchorMissingAccountReference
        | Self::AnchorMissingInstructionArgument
        | Self::AnchorConstraintExpression
        | Self::AnchorAccountUsage
        | Self::SecuritySigner
        | Self::SecurityCpiProgram
        | Self::SecurityUncheckedAccount
        | Self::SecurityOwnerCheck
        | Self::SecurityTypeCosplay
        | Self::PdaSeedResolution => TruthSource::SingleFileSyntax,

        // (B) Pinned catalog
        Self::AnchorSplTokenInterface
        | Self::SecuritySysvar => TruthSource::PinnedCatalog,

        // (A+B) Compound
        Self::AnchorMissingInitConstraint => TruthSource::Compound,

        // (C) Toolchain oracle / file-system state
        Self::AnchorCheckCfg
        | Self::AnchorSbfArtifact
        | Self::AnchorProgramKeypair
        | Self::AnchorIdlArtifact
        | Self::AnchorTypesArtifact
        | Self::SolanaIdlArtifact
        | Self::SolanaProgramMetadata
        | Self::SolanaTestHarness
        | Self::SolanaSurfpoolWorkspace
        | Self::AnchorProjectId => TruthSource::ToolchainOracle,

        // None — cut or Speculative
        Self::SecurityTokenAccount
        | Self::SecurityDuplicateAccount
        | Self::SecurityStaticPda
        | Self::SolanaCodeQuality => TruthSource::None,
    }
}
```

Add enforcement test to the `#[cfg(test)]` block:

```rust
/// Every Heuristic/Speculative variant with TruthSource::None must have
/// provability Heuristic or Speculative, never Syntactic or WholeProgram.
#[test]
fn none_backed_variants_are_not_syntactic_or_whole_program() {
    for kind in AnchorDiagnosticKind::all() {
        if kind.truth_source() == TruthSource::None {
            assert!(
                matches!(kind.provability(), Provability::Heuristic | Provability::Speculative),
                "Diagnostic `{}` has TruthSource::None but provability {:?}; \
                 only Heuristic/Speculative diagnostics may lack a truth source.",
                kind.code(),
                kind.provability()
            );
        }
    }
}
```

Also update `all_variants_are_covered_by_all` to call `kind.truth_source()` so that new variants without a truth_source arm are caught at compile time.

### Step 3 — Implement cut/demote decisions for NONE-backed lints

This is the core code-change step. Work through each NONE-backed variant in order.

#### 3a — Cut `SecurityTokenAccount`

**What it does:** `TokenAccountUnpackingVisitor` fires when an `AccountInfo`/`UncheckedAccount` field appears to hold a token account (inferred from field name or neighboring constraints) but no unpacking/ownership check is found in the handler body.

**Why it has no truth source:** Determining whether a raw account holds a token account requires type information unavailable from single-file syntax. The name/context heuristic produces false positives on valid programs (e.g., any `AccountInfo` field named near a token constraint, even when properly validated by an associated library call). A well-informed engineer can write valid code this flags as wrong.

**Action:** Delete the variant.

1. Remove `SecurityTokenAccount` from `AnchorDiagnosticKind` enum.
2. Remove `SecurityTokenAccount` from `all()` array; update the array size from `[Self; 31]` to `[Self; 30]`.
3. Remove the `SecurityTokenAccount` arm from every `match self` in `registry.rs` (`code()`, `provability()`, `default_severity()`, `docs_url()`, `rule()`, `confidence()`, `topic()`, `anchor_error_names()`, `truth_source()`).
4. Delete `token_account_unpacking_diagnostics` function and `TokenAccountUnpackingVisitor` struct/impls from `src/lsp/diagnostics/security/mod.rs`. Remove its call from the `collect` function in the same file.
5. Remove `ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE` constant from `registry.rs`.
6. Delete or mark `#[ignore]` all tests in `src/lsp/diagnostics/security/tests/core_tests.rs` that exclusively test `SecurityTokenAccount` (grep for `token-account` or `TokenAccountUnpacking`). Remove the `#[ignore]` approach — delete the tests outright; they test behavior being removed.
7. Remove `AnchorDiagnosticKind::SecurityTokenAccount` from the `all_kinds` array in `src/lsp/diagnostics/tests/mod.rs` (the `every_diagnostic_kind_carries_metadata_axes` test). Failure to do this will produce a compile error.
8. In `src/lsp/actions/security.rs` (~line 533), remove `| Some("anchor-security-token-account")` from the filter in `replace_account_type_actions`. If that arm was the only reason for an enclosing conditional block, simplify or remove that block. Failure to do this will cause `cargo clippy -D warnings` to fail with a dead-code match arm.
9. Remove `seagrass/security.token-account` from `docs/topics.json`. The `check-diagnostic-topics.ts` script enforces bidirectional sync; once the emitter is gone this entry becomes stale and CI will fail with "declared but not emitted by diagnostics source".
10. In `docs/diagnostic-audit.md`: remove the row for `lsp/diagnostics/security/mod.rs` that references `seagrass/security.token-account` (line ~116 and ~20), and remove the quickfix matrix row for `anchor-security-token-account` (line ~191). After step 9, `check-diagnostic-audit.ts` will fail if these rows remain.
11. In `src/anchor/support/mod.rs`, change the coverage entry for `"account-data-matching"` from `"covered"` to `"not-covered"` (or remove the entry), since `anchor-security-token-account` was the sole backing diagnostic. Keeping the claim `"covered"` will be incorrect after the cut.
12. Run `cargo test` to verify no compilation errors.

#### 3b — Cut `SecurityDuplicateAccount`

**What it does:** `DuplicateMutableAccountVisitor` flags two fields in the same struct that share the same innermost generic type name (e.g., both `Account<'info, TokenAccount>`) and both have mutability-implying constraints, with no `dup` flag visible.

**Why it has no truth source:** Equality of innermost generic *type name strings* is not equality of types. Two fields using the same type name in different crate paths, or aliased types, are legitimate. The check cannot distinguish `my_crate::TokenAccount` from `anchor_spl::token::TokenAccount` without cross-crate resolution. A well-informed engineer writing two mutable `TokenAccount` fields in a valid dual-token swap instruction will see this fire on valid code.

**Action:** Delete the variant.

1. Remove `SecurityDuplicateAccount` from `AnchorDiagnosticKind` enum.
2. Update `all()` to size `[Self; 29]` and remove the variant.
3. Remove all match arms in `registry.rs` for `SecurityDuplicateAccount`.
4. Remove `ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE` constant.
5. Delete `src/lsp/diagnostics/security/duplicates.rs` entirely. Remove `mod duplicates;` from `src/lsp/diagnostics/security/mod.rs`. Remove the `use duplicates::...` import and the call to `duplicate_account_diagnostics` (or equivalent) from the `collect` function.
6. Delete all tests in `src/lsp/diagnostics/security/tests/duplicate_tests.rs`. Remove `mod duplicate_tests;` from `src/lsp/diagnostics/security/tests/mod.rs`.
7. Remove `AnchorDiagnosticKind::SecurityDuplicateAccount` from the `all_kinds` array in `src/lsp/diagnostics/tests/mod.rs` (the `every_diagnostic_kind_carries_metadata_axes` test). Failure to do this will produce a compile error.
8. In `src/lsp/actions/accounts/mod.rs`, delete the `duplicate_account_actions` function (~lines 587–605) and the `duplicate_account_actions_for_diagnostic` function (line 607+), and remove the call site at line 66 (`actions.extend(duplicate_account_actions(...))`). These functions filter exclusively on `"anchor-security-duplicate-account"` and become dead code after this cut. `cargo clippy -D warnings` will fail if they remain.
9. In `src/lsp/actions/tests/security_mut.rs`, delete the test cases that search for `"anchor-security-duplicate-account"` (lines ~251 and ~329 and their surrounding test functions). These assertions reference removed behavior and will become misleading dead code. Run `cargo test lsp::actions::tests` to confirm no test references the removed diagnostic code.
10. Remove `seagrass/security.account.duplicate-mutable` from `docs/topics.json`. The `check-diagnostic-topics.ts` script enforces bidirectional sync; once the emitter is gone this entry becomes stale and CI will fail.
11. In `docs/diagnostic-audit.md`: remove the two rows for `lsp/diagnostics/security/duplicates.rs` (~lines 21 and 120). Remove the quickfix matrix row for `anchor-security-duplicate-account` (~line 186). After step 10, `check-diagnostic-audit.ts` will report `sourcePathFailures` (file does not exist) and `quickfixCoverageFailures` (code in matrix but not in registry) if these rows remain.
12. In `src/anchor/support/mod.rs`, change the coverage entry for `"duplicate-mutable-accounts"` from `"covered"` to `"not-covered"` (or remove the entry), since `anchor-security-duplicate-account` was the sole backing diagnostic.
13. Run `cargo test`.

#### 3c — Demote `SecurityStaticPda` to Speculative

**What it does:** `static_pda_seed_diagnostic` fires when ALL seeds in a `seeds = [...]` list are byte-string/byte-char literals (no dynamic component).

**Why it is NONE-backed at Heuristic level:** Static seeds are sometimes intentional and valid (e.g., global state PDAs, singleton program accounts). Flagging every program with a static-seed PDA as potentially unsafe is a high-FP heuristic.

**Why keep (as Speculative rather than cut):** The underlying pattern — fully static PDA seeds with no unique key component — is a genuine design smell that experienced engineers want optionally surfaced. It cannot generate false positives in the mathematical sense (the detection is syntactically correct), but its security implication is context-dependent.

**Action:** Change provability to `Speculative`; keep the variant but mark `truth_source` as `None` (the claim "this is insecure" has no proof source; only the syntactic observation "all seeds are static" is verifiable).

1. In `registry.rs`, `provability()` match: move `SecurityStaticPda` from the `Heuristic` arm to a new `Speculative` arm.
2. `default_severity()` already dispatches on `provability()`; after step 1, `SecurityStaticPda` will return `HINT` automatically.
3. Keep `truth_source()` returning `TruthSource::None` for `SecurityStaticPda`.
4. Update the doc comment on `SecurityStaticPda` in the variant or a doc string to read: "Speculative: static seeds are valid; this surfaces a design-review prompt, not a definite bug."
5. Verify `severity_is_bounded_by_provability` test handles `Speculative` → must assert `HINT`, not `WARNING` or `ERROR`.
6. Leave the implementation in `src/lsp/diagnostics/constraint_shape/pda.rs` unchanged — only registry metadata changes.
7. Keep existing tests in `src/lsp/diagnostics/constraint_shape/tests/pda_tests.rs`; update any test that asserts `WARNING` severity for `SecurityStaticPda` to assert `HINT`.

#### 3d — Split `SolanaCodeQuality` sub-rules: cut name-match arithmetic, keep trie-collision and non-canonical bump

`SolanaCodeQuality` is a single registry variant covering multiple sub-rules emitted by different visitors. Each sub-rule must be evaluated independently.

**Sub-rule: `unchecked-arithmetic`** — `UncheckedArithmeticVisitor` fires when `+/-/*` operands contain an identifier matching any word in `BALANCE_TERMS`. This is a pure string heuristic with no truth source. A well-informed engineer writing `total_count += 1` (where `total` matches `BALANCE_TERMS["total"]`) gets a false positive. The rule cannot distinguish integer overflow in financial math from overflow in unrelated arithmetic. **Decision: CUT this sub-rule.** Remove `unchecked_balance_arithmetic_diagnostics` from `collect_with_framework`, delete `UncheckedArithmeticVisitor`, `BALANCE_TERMS` constant, `is_unchecked_balance_arithmetic`, `expr_contains_balance_term`, `identifier_has_balance_term`, `unchecked_arithmetic_diagnostic`, and all helper fns. Delete tests in `src/lsp/diagnostics/code_quality/tests/arithmetic.rs` that test this sub-rule.

**Sub-rule: `pda-seed-collision`** — `pda_seed_collision_diagnostics` uses a trie prefix-collision algorithm over literal byte-string seed prefixes. The detection is structural, not semantic: if two PDAs share a byte-prefix, the Solana runtime cannot statically distinguish them. This is provable from single-file syntax (truth source A). **Decision: KEEP.** No code changes needed for this sub-rule.

**Sub-rule: `non-canonical-bump`** — `NonCanonicalPdaBumpVisitor` fires when `Pubkey::create_program_address` is called (bypassing bump canonicalization). The call itself is a verifiable syntactic fact (truth source A). **Decision: KEEP.**

**Sub-rule: `unsafe-unwrap`** — `UnsafeUnwrapVisitor` fires on `.unwrap()` / `.expect()` method calls. Detectable from single-file syntax (truth source A). **Decision: KEEP.**

**Sub-rule: `instruction-data-bounds`** — detects missing bounds checks on instruction data deserialization. Syntactic, truth source A. **Decision: KEEP.**

**Sub-rule: `manual-close` / `stale-cpi`** — structural checks on CPI patterns; single-file syntax (truth source A). **Decision: KEEP.**

After cutting `unchecked-arithmetic`:
1. Delete `UncheckedArithmeticVisitor` struct and impl in `src/lsp/diagnostics/code_quality/mod.rs`.
2. Delete `BALANCE_TERMS` constant.
3. Delete functions: `unchecked_balance_arithmetic_diagnostics`, `is_unchecked_balance_arithmetic`, `is_unchecked_arithmetic_operator`, `expr_contains_balance_term`, `identifier_has_balance_term`, `unchecked_arithmetic_diagnostic`.
4. Remove the call `diagnostics.extend(unchecked_balance_arithmetic_diagnostics(document, program_kind));` from `collect_with_framework`.
5. In `src/lsp/diagnostics/code_quality/tests/arithmetic.rs`: delete all tests. Remove `mod arithmetic;` from `src/lsp/diagnostics/code_quality/tests/mod.rs`.
6. Update `truth_source()` for `SolanaCodeQuality` in registry: the remaining sub-rules are all truth source A. Change `TruthSource::None` → `TruthSource::SingleFileSyntax`.
7. Remove `seagrass/solana.code-quality.unchecked-arithmetic` from `docs/topics.json`. The `check-diagnostic-topics.ts` script will flag it as stale once the emitter is gone.
8. In `docs/diagnostic-audit.md`: update the row for `lsp/diagnostics/code_quality/mod.rs` that lists `unchecked_balance_arithmetic` as a provider (~line 11). Remove or update the `unchecked_balance_arithmetic` sub-row and its topic reference so `check-diagnostic-audit.ts` does not report a stale source path.
9. Run `bun run scripts/check-diagnostic-topics.ts` and `bun run scripts/check-diagnostic-audit.ts` and confirm zero failures.
10. Run `cargo test`.

### Step 4 — Fix FP family A: `AnchorConstraintShape` token-program type check

**File:** `src/lsp/diagnostics/constraint_shape/token.rs`

**Problem:** `is_token_program_field` currently uses an inline list (`Token`, `Token2022`, `TokenInterface`) to decide whether a `token_program` field is valid. For `InterfaceAccount`-typed initialized accounts, only `Interface<'info, TokenInterface>` is accepted. The FP is that `Program<'info, Token2022>` is rejected even when the catalog (truth source B) recognizes it as a valid TokenInterface implementor.

Specifically, line ~350:
```rust
fn is_token_program_field(program: &FieldEvidence<'_>, initialized: &FieldEvidence<'_>) -> bool {
    if initialized.type_name() == Some("InterfaceAccount") {
        return program.type_name() == Some("Interface")
            && program.has_generic_type("TokenInterface");
    }
    (program.type_name() == Some("Program") || program.type_name() == Some("Interface"))
        && (program.has_generic_type("Token")
            || program.has_generic_type("Token2022")
            || program.has_generic_type("TokenInterface"))
}
```

The non-`InterfaceAccount` branch already accepts `Token2022` — the FP is in the `InterfaceAccount` branch, which requires `Interface<'info, TokenInterface>` and rejects `Program<'info, Token2022>`.

**Fix (general rule, not allowlist):**

The catalog lists which types satisfy the token-interface relation. Query the catalog rather than maintaining an inline list. Add a function in `src/anchor/types/mod.rs` (or an appropriate existing module):

```rust
/// Returns true if `type_name` with `generic_args` is a valid token-program account
/// for a token-interface (multi-program) context, per the pinned catalog.
pub fn is_token_interface_program_type(type_name: &str, generic_arg: &str) -> bool {
    // Per catalog: Interface<'info, TokenInterface>, Program<'info, Token>,
    // Program<'info, Token2022> all satisfy the TokenInterface contract.
    // Derived from anchor-syn @ 4addac5 / solana-program =2.2.1.
    matches!(
        (type_name, generic_arg),
        ("Interface", "TokenInterface")
            | ("Program", "Token")
            | ("Program", "Token2022")
            | ("Program", "TokenInterface")
    )
}
```

Before editing `token.rs`, verify the `FieldEvidence` API:

```bash
grep -n "fn has_generic_type\|fn first_generic_type\|generic_type_names" src/core/evidence/mod.rs
```

**Verified API (as of this plan):** `FieldEvidence` exposes `has_generic_type(name: &str) -> bool` (checks `field.generic_type_names` for an exact match) but does **not** expose `first_generic_type() -> Option<&str>`. Do not call `first_generic_type()` — it does not exist and will fail to compile.

If you need to extract the first generic type name, first add it to `FieldEvidence` in `src/core/evidence/mod.rs`:

```rust
/// Returns the first generic type argument of this field's account wrapper type, if any.
pub fn first_generic_type(&self) -> Option<&str> {
    self.field.generic_type_names.first().map(String::as_str)
}
```

Then add `crate::anchor_types` to the `use` block in `src/lsp/diagnostics/constraint_shape/token.rs`. The current `use` block in that file imports only `crate::{diagnostics::..., evidence::...}`. Add the import explicitly:

```rust
use crate::{
    anchor_types,
    diagnostics::...,  // keep existing imports
    evidence::...,     // keep existing imports
};
```

Without this import, the call `anchor_types::is_token_interface_program_type(...)` will fail to resolve. The pattern is established in `src/lsp/diagnostics/security/mod.rs` and `src/anchor/anchor_syn/mod.rs`.

Then change `is_token_program_field` to use `first_generic_type()` (either the existing one if you confirmed it exists, or the one added above) or rewrite it using the existing `has_generic_type` API without `first_generic_type`:

```rust
fn is_token_program_field(program: &FieldEvidence<'_>, initialized: &FieldEvidence<'_>) -> bool {
    let Some(type_name) = program.type_name() else { return false };

    if initialized.type_name() == Some("InterfaceAccount") {
        // For InterfaceAccount-typed token data, any catalog-valid token-interface program is valid.
        // Catalog (truth source B): Interface<TokenInterface>, Program<Token>, Program<Token2022>.
        anchor_types::is_token_interface_program_type(type_name, program)
    } else {
        // For Account<'info, TokenAccount>-typed token data, standard token program types.
        (type_name == "Program" || type_name == "Interface")
            && (program.has_generic_type("Token")
                || program.has_generic_type("Token2022")
                || program.has_generic_type("TokenInterface"))
    }
}
```

Where `is_token_interface_program_type` is updated to accept a `&FieldEvidence` and use `has_generic_type`:

```rust
/// Returns true if the program field is a valid token-program account
/// for a token-interface (multi-program) context, per the pinned catalog.
/// Catalog: anchor-syn @ 4addac5 / solana-program =2.2.1.
pub fn is_token_interface_program_type(type_name: &str, program: &FieldEvidence<'_>) -> bool {
    matches!(type_name, "Interface" | "Program")
        && (program.has_generic_type("TokenInterface")
            || program.has_generic_type("Token")
            || program.has_generic_type("Token2022"))
}
```

Alternatively, if you added `first_generic_type()` to `FieldEvidence`, use the tuple-match form shown earlier — both approaches are correct. The key requirement is: **use only methods that exist on `FieldEvidence`**. Do not leave the code calling a non-existent method.

**Acceptance:** After this change, `cargo test` must pass including `src/lsp/diagnostics/constraint_shape/tests/token_tests.rs`. Add a test case in `token_tests.rs` that asserts no diagnostic is emitted for `Program<'info, Token2022>` as `token_program` alongside an `InterfaceAccount`-typed field.

**Doctrine check:** The valid type list in `is_token_interface_program_type` is derived from catalog (truth source B), not from an allowlist of program names. If the catalog changes (new anchor version), the generated file is regenerated and this function is updated — single source of truth.

### Step 5 — Fix FP family B: composite-accounts reference resolution

**File:** `src/lsp/diagnostics/account_references/mod.rs`

**Problem:** `accounts.has_account(reference.name)` resolves names only in the flat field list of the immediate `#[derive(Accounts)]` struct. When a field's type is itself a `#[derive(Accounts)]` struct (composite accounts), its sub-fields are invisible to the name lookup. A constraint like `payer = trade.taker` where `trade` is a nested Accounts struct and `taker` is a `#[account(mut)]` field inside it causes a false `AnchorMissingAccountReference` diagnostic.

**Fix (general rule):**

Extend `AccountSetEvidence` (or the evidence graph) to include fields reachable through nested Accounts structs. The evidence graph must flatten the account-name namespace to include `<parent>.<child>` paths as resolvable references.

Do not special-case any program name or field name. The rule is: if the `WorkspaceIndex` can identify that a field's type matches a known `#[derive(Accounts)]` struct in the same document (or workspace), its sub-fields must be included in the name resolution space.

Concrete changes:
1. In `src/core/evidence/mod.rs` (or wherever `AccountSetEvidence` is defined), add a method `has_composite_account_path(path: &str) -> bool` that checks whether `path` matches `<parent_field>.<child_field>` where `parent_field` is a field whose type is an Accounts struct in the current document.
2. In `account_references/mod.rs`, in the filter closure for `reference.name`, additionally call `accounts.has_composite_account_path(reference.name)` or expand the `accounts.has_account` check to include the composite lookup.
3. The composite lookup must use only information available in the current document (truth source A). Cross-document Accounts structs require `WorkspaceIndex` (truth source A at workspace scope). When `workspace_index` is `None`, the check should conservatively assume a dot-path reference (`x.y`) is resolvable — i.e., if the reference contains a `.`, do not flag it as missing without workspace evidence.

**Acceptance:** Add at least one test in `src/lsp/diagnostics/account_references/tests/mod.rs` that verifies `payer = trade.taker` does not emit a diagnostic when `trade` is a nested Accounts struct field in the same document.

**Doctrine check:** Resolution is still syntactic (truth source A at document scope, or A+WorkspaceIndex). No type inference across crate boundaries.

### Step 6 — Update the enforcement test in registry

**File:** `src/lsp/diagnostics/registry.rs`

Extend `severity_is_bounded_by_provability` test to cover `Speculative`:

```rust
Provability::Speculative => {
    assert_eq!(
        severity,
        DiagnosticSeverity::HINT,
        "Speculative diagnostic `{}` must default to HINT \
         (shown only on explicit user opt-in)",
        kind.code()
    );
}
```

Add a new test `truth_source_declared_for_all_variants`:

```rust
#[test]
fn truth_source_declared_for_all_variants() {
    // Every variant must declare a truth source; calling truth_source()
    // exercises the exhaustive match which will fail to compile if any variant is missing.
    for kind in AnchorDiagnosticKind::all() {
        let _ = kind.truth_source();
        // Variants with TruthSource::None must not be Syntactic or WholeProgram.
        if kind.truth_source() == TruthSource::None {
            assert!(
                matches!(kind.provability(), Provability::Heuristic | Provability::Speculative),
                "TruthSource::None on a non-Heuristic/Speculative variant: {}",
                kind.code()
            );
        }
    }
}
```

### Step 7 — Update docs: strategy, lint pages, and lints index

**Files:** `docs/diagnostics-strategy.md`, `docs/lints/`, `docs/lints/index.html`

First, delete the three stale lint documentation pages for the cut diagnostics:

```bash
rm docs/lints/seagrass-security-token-account.md
rm docs/lints/seagrass-security-account-duplicate-mutable.md
rm docs/lints/seagrass-solana-code-quality-unchecked-arithmetic.md
```

Then update `docs/lints/index.html` to remove the three table rows that reference those deleted pages (grep for `seagrass/security.account.duplicate-mutable`, `seagrass/security.token-account`, and `seagrass/solana.code-quality.unchecked-arithmetic`). Whether or not the audit script validates lint doc pages against registry codes, leaving stale pages and index entries creates misleading user-facing documentation.

Then append to `docs/diagnostics-strategy.md` a section titled `## Truth-Source Audit Results (Plan 02)` containing:

1. A summary of the audit table from the plan (compact form).
2. **Dispositions:**
   - CUT: `SecurityTokenAccount` — no truth source; name/context heuristic produces FPs on valid token programs
   - CUT: `SecurityDuplicateAccount` — type-name string equality ≠ type equality; FPs on valid dual-token instructions
   - CUT sub-rule: `SolanaCodeQuality/unchecked-arithmetic` — BALANCE_TERMS name matching cannot distinguish financial arithmetic from other integer math
   - DEMOTE to Speculative: `SecurityStaticPda` — static seeds are valid; surfaces as HINT (opt-in only)
   - KEEP: remaining variants with reasons (truth source, brief note)
3. **FP fixes:**
   - FP family A (~22 cases): `AnchorConstraintShape` token-program type check extended to accept `Program<'info, Token2022>` for `InterfaceAccount`-typed fields, derived from catalog
   - FP family B (~16 cases): `AnchorMissingAccountReference` extended to resolve composite-accounts dot-path references
4. **New registry mechanisms:** `TruthSource` enum, `truth_source()` method, `Speculative` provability tier, enforcement test.
5. **Variant count change:** 31 → 29 (removed `SecurityTokenAccount`, `SecurityDuplicateAccount`).

Write in prose + bullet format. No emojis. Terse.

### Step 8 — Final validation

Run in order; all must pass:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
bun run scripts/check-diagnostic-topics.ts
bun run scripts/check-diagnostic-audit.ts
```

Specifically verify:
- `cargo test registry::tests::severity_is_bounded_by_provability` passes
- `cargo test registry::tests::all_variants_are_covered_by_all` passes
- `cargo test registry::tests::truth_source_declared_for_all_variants` passes
- `cargo test registry::tests::none_backed_variants_are_not_syntactic_or_whole_program` passes
- `cargo test lsp::diagnostics::constraint_shape::tests::token_tests` passes (FP-A regression)
- For the FP-B regression test: run `cargo test lsp::diagnostics::account_references::tests` first. If that filter returns zero results, run `cargo test -- account_references` as the fallback — the module path depends on crate structure. Confirm the new composite-accounts dot-path test appears in the output.
- No `SecurityTokenAccount`, `SecurityDuplicateAccount`, or `unchecked-arithmetic` test failures (they were deleted)

---

## Acceptance Criteria

Every criterion below must be satisfied before this plan is considered complete.

| # | Command / Check | Expected outcome |
|---|---|---|
| 1 | `cargo test` | All tests pass; no compilation errors |
| 2 | `cargo clippy --all-targets -- -D warnings` | Zero warnings |
| 3 | `grep -n "SecurityTokenAccount\|SecurityDuplicateAccount" src/lsp/diagnostics/registry.rs` | Zero matches |
| 4 | `grep -rn "BALANCE_TERMS\|UncheckedArithmetic" src/lsp/diagnostics/` | Zero matches |
| 5 | `cargo test registry::tests::severity_is_bounded_by_provability` | PASS |
| 6 | `cargo test registry::tests::truth_source_declared_for_all_variants` | PASS |
| 7 | `cargo test lsp::diagnostics::constraint_shape::tests::token_tests` | PASS (includes new FP-A regression test) |
| 8 | `cargo test -- account_references` (fallback: run `cargo test lsp::diagnostics::account_references::tests` first; if it returns zero tests, use `cargo test -- account_references` to match the actual module path) | PASS (includes new FP-B regression test) |
| 9 | `grep -c "SecurityStaticPda" src/lsp/diagnostics/registry.rs` | At least 1 match (variant kept as Speculative) |
| 10 | `grep "Speculative" src/lsp/diagnostics/registry.rs` | Shows `Provability::Speculative` variant and match arm |
| 11 | `grep "TruthSource" src/lsp/diagnostics/registry.rs` | Shows enum and `truth_source()` method |
| 12 | `grep "Plan 02" docs/diagnostics-strategy.md` | Section appended |
| 13 | `AnchorDiagnosticKind::all().len() == 29` | Verified by test or `grep "\[Self; 29\]" src/lsp/diagnostics/registry.rs` |
| 14 | `bun run scripts/check-diagnostic-topics.ts` | Zero failures (stale entries removed from `docs/topics.json`) |
| 15 | `bun run scripts/check-diagnostic-audit.ts` | Zero failures (stale rows removed from `docs/diagnostic-audit.md`) |
| 16 | `grep -rn "anchor-security-duplicate-account\|anchor-security-token-account" src/lsp/actions/` | Zero matches (dead action code removed) |
| 17 | `grep -n "account-data-matching.*anchor-security-token-account\|duplicate-mutable-accounts.*anchor-security-duplicate-account" src/anchor/support/mod.rs` | Zero matches (coverage table updated) |
| 18 | `ls docs/lints/ \| grep -E "token-account\|duplicate-mutable\|unchecked-arithmetic"` | Zero matches (stale lint pages deleted) |

---

## Risks / Edge Cases

1. **`FieldEvidence::first_generic_type()` does not exist.** This has been confirmed: `src/core/evidence/mod.rs` exposes `has_generic_type(name: &str) -> bool` (backed by `field.generic_type_names`) but has no `first_generic_type()` method. Step 4 has been rewritten to use only `has_generic_type`. If you choose to add `first_generic_type()`, add it to `FieldEvidence` in `src/core/evidence/mod.rs` returning `self.field.generic_type_names.first().map(String::as_str)` before using it anywhere.

2. **Composite-accounts lookup in step 5 may require salsa query.** The `AccountSetEvidence` structure is built from the parsed document. If nested Accounts structs are not already parsed into the evidence graph, the fix may require changes to the evidence-graph construction in `src/core/evidence/mod.rs`. Read that file before editing `account_references/mod.rs`. Minimal approach: treat any reference containing `.` (dot-path) as provisionally valid when `workspace_index` is `None`.

3. **Removing `SecurityDuplicateAccount` deletes `src/lsp/diagnostics/security/duplicates.rs`.** Verify no other file imports from it: `grep -rn "duplicates::\|mod duplicates" src/` before deleting.

4. **`all()` array size must stay in sync with the enum.** After steps 3a+3b remove two variants, the array literal in `all()` must be updated to `[Self; 29]`. A compile error will catch a mismatch because the array type is fixed-size. Do not leave dead entries in the array.

5. **Tests that check variant counts.** `grep -rn "31\|thirty.one\|31 variant" src/ docs/` before committing — update any hardcoded count references.

6. **`DiagnosticSeverity::HINT` may not be the correct tower-lsp constant name.** Verify: `grep -rn "DiagnosticSeverity::HINT\|DiagnosticSeverity::INFORMATION" src/` — tower-lsp uses LSP spec values; HINT is severity 4. Check the `tower_lsp::lsp_types::DiagnosticSeverity` API before using it; the constant may be `HINT` or may require a custom numeric value.

7. **`SolanaCodeQuality` variant must not be deleted** — it still covers `pda-seed-collision`, `non-canonical-bump`, `unsafe-unwrap`, `instruction-data-bounds`, `manual-close`, `stale-cpi`. Only the `unchecked-arithmetic` sub-rule is removed. The variant remains in the registry.

8. **Doc string for `SecurityStaticPda` must not claim the diagnostic is removed** — it is demoted to Speculative/HINT, not deleted. Any user-facing message must say "design review hint" not "security error".

9. **Corpus tests are not run by default.** Steps 4 and 5 fix corpus-discovered FP families A and B. The acceptance criteria above do not include `CORPUS_ENABLED=1` runs — those require network fetch and are a separate CI gate. After this plan, a corpus operator should verify FP family A and B counts drop to zero.
