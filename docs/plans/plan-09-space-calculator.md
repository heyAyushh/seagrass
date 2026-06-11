# Plan 09 — `space =` Calculator and Quick Fixes: Visible Daily-Use Wins

## Context

Every Anchor developer hand-computes account sizes (`space = 8 + 32 + 8 + …`) and
gets it wrong often enough that it's a meme. seagrass already parses everything
needed to compute this: account struct fields with types and generics
(`SymbolRange`, `src/core/document/mod.rs:294–308`), the three space idioms
(`src/lsp/completions/constraint_values/space_values.rs` — discriminator constant
`ANCHOR_DISCRIMINATOR_BYTES = 8` at :13), and a full code-lens/hover/code-action
pipeline. This plan turns that parsing into three user-visible features plus quick
fixes for diagnostics that currently nag without helping.

Deliverables:

- **A.** A byte-size engine for `#[account]` data structs (Borsh layout, mirroring
  Anchor's `InitSpace` semantics).
- **B.** Hover on `#[account]` structs and on `space =` constraint values showing
  the computed size with a per-field breakdown.
- **C.** A code lens over each `#[account]` struct: `space: 8 + 47 = 55 bytes`.
- **D.** Quick fix for `unchecked-arithmetic` diagnostics (rewrite to `checked_*`).
- **E.** Quick fix "derive InitSpace" on account types used in `space` expressions
  without the derive.

### Current state (verified file paths)

| File | Role |
|---|---|
| `src/core/document/mod.rs` | `SymbolRange` :294–308 (`name`, `type_name`, `generic_type_names`, `is_optional`, `fields`, `account_constraints`); `field_symbols()` :522–561 extracts per-field type info |
| `src/lsp/hover/mod.rs` | `hover()` :15–20 tries `hover_from_cursor_context` → `anchor_account_type_hover` :118 → `anchor_symbol_hover` :180; output is `MarkupKind::Markdown` |
| `src/lsp/code_lens/mod.rs` | Accounts-context lenses :13–36 and instruction lenses :38–61; `resolve_provider: Some(false)` so each lens ships its `command` inline (`seagrass/analyze`) |
| `src/server/helpers.rs` | Capability registration: hover :397, code action + resolve :403–411, code lens :412–414, inlay hints :415 |
| `src/lsp/actions/mod.rs` | `code_actions_unfiltered()` :32–76 dispatches per-submodule providers; actions attach `diagnostics: Some(vec![diagnostic.clone()])` for cursor filtering (`rank_and_filter_for_cursor()` :130–155); `resolve()` :179–186 |
| `src/lsp/actions/common.rs` | `diagnostic_code()` :67–74; `data.quickfix` routing key pattern :79–85 |
| `src/lsp/actions/accounts/mod.rs` | `add_mut_constraint_actions()` :59–63 — the model to copy: diagnostic carries `data.quickfix = "add-mut-constraint"`, action filters on it |
| `src/lsp/actions/init_constraints.rs` | Existing missing-payer/space actions; :93 emits `8 + {account_type}::INIT_SPACE` |
| `src/lsp/completions/constraint_values/space_values.rs` | The three idioms: classic `8 + T::INIT_SPACE`; custom-discriminator-safe `T::DISCRIMINATOR.len() + T::INIT_SPACE` (Anchor ≥0.31); zero-copy `8 + std::mem::size_of::<T>()` |
| `src/lsp/diagnostics/code_quality/arithmetic.rs` | `unchecked-arithmetic` diagnostic; `data` payload :252–260 includes `rule`, `programKind`, `evidenceSource`, `suggestion` — but **no quick fix exists** |
| `src/core/workspace/mod.rs` | `WorkspaceIndex::accounts_struct(name)` for cross-file struct lookup |

### Architecture doctrine (must hold in every step)

1. Size facts are Borsh serialization facts — a small constant table, unit-tested,
   in the spirit of the runtime catalog. They must match Anchor's `InitSpace`
   derive semantics exactly (anchor-syn pinned @ 4addac5 is the reference; the
   derive lives in Anchor's `derive(InitSpace)` — verify each rule against its
   source, cite the file in a comment).
2. Silence over guess: a struct containing any type the engine can't size yields a
   *partial* result rendered honestly ("+ size of `Pubkey`-keyed map (unknown)"),
   never a fabricated number. Unknown beats wrong everywhere in this plan.
3. No new diagnostics. Hover/lens/actions only. Specifically: do NOT emit a
   "space mismatch" diagnostic when a literal disagrees with the computed size —
   that is future work gated on confidence in the engine (revisit after this plan
   has soaked).
4. Quick-fix edits must produce code that compiles when applied. An edit that
   needs the user to finish it must say so in its title ("… (fill in error)") —
   avoid these where possible.

## Non-Goals

- No `space` mismatch/lint diagnostics (doctrine 3).
- No sizing of `zero_copy` / `repr(C)` structs (`std::mem::size_of` layout differs
  from Borsh; the engine returns Unknown with reason "zero-copy layout").
- No evaluation of arbitrary const expressions in `space =` values (only literal
  ints and the three catalog idioms are recognized).
- No cross-crate type resolution beyond what `WorkspaceIndex::accounts_struct`
  already provides; unresolvable field types are Unknown.

## Steps

### Step 0 — Baseline

```bash
cargo test 2>&1 | tail -3
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
```

### Step 1 — Size engine (`src/anchor/space.rs`, NEW)

```rust
/// Byte size of an account data struct under Borsh, mirroring Anchor's
/// `derive(InitSpace)` rules. Discriminator NOT included (callers add it,
/// because the idiom differs: 8 vs DISCRIMINATOR.len()).
pub enum SpaceEstimate {
    /// Every field sized exactly.
    Exact(u64),
    /// Fixed part + named symbolic parts (e.g. `#[max_len]`-dependent fields).
    Formula { fixed: u64, symbolic: Vec<SymbolicPart> },
    /// Cannot size at all (with a human-readable reason).
    Unknown(&'static str),
}

pub struct SymbolicPart {
    pub field: String,
    pub expr: String,   // e.g. "4 + max_len" — what the user must determine
}

pub fn account_space(strukt: &SymbolRange, workspace: Option<&WorkspaceIndex>) -> SpaceEstimate;
pub fn field_space(type_name: &str, generics: &[String], …) -> SpaceEstimate;
```

The fact table (verify each against Anchor's InitSpace derive before writing):
`bool`/`u8`/`i8` = 1; `u16`/`i16` = 2; `u32`/`i32`/`f32` = 4; `u64`/`i64`/`f64` = 8;
`u128`/`i128` = 16; `Pubkey` = 32; `[T; N]` = N × size(T) (parse N from the type
text; non-literal N → symbolic); `Option<T>` = 1 + size(T); enum = 1 + max(variant
payloads) when all variants resolvable; nested struct = recurse via local symbols
then `WorkspaceIndex::accounts_struct`; `String` = 4 + `#[max_len]` if the
attribute is present on the field, else symbolic `"4 + max_len"`; `Vec<T>` = 4 +
max_len × size(T) likewise. Recursion guard: a `visited: HashSet<String>` of
struct names — self-referential types → `Unknown("recursive type")`.

Tests (same file): one per table row; nested struct; `Option<Pubkey>`; array with
literal len; `String` with and without `#[max_len]`; enum; recursion guard;
unresolvable type → `Unknown`.

### Step 2 — Hover (extend `src/lsp/hover/mod.rs`)

1. **On an `#[account]` struct name** (extend `anchor_symbol_hover`, :180): append
   a section to the existing markdown —

   ```
   ### Space
   | field | type | bytes |
   |---|---|---|
   | authority | Pubkey | 32 |
   | amount    | u64    | 8  |

   **8 (discriminator) + 40 = 48 bytes** — `space = 8 + Escrow::INIT_SPACE`
   ```

   For `Formula` results, render the symbolic rows as e.g. `4 + max_len` and the
   total as a formula. For `Unknown`, render nothing (silence over noise).
2. **On a `space` constraint value** (extend `hover_from_cursor_context`, :22):
   when the cursor is on the value of a `space =` constraint and the init account's
   data type resolves, show the computed requirement. When the value is an integer
   literal and the estimate is `Exact`, show both numbers side by side — informational
   only, no judgment language (doctrine 3).

### Step 3 — Code lens (extend `src/lsp/code_lens/mod.rs`)

One lens per `#[account]` data struct (NOT per `#[derive(Accounts)]` context —
those already have lenses). Title: `space: 8 + 40 = 48 bytes` (Exact),
`space: 8 + 24 + n·item (set max_len)` (Formula); no lens for Unknown. Since
`resolve_provider` is `false`, ship a `command` inline: reuse the `seagrass/analyze`
command pattern (:23–32) with payload `{ "uri": …, "accountType": name }` so a
click can later open the breakdown; data keys `kind = "anchor.accountSpace"`,
`bytes` when exact. Tests mirror the existing lens tests in the module.

### Step 4 — Quick fix: unchecked arithmetic (`src/lsp/actions/` NEW submodule `arithmetic.rs`)

The diagnostic (`arithmetic.rs` :252) already carries `data.rule =
"unchecked-arithmetic"` and `data.suggestion`. Add the routing key
`"quickfix": "checked-arithmetic"` to its `data` payload, then a new action
provider registered in `code_actions_unfiltered()` (:32–76):

- `a + b` → `a.checked_add(b).ok_or(ProgramError::ArithmeticOverflow)?`
  (`checked_sub`/`checked_mul`/`checked_div` for `-`/`*`/`/`).
- `a += b` → `a = a.checked_add(b).ok_or(ProgramError::ArithmeticOverflow)?` —
  re-parse the diagnostic's enclosing expression from the document (the diagnostic
  range covers the operator; walk up via the syn AST to the full binary/assign-op
  expression) rather than string surgery.
- `ProgramError::ArithmeticOverflow` converts into `anchor_lang::error::Error` via
  `From<ProgramError>`, so the same edit compiles for Anchor and native handlers
  returning their respective `Result` types (doctrine 4). If `ProgramError` is not
  imported (check `AnchorSymbols` imported names), include the fully-qualified path
  in the edit instead of adding a `use` (smaller, always-correct edit).
- Skip (no action) when either operand contains a method call or `?` — operand
  duplication/order issues aren't worth a wrong edit.
- Attach `diagnostics: Some(vec![diag])` for cursor ranking; kind QUICKFIX;
  `is_preferred: Some(true)`.

Tests: one per operator; compound assignment; fully-qualified path fallback;
skip-on-method-call; action only offered on its own diagnostic.

### Step 5 — Quick fix: derive InitSpace

When a `space` constraint value references `T::INIT_SPACE` and the struct `T` is
local (resolvable via local symbols or `WorkspaceIndex::accounts_struct`) but lacks
`#[derive(InitSpace)]`: offer "Add `#[derive(InitSpace)]` to `T`" — an edit on the
derive attribute of `T`'s definition (possibly cross-file: `WorkspaceEdit` keyed by
the defining document's URI). When `T` has `String`/`Vec` fields, the same action
also inserts `#[max_len(/* TODO */)]` stubs — this is the one permitted
"finish-me" edit; title must read "… and add max_len stubs". Place in
`src/lsp/actions/init_constraints.rs` next to the existing space machinery (:93).

Tests: derive added same-file; cross-file edit; max_len stubs for String/Vec;
no action when derive already present; no action when `T` is external.

### Step 6 — Docs

- `docs/agents.md` / feature docs if they enumerate LSP features: add space lens +
  hover and the two quick fixes.
- A short `## Space engine (Plan 09)` section in `docs/diagnostics-strategy.md`
  noting the fact-table provenance (Borsh/InitSpace, anchor-syn pin) and that
  mismatch *diagnostics* remain future work.

## Acceptance Criteria

1. `cargo test` — 0 failed; count strictly greater than baseline.
2. `cargo clippy --all-targets -- -D warnings` — clean.
3. `cargo test space` — fact-table, recursion-guard, and Formula/Unknown tests pass.
4. Hover over an `#[account]` struct in a test fixture yields the breakdown table;
   hover over a `space =` literal shows computed vs declared with no judgment word
   (grep test asserts absence of "wrong"/"should"/"error" in that hover string).
5. Code lens appears on `#[account]` structs only (none on `#[derive(Accounts)]`
   contexts, none on Unknown estimates).
6. Applying the checked-arithmetic quick fix on each operator fixture yields code
   that parses (`syn::parse_str` in test) — and the committed corpus still passes
   (`cargo test corpus`).
7. The InitSpace action edits the *defining* file in the cross-file fixture.

## Risks / Edge Cases

- **InitSpace semantic drift.** Anchor could change derive rules between versions.
  The fact table cites the pinned anchor-syn source per rule; the pin is the truth
  source, same as the constraint catalog. When the pin moves, the table is re-verified
  (add a checklist comment at the top of `space.rs`).
- **`String`/`Vec` without `#[max_len]`** dominate real structs. The Formula
  rendering is the product answer — resist the urge to guess a default length.
- **Zero-copy structs** (`#[account(zero_copy)]`): `size_of` ≠ Borsh size; engine
  must detect the attribute and return Unknown("zero-copy layout") rather than a
  Borsh number that is wrong for the idiom users actually need.
- **Operator-fix scope creep.** `%` has `checked_rem`; include it. Shifts and
  bitwise ops are not flagged by the diagnostic — do not handle them.
- **Cursor-filter interaction.** `rank_and_filter_for_cursor` (:142–154) drops
  actions without attached diagnostics when the cursor touches a diagnostic —
  the InitSpace action (Step 5) attaches none, so verify it still surfaces when
  the cursor is on a diagnostic-free `space =` line (it does; filter only
  activates when cursor touches ≥1 diagnostic — test it anyway).
- **Lens noise.** Programs with many small `#[account]` structs get many lenses.
  Acceptable for v1; if feedback says noisy, gate behind a setting in a follow-up
  (do not pre-build the setting).
