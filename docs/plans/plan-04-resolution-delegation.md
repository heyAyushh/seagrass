# Plan 04: Delegate Rust-Resolution Facts to the Toolchain

## Context

**Project**: seagrass — Solana Anchor/native/Pinocchio LSP.  
**Stack**: tower-lsp + syn + salsa 0.26 (Rust); `cargo_toml = "0.22.3"` in workspace deps.  
**Baseline**: 1485 tests green, clippy clean, large uncommitted working tree.  
**Canonical branch**: `main`.

### Current state

seagrass already parses `Cargo.toml` text in two places:

1. `src/lsp/diagnostics/check_cfg/mod.rs` — text-scans the manifest for
   `anchor-lang` dependency features via `cargo_toml::Manifest::from_str` and
   `dependency.req_features()`. This is single-package, single-dep, string-grep
   only. It does NOT read workspace-level `[workspace.dependencies]` or resolve
   feature inheritance.

2. `src/solana/project/mod.rs` — `parse_cargo_manifest` reads `Manifest` to
   extract `CargoManifest { deps, dev_deps }` for framework classification.

The `AnchorCheckCfg` diagnostic (`Heuristic/WARNING`) fires when:
- A file uses `init_if_needed` but the local `Cargo.toml` does not have
  `anchor-lang = { ..., features = ["init-if-needed"] }` on a single inline
  `[dependencies]` line.

**FP surface**: If the project uses workspace-inherited dependencies
(`anchor-lang.workspace = true`) with the feature enabled at the
`[workspace.dependencies]` level, seagrass misses it and emits a false positive.
This is the "init\_if\_needed-feature FP class" referenced in the brief.

### Resolution claims audit

`src/lsp/diagnostics/constraint_expressions/resolution.rs` — functions
`unresolved_path_identifier` and `unresolved_call_identifier` — emit FPs when:

1. A glob import (`use some_module::*;`) brings names into scope that seagrass
   cannot enumerate. `imports.rs` (`collect_imported_names`) silently skips
   `UseTree::Glob(_)` — no flag is set on `AnchorSymbols`. The constraint
   expression resolver in `resolution.rs` has no awareness of this. By contrast,
   `context_accounts.rs:document_has_local_glob_import` already implements a
   correct glob-suppression guard for the `AnchorContextAccounts` rule; that
   logic is local to that one rule.

2. An `impl` block method is defined in a different file from the type. `collect_from_impl`
   in `associated_values.rs` collects same-file `impl` items only. If `Foo::bar()`
   is defined in `foo/methods.rs` but `Foo` is declared in `foo/mod.rs`, and the
   constraint references `foo.bar()`, `workspace_has_associated_value` in
   `resolution.rs` needs `WorkspaceIndex::symbol_locations_in_container`. That
   function exists and is called — but only for `[CONSTANT, METHOD, FUNCTION]`
   kinds AND only when the index has been built.  Coverage is good only when the
   full workspace index is available; document-only mode (test / early open) still
   emits FPs.

3. Single-segment identifiers that resolve only through a const/function
   declaration inside a `use` re-export that ended in `*`. Same gap as (1).

`src/lsp/diagnostics/context_accounts.rs` — `AnchorContextAccounts` —
already suppresses when `document_has_local_glob_import && workspace_lacks_visibility`.
This pattern is the model the other rules should follow.

**Boundary rule (doctrine)**: inside Anchor-DSL entities (accounts-struct
fields, PDA seeds, constraint values) seagrass may make provable claims
derived from the catalog (constraint keys, account wrapper types, signer/owner
shapes). For arbitrary Rust expressions inside constraint value positions
(`= <expr>`, `constraint = <bool_expr>`) and handler bodies: the open-world
principle applies — if resolution cannot be proved with certainty, silence.

### Key file paths

All paths verified to exist unless marked NEW.

| Path | Role |
|---|---|
| `src/lsp/diagnostics/check_cfg/mod.rs` | `AnchorCheckCfg` collector; `anchor_lang_dependency_has_feature` |
| `src/lsp/diagnostics/constraint_expressions.rs` | `AnchorConstraintExpression` orchestrator |
| `src/lsp/diagnostics/constraint_expressions/resolution.rs` | `unresolved_path_identifier`, `unresolved_call_identifier`, `path_resolves` |
| `src/core/document/imports.rs` | `collect_imported_names`; silently drops `UseTree::Glob` |
| `src/core/document/mod.rs` | `AnchorSymbols` struct; `from_items` single-pass loop |
| `src/core/document/associated_values.rs` | `collect_from_impl`; same-file only |
| `src/lsp/diagnostics/context_accounts.rs` | reference impl of glob-suppression guard |
| `src/lsp/diagnostics/engine.rs` | `DiagnosticInput` struct; `manifest: Option<(&Url, &str)>` |
| `src/lsp/diagnostics/rules.rs` | static `RULES` array; `collect_constraint_expressions` at line 85, `collect_context_accounts` at line 43 |
| `src/solana/project/mod.rs` | `parse_cargo_manifest`; `CargoManifest` struct |
| `src/lsp/diagnostics/check_cfg/tests/mod.rs` | existing tests for init-if-needed feature check |
| `docs/plans/plan-04-resolution-delegation.md` | **THIS FILE** (NEW) |
| `src/lsp/diagnostics/check_cfg/tests/workspace_feature_tests.rs` | NEW — tests for workspace-inherited feature |

---

## Non-goals

- Full Rust name/type resolution (rust-analyzer's domain). Do not build type
  inference, trait resolution, or cross-crate symbol lookup.
- Parsing arbitrary Rust expressions in handler bodies for type information.
- `cargo check` / `cargo metadata` subprocess invocation at diagnostic time
  (latency budget violation). Use `cargo_toml` crate text parsing only.
- Fixing corpus FP family A (TokenInterface) or family B (composite accounts).
  Those are separate plans.
- Changing provability tiers in `registry.rs`. All affected diagnostics remain
  `Heuristic/WARNING`.
- New LSP features (completions, hover, inlay hints).

---

## Pre-flight

**Before starting any step:**

```bash
cd /path/to/seagrass
cargo test --workspace 2>&1 | tail -5
```

All 1485 tests must pass. If they do not, stop and report; do not proceed.  
If the working tree is uncommitted, commit it first:

```bash
git add -p   # stage selectively — avoid .env, secrets
git commit -m "chore: baseline before plan-04"
```

Verify `cargo clippy --workspace --all-targets -- -D warnings` passes before
starting.

---

## Steps

### Step 1: Extend `AnchorSymbols` with a glob-import presence flag

**File**: `src/core/document/imports.rs`  
**File**: `src/core/document/mod.rs`

**What to change:**

In `imports.rs`, change `collect_imported_names` to return a `bool` indicating
whether any `UseTree::Glob` was encountered, OR add a separate function
`document_has_glob_import(syntax: &syn::File) -> bool` that performs a single
pass. The latter is simpler and avoids touching the `collect_imported_names`
signature.

Add to `AnchorSymbols` (in `mod.rs`, inside the struct):

```rust
/// True when the file contains at least one glob `use` (`use foo::*;`),
/// meaning names are in scope that seagrass cannot enumerate.
/// Diagnostics that assert "name X does not exist" MUST suppress when this
/// flag is set, unless the WorkspaceIndex provides positive evidence of absence.
pub has_glob_import: bool,
```

In `from_items`, set it during the `Item::Use` arm:

```rust
Item::Use(item_use) => {
    collect_imported_names(&item_use.tree, &mut symbols.imported_names, &mut symbols.use_aliases);
    if has_glob_in_use_tree(&item_use.tree) {
        symbols.has_glob_import = true;
    }
}
```

Add a private `has_glob_in_use_tree(tree: &syn::UseTree) -> bool` in
`imports.rs` (mirrors `ends_in_glob` already in `context_accounts.rs` — extract
the shared logic here, then use it from `context_accounts.rs` too, replacing
its inline closure; DO NOT duplicate).

**Doctrine**: this is a syntax fact, provable from a single file. No
whole-program inference.

**Tests**: add inline unit tests in `src/core/document/mod.rs` with these exact
function names (acceptance criterion #6 greps for `has_glob_import` in test names):
- `has_glob_import_false_for_named_import` — file with `use foo::bar;` → `has_glob_import = false`
- `has_glob_import_true_for_star_import` — file with `use foo::*;` → `has_glob_import = true`
- `has_glob_import_true_for_group_containing_star` — file with `use foo::{A, *};` → `has_glob_import = true`

---

### Step 2: Apply glob-suppression guard to `constraint_expressions` resolution

**File**: `src/lsp/diagnostics/constraint_expressions/resolution.rs`  
**File**: `src/lsp/diagnostics/constraint_expressions.rs`

**What to change:**

In `resolution.rs`, both `unresolved_path_identifier` and
`unresolved_call_identifier` receive `document: &ParsedDocument`. Add a guard
at the top of each function (or in their shared caller `expression_issues` in
`constraint_expressions.rs`):

```rust
// Open-world: if glob imports are present and the workspace index cannot
// confirm absence, suppress the diagnostic rather than guess.
if document.symbols().has_glob_import
    && workspace_index.is_none()
{
    return None; // cannot claim unresolved
}
```

If `workspace_index` IS present, proceed with normal resolution (the index
already handles `symbol_exists_at_qualified_path` and
`symbol_locations_in_container`). Do not suppress when the index is available
and confirms the name is absent.

The guard in `context_accounts.rs:document_has_local_glob_import` distinguishes
local globs from external prelude globs (e.g. `anchor_lang::prelude::*`). For
constraint expressions, use a simpler rule: any glob import suppresses
single-identifier resolution claims when there is no workspace index. The
justification: constraint expressions reference local consts/functions by
single-segment identifiers; external prelude globs do not add user-defined
consts that would appear in constraints.

**Doctrine**: silence over guess — open-world principle.

**Tests**: extend the existing `src/lsp/diagnostics/constraint_expressions/tests.rs`
file (do NOT create a separate new file for these tests). Adding a new file would
require a `#[cfg(test)] #[path = "constraint_expressions/<file>.rs"] mod <file>;`
declaration in `constraint_expressions.rs` (mirroring lines 775–793 of that
file); omitting it leaves the tests as dead code the compiler never sees.
Extending `tests.rs` avoids this registration requirement entirely.

1. `glob_import_suppresses_unresolved_identifier_without_workspace` — file with
   `use state::*; ... constraint = unknown_const` → zero diagnostics.
2. `no_glob_import_reports_unresolved_identifier` — same fixture without the
   glob → one `AnchorConstraintExpression` diagnostic.
3. `glob_import_with_workspace_evidence_still_reports_absent_name` — provide a
   `WorkspaceIndex` that does not contain `unknown_const` → diagnostic fires.

---

### Step 3: Lift glob-suppression in `context_accounts.rs` to use the shared flag

**File**: `src/lsp/diagnostics/context_accounts.rs`

**What to change:**

Replace the call to `document_has_local_glob_import(document)` (which
re-walks the syn AST) with `document.symbols().has_glob_import`. Remove the
`document_has_local_glob_import` function and its helpers (`GlobScan`,
`glob_root_segment`, `ends_in_glob`). The distinction between local vs external
globs was conservative for context_accounts but is now encoded centrally.

**Caution**: the existing tests at lines 452 and 481 of `context_accounts.rs`
test glob suppression behavior; ensure they still pass. If a test was relying on
the "local-only glob" distinction (e.g., `anchor_lang::prelude::*` does NOT
suppress), check whether the new flag (which captures all globs) changes that
test's expected outcome. If so, update the test fixture to add an import that
makes the glob truly external-only vs a fixture where the local glob is
present — the behavior intent is: suppress when there is ambiguity, which is
correct for any glob. Do not delete passing assertions; update fixtures only.

**Tests**: the existing context_accounts glob tests must remain green. No new
tests required here beyond confirming the existing suite.

---

### Step 4: Add `has_glob_import` to the workspace indexer

**File**: `src/core/workspace/indexing.rs`

The `document_indexed_symbols` extraction (or equivalent) populates indexed
data from `ParsedDocument`. Confirm that `AnchorSymbols::has_glob_import` is
available on the indexed document. No change to the index itself is needed
(the flag lives on `ParsedDocument.symbols()`); this step is a verification
pass only.

Run:

```bash
cargo test --workspace -q 2>&1 | grep -E "FAILED|error"
```

Must produce no output.

---

### Step 5: Extend feature-detection to workspace-inherited dependencies

**File**: `src/lsp/diagnostics/check_cfg/mod.rs`  
**File**: `src/lsp/diagnostics/check_cfg/tests/mod.rs`  
**File**: `src/lsp/diagnostics/check_cfg/tests/workspace_feature_tests.rs` (NEW)

**Background**: `anchor_lang_dependency_has_feature` currently only checks a
`[dependencies]` line of the form `anchor-lang = { ..., features = [...] }` in
the current package's Cargo.toml text. It returns `false` (triggering a FP
diagnostic) when the package uses:

```toml
# package Cargo.toml
anchor-lang.workspace = true

# workspace Cargo.toml
[workspace.dependencies]
anchor-lang = { version = "0.30", features = ["init-if-needed"] }
```

**What to change:**

The `DiagnosticInput` already carries `manifest: Option<(&Url, &str)>` which
is the package-level `Cargo.toml`. The diagnostic pipeline in
`src/server/diagnostic_pipeline.rs` (lines 518, 530) provides this text.

Extend `DiagnosticInput` with a second optional field:

```rust
pub workspace_manifest: Option<(&'a Url, &'a str)>,
```

(DEFAULT `None` in all existing constructors, including `document_only`.)

In `src/lsp/diagnostics/rules.rs`, thread `workspace_manifest` from
`DiagnosticInput` to `check_cfg::collect`:

```rust
check_cfg::collect(
    input.document,
    manifest_uri,
    manifest_text,
    input.workspace_manifest.map(|(_, text)| text),
)
```

In `check_cfg/mod.rs`, update `collect` signature and
`manifest_needs_init_if_needed`:

```rust
pub fn manifest_needs_init_if_needed(
    document: &ParsedDocument,
    manifest_text: &str,
    workspace_manifest_text: Option<&str>,
) -> bool {
    if init_if_needed_sites(document).is_empty() {
        return false;
    }
    // Feature present directly in package manifest
    if anchor_lang_dependency_has_feature(manifest_text, "init-if-needed") {
        return false;
    }
    // Feature inherited from workspace: package declares `anchor-lang.workspace = true`
    // and workspace manifest has the feature enabled
    if anchor_lang_is_workspace_inherited(manifest_text)
        && workspace_manifest_text.is_some_and(|ws| {
            anchor_lang_workspace_dep_has_feature(ws, "init-if-needed")
        })
    {
        return false;
    }
    true
}
```

Add two new private functions:

```rust
/// True when the package Cargo.toml has `anchor-lang.workspace = true`
/// or `anchor-lang = { workspace = true, ... }`.
fn anchor_lang_is_workspace_inherited(manifest_text: &str) -> bool {
    // Parse with cargo_toml::Manifest::from_str.
    // IMPORTANT: `cargo_toml::Dependency::detail()` returns `None` for
    // workspace-inherited entries — those parse as `Dependency::Inherited`,
    // not `Dependency::Detailed`. Calling `.detail()` silently returns `None`
    // and would never suppress the FP. The correct check is:
    //   matches!(dep, cargo_toml::Dependency::Inherited(_))
    // The `Inherited` variant's `workspace` field is always `true` when it
    // appears, so no further field inspection is needed.
    let Ok(manifest) = cargo_toml::Manifest::from_str(manifest_text) else {
        return false;
    };
    manifest
        .dependencies
        .get("anchor-lang")
        .is_some_and(|dep| matches!(dep, cargo_toml::Dependency::Inherited(_)))
}

/// True when the workspace Cargo.toml [workspace.dependencies] entry for
/// anchor-lang includes the given feature.
fn anchor_lang_workspace_dep_has_feature(workspace_manifest_text: &str, feature: &str) -> bool {
    // Parse with cargo_toml::Manifest::from_str, inspect
    // manifest.workspace.dependencies["anchor-lang"].req_features()
    ...
}
```

Implementation note: `cargo_toml::Manifest::from_str` parses both package and
workspace manifests. For workspace manifest, the `workspace` field
(`Option<Workspace>`) holds `workspace.dependencies`. Use
`manifest.workspace.as_ref()?.dependencies.get("anchor-lang")` then
`.req_features()`. The `cargo_toml` crate version `0.22.3` exposes this API.

**Feed the workspace manifest in the server**:

In `src/server/diagnostic_pipeline.rs`, where `DiagnosticInput` is constructed,
locate the workspace `Cargo.toml` and pass it as `workspace_manifest`. If not
found or not readable, pass `None` (graceful degradation = silence for that
case, not a new FP).

**Important**: `SolanaProgram` and `WorkspaceIndex` do NOT expose a workspace
`Cargo.toml` path — there is no public API for this today. The private function
`cargo_build_root` in `src/solana/project/mod.rs` already performs the needed
upward traversal (scanning for a `Cargo.toml` with `[workspace]`), but it is not
exported. Before wiring Step 5's server plumbing, add a new public function to
`src/solana/project/mod.rs`:

```rust
/// Locates the workspace-root `Cargo.toml` for the package that owns `uri`.
/// Mirrors `nearest_manifest` but scans upward for the first manifest that
/// contains a `[workspace]` section.
/// Returns `(workspace_manifest_url, manifest_text)`, or `None` if no
/// workspace manifest is found or readable.
pub fn nearest_workspace_manifest(uri: &Url) -> Option<(Url, String)> {
    let mut path = uri.to_file_path().ok()?;
    if path.is_file() {
        path.pop();
    }
    loop {
        let manifest_path = path.join("Cargo.toml");
        if manifest_path.is_file() {
            if let Some(text) = file_text::read_limited_text(&manifest_path).ok().flatten() {
                if Manifest::from_str(&text)
                    .is_ok_and(|m| m.workspace.is_some())
                {
                    let url = Url::from_file_path(manifest_path).ok()?;
                    return Some((url, text));
                }
            }
        }
        if !path.pop() {
            return None;
        }
    }
}
```

(This consolidates the logic already inside the private `cargo_build_root` fn;
do not delete `cargo_build_root` — it is used internally — just add the new
public counterpart alongside `nearest_manifest`.)

Then in `diagnostic_pipeline.rs` call `nearest_workspace_manifest(document_uri)`
and assign the result to `workspace_manifest` in the `DiagnosticInput`
constructor. Read the manifest text on demand using the same pattern as the
package manifest read (file URI, text from `file_text::read_limited_text`).

**Doctrine**: facts sourced from `Cargo.toml` files (single-file syntax, not
whole-program Rust name resolution). All workspace reads are done via
`cargo_toml` crate parsing — the toolchain's own data format — not subprocess
invocations.

**Register the new test file**: add `mod workspace_feature_tests;` to
`src/lsp/diagnostics/check_cfg/tests/mod.rs` (currently that file only declares
existing test functions inline; it has no submodule declarations). Without this
line the Rust compiler never compiles `workspace_feature_tests.rs` — acceptance
criterion #3 would match nothing.

**Tests** (add to `check_cfg/tests/workspace_feature_tests.rs`, NEW):

```rust
// fixture: package has anchor-lang.workspace = true; workspace manifest
// has features = ["init-if-needed"] — must NOT fire diagnostic
fn workspace_inherited_feature_suppresses_diagnostic()

// fixture: package has anchor-lang.workspace = true; workspace manifest
// does NOT have "init-if-needed" — MUST fire diagnostic
fn workspace_inherited_feature_missing_fires_diagnostic()

// fixture: package has anchor-lang.workspace = true; workspace_manifest
// text is None (not provided) — MUST fire diagnostic (conservative: no
// assumption of features when workspace manifest unavailable)
fn workspace_inherited_feature_without_workspace_text_fires_diagnostic()

// fixture: package does NOT use workspace inheritance for anchor-lang,
// has direct features = ["init-if-needed"] — must NOT fire (regression)
fn direct_feature_still_suppresses_diagnostic()
```

---

### Step 6: Verify no resolution FPs from trait-impl methods in workspace

**File**: `src/core/document/associated_values.rs`  
**File**: `src/core/document/mod.rs`

**Audit**: `collect_from_impl` skips `ItemImpl` when `impl_self_type_name`
returns `None` (i.e., self-type is not a plain `Type::Path`). It does NOT filter
on `item_impl.trait_` — so trait-impl methods ARE currently indexed under their
self-type name. Verify this is correct by checking whether
`associated_values::collect_from_impl` is called for `impl SomeTrait for Foo`
blocks and whether that causes false negatives or false positives.

Expected finding: no change needed — `collect_from_impl` already indexes trait
impls. Add a test to confirm:

In `src/core/document/mod.rs` inline tests (or a sibling test module), add:

```rust
fn trait_impl_method_resolves_as_associated_value() {
    // document with `impl AnchorSerialize for MyType { fn serialize(...) }`
    // type_has_associated_value("MyType", "serialize") must return true
}
```

If the test fails, fix `collect_from_impl` to not filter on trait presence
(current behavior per code reading is already correct, but confirm).

---

### Step 7: Add fixture workspace for FP regression testing

**Directory**: `tests/fixtures/resolution_fp/` (NEW — create the directory)

Create a minimal fixture Anchor workspace that exercises the three FP classes
fixed in this plan:

```
tests/fixtures/resolution_fp/
  Cargo.toml              (workspace root with [workspace.dependencies])
  program/
    Cargo.toml            (anchor-lang.workspace = true)
    src/
      lib.rs              (uses init_if_needed; glob import; cross-file const)
  state.rs                (defines the const that the glob import brings in)
```

`Cargo.toml` workspace root:

```toml
[workspace]
members = ["program"]
resolver = "2"

[workspace.dependencies]
anchor-lang = { version = "0.30.1", features = ["init-if-needed"] }
```

`program/src/lib.rs` — must use `init_if_needed`, have a glob import from a
local module, and reference a cross-file const in a constraint expression.

This fixture is NOT compiled by `cargo build` in CI (it lacks a Solana SBF
toolchain in the test environment). It is used only as text for unit tests.

Add a test in `src/lsp/diagnostics/check_cfg/tests/workspace_feature_tests.rs`
that loads these fixture files as string literals (via `include_str!`) and
passes them to `manifest_needs_init_if_needed`. Add a test in
`src/lsp/diagnostics/constraint_expressions/` that loads `lib.rs` via
`ParsedDocument::parse_or_empty`, builds a minimal `WorkspaceIndex`, and runs
`collect_with_workspace` — expects zero diagnostics.

---

### Step 8: Clippy + full test run

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Both must pass with no new failures. Confirm test count equals or exceeds 1485.

---

## Acceptance Criteria

All commands below must exit 0 and produce the stated output.

```bash
# 1. Full test suite green (count >= 1485)
cargo test --workspace 2>&1 | tail -3
# Expected last line: "test result: ok. N passed; 0 failed; ..."

# 2. Clippy clean
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -c "^error" || true
# Expected: 0

# 3. New tests for workspace-inherited feature exist and pass
cargo test --workspace -q check_cfg::tests::workspace_feature 2>&1
# Expected: all pass, 0 failures

# 4. New glob-suppression tests exist and pass
cargo test --workspace -q constraint_expressions 2>&1 | grep -E "FAILED|passed"
# Expected: 0 FAILED

# 5. context_accounts glob tests still pass (regression)
cargo test --workspace -q context_accounts 2>&1 | grep -E "FAILED|passed"
# Expected: 0 FAILED

# 6. AnchorSymbols glob flag tests pass
cargo test --workspace -q has_glob_import 2>&1
# Expected: all pass

# 7. Fixture fixture loads without resolution FP
cargo test --workspace -q resolution_fp 2>&1
# Expected: all pass, 0 failed

# 8. Corpus smoke (if CORPUS_ENABLED in env)
CORPUS_ENABLED=1 cargo test --workspace -q corpus 2>&1 | grep -E "FAILED|ok"
# Expected: all previously passing corpus tests still pass
```

Observable outcomes:
- A project with `anchor-lang.workspace = true` and `init-if-needed` in
  `[workspace.dependencies]` produces zero `AnchorCheckCfg` diagnostics for
  `init_if_needed` usage.
- A file with `use state::*; ... payer = unknown_fn()` (glob import, no
  workspace index) produces zero `AnchorConstraintExpression` diagnostics.
- The above with a workspace index that confirms `unknown_fn` is absent:
  one `AnchorConstraintExpression` diagnostic fires (correct positive).

---

## Risks and Edge Cases

**R1: `cargo_toml 0.22.3` workspace dep API**.  
Verify that `Manifest::from_str` on a workspace `Cargo.toml` populates
`manifest.workspace.as_ref()?.dependencies`. If the type differs (e.g.
`WorkspaceInheritedDeps` vs `DepsSet`), adjust accordingly. Do not upgrade the
crate version without a separate plan — a semver bump requires workspace-wide
Cargo.lock review.

**R2: Workspace manifest not available in `DiagnosticInput`**.  
The server may not yet plumb a workspace Cargo.toml URI separately from the
package one. Investigate `src/server/diagnostic_pipeline.rs` lines 518 and 530.
If workspace root is not available, the fallback MUST be `None` (graceful
degradation), never a guess. Do not emit a new diagnostic if the workspace
manifest is unresolvable.

**R3: `context_accounts.rs` local-glob distinction removal**.  
The existing guard distinguishes `anchor_lang::prelude::*` (external) from
`crate::instructions::*` (local). Replacing it with `has_glob_import = true`
for ALL globs widens suppression. No existing test covers the case where a file
has ONLY `use anchor_lang::prelude::*;` and no local globs — there is no
baseline confirming whether this currently suppresses `AnchorContextAccounts` or
not. Before making the Step 3 change, add a test named
`prelude_only_glob_does_not_suppress_missing_struct` (or similar) that documents
the current behavior for this fixture. Then, after the change, either confirm the
test still passes (behavior unchanged) or update it to reflect the new (more
conservative, fewer FPs) outcome. Either way, document the decision in the code
comment at the call site. Do not rely on line numbers in `context_accounts.rs`
for this baseline — locate tests by function name.

**R4: `has_glob_import = false` default**.  
`AnchorSymbols` is constructed in several test helpers via struct literal.
Adding a new field requires updating all such construction sites, or the field
must have a `Default` impl. Prefer adding `#[derive(Default)]` to `AnchorSymbols`
if not already present, or set `has_glob_import: false` explicitly in every
construction site. The compiler will catch all missed sites.

**R5: Trait-impl method resolution already works**.  
If Step 6's test confirms `collect_from_impl` already indexes trait impls, no
code change is needed. The test is a canary; skip the fix if the test passes
without code changes.

**R6: Fixture compilation**.  
The `tests/fixtures/resolution_fp/` workspace must NOT be added to any
workspace `members` list or built by CI `cargo build`. It is text-only input
for unit tests. Ensure `.cargo/config.toml` or workspace `exclude` prevents
accidental compilation attempts.

**R7: Corpus caveats (from discovery)**.  
If `CORPUS_ENABLED=1` corpus tests are run: SPL manifest subpath at pinned SHA
may be wrong for `token/program`; use `token-swap/program` or repin. `fetch-corpus.sh`
requires Python >= 3.11. `.github` corpus workflow rust-toolchain action hash
may be stale. These are pre-existing issues; do not fix them in this plan unless
a corpus test failure is directly caused by changes made here.
