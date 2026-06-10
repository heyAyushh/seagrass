# Plan 01: Fix Corpus-Discovered FP Families + Corpus Mechanism Caveats

**Status**: Ready for execution  
**Affects**: `AnchorConstraintShape` diagnostics, `AnchorMissingAccountReference` diagnostics,
`EvidenceGraph` composite resolution, `scripts/fetch-corpus.sh`, `corpus/manifest.toml`,
`.github/workflows/corpus.yml`

---

## Context

seagrass is an LSP for Solana/Anchor programs. The diagnostic engine runs per-file;
its constraint-shape rules validate `#[derive(Accounts)]` structs in Anchor programs.

A discovery run of `CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture` against
6 pinned mainnet programs (drift, marginfi, mpl-token-metadata, tensor-amm, spl-token,
program-examples) revealed two systematic ERROR-severity false-positive families (FPs)
plus one case requiring manual triage, plus three mechanical caveats in the corpus tooling.

**Baseline prerequisite**: the working tree has uncommitted changes. Before starting any
step below, verify `cargo test` is green and commit the baseline if needed. All steps
assume tests pass at step entry.

### Architecture doctrine (executor must not violate)

1. THREE TRUTH SOURCES: every diagnostic claim must be backed by (a) single-file syntax,
   (b) the pinned generated catalog (`src/anchor/generated/constraint_catalog_generated.rs`,
   anchor-syn rev 4addac5 / solana-program =2.2.1), or (c) the toolchain
   (`cargo metadata` / `cargo check`). In-house whole-program Rust name/type resolution
   is FORBIDDEN as a claim basis.
2. When extraction/proof fails: silence, never a guess.
3. CORPUS-FIX RULE: a corpus failure may only be fixed as a GENERAL rule in the
   model/extractor; never by special-casing or suppressing the specific program that
   tripped it.
4. Provability ceiling (already enforced): `Syntactic` -> ERROR, `WholeProgram`/`Heuristic`
   -> WARNING. Do not change `Provability` tiers as part of this plan.

### Current state of relevant files

| File | Role |
|---|---|
| `src/lsp/diagnostics/constraint_shape/token.rs` | `is_token_program_field` (line 349), `token_program_init_diagnostics` (line 167) |
| `src/lsp/diagnostics/constraint_shape/init_lifecycle.rs` | `payer_mutability_diagnostics` (line 174) |
| `src/lsp/diagnostics/constraint_shape/mod.rs` | entry point `collect_with_workspace` |
| `src/core/evidence/mod.rs` | `EvidenceGraph::from_document` — flat iteration, no composite expansion |
| `src/lsp/diagnostics/security/duplicates.rs` | only rule with composite expansion (reference implementation) |
| `src/lsp/diagnostics/constraint_shape/tests/token_tests.rs` | token rule tests |
| `src/lsp/diagnostics/constraint_shape/tests/init_lifecycle_tests.rs` | payer rule tests |
| `src/lsp/diagnostics/constraint_shape/tests/mod.rs` | test helpers `diagnostics_for`, `diagnostics_for_workspace` |
| `src/lsp/diagnostics/tests/corpus.rs` | corpus test functions |
| `corpus/manifest.toml` | 6 pinned entries, all materialized |
| `scripts/fetch-corpus.sh` | requires python3 >=3.11 (tomllib) |
| `.github/workflows/corpus.yml` | pinned action hashes |

---

## RESUME STATE (2026-06-10 — read this first, supersedes step ordering below)

A previous executor was stopped mid-plan. Its work is **uncommitted in the working
tree** (21 modified files — do NOT discard or re-baseline over them). Current
state, verified by an independent reviewer:

### Already done (do not redo)
- **Step 1 (FP family A)** — `is_token_program_field` in
  `src/lsp/diagnostics/constraint_shape/token.rs` now accepts
  `Program|Interface` × `Token|Token2022|TokenInterface` regardless of the
  initialized field's wrapper. `token_program_type_for_initialized_field` returns
  the new `TOKEN_PROGRAM_EXPECTED_TYPE` multi-option string constant.
- **Step 2 (FP family B)** — composite account names landed in
  `src/core/evidence/mod.rs` (+20); regression tests added in
  `constraint_shape/tests/init_lifecycle_tests.rs` (+35) and
  `account_references/tests/mod.rs` (+29).
- **Step 4 (alias triage) — resolved as a code fix, beyond plan scope but correct**:
  the `InterfaceAccount<MintAccount>` case was a single-file import alias
  (`use ...::Mint as MintAccount`). Fixed generally: `spl_semantics.rs` (+114)
  resolves import aliases, and `constraint_shape/applicability.rs` now calls
  `account_semantics::resolve_declared_field_account_type_with_symbols(document.symbols(), ...)`
  (alias-aware). The triage comment required by Step 4 has NOT been written yet.
- New regression tests in `constraint_shape/tests/token_tests.rs` (+110).
- First full corpus run after Step 1/2 fixes: token-program greps returned zero.
  A second confirmation run (after the alias fix) was interrupted — must re-run.

### BROKEN RIGHT NOW — fix before anything else
`cargo test --workspace` = **1488 passed, 4 failed**. All 4 in
`src/lsp/actions/tests/account_types.rs`, all panicking at the
diagnostic-find `.unwrap()` (the expected `anchor-constraint-shape` token-program
diagnostic is not in `crate::diagnostics::collect()` output for the fixture):

1. `offers_missing_token_program_field_quickfix` (panic line 170)
2. `missing_token_program_quickfix_uses_token_interface_for_interface_account` (216)
3. `offers_program_field_type_quickfix_for_token_program` (263)
4. `token_program_type_quickfix_uses_token_interface_for_interface_account` (292)

Diagnosis so far (verify before editing):
- Test 4's fixture (`InterfaceAccount` data + `Program<'info, Token>` program) IS
  FP family A — the diagnostic correctly no longer fires. This test asserts the
  old wrong behavior. Update it: assert NO token-program-type diagnostic fires
  (and remove/replace its quickfix expectations).
- Tests 1–3 are TRUE positives (missing token_program field / `Program<System>`
  as token program) that should still fire. The token.rs widening alone cannot
  suppress a missing-field diagnostic, so the prime suspect is the
  applicability change: `resolve_declared_field_account_type_with_symbols` may
  return `Unknown` for fixtures that do not `use anchor_spl::token::{TokenAccount, Mint}`
  (the test sources have no imports), and the new early-return
  `if declared_type == ResolvedAccountType::Unknown { return Vec::new(); }` then
  suppresses everything. Confirm by dumping `crate::diagnostics::collect()` for
  test 1's fixture. If confirmed, the fix must keep bare unimported type names
  resolvable (single-file syntax: a bare `TokenAccount` ident with no conflicting
  local definition and no alias is still catalog truth) — do NOT fix by adding
  imports to the test fixtures; real user code also omits imports mid-edit.
- Separately verify the "Add `token_program`" quickfix insertion text: with
  `token_program_type_for_initialized_field` now returning the multi-option
  string, check whether any code action interpolates it into inserted code
  (`pub token_program: <THAT STRING>` would be invalid Rust). The insertion
  source may be `default_program_field_type` in `src/lsp/actions/accounts/mod.rs`
  (keyed by field name) — confirm which one feeds the edit, and keep diagnostic
  message text and quickfix insertion text separate concerns.

### Remaining steps, in order
1. Fix the 4 tests as diagnosed above (general fixes only — corpus-fix rule).
2. Step 3 — committed fixtures `fp_family_a_token_interface` / `fp_family_b_composite_payer`.
3. Step 4 — write the triage outcome comment near the alias handling in
   `spl_semantics.rs` (the fix exists; the record does not).
4. Step 5 — corpus caveats. For 5c: CONFIRMED the hash in
   `.github/workflows/corpus.yml` is a one-character corruption — it reads
   `...bca0028893a2d9` while every other workflow pins
   `dtolnay/rust-toolchain@e97e2d8cc328f1b50210efc529dca0028893a2d9` (`dca`).
   Fix by matching the other workflows' hash exactly.
5. Step 6 — final validation, including a full
   `CORPUS_ENABLED=1 cargo test external_corpus` re-run (the post-alias-fix
   confirmation run was interrupted).
6. Commit the whole plan-01 work as focused commits only when green.

---

## Non-Goals

- Do NOT implement whole-program Rust type/name resolution.
- Do NOT add a `Token2022` or `TokenInterface` allowlist. The fix must be derived from
  the actual type structure (outer wrapper + generic), not a hardcoded name list.
- Do NOT change provability tiers of any existing diagnostic.
- Do NOT fix the `InterfaceAccount<MintAccount>` alias case in code; triage it manually
  first (Step 4).
- Do NOT replace the discovery-mode corpus gate with a hard-fail gate in this plan.
  That is a separate decision after triage is complete.
- Do NOT add `anchor build` or BPF toolchain dependencies to the test suite.
- Do NOT commit changes to `corpus/programs/` (it is gitignored).

---

## Steps

### Step 0: Verify baseline

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```

Both must succeed. If either fails, stop and fix before proceeding. Commit any uncommitted
changes that represent a stable baseline (`git add -p` + commit using conventional commits
format) before making any plan changes.

---

### Step 1: Fix FP Family A — TokenInterface relation (22 FPs)

**The exact code path is unknown; investigation is required first.**

**Strong lead (verified during plan-04 grounding):** the likely true source is
`src/lsp/diagnostics/spl_semantics.rs` — `token_program_kind()` (~line 369) only
recognizes `Program<Token>`, `Program<Token2022>`, and `Interface<TokenInterface>`;
`token_program_override_diagnostics` (~line 112) emits `AnchorSplTokenInterface`
whenever it returns `None`. Start the investigation there, but still confirm with
the corpus output below before editing — do not skip the confirmation.

`is_token_program_field` in `src/lsp/diagnostics/constraint_shape/token.rs` (line 349)
has already been verified to accept `Program<Token>`, `Program<Token2022>`, and
`Interface<TokenInterface>` for non-`InterfaceAccount` initialized fields (lines 355-358
include `program.has_generic_type("TokenInterface")`). Therefore the FP is NOT caused by
a missing `TokenInterface` branch — that logic is already present.

**Investigate first (required before any coding)**:

Run the corpus test with `--nocapture` and grep the raw output for the exact error message
text to identify which function is emitting the diagnostic:

```bash
CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture 2>&1 | \
  grep -E "token program|TokenInterface|Token2022|token_program" | head -40
```

Read the exact messages. Cross-reference with `token_program_init_diagnostics` and
`required_token_program_diagnostic` in
`src/lsp/diagnostics/constraint_shape/token.rs` lines 167-290 to identify the exact
code path triggering each error.

Also check whether `has_generic_type` resolves correctly for the observed type strings:

```bash
grep -n "has_generic_type\|generic_type_names" /Users/ay/Documents/codes/solana/seagrass/src/core/evidence/mod.rs | head -20
```

**Fix (apply only after the investigation above identifies the triggering function)**:

The general fix must ensure the set of accepted token-program types for a
non-`InterfaceAccount` initialized field includes all three variants (`Token`, `Token2022`,
`TokenInterface`) without privileging any one. The non-`InterfaceAccount` branch in
`is_token_program_field` (lines 355-358) already expresses this; if the corpus run confirms
it is not being reached, the cause lies elsewhere (e.g., in how `initialized_field` is
selected, or in `token_program_type_for_initialized_field` producing a misleading expected
string). Identify the gap from the grep output before editing.

The accepted set when `initialized` is NOT `InterfaceAccount` must be (and currently is):
- `Program<Token>`, `Program<Token2022>`, `Program<TokenInterface>` (if `Program` wrapper)
- `Interface<TokenInterface>` (if `Interface` wrapper)

Similarly, update `token_program_type_for_initialized_field` to return a more accurate
expected-type string that reflects the accepted set, so the quickfix message is not
misleading. For `Account<TokenAccount>` or `Account<Mint>` initialized fields, return
`"Program<'info, Token> or Program<'info, Token2022>"` (or the canonical simplest form).

**Constraint**: NEVER add a named allowlist of concrete type strings (`Token2022`,
`marginfi`, etc.) — the fix must generalize to any future SPL token program variant
that Anchor's type system accepts.

**Test to add** in `src/lsp/diagnostics/constraint_shape/tests/token_tests.rs`:

```rust
#[test]
fn accepts_program_token2022_for_account_token_account_init() {
    // Program<Token2022> is a valid token program for Account<TokenAccount> init.
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct CreateToken2022<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}
"#,
    );
    assert!(
        !has_code_and_message(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE, "must be an Anchor token program account"),
        "Program<Token2022> must not trigger token-program-type diagnostic for Account<TokenAccount> init"
    );
}

#[test]
fn accepts_interface_token_interface_for_account_token_account_init() {
    // Interface<TokenInterface> is also valid for Account<TokenAccount> init.
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct CreateAny<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
"#,
    );
    assert!(
        !has_code_and_message(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE, "must be an Anchor token program account"),
        "Interface<TokenInterface> must not trigger token-program-type diagnostic for Account<TokenAccount> init"
    );
}
```

Both tests must pass with zero `AnchorConstraintShape` errors on the token-program field.

**Acceptance for Step 1**:
- `cargo test constraint_shape::tests::token_tests` passes (all existing + new tests green)
- `CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture 2>&1 | grep -E "token program|token_program|TokenInterface|Token2022"` returns no lines (no token-program-type errors remain in corpus output)

---

### Step 2: Fix FP Family B — Composite Accounts struct resolution for payer= (16 FPs)

**Root cause**: `payer_mutability_diagnostics` in
`src/lsp/diagnostics/constraint_shape/init_lifecycle.rs` (line 174) resolves the payer
account by name using `accounts.fields().iter().find(|field| field.field.name == reference.name)`.
`accounts` here is an `AccountSetEvidence` built from a single `#[derive(Accounts)]` struct's
direct fields. If the actual `payer` field lives in a NESTED `#[derive(Accounts)]` sub-struct
(composite/nested accounts pattern), the lookup returns `None` and the diagnostic falls
through to either a false missing-account-reference error or a false payer-mutability error.

The same flat-lookup problem exists in `AnchorMissingAccountReference`
(`src/lsp/diagnostics/account_references/mod.rs`), which uses `accounts.has_account(name)`
(also flat).

**Design constraint**: do NOT implement whole-program type resolution. The fix must remain
Syntactic-tier (single-file or same-document). For same-document composite structs, the
document's `ParsedDocument::symbols().accounts_structs` already contains all structs in
the file. The reference implementation for composite lookup is in
`src/lsp/diagnostics/security/duplicates.rs`: `local_composite_accounts` and
`workspace_composite_accounts` (lines 193-208). Use the same mechanism.

**Approach: extend `AccountSetEvidence` to include composite field names from
same-document nested structs.**

The goal: when building `account_names` in `AccountSetEvidence::new`
(`src/core/evidence/mod.rs` line 70-76), also include field names reachable through
one level of composite struct expansion (same-document only, no cross-file). This makes
`has_account(name)` return `true` for fields that are in a nested struct, suppressing
false "missing account reference" and false payer diagnostics.

**Sub-steps**:

**2a**: In `src/core/evidence/mod.rs`, add a helper
`composite_account_names(document, accounts) -> HashSet<&str>` that mirrors
`local_composite_accounts` from `duplicates.rs` exactly:
- Iterates `accounts.fields`
- For each field, checks if `field.type_name` is a key in
  `document.symbols().accounts_structs` (no `account_constraints` check is needed or
  used in the reference implementation — annotated fields are not excluded)
- If found, adds all field names from that nested struct to the set
- Does NOT recurse beyond one level (avoids cycles; one level covers all observed corpus patterns)

Add the resulting names into `account_names` in `AccountSetEvidence::new`.

**2b**: Do NOT add composite field names to `fields()` — that would cause constraint-shape
rules to emit spurious diagnostics on fields they cannot see constraints for. Only
`account_names` (used by `has_account`) and the `account_references` existence check need
to be widened.

**2c**: In `payer_mutability_diagnostics` (`init_lifecycle.rs` line 186-208), the payer
lookup `accounts.fields().iter().find(...)` returns `None` when payer is in a composite
struct. When `None`, do NOT emit a diagnostic — silence is the correct behavior when the
payer is unreachable (doctrine: when extraction/proof fails, silence, never a guess).
Currently the code uses `filter_map` which already silently drops `None`; verify this is
the case and that no diagnostic is emitted on the `None` path.

If the `None` path does emit a diagnostic (i.e., there is a downstream path that emits
even when payer is not found), add a guard: only emit payer diagnostics when the payer
field is visible in the current struct's direct fields.

**2d**: The `account_names` widening from step 2a automatically fixes the
`AnchorMissingAccountReference` FPs for payer references pointing into composite structs,
because `missing_account_references` checks `accounts.has_account(reference.name)` which
uses `account_names`.

**Test to add** in `src/lsp/diagnostics/constraint_shape/tests/init_lifecycle_tests.rs`:

```rust
#[test]
fn no_payer_diagnostic_when_payer_is_in_composite_sub_struct() {
    // payer field lives in a nested Accounts sub-struct; the check in the parent
    // struct cannot see it and must stay silent rather than emitting a false positive.
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct SharedSigners<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + 32)]
    pub record: Account<'info, Record>,
    pub signers: SharedSigners<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );
    assert!(
        !has_code_and_message(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE, "missing `#[account(mut)]`"),
        "payer in a nested sub-struct must not trigger a false missing-mut diagnostic"
    );
    assert!(
        !has_code_and_message(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE, "pays for"),
        "no payer-shape diagnostic expected when payer is in a composite sub-struct"
    );
}
```

**Test to add** in `src/lsp/diagnostics/account_references/tests/mod.rs` (locate the file
at `src/lsp/diagnostics/account_references/tests/mod.rs` — verify it exists, else check
for the test module location with `find . -path '*/account_references/tests*'`):

```rust
#[test]
fn no_missing_reference_for_account_in_composite_sub_struct() {
    // has_one = authority where authority lives in a nested sub-struct.
    // The rule must not flag it as missing.
    let diagnostics = collect(
        &ParsedDocument::parse(r#"
#[derive(Accounts)]
pub struct SharedSigners<'info> {
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub record: Account<'info, Record>,
    pub signers: SharedSigners<'info>,
}
"#).unwrap()
    );
    assert!(
        diagnostics.iter().all(|d| !d.message.contains("authority")),
        "account in composite sub-struct must not be flagged as missing reference"
    );
}
```

**Acceptance for Step 2**:
- All existing tests pass (no regressions in `constraint_shape::tests`, `account_references::tests`)
- New tests pass
- `CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture 2>&1 | grep -E "payer|mut.*payer|missing.*mut"` returns no lines (no payer-mutability or missing-mut errors remain in corpus output)

---

### Step 3: Add regression fixtures to `fixtures/corpus/`

After Steps 1 and 2 pass the external corpus, add two minimal Anchor source fixtures
under `fixtures/corpus/` (committed, always-on hard gate). These are the general
regression cases for FP families A and B, NOT copies of corpus programs.

**3a**: Create `fixtures/corpus/fp_family_a_token_interface/`:

```
fixtures/corpus/fp_family_a_token_interface/
  Anchor.toml           (minimal, any valid project name)
  programs/
    token_interface_compat/
      Cargo.toml        (anchor-lang dep, no version pin needed for fixture)
      src/
        lib.rs          (see below)
```

`lib.rs` content:

```rust
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Token2022, token_interface::TokenInterface};

declare_id!("FpA1111111111111111111111111111111111111111");

#[program]
pub mod token_interface_compat {}

/// Accepts Program<Token2022> as a valid token program for Account<TokenAccount> init.
#[derive(Accounts)]
pub struct InitWithToken2022<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, anchor_spl::token::TokenAccount>,
    pub mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

/// Accepts Interface<TokenInterface> as a valid token program for Account<TokenAccount> init.
#[derive(Accounts)]
pub struct InitWithTokenInterface<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, anchor_spl::token::TokenAccount>,
    pub mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// InterfaceAccount with Interface<TokenInterface> must still pass.
#[derive(Accounts)]
pub struct InitInterfaceAccount<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer, token::token_program = token_program)]
    pub token: InterfaceAccount<'info, anchor_spl::token_interface::TokenAccount>,
    pub mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
```

**3b**: Create `fixtures/corpus/fp_family_b_composite_payer/`:

```
fixtures/corpus/fp_family_b_composite_payer/
  Anchor.toml
  programs/
    composite_payer/
      Cargo.toml
      src/
        lib.rs          (see below)
```

`lib.rs` content:

```rust
use anchor_lang::prelude::*;

declare_id!("FpB2222222222222222222222222222222222222222");

#[program]
pub mod composite_payer {}

#[account]
pub struct Record {
    pub data: u64,
}

/// Shared signer struct — payer lives here, not in the main Accounts struct.
#[derive(Accounts)]
pub struct SharedSigners<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
}

/// Main struct that references payer from the nested sub-struct.
/// seagrass must not emit false-positive payer-mutability or
/// missing-account-reference diagnostics here.
#[derive(Accounts)]
pub struct CreateRecord<'info> {
    #[account(init, payer = payer, space = 8 + 8)]
    pub record: Account<'info, Record>,
    pub signers: SharedSigners<'info>,
    pub system_program: Program<'info, System>,
}

/// has_one referencing a field in a nested sub-struct.
#[derive(Accounts)]
pub struct UpdateRecord<'info> {
    #[account(mut, has_one = authority)]
    pub record: Account<'info, Record>,
    pub signers: SharedSigners<'info>,
}
```

Both fixtures are automatically picked up by `all_corpus_programs_have_no_error_diagnostics`.
The test (`find_lib_rs_dirs`) recursively walks each `fixtures/corpus/<entry>/` subdirectory
for any file named `lib.rs` at any depth — it does NOT require `lib.rs` to be at a specific
path. The canonical fixture layout `programs/<name>/src/lib.rs` satisfies this. Do NOT place
`lib.rs` directly in the fixture root or any path you expect to be excluded — the walker
will find it wherever it lives. No test file change needed.

**Acceptance for Step 3**:
- `cargo test all_corpus_programs_have_no_error_diagnostics` passes (includes new fixtures)
- `cargo test blueshift_anchor_escrow_has_no_error_diagnostics` still passes

---

### Step 4: Triage — InterfaceAccount<MintAccount> alias (1 case)

This is a human triage step, not a code change.

**Locate the occurrence**:

```bash
CORPUS_ENABLED=1 cargo test external_corpus -- --nocapture 2>&1 | grep -i "MintAccount\|mint_account" | head -10
```

If not surfaced by the corpus run directly, search the corpus programs:

```bash
grep -rn "InterfaceAccount.*MintAccount\|MintAccount.*InterfaceAccount" \
  /Users/ay/Documents/codes/solana/seagrass/corpus/programs/ | head -20
```

**Decision tree**:

A. If the `MintAccount` type is a local type alias for `anchor_spl::token_interface::Mint`
   (i.e., `type MintAccount = anchor_spl::token_interface::Mint;` or similar), then
   seagrass cannot resolve type aliases (single-file only, no in-house type resolution),
   so this is a PROVABILITY CEILING case. The correct action is:
   - Downgrade the diagnostic to WARNING (change provability to `WholeProgram`) OR
   - Silence the check when the inner type is not one of the known SPL mint types
     (`Mint`, `MintAccount` is not in the catalog).
   - Implement the silence path: in `token_interface_mint_reference_diagnostic`
     (`token.rs` line 127), add a guard: only emit if `mint.has_generic_type("Mint")`
     AND `mint.type_name() == Some("Account")` (current code at line 147 already does
     this, so verify whether the FP is from this function at all).

B. If `MintAccount` is genuinely the wrong type (e.g., a custom account struct, not a Mint),
   then the diagnostic is a TRUE POSITIVE. Document this finding; no code change needed.

C. If the corpus occurrence cannot be reproduced (e.g., only in a different version of the
   program than the pinned SHA), document it as STALE and remove from the triage list.

**Record the outcome** in a comment inside `src/lsp/diagnostics/constraint_shape/token.rs`
near `token_interface_mint_reference_diagnostic`, e.g.:

```rust
// Triage 2026-06-10: InterfaceAccount<MintAccount> in mpl-token-metadata at SHA 349e061
// is a type alias for anchor_spl::token_interface::Mint — seagrass cannot resolve aliases,
// provability ceiling applies. Silenced by the `mint.has_generic_type("Mint")` guard at
// line NNN which only fires on the canonical generic name "Mint".
```

No code change unless case A requires a silence guard not already present.

---

### Step 5: Fix corpus caveats

**5a**: SPL manifest subpath

The current `corpus/manifest.toml` entry for `spl-token` has `subpath = "token/program"`.
Verify this is correct at the pinned SHA `264ca72`:

```bash
cd /Users/ay/Documents/codes/solana/seagrass
# If the corpus is already fetched:
ls corpus/programs/spl-token/
```

If the directory is empty or the fetch fails, the subpath is wrong. According to the
plan brief, the correct subpath at this SHA may be `token-swap/program` or may require
repinning. Check the GitHub tree at the pinned SHA:

```bash
# Manual check: open https://github.com/solana-labs/solana-program-library/tree/264ca72
# and verify whether token/program exists. If not, update corpus/manifest.toml subpath.
```

If `token/program` exists at that SHA and the fetch works, this caveat is a false alarm —
leave manifest unchanged and document it.

If `token/program` does NOT exist: update `subpath` in `corpus/manifest.toml` to the
correct path, or change the SHA to one where `token/program` exists (requires re-fetching
with `scripts/fetch-corpus.sh --force`).

**5b**: fetch-corpus.sh python>=3.11 requirement

The script uses `tomllib` which is stdlib only from Python 3.11. Ubuntu 22.04 LTS
(GitHub Actions `ubuntu-latest` as of this writing) ships Python 3.10 by default
(may differ by runner image version). Add a version check to the script:

In `scripts/fetch-corpus.sh`, after the `set -euo pipefail` line, add:

```bash
# Verify python3 >= 3.11 (required for stdlib tomllib).
python3 - <<'PYEOF'
import sys
if sys.version_info < (3, 11):
    print(f"ERROR: python3 >= 3.11 required (got {sys.version}); tomllib is stdlib from 3.11.", file=sys.stderr)
    sys.exit(1)
PYEOF
```

This fails early with a clear error rather than a confusing `ModuleNotFoundError` on
`import tomllib`.

**5c**: Verify corpus workflow action hash

The corpus workflow at `.github/workflows/corpus.yml` uses:

```yaml
uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
```

**Important**: `actions/checkout` has no v6 release. The latest stable major version is
v4 (as of 2026). The `# v6.0.2` comment in the file is almost certainly incorrect —
the hash may resolve to a v4.x commit with a wrong label. Do NOT treat the comment as
authoritative; verify the hash independently.

Verify the hash `de0fac2e4500dabe0009e67214ff5f5447ce83dd` is a real commit on the
`actions/checkout` repository:

```bash
# Manual check:
curl -s "https://api.github.com/repos/actions/checkout/git/commits/de0fac2e4500dabe0009e67214ff5f5447ce83dd" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('sha','NOT FOUND'))"
```

If the hash resolves to a valid commit, look up which tag it corresponds to (it should
be a v4.x tag, not v6) and update the comment to the correct version label, e.g.
`# v4.x.y`.

If the hash is invalid or returns 404: replace with a known-good pinned hash for
`actions/checkout` v4 (the latest stable major version):

```bash
# Find the hash for actions/checkout v4.x latest tag:
curl -s "https://api.github.com/repos/actions/checkout/git/ref/tags/v4" | \
  python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('object',{}).get('sha',''))"
```

Update the `uses:` line in `.github/workflows/corpus.yml` with the verified hash and
the correct version comment.

Note: do NOT change the hash without first verifying the replacement is correct. An
incorrect hash update is worse than leaving the existing hash in place.

---

### Step 6: Final validation

Run the full test suite and linter:

```bash
cargo test 2>&1 | tail -20
cargo clippy --all-targets -- -D warnings 2>&1 | tail -20
```

Run the corpus test and inspect raw error output:

```bash
CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture 2>&1 | \
  grep -E "total ERROR|token_program|TokenInterface|Token2022|payer|missing.*mut|mut.*payer" | head -30
```

Note: the corpus test prints raw per-file error locations — the words "family A" or
"family B" never appear in the output. Count the matching lines manually.

Expected outcome: lines matching token-program-type errors have dropped to 0 (family A
fixed), and lines matching payer-mutability errors have dropped to 0 (family B fixed).
Total ERROR count reduced by ≥38 from the pre-fix baseline. Ideally 0 if no other error
families exist.

Run committed-corpus hard gate:

```bash
cargo test all_corpus_programs_have_no_error_diagnostics blueshift_anchor_escrow_has_no_error_diagnostics
```

Both must pass.

---

## Acceptance Criteria

All of the following must be true before the plan is complete:

1. `cargo test` — all 1485+ tests green (baseline ≥ 1485; count may grow with new tests)
2. `cargo clippy --all-targets -- -D warnings` — zero warnings
3. `cargo test all_corpus_programs_have_no_error_diagnostics` — passes (includes new fixtures)
4. `cargo test blueshift_anchor_escrow_has_no_error_diagnostics` — passes
5. `CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture 2>&1`:
   - `grep -E "token_program|TokenInterface|Token2022"` returns no lines (token-program-type FPs eliminated)
   - `grep -E "payer|missing.*mut|mut.*payer"` returns no lines (payer-mutability FPs eliminated)
   - Total ERROR count reduced by ≥38 from the pre-fix baseline
6. New unit tests added:
   - `token_tests::accepts_program_token2022_for_account_token_account_init` — passes
   - `token_tests::accepts_interface_token_interface_for_account_token_account_init` — passes
   - `init_lifecycle_tests::no_payer_diagnostic_when_payer_is_in_composite_sub_struct` — passes
   - `account_references::tests::no_missing_reference_for_account_in_composite_sub_struct` — passes
7. Step 4 triage outcome documented in a comment in `token.rs` near
   `token_interface_mint_reference_diagnostic`
8. `scripts/fetch-corpus.sh` emits a clear error when Python < 3.11
9. SPL subpath caveat assessed and either confirmed correct or fixed in `corpus/manifest.toml`

---

## Risks and Edge Cases

**R1: Composite struct expansion introduces false negatives.**
Expanding `account_names` to include names from nested structs means `has_account("x")`
returns true for names that are NOT directly accessible as constraint values in the
current struct's attribute syntax. This is correct behavior — Anchor itself expands
composite structs during macro processing. Monitor for any new false-negatives (missed
real errors) after landing.

**R2: Cyclic composite struct references.**
A cycle `A { inner: B }` and `B { inner: A }` would infinite-loop a recursive expansion.
The fix in Step 2 deliberately limits expansion to ONE level (no recursion beyond the
immediate nested struct). Verify the implementation does not recurse.

**R3: Token2022 fix may be a no-op if the existing code already accepts it.**
Re-read `is_token_program_field` carefully before writing any code. If `Token2022` is
already accepted, the 22 FPs come from a different code path. Investigate before editing.

**R4: fetch-corpus.sh python version check may fail on macOS development machines.**
macOS ships Python 3.9 with Xcode Command Line Tools. The version check addition
(Step 5b) will block local fetching on machines with Python < 3.11. This is intentional
and correct; developers must install Python 3.11+ (via Homebrew: `brew install python@3.11`).
The error message added in 5b must be actionable.

**R5: SPL subpath caveat may require a SHA change.**
Repinning the SHA for spl-token changes the corpus snapshot. If a new SHA is needed,
update `corpus/manifest.toml`, re-run `scripts/fetch-corpus.sh --force` locally, and
re-run the corpus test to confirm the fetch works and new programs/new SHA do not
introduce new FPs. Do not update the SHA without verifying the re-fetch.

**R6: The `InterfaceAccount<MintAccount>` case (Step 4) may not be reproducible.**
If the corpus run does not show this specific error after family A/B are fixed, the case
may have been a mis-attribution in the original triage. Document the outcome either way
but do not add dead code to handle a case that does not reproduce.

**R7: fixture Anchor.toml and Cargo.toml content.**
The committed fixtures do not need to be buildable with `anchor build` — they only need
to be parseable by seagrass's `ParsedDocument::parse`. The `Anchor.toml` and `Cargo.toml`
files are for human orientation only and are not read by the test harness. Use minimal
placeholder content. Do NOT add `anchor build` to the CI test for committed fixtures.
