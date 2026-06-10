# Ecosystem-Knowledge Debt Inventory (2026-06-10)

Findings from a three-way repo scan for "memory of the ecosystem" debt: hardcoded
names, structural assumptions, and duplicated truths that rot as Solana/Anchor/SPL
evolve. Each item maps to the plan that fixes it, or is marked UNCOVERED.

Already-tracked debt (BALANCE_TERMS, token_program_kind inline list, the
solana-program pin, lint.rs twins) is listed only where the scan added new precision.

---

## P0 — Live bugs (wrong today, not just fragile)

### P0.1 Topic mismatch breaks arbitration dedup — UNCOVERED, fix immediately
- `src/lsp/diagnostics/handler_members.rs:33` and
  `src/lsp/diagnostics/handler_struct_literals.rs:26` both declare
  `TOPIC = "seagrass/anchor.account.usage"` while emitting
  `AnchorMissingAccountReference`, whose registry `topic()` is
  `"seagrass/anchor.constraint.account-reference"`.
- Arbitration dedupes by `(range, topic)` → overlapping diagnostics from these
  modules vs `constraint_expressions` are NOT merged → **double diagnostics at the
  same span**, live.
- Fix: emit the registry topic (reference the registry constant; never a literal).
  One-line per file + a test asserting every emitter's topic equals
  `kind.topic()`.

### P0.2 Generated catalogs have drifted between crates — UNCOVERED
- `src/anchor/generated/constraint_catalog_generated.rs` (43 constraints) vs
  `crates/seagrass-anchor-v2-preview/src/generated/constraint_catalog_generated.rs`
  (40): v2-preview is missing `extensions::group_pointer::authority`,
  `extensions::group_pointer::group_address`, `extensions::pausable::authority`.
- Error catalog likewise: main has `ConstraintMintPausableExtension` (2043),
  `ConstraintMintPausableAuthority` (2044); v2-preview lacks both.
- No CI gate compares the three generated copies. Fix: regen all from one script
  run + a parity test/CI check that fails on cross-crate catalog drift.

### P0.3 corpus.yml action hash is corrupted — covered by plan-01 Step 5c (confirmed)
- Every workflow pins `dtolnay/rust-toolchain@e97e2d8...dca0028893a2d9`;
  `corpus.yml:37` alone has `...bca0028893a2d9` — one character off, a copy-paste
  corruption pointing at a nonexistent/unknown commit.

---

## P1 — Architectural debt, owner plan exists

### P1.1 Account wrapper-type lists: 14 independent copies → plan-04
Fourteen sites enumerate subsets of Anchor wrappers with no canonical list, and
they already disagree: `account_generic_argument_with_box_state`
(`anchor_syn/mod.rs:432`) includes `Migration`; its sibling
`is_anchor_account_generic_wrapper` 50 lines above does not. Other copies:
`anchor/types/mod.rs:90`, `local_types/type_names.rs:14`,
`account_usage/mod.rs:97+470` (verbatim same-file duplicate),
`evidence/mod.rs:393` + `security/mod.rs:481` (is_unchecked_account twice),
3× signer lists, 3× program lists, 1 sort-priority table.
→ plan-04's `AccountType` enum is the single source; add "collapse the 14 wrapper
lists onto AccountType" to its Stage 2/3 scope when executed.

### P1.2 Framework/crate detection lists duplicated and incomplete → plan-06
- `PINOCCHIO_DEPENDENCIES` exists in both `solana/frameworks.rs:23` and
  `solana/project/mod.rs:13` (hyphen vs underscore forms, two files, no
  drift test). `NATIVE_SOLANA_DEPENDENCIES` similarly duplicated and will miss
  every new crate from the anza-xyz/solana-sdk split.
- `classify_program` requires `crate-type = ["cdylib"]` — breaks if Cargo grows a
  first-class `sbf` crate-type. `process_instruction` substring check misfires on
  Pinocchio.
→ plan-06's registry becomes the one place crate names live; extend its Step 4 to
rewire `SolanaProjectKind` detection through the registry.

### P1.3 SPL type-name strings scattered across 8+ files → plan-04 (+01)
`account_semantics/mod.rs` alone has ~18 `"Mint"`/`"TokenAccount"` literals — an
inline SPL catalog. Plus `constraint_shape/token.rs`, `spl_semantics.rs`,
`candidates.rs`, `space_values.rs`, `assists/program_fields.rs`,
`account_references/mod.rs:280`, `security/mod.rs:246`.
→ plan-04's `token_interface_candidate` / catalog-derived type relations are the
fix; plan-01 already fixes the diagnostic-emitting subset.

### P1.4 Sysvar names hardcoded outside the catalog → small task, attach to plan-06
- `constraint_expressions/resolution.rs:15` `BUILTIN_ASSOCIATED_PATH_ROOTS`
  hardcodes `"Clock"`, `"Rent"` → a newly-active sysvar (e.g. `EpochRewards`)
  becomes an "unresolved path" FP.
- `anchor_syn/mod.rs:394,553` hardcode Clock/Rent in examples and user-facing
  message text.
- `program_ids.rs:72` `SYSVAR_ID_NAMES = ["rent","clock","instructions"]` — a
  manual subset of the catalog.
→ derive all three from `runtime_catalog::SYSVARS`.

### P1.5 Accept≠offer parity gaps (known FP family) → extend parity tests
`NON_SYSVAR_ADDRESSES` (program_ids.rs) offers `token_2022::ID` and
`associated_token::ID`, but the parity test only covers `token::ID`,
`mpl_token_metadata::ID`, `crate::ID`. The diagnostic side accepts an offered
value only if the import exists → completion can insert a value the diagnostic
then flags. Also: the five non-sysvar base58 addresses have no parity check at
all (sysvar ones do).
→ extend `parity_tests.rs` to cover every NON_SYSVAR_ADDRESSES entry; add
ID-vs-real-crate parity for the five addresses (anchor-spl/dev-dep or catalog).

### P1.6 Diagnostic-code raw strings bypass registry constants → fold into plan-02
~25 production sites type codes like `"anchor-constraint-shape"` raw
(actions/*, engine.rs:232,286, arbitration.rs:265). Renaming a registry constant
breaks nothing at compile time; the action silently stops matching.
→ mechanical sweep: replace literals with registry constants; add to plan-02's
registry-hardening scope. Same for `confidence_rank()` in arbitration.rs:202
(string round-trip with dead `"high"|"medium"|"low"` arms — should match on a
shared enum, plan-02's derive-confidence-from-provability note).

### P1.7 Anchor idiom names (macro-emitted symbols) → centralize, plan-04 extractor
`"DISCRIMINATOR"`, `"INIT_SPACE"` (defined independently in 2 files each),
`"CLOSED_ACCOUNT_DISCRIMINATOR"`, `"declare_id"` (3 sites; `declare_program!`
already missed), `CpiContext::new/new_with_signer/invoke` lists (2 files),
`"reload"` suppression, borsh deserialize method lists. These are facts about
what anchor-lang macros emit — version-coupled to the pinned anchor-syn.
→ centralize into one `anchor_idioms` module colocated with the generated
catalog, regenerated/reviewed on Anchor bump; plan-04's extractor becomes the
sole consumer.

---

## P2 — Structural assumptions, accept-with-tests or delegate

- **Anchor.toml parsed by hand-rolled line scanner** (`core/project/mod.rs:29`) —
  not a TOML parser; any section rename silently empties program lists. Replace
  with `cargo_toml`-style real TOML parsing (same crate family already a dep).
- **Artifact layout** (`target/idl`, `target/deploy`, `target/types` hardcoded in
  program_artifacts, artifacts.rs, definition_bridge/paths.rs:38, ecosystem.rs) —
  Anchor 0.30+ is already shifting IDL output. Centralize the path set in one
  module so a layout change is a one-file fix.
- **IDL field-name shims duplicated** (`idl_program_name`/`idl_address` exist in
  both program_artifacts/mod.rs:503 and ecosystem/mod.rs:475) — must change in
  sync; merge to one helper.
- **mtime-based staleness** (program_artifacts:146) — unreliable on containers/
  network FS; tracked `Xargo.toml` is dead upstream. Note in docs; low urgency
  since artifact diagnostics are already default-off.
- **`anchor-lang` literal dependency-name checks** (check_cfg:439,
  dependency_source.rs:383) — v2 crate-name change breaks both; plan-06 registry
  covers the mechanism, add these call sites to its Step 4 list.

## P3 — Ops/toolchain pins (review cadence, not code)

- All workflows pin `dtolnay/rust-toolchain` by hash + Rust 1.89.0; Bun 1.3.8
  with per-platform SHA-512; `VSCE_VERSION 3.9.1`; Node 24; binary target list
  hardcoded in 4 places (script, release.yaml, 2 tests); Zed target
  `wasm32-wasip2` (unstable ABI). Acceptable as pins — the debt is that the
  target list has no single source. Revisit at release cadence (plan-05 touches
  this machinery anyway).

---

## Duplicated-infrastructure register (known, deferred deliberately)

- `src/lsp/diagnostics/lint.rs` (439 ln) vs `crates/seagrass-framework/src/lint.rs`
  (335 ln): ~95% identical; framework copy has 4 `Applicability` variants vs main's
  1, plus `allows_executable_lints`. Deferred by plan-04 ("do not merge in this
  plan") — keep on the register so it isn't forgotten.
- `SOLANA_CODE_QUALITY_CODE`/`SOURCE` constants and `topic_lint_doc_url` defined
  in both registry.rs and seagrass-framework/diagnostics.rs (identical values
  today, no sync test).
- `companion_is_handled_elsewhere` exclusion list (constraint_shape/catalog.rs:63)
  shadows the generated catalog's `required_companions` — shrink as planned, or
  derive the exclusion from rule registration.
