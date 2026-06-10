# Plan 04 — Framework-Agnostic Solana Semantic Model (Staged Migration)

## Context

### What seagrass is

seagrass is an LSP for Solana programs (Anchor, native, Pinocchio). It is built on
tower-lsp + syn + salsa 0.26 (Rust). The repo lives at the path this file is in.

### Why this plan exists

Diagnostics and completions currently share no domain model. Each diagnostic rule
re-walks the syn AST via its own visitor, re-derives the same facts (accounts, constraints,
signer checks) independently. This produces:

1. **FP family A (~22 hits on real mainnet programs):** `AnchorConstraintShape` rejects
   `Program<'info, Token2022>` and `Program<'info, Token>` where `TokenInterface` is the
   valid relation. The rule encodes accepted types as a string pattern rather than querying
   "does this type satisfy the TokenInterface relation?".
2. **FP family B (~16 hits):** `payer = trade.taker` where `taker` is `#[account(mut)]`
   inside a **nested** `#[derive(Accounts)]` sub-struct. The constraint-reference resolver
   only looks at the flat field list of the *current* struct and misses fields resolved
   through composite nesting.
3. Duplicated iterator-tracking logic (~780 lines each) in
   `crates/seagrass-framework/src/native_rules/raw_account.rs` and
   `crates/seagrass-framework/src/native_rules/validation.rs`.
4. No path for native/Pinocchio rules to reuse Anchor-derived facts.

The north-star fix is a **single typed semantic model** populated by per-framework
extractors, with diagnostic/completion queries declared over node kinds. Queries that
require a node kind the extractor could not populate are silenced (no emission), never
guessed.

### Current state (verified facts, do not re-derive)

**Salsa wiring:** `src/runtime/salsa_db/mod.rs` — one `#[salsa::tracked]` function
(`document_symbols`), used only in the `textDocument/documentSymbols` handler. All other
paths (diagnostics, completions, hover, inlay hints, workspace index) bypass salsa and
call `ParsedDocument` / `WorkspaceIndex` directly.

**WorkspaceIndex:** `src/core/workspace/mod.rs` — plain HashMap/Trie, manually
maintained, no salsa change propagation. Build entrypoint is `WorkspaceIndex::build`.

**AnchorSymbols:** `src/core/document/mod.rs` — single-pass flat `HashMap` fields
populated by `AnchorSymbols::from_items`. No cross-item graph edges.

**EvidenceGraph:** `src/core/evidence/mod.rs` — the closest thing to a graph: per-context
`AccountSetEvidence` with cross-instruction reachability. Reconstructed on demand from
`ParsedDocument`; not cached.

**LintVisitor duplication (two near-identical copies):**
- `src/lsp/diagnostics/lint.rs` — Anchor/security diagnostics (takes `&ParsedDocument`)
- `crates/seagrass-framework/src/lint.rs` — native/Pinocchio rules (takes `FrameworkDocument`)
Both have identical `Region` variants, `FunctionBodyRunner`, `RegionMap`.

**Native rule duplication (two near-identical copies):**
- `crates/seagrass-framework/src/native_rules/raw_account.rs` (~780 lines)
- `crates/seagrass-framework/src/native_rules/validation.rs` (~780 lines)
Both reimplement: `ACCOUNT_ITERATOR_METHODS`, `TRANSPARENT_ACCOUNT_ACCESS_METHODS`,
`INITIAL_ITERATOR_ACCOUNT_INDEX`, `AccountIteratorOrigin`, `account_iterator_collection`,
`next_account_info_iterator_name`, `account_iterator_name`, account-index extraction helpers.

**Diagnostic registry:** `src/lsp/diagnostics/registry.rs` — 31 variants of
`AnchorDiagnosticKind`. `default_severity()` derives from `provability()` (Syntactic→ERROR,
WholeProgram|Heuristic→WARNING), enforced by exhaustive match. Do not break this invariant.

**Corpus infrastructure (committed):**
- `corpus/manifest.toml` — pinned repos + SHAs
- `scripts/fetch-corpus.sh` — requires python>=3.11 (tomllib); outputs to `corpus/programs/`
  (gitignored)
- Test gate: `CORPUS_ENABLED=1 cargo test` path-filtered by CI

**Test baseline:** 1485 tests green, clippy clean on `main` as of plan-start commit.

### Architecture doctrine (must be respected in every step)

1. **THREE TRUTH SOURCES.** Every diagnostic claim must be backed by: (a) single-file
   syntax, (b) a pinned generated catalog (anchor-syn fork pinned at rev `4addac5` /
   `solana-program = 2.2.1`, parity-tested), or (c) the toolchain itself (`cargo metadata`
   / `cargo check`). In-house whole-program Rust name/type resolution is **FORBIDDEN** as a
   claim basis. When extraction/proof fails: silence, never a guess.
2. **PROVABILITY CEILING** already landed in `src/lsp/diagnostics/registry.rs`:
   `default_severity()` derives from `provability()`. Do not raise severity above what
   provability allows.
3. **CORPUS-FIX RULE.** A corpus failure may only be fixed as a general rule in the model
   or extractor, never by special-casing or suppressing the specific program that tripped it.
4. **GRACEFUL DEGRADATION = SILENCE.** Queries declare required node kinds; if the extractor
   could not populate them, the query does not run and emits nothing.
5. **NO IN-HOUSE HIR.** Do not implement Rust type inference or name resolution. Arbitrary
   Rust expressions remain out of scope.

---

## Non-Goals

- Full Rust HIR / type inference / competing with rust-analyzer.
- Resolving arbitrary Rust expressions inside constraint values (beyond what syn tokens can
  prove).
- Rewriting the LSP transport, salsa wiring beyond what is listed, or the workspace index
  HashMap structure.
- Porting all 31 diagnostic kinds in this plan; only two are proof-of-concept ports
  (Stage 3). Remaining ports are future work.
- Adding new diagnostic kinds or new completions beyond what emerges naturally from fixing
  FP families A and B.
- Changing the `Provability` enum or severity derivation in `registry.rs`.

---

## Stages

### Stage 0 — Baseline commit + distance audit

**Goal:** green baseline on disk; written distance classification of every diagnostic rule.

#### Step 0.1 — Verify and commit the working tree baseline

Run in order:

```bash
cd /Users/ay/Documents/codes/solana/seagrass
cargo test 2>&1 | tail -3        # must show "1485 passed (2 suites, ...)"
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5  # must show no errors
```

If both pass, commit the current working tree as a baseline:

```bash
git add -A
git commit -m "chore: baseline commit before plan-04 semantic graph migration"
```

If tests or clippy fail, **stop and report**; do not proceed.

#### Step 0.2 — Distance audit (file, not executable)

Produce a file `docs/plans/plan-04-distance-audit.md` (NEW) classifying every rule in
`src/lsp/diagnostics/rules.rs`. For each of the 19 rule entries, record:

- Rule name (code string from registry)
- Current shape: `visitor` (walks syn AST at diagnostic time) or `model` (queries
  pre-built structured data)
- Required node kinds from the target semantic model (see Stage 1 definitions)
- Migration priority: `high` (blocks FP fix), `medium` (will benefit), `low` (heuristic,
  defer or prune)

Source to read for the audit:
- `src/lsp/diagnostics/rules.rs` — static `RULES` array (19 entries)
- `src/lsp/diagnostics/registry.rs` — `AnchorDiagnosticKind` (31 variants), provability
  tiers
- Per-rule implementing modules listed in registry.rs comments

Do not change any Rust source in this step. The audit is a markdown file only.

---

### Stage 1 — Define the semantic model

**Goal:** a new Rust module `src/core/semantic/mod.rs` (NEW) with plain structs (no salsa
at this stage) that define the model nodes, edges, and populatedness metadata.

#### Step 1.1 — Create `src/core/semantic/` module (NEW files)

Files to create:

- `src/core/semantic/mod.rs` — re-exports; declares `SemanticModel`
- `src/core/semantic/nodes.rs` — all node type definitions
- `src/core/semantic/populated.rs` — `Populated<T>` wrapper and `NodeKind` enum

**Node definitions** (implement in `nodes.rs`):

```rust
/// Top-level handle for one parsed file's semantic content.
pub struct SemanticModel {
    pub program: Option<Populated<Program>>,
    pub instructions: Vec<Populated<Instruction>>,
    pub accounts_structs: Vec<Populated<AccountsStruct>>,
    pub error_types: Vec<Populated<ErrorType>>,
}

pub struct Program {
    pub id: ProgramId,   // declared program ID string, source range
}

pub struct Instruction {
    pub name: String,
    pub context_type: Option<String>,       // name of AccountsStruct used
    pub parameters: Vec<InstructionParam>,
    pub cpi_calls: Vec<CpiCall>,
    pub signer_checks: Vec<Check>,          // runtime signer checks in body
    pub owner_checks: Vec<Check>,
    pub discriminator_checks: Vec<Check>,
}

pub struct AccountsStruct {
    pub name: String,
    pub fields: Vec<AccountField>,
    /// Composite nesting: fields whose type is another #[derive(Accounts)] struct.
    pub composite_refs: Vec<CompositeRef>,
}

pub struct CompositeRef {
    pub field_name: String,
    pub target_struct_name: String,
    /// Resolved target, if the struct is defined in the same document/workspace.
    pub resolved: Option<Box<AccountsStruct>>,
}

pub struct AccountField {
    pub name: String,
    pub account_type: AccountType,
    pub constraints: Vec<Constraint>,
    /// Set by the extractor when this field's wrapper type or generic parameter satisfies
    /// the SPL token-interface relation (Interface<T>, InterfaceAccount<T>, or
    /// Program<T> where T is a catalog-confirmed SPL token program type).
    /// Queries that check token-program compatibility must read this flag rather than
    /// pattern-matching type names directly.
    pub token_interface_candidate: bool,
}

/// The recognized Anchor / native wrapper kind for an account field.
/// Use `Unknown` when the wrapper cannot be determined from single-file syntax.
pub enum AccountType {
    /// `AccountInfo<'info>` — raw account info, no ownership or discriminator checks.
    RawAccountInfo,
    /// `Account<'info, T>` — Anchor-owned, discriminator-checked.
    Account,
    /// `UncheckedAccount<'info>` — explicitly unchecked; requires `/// CHECK:` doc comment.
    UncheckedAccount,
    /// `Interface<'info, T>` — satisfies an interface (e.g. `TokenInterface`).
    Interface,
    /// `InterfaceAccount<'info, T>` — account checked against an interface type.
    InterfaceAccount,
    /// `Program<'info, T>` — executable program account.
    Program,
    /// `Signer<'info>` — must be a transaction signer.
    Signer,
    /// `SystemAccount<'info>` — owned by the system program.
    SystemAccount,
    /// Type could not be determined from syntax alone; do not emit diagnostics that
    /// require a known type.
    Unknown,
}

pub struct Constraint {
    pub key: String,
    pub value: ConstraintValue,
    pub source_range: lsp_types::Range,
}

pub enum ConstraintValue {
    AccountRef(String),     // e.g. payer = user → AccountRef("user")
    Expression(String),     // raw token string, not further resolved
    Bool(bool),
    Absent,
}

pub struct PdaSeedSet {
    pub seeds: Vec<PdaSeed>,
    pub bump: Option<ConstraintValue>,
    pub program_id: Option<ConstraintValue>,
}

pub enum PdaSeed {
    Literal(Vec<u8>),
    AccountRef(String),
    Expression(String),
}

/// A non-context parameter of an instruction handler function.
/// `type_name` is the raw token string of the Rust type; not further resolved.
pub struct InstructionParam {
    pub name: String,
    pub type_name: Option<String>,
}

pub struct CpiCall {
    pub callee_program_ref: Option<String>,   // account field name used as program
    pub source_range: lsp_types::Range,
}

pub struct Check {
    pub kind: CheckKind,
    pub subject_ref: Option<String>,          // account field name, if resolved
    pub source_range: lsp_types::Range,
}

pub enum CheckKind { Signer, Owner, Discriminator, KeyEquality }

pub struct ErrorType {
    pub name: String,
    pub codes: Vec<ErrorCode>,
}

pub struct ErrorCode {
    pub name: String,
    pub discriminant: Option<u32>,
}
```

**Populatedness wrapper** (implement in `populated.rs`):

```rust
/// Wraps a node and records which fields were successfully extracted.
/// A query that requires a field checks `populated_fields` before using it.
pub struct Populated<T> {
    pub inner: T,
    pub populated_fields: PopulatedFields,
    pub extraction_confidence: ExtractionConfidence,
}

pub enum ExtractionConfidence {
    /// Derived from macro-announced structure (Anchor); near-certain.
    MacroAnnounced,
    /// Derived from idiomatic API patterns (Pinocchio, native); good but not certain.
    IdiomBased,
    /// Partial extraction; some fields missing.
    Partial,
}

/// Bitflags or an enum set recording which optional fields are populated.
/// Defined per node type as a bitflag struct (use the `bitflags` crate if
/// already a dep; otherwise a plain u64 newtype is acceptable).
pub struct PopulatedFields(pub u64);

impl PopulatedFields {
    pub const COMPOSITE_REFS_RESOLVED: u64 = 1 << 0;
    pub const SIGNER_CHECKS: u64           = 1 << 1;
    pub const OWNER_CHECKS: u64            = 1 << 2;
    pub const DISCRIMINATOR_CHECKS: u64    = 1 << 3;
    pub const PDA_SEEDS: u64               = 1 << 4;
    pub const CPI_CALLS: u64               = 1 << 5;
    pub const ERROR_DISCRIMINANTS: u64     = 1 << 6;

    pub fn has(&self, flag: u64) -> bool { self.0 & flag != 0 }
    pub fn set(&mut self, flag: u64) { self.0 |= flag; }
}
```

**`SemanticModel` query helper** (in `mod.rs`):

```rust
impl SemanticModel {
    /// Return all AccountField records visible from `struct_name`,
    /// following composite_refs one level deep (then recursively).
    /// Returns empty vec if struct_name is not found.
    pub fn all_fields_for_struct(&self, struct_name: &str) -> Vec<&AccountField> { ... }
}
```

This is the method that fixes FP family B: callers that previously only looked at the
flat field list of the directly-named struct now call this and get composite-resolved fields.

#### Step 1.2 — Wire `src/core/mod.rs`

Add `pub mod semantic;` to `src/core/mod.rs` (EXISTING file:
`src/core/mod.rs`).

#### Step 1.3 — Tests for the model module

Create `src/core/semantic/tests.rs` (NEW). Write unit tests for:
- `SemanticModel::all_fields_for_struct` — flat struct, composite one level deep, circular
  reference guard (no infinite loop).
- `Populated<T>::populated_fields.has()` for each defined flag.
- `ConstraintValue::AccountRef` round-trip (construction + destructuring).

No extractor is called in these tests; construct model nodes directly.

Run after this step:

```bash
cargo test --lib core::semantic 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

Both must pass. Commit:

```bash
git add src/core/semantic/ src/core/mod.rs
git commit -m "feat(semantic-model): define framework-agnostic node types and populatedness wrapper"
```

---

### Stage 2 — Anchor extractor

**Goal:** a new module `src/anchor/extractor/mod.rs` (NEW) that converts
`ParsedDocument` → `SemanticModel` for Anchor programs. This replaces/wraps portions of
`AnchorSymbols::from_items` without deleting it (existing callers must keep compiling).

#### Step 2.1 — Create `src/anchor/extractor/` (NEW files)

- `src/anchor/extractor/mod.rs` — `pub fn extract(document: &ParsedDocument) -> SemanticModel`
- `src/anchor/extractor/accounts_struct.rs` — `extract_accounts_struct(item: &syn::ItemStruct) -> Populated<AccountsStruct>`
- `src/anchor/extractor/instruction.rs` — `extract_instruction(item: &syn::ItemFn, symbols: &AnchorSymbols) -> Populated<Instruction>`
- `src/anchor/extractor/constraints.rs` — `extract_constraints(attr: &syn::Attribute) -> Vec<Constraint>`

**Extraction rules for Anchor (apply these, no others):**

- An `AccountsStruct` is any `#[derive(Accounts)]` struct item (provable from single-file
  syntax — truth source a).
- A field's type is `AccountField.account_type` derived from the generic wrapper:
  `Account<'info, T>`, `AccountInfo<'info>`, `UncheckedAccount<'info>`,
  `Interface<'info, T>`, `InterfaceAccount<'info, T>`, `Program<'info, T>`,
  `Signer<'info>`, `SystemAccount<'info>` — identifiers parsed from the type token stream
  (truth source a + catalog b).
- A composite field: type is an ident with no recognized wrapper AND the ident appears as
  the name of another `#[derive(Accounts)]` struct in the same `ParsedDocument` (single-
  file) or `WorkspaceIndex` (cross-file). Set `CompositeRef.resolved` only if lookup
  succeeds; leave `None` otherwise; do NOT set `COMPOSITE_REFS_RESOLVED` flag unless all
  composite refs in the struct resolved.
- Constraints: parse from `#[account(...)]` token stream. For `payer = IDENT`,
  `has_one = IDENT`, `close = IDENT`, `realloc::payer = IDENT`, and similarly-shaped
  assignment constraints, extract `ConstraintValue::AccountRef(ident_string)`. Everything
  else: `ConstraintValue::Expression(token_string)`.
- PDA: from `seeds = [...]` and `bump` / `seeds::program = ...`. Extract literal seeds
  into `PdaSeed::Literal`; ident seeds into `PdaSeed::AccountRef`; anything else into
  `PdaSeed::Expression`. Set `POPULATED.PDA_SEEDS` if seeds list was parseable.
- Signer checks in instruction body: reuse `AccountUsageVisitor` output already stored in
  `InstructionSymbol.signer_usages`; convert to `Check { kind: CheckKind::Signer, ... }`.
  Set `SIGNER_CHECKS` flag.
- Do NOT attempt cross-file type resolution for generic parameters (e.g. do not resolve
  `T` in `Account<'info, T>`). Record the ident string only.

**TokenInterface relation fix (FP family A):**

In `extract_accounts_struct`, for each field whose wrapper is `Program<'info, T>`:
- Check if `T` is one of: `Token2022`, `Token`, or any ident that is declared with
  `#[interface(token_program)]` in the same document (lookup via `AnchorSymbols`).
  Actually: do NOT hardcode type names. Instead, set a new flag
  `AccountField.token_interface_candidate = true` when the wrapper is `Interface<'info, T>`
  or `InterfaceAccount<'info, T>`, AND set it for `Program<'info, T>` where the catalog
  (truth source b) lists `T` as a known SPL token program type.
  The catalog check: read from
  `crates/seagrass-anchor-v2-preview/src/generated/anchor_field_completions_generated.rs`
  (truth source b). That file lists `Program<'info, Token>`, `Program<'info, Token2022>`,
  `Program<'info, TokenInterface>`, `InterfaceAccount<'info, Mint>`, and related entries
  that establish which generic type names qualify as SPL token program types. The
  anchor-v1 catalog at `crates/seagrass-anchor-v1/src/generated/` contains only a
  metadata manifest (`anchor_support_generated.rs`) with no field-completion entries;
  do not read it for type names. Do not invent new catalog entries; only use what is
  present in the v2-preview field-completions file.

#### Step 2.2 — Wire extractor to `src/anchor/mod.rs`

In `src/anchor/mod.rs` (EXISTING: `src/anchor/mod.rs`), add:

```rust
pub mod extractor;
```

Do not remove any existing `pub mod` declarations.

#### Step 2.3 — Tests for the Anchor extractor

Create `src/anchor/extractor/tests.rs` (NEW). Tests must cover:

- Flat `#[derive(Accounts)]` struct with `init`, `payer`, `has_one`, `seeds`/`bump`.
- Composite field where inner struct is defined in the same source string.
- `Program<'info, Token>` field: verify `token_interface_candidate` is set correctly.
- `InterfaceAccount<'info, Mint>` field: verify `token_interface_candidate` is set.
- Field with `payer = trade.taker` where `taker` is in a nested sub-struct: verify
  `all_fields_for_struct` returns `taker` so a query looking for `taker` finds it.
- Instruction with `AccountUsageVisitor`-derived signer checks populated.

Run after this step:

```bash
cargo test --lib anchor::extractor 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

Both must pass. Commit:

```bash
git add src/anchor/extractor/ src/anchor/mod.rs
git commit -m "feat(anchor-extractor): populate SemanticModel from ParsedDocument for Anchor programs"
```

---

### Stage 3 — Port two diagnostics as proof-of-concept queries

**Goal:** fix FP family B (nested payer) and the `AnchorMissingAccountReference` rule by
routing them through `SemanticModel` instead of the flat `EvidenceGraph`. Simultaneously
show a signer-style query that could run unmodified against the Pinocchio extractor later.

The **CORPUS-FIX RULE** applies: the fix must be a general model/query change, not
suppression of specific programs.

#### Step 3.1 — Fix `AnchorMissingAccountReference` to resolve through composites

File: `src/lsp/diagnostics/account_references/mod.rs` (EXISTING).

Current behaviour: `EvidenceGraph::from_document` → `AccountSetEvidence.has_account(name)`
checks only the flat field list of the struct.

New behaviour:

1. Call the Anchor extractor to get `SemanticModel`.
2. In `missing_account_references`, replace the `accounts.has_account(reference.name)` call
   with `semantic_model.all_fields_for_struct(&accounts.accounts.name).iter().any(|f| f.name == reference.name)`.
3. Keep the existing `workspace_instruction_args` path for cross-file instruction arguments.
4. Keep existing `document_has_constant` check.

The `EvidenceGraph` / `AccountSetEvidence` pipeline is **NOT deleted**; only the field
lookup is delegated to the model. This is a minimal, targeted change.

Acceptance for this sub-step: write a new test in
`src/lsp/diagnostics/account_references/tests/mod.rs` (EXISTING directory, check if
`tests/mod.rs` or `tests.rs` exists; adapt accordingly):

```rust
// EXISTING: src/lsp/diagnostics/account_references/tests/mod.rs  (or add inline)
#[test]
fn no_false_positive_for_composite_struct_payer() {
    // Anchor source with:
    //   #[derive(Accounts)] struct Trade<'info> {
    //     #[account(mut)] pub taker: Signer<'info>,
    //     pub escrow: ... ,
    //   }
    //   #[derive(Accounts)] struct CloseEscrow<'info> {
    //     pub trade: Trade<'info>,
    //     #[account(init, payer = trade.taker, space = 8)] pub new_acc: Account<'info, State>,
    //   }
    // Expect: zero AnchorMissingAccountReference diagnostics
    let source = r#"...fill in..."#;
    let document = ParsedDocument::parse_or_empty(source.to_string());
    let diags = collect(&document);
    assert!(diags.is_empty(), "got unexpected diagnostics: {diags:?}");
}
```

#### Step 3.2 — Token interface query (fixes FP family A)

**Audit note (verified before writing this step):**
`src/lsp/diagnostics/constraint_shape/token.rs` — `is_token_program_field()` at lines
349–358 already accepts `Token`, `Token2022`, and `TokenInterface` as valid token program
generics; it is **not** the source of FP family A false positives.

The true FP source is **`src/lsp/diagnostics/spl_semantics.rs`**: the `token_program_kind()`
function (lines 369–378) only recognises `Program<Token>`, `Program<Token2022>`, and
`Interface<TokenInterface>`. When a field uses `InterfaceAccount<Mint>` or
`InterfaceAccount<TokenAccount>` with a `token::token_program`, `mint::token_program`, or
`associated_token::token_program` constraint pointing at a token-program field whose type
does not match those exact patterns, `token_program_override_diagnostics` emits
`AnchorSplTokenInterface`. This path is unaware of the `token_interface_candidate` flag set
by the Anchor extractor.

File to change: `src/lsp/diagnostics/spl_semantics.rs` (EXISTING).

Current behaviour: `token_program_override_diagnostics` calls `token_program_kind(program)`
directly; if it returns `None`, the diagnostic is always emitted regardless of whether the
program field was flagged as a token-interface candidate by the extractor.

New behaviour: before emitting `AnchorSplTokenInterface` for a `token::token_program` /
`mint::token_program` / `associated_token::token_program` override constraint:

1. Obtain the `SemanticModel` for the document (call the Anchor extractor).
2. Find the `AccountField` for the referenced program field by name.
3. If `AccountField.token_interface_candidate == true`, suppress the diagnostic — the field
   satisfies the token-interface relation per the catalog, even if `token_program_kind()`
   cannot resolve it from syntax alone.

`constraint_shape/token.rs` does **not** need to change for FP family A.

This must be a **general check** (any `T` that satisfies the token-interface relation per
the catalog/extractor), not a check for `Token2022` specifically.

Write tests in `src/lsp/diagnostics/spl_semantics.rs` or an adjacent test module:

```rust
#[test]
fn no_false_positive_for_program_token2022_on_mint_constraint() { ... }
#[test]
fn no_false_positive_for_program_token_on_token_account_constraint() { ... }
```

#### Step 3.3 — Signer-auth query as a model-based rule (proof-of-concept)

This is a NEW, parallel implementation of the `SecuritySigner` check that runs from the
`SemanticModel` rather than from `SignerAuthorizationVisitor`.

Create `src/lsp/diagnostics/security/signer_query.rs` (NEW):

```rust
/// Query: for each AccountsStruct, find Account fields with no `signer` constraint
/// and no runtime signer check visible in any reachable instruction body.
/// Requires SIGNER_CHECKS flag to be set; if not set, returns empty (silence).
pub fn missing_signer_diagnostics(model: &SemanticModel) -> Vec<Diagnostic> { ... }
```

This function is **not yet registered** in `rules.rs`. It is tested in isolation:

Create `src/lsp/diagnostics/security/tests/signer_query_tests.rs` (NEW):
- Test: model with SIGNER_CHECKS populated → correct diagnostics emitted.
- Test: model without SIGNER_CHECKS populated → no diagnostics (graceful degradation).
- Test: Anchor source with explicit `constraint = is_signer` → no diagnostic.

Run after Stage 3 steps complete:

```bash
cargo test 2>&1 | tail -3    # must still show 1485+ passed (2 suites, ...), 0 failed
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

Commit:

```bash
git add src/lsp/diagnostics/account_references/ \
        src/lsp/diagnostics/spl_semantics.rs \
        src/lsp/diagnostics/security/signer_query.rs \
        src/lsp/diagnostics/security/tests/
git commit -m "fix(diagnostics): route account-reference and token-interface checks through semantic model"
```

Corpus validation (run if `CORPUS_ENABLED=1` environment is available):

```bash
CORPUS_ENABLED=1 cargo test 2>&1 | grep -E "FAILED|ok\." | tail -20
```

Expected: FP family A count drops from ~22 toward 0; FP family B count drops from ~16
toward 0. If new failures appear, apply CORPUS-FIX RULE: fix the general model, not the
specific program.

---

### Stage 4 — Native and Pinocchio extractors; collapse duplicated rules

**Goal:** (a) implement extractors for native/Pinocchio that populate the same
`SemanticModel`; (b) delete the duplicated iterator-tracking constants and helpers from
`raw_account.rs` and `validation.rs` into a single shared location; (c) route the
signer-auth proof-of-concept query (Stage 3.3) through the framework-agnostic model so it
runs for native and Pinocchio programs too.

#### Step 4.1 — Extract shared account-iterator helpers into `common.rs`

File: `crates/seagrass-framework/src/native_rules/common.rs` (EXISTING, currently ~7
helpers).

Move from both `raw_account.rs` and `validation.rs` into `common.rs`:

- `ACCOUNT_ITERATOR_METHODS: &[&str]`
- `TRANSPARENT_ACCOUNT_ACCESS_METHODS: &[&str]`
- `INITIAL_ITERATOR_ACCOUNT_INDEX: usize`
- `struct AccountIteratorOrigin { next_index: usize }`
- `fn account_iterator_collection(...)`
- `fn next_account_info_iterator_name(...)`
- `fn account_iterator_name(...)`
- All account-index extraction helpers that are identical in both files

In `raw_account.rs` and `validation.rs`, replace each definition with a `use
super::common::...` import. Do **not** change any logic; only move definitions.

**Verification:** after moving, run:

```bash
cargo test -p seagrass-framework 2>&1 | tail -10
cargo clippy -p seagrass-framework -- -D warnings 2>&1 | tail -5
```

Both must pass before proceeding.

#### Step 4.2 — Create `crates/seagrass-framework/src/extractor/` (NEW files)

- `crates/seagrass-framework/src/extractor/mod.rs` — `pub fn extract_native(document: FrameworkDocument<'_>, kind: FrameworkKind) -> SemanticModel`
- `crates/seagrass-framework/src/extractor/account_iter.rs` — converts account-iterator
  tracking (from `common.rs` helpers) into `AccountField` + `Check` nodes

**Extraction rules for native/Pinocchio:**

- **Program node:** not extractable from native source without toolchain evidence; leave
  `SemanticModel.program = None`. Do not guess.
- **AccountsStruct equivalent:** there is no `#[derive(Accounts)]`; instead, extract
  account bindings from `next_account_info` call sequences at the top of instruction
  entrypoints. Use `account_iterator_collection` from `common.rs` to identify the iterator,
  then track each `next_account_info(...)` call in order → `AccountField { name: bound_ident,
  account_type: AccountType::RawAccountInfo, constraints: [] }`. Set
  `ExtractionConfidence::IdiomBased`. Set `COMPOSITE_REFS_RESOLVED = false` (no composite
  concept in native).
- **Signer checks:** use `NativeAccountValidationVisitor`'s existing detection of
  `.is_signer` field access. Translate into `Check { kind: CheckKind::Signer,
  subject_ref: Some(bound_ident), ... }`. Set `SIGNER_CHECKS` flag.
- **Owner checks:** translate from `NativeRawAccountInvariantVisitor`'s owner-check
  detection. Set `OWNER_CHECKS` flag.
- **Discriminator checks:** set `DISCRIMINATOR_CHECKS` if a discriminator byte comparison
  is detected.

**Do NOT** implement type inference for generic program-account types in native. If the
account type cannot be determined from the `next_account_info` pattern alone, use
`AccountType::Unknown`.

#### Step 4.3 — Wire native extractor to `native_rules/mod.rs`

In `crates/seagrass-framework/src/native_rules/mod.rs` (EXISTING), add an extraction call
at the top of `diagnostics(document, kind)`:

```rust
let model = crate::extractor::extract_native(document, kind);
```

Pass `&model` alongside the existing visitor inputs. Do not remove any existing visitor
calls yet; add model extraction in parallel.

Add `pub mod extractor;` to `crates/seagrass-framework/src/lib.rs` (EXISTING).

#### Step 4.4 — Register `missing_signer_diagnostics` for native/Pinocchio

In `crates/seagrass-framework/src/native_rules/mod.rs`, call
`missing_signer_diagnostics(&model)` and extend the returned diagnostics vec with the
result. The function signature from Stage 3.3 is:
`fn missing_signer_diagnostics(model: &SemanticModel) -> Vec<Diagnostic>`.

To share this function: move it to `src/core/semantic/queries.rs` (NEW) and have both the
Anchor (`src/lsp/diagnostics/security/signer_query.rs`) and native paths delegate to it.
`src/core/semantic/queries.rs` takes `&SemanticModel` only — no framework-specific types.

#### Step 4.5 — Tests for native extractor and shared query

Create `crates/seagrass-framework/src/extractor/tests.rs` (NEW):
- Test: native entrypoint with `next_account_info` sequence → correct `AccountField` names in order.
- Test: signer check detected → `Check { kind: Signer }` populated, `SIGNER_CHECKS` flag set.
- Test: no signer check → `SIGNER_CHECKS` not set → `missing_signer_diagnostics` returns
  empty (graceful degradation).

Create `src/core/semantic/tests/queries.rs` (NEW, add to the `tests.rs` module from Stage 1.3):
- Test: `missing_signer_diagnostics` with SIGNER_CHECKS populated → correct diagnostics.
- Test: `missing_signer_diagnostics` without SIGNER_CHECKS → empty.

Run after all Stage 4 steps:

```bash
cargo test 2>&1 | tail -10
cargo test -p seagrass-framework 2>&1 | tail -10
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

All must pass with 0 failures. Total test count should be >= 1485 (new tests added).

Commit:

```bash
git add crates/seagrass-framework/src/native_rules/common.rs \
        crates/seagrass-framework/src/native_rules/raw_account.rs \
        crates/seagrass-framework/src/native_rules/validation.rs \
        crates/seagrass-framework/src/extractor/ \
        crates/seagrass-framework/src/lib.rs \
        crates/seagrass-framework/src/native_rules/mod.rs \
        src/core/semantic/
git commit -m "feat(native-extractor): populate SemanticModel for native/Pinocchio; collapse duplicated iterator helpers"
```

---

## Acceptance Criteria

All commands must be run from `/Users/ay/Documents/codes/solana/seagrass`.

### After Stage 0

```
cargo test 2>&1 | tail -3
# Expected: 1485 passed (2 suites, ...)
# Note: workspace total is 1485 across two suites (main crate + seagrass-framework)
cargo clippy --all-targets -- -D warnings 2>&1 | grep -c "^error"
# Expected: 0
```

### After Stage 1

```
cargo test --lib core::semantic 2>&1 | tail -3
# Expected: N passed (N >= 5); 0 failed
cargo clippy --all-targets -- -D warnings 2>&1 | grep -c "^error"
# Expected: 0
```

### After Stage 2

```
cargo test --lib anchor::extractor 2>&1 | tail -3
# Expected: N passed (N >= 6); 0 failed
cargo test 2>&1 | tail -3
# Expected: 0 failed; total >= 1485 (2 suites)
```

### After Stage 3

```
cargo test 2>&1 | tail -3
# Expected: 0 failed; total >= 1485 (2 suites)
cargo test lsp::diagnostics::account_references 2>&1 | grep -E "no_false_positive|FAILED"
# Expected: "no_false_positive_for_composite_struct_payer ... ok"
cargo test lsp::diagnostics::spl_semantics 2>&1 | grep -E "no_false_positive|FAILED"
# Expected: two new token interface tests pass (no_false_positive_for_program_token2022_on_mint_constraint, no_false_positive_for_program_token_on_token_account_constraint)
cargo test lsp::diagnostics::security::tests::signer_query 2>&1 | grep -E "ok|FAILED"
# Expected: all three signer_query tests pass
cargo clippy --all-targets -- -D warnings 2>&1 | grep -c "^error"
# Expected: 0
```

Corpus check (requires `CORPUS_ENABLED=1` and fetched corpus; skip if unavailable):

```
CORPUS_ENABLED=1 cargo test 2>&1 | grep -E "FAILED|test result"
# Expected: 0 failed; FP count for anchor-constraint-shape and
#           anchor-missing-account-reference reduced from corpus baseline
```

### After Stage 4

```
cargo test 2>&1 | tail -3
# Expected: 0 failed; total > Stage 3 total (new tests added)
cargo test -p seagrass-framework 2>&1 | tail -3
# Expected: 0 failed
cargo clippy --all-targets -- -D warnings 2>&1 | grep -c "^error"
# Expected: 0
# Verify duplicate constants gone:
grep -rn "ACCOUNT_ITERATOR_METHODS" \
  crates/seagrass-framework/src/native_rules/raw_account.rs \
  crates/seagrass-framework/src/native_rules/validation.rs
# Expected: zero matches (definitions moved to common.rs; only `use` statements remain)
```

---

## Risks and Edge Cases

### Circular composite references

`SemanticModel::all_fields_for_struct` follows `CompositeRef.resolved` recursively.
Implement a `visited: HashSet<&str>` guard: if the struct name is already in visited, stop
recursing and return the fields collected so far. Without this, a malformed program with
circular `#[derive(Accounts)]` embedding would stack-overflow.

### CompositeRef resolution across files

Stage 2 populates `CompositeRef.resolved` only if the target struct is in the same
`ParsedDocument`. For cross-file composites, `resolved` is `None` and
`COMPOSITE_REFS_RESOLVED` is not set. The query in Stage 3.1 must fall back gracefully:
if `COMPOSITE_REFS_RESOLVED` is not set for a struct, do not emit diagnostics for
unresolved composite refs — silence, not a guess. Cross-file resolution requires
`WorkspaceIndex` look-up; wire it in Stage 2 if `Option<&WorkspaceIndex>` is already
available at the extraction call site (check `engine.rs` and `collect_with_workspace`
signatures), otherwise defer.

### Existing callers of `AnchorSymbols` and `EvidenceGraph`

Stages 2–4 add the semantic model alongside existing structures; they do not remove
`AnchorSymbols`, `EvidenceGraph`, or `AccountSetEvidence`. All 19 rules in `rules.rs` must
still compile and run. Only the two rules touched in Stage 3 are changed. Do not refactor
rules not listed in Stage 3.

### Catalog structure for token interface (FP family A)

Before implementing `token_interface_candidate` in Step 2.1, read
`crates/seagrass-anchor-v2-preview/src/generated/anchor_field_completions_generated.rs`
to extract the SPL token program type names it lists (e.g. `Token`, `Token2022`,
`TokenInterface`, `AssociatedToken`, `Mint`). Use those as the truth-source-b set.
The anchor-v1 generated directory contains only a metadata manifest with no field-completion
type entries; the v2-preview field-completions file is the authoritative catalog for this
check. Do not hardcode type name strings in extractor code — derive them from the catalog.

### SPL corpus manifest subpath

`corpus/manifest.toml` has a known bad subpath for the SPL repo: `token/program` is wrong
at the pinned SHA. Use `token-swap/program` or repin the SHA before running corpus tests.
See `scripts/fetch-corpus.sh` for the subpath variable.

### `fetch-corpus.sh` python requirement

The script uses `tomllib` (stdlib since Python 3.11). If the system python is older,
`CORPUS_ENABLED=1` tests will fail at the fetch step. Install python>=3.11 or use
`uv run --python 3.11 scripts/fetch-corpus.sh` if `uv` is available.

### Test count regression

Stages 2–4 add new tests. If `cargo test` reports a total lower than 1485, a test module
is not being compiled. Check `src/lib.rs` and `crates/*/src/lib.rs` for missing `mod`
declarations.

### Salsa: do not expand scope in Stage 1

Stage 1 defines model nodes as plain Rust structs, **not** salsa-tracked. Wrapping them in
salsa at this stage would require all callers to hold a `&dyn Database`, changing many
function signatures. If salsa-backed incremental extraction is desired later, it is a
separate plan. The single existing salsa use (`document_symbols` in
`src/runtime/salsa_db/mod.rs`) must remain untouched.

### `LintVisitor` duplication: do not merge in this plan

The two copies of `LintVisitor` (`src/lsp/diagnostics/lint.rs` and
`crates/seagrass-framework/src/lint.rs`) are noted as technical debt but are not merged
here. Merging requires either moving the main crate's `ParsedDocument` into
`seagrass-framework` (a large refactor) or introducing a new shared crate (another PR).
File as a separate plan item; do not touch in Stages 0–4.

---

## File Index (all paths verified or marked NEW)

| Path | Status | Stage |
|------|--------|-------|
| `src/core/semantic/mod.rs` | NEW | 1 |
| `src/core/semantic/nodes.rs` | NEW | 1 |
| `src/core/semantic/populated.rs` | NEW | 1 |
| `src/core/semantic/tests.rs` | NEW | 1 |
| `src/core/semantic/queries.rs` | NEW | 4 |
| `src/core/mod.rs` | EXISTING — add `pub mod semantic;` | 1 |
| `src/anchor/extractor/mod.rs` | NEW | 2 |
| `src/anchor/extractor/accounts_struct.rs` | NEW | 2 |
| `src/anchor/extractor/instruction.rs` | NEW | 2 |
| `src/anchor/extractor/constraints.rs` | NEW | 2 |
| `src/anchor/extractor/tests.rs` | NEW | 2 |
| `src/anchor/mod.rs` | EXISTING — add `pub mod extractor;` | 2 |
| `src/lsp/diagnostics/account_references/mod.rs` | EXISTING — modify field lookup | 3 |
| `src/lsp/diagnostics/constraint_shape/token.rs` | EXISTING — no change needed; is_token_program_field() already accepts Token/Token2022/TokenInterface | — |
| `src/lsp/diagnostics/spl_semantics.rs` | EXISTING — add token_interface_candidate guard in token_program_override_diagnostics | 3 |
| `src/lsp/diagnostics/security/signer_query.rs` | NEW | 3 |
| `src/lsp/diagnostics/security/tests/signer_query_tests.rs` | NEW | 3 |
| `crates/seagrass-framework/src/native_rules/common.rs` | EXISTING — add moved helpers | 4 |
| `crates/seagrass-framework/src/native_rules/raw_account.rs` | EXISTING — replace defs with `use` | 4 |
| `crates/seagrass-framework/src/native_rules/validation.rs` | EXISTING — replace defs with `use` | 4 |
| `crates/seagrass-framework/src/extractor/mod.rs` | NEW | 4 |
| `crates/seagrass-framework/src/extractor/account_iter.rs` | NEW | 4 |
| `crates/seagrass-framework/src/extractor/tests.rs` | NEW | 4 |
| `crates/seagrass-framework/src/lib.rs` | EXISTING — add `pub mod extractor;` | 4 |
| `crates/seagrass-framework/src/native_rules/mod.rs` | EXISTING — add model extraction call | 4 |
| `docs/plans/plan-04-distance-audit.md` | NEW | 0 |
| `corpus/manifest.toml` | EXISTING (read only; fix SPL subpath if needed) | 0 |
| `scripts/fetch-corpus.sh` | EXISTING (read only) | 0 |
