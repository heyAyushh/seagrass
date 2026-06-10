# Plan 05 — Capability Registry: Crate-Agnostic Recognition of Runtime Concepts

## Context

seagrass recognizes Solana runtime concepts (sysvars, account types, signer/owner
checks) by matching **import paths against one pinned crate** (`solana-program =2.2.1`,
catalog in `src/solana/runtime_catalog.rs`). This breaks as the ecosystem splits and
multiplies crates:

- Anza is decomposing `solana-program` into modular crates (`solana-clock`,
  `solana-rent`, `solana-pubkey`, `solana-sysvar`, … — see
  https://github.com/anza-xyz/solana-sdk). Programs increasingly import
  `solana_clock::Clock` instead of `solana_program::clock::Clock`. seagrass's
  path-matching misses these → unknown-type FPs.
- Pinocchio exposes the same concepts under `pinocchio::sysvars::*` and its own
  `AccountInfo`.
- Any future framework (e.g. Quasar) repeats the problem.

Per-crate pinning + regeneration is a treadmill. The ground-up fix: **separate facts
from naming.**

- **Facts** (sysvar IDs, struct layouts) are universal constants. They stay in the
  generated, parity-tested catalog (`src/solana/runtime_catalog.rs`). Unchanged by
  crate splits.
- **Naming** (which crates expose which concept under which path) moves to a small,
  declarative, *verified* capability registry.

### The resolution chain (every link is a doctrine truth source)

```
identifier in user source
  → import map: name came from crate root X          (A: single-file syntax)
  → manifest: X is a declared dep in version range    (C: toolchain/cargo_toml)
  → registry: X@range provides concept "sysvar.Clock" (verified mapping)
  → catalog: the FACT for sysvar.Clock (ID, layout)   (B: generated catalog)
```

A claim is made only when the whole chain holds. Any broken link → silence, never
a guess. A user-defined local `struct Clock` never matches: its import origin is not
a registered crate.

### Current state (verified file paths)

| File | Role |
|---|---|
| `src/solana/runtime_catalog.rs` | Sysvar facts (IDs, layouts) + parity tests vs `solana_program` |
| `src/core/document/imports.rs` | `collect_imported_names` — imported names + `use_aliases`; does NOT record which crate a name came from |
| `src/core/document/mod.rs` | `AnchorSymbols` (imported_names, use_aliases fields) |
| `src/solana/project/mod.rs` | `parse_cargo_manifest` via `cargo_toml` crate; `SolanaProjectKind` framework detection from deps |
| `src/lsp/diagnostics/security/mod.rs` | `SysvarAddressVisitor` (`SecuritySysvar`) — consumes sysvar knowledge |
| `Cargo.toml` (workspace) | `solana-program = "=2.2.1"` pinned; `cargo_toml = "0.22.3"` |
| `docs/plans/plan-05-capability-registry.md` | THIS FILE |
| `src/solana/capability_registry.rs` | NEW — the registry + resolution |

### Architecture doctrine (must hold in every step)

1. Three truth sources: (A) single-file syntax, (B) pinned generated catalog,
   (C) toolchain (`cargo_toml` manifest parsing; no subprocess at diagnostic time).
2. Silence over guess: chain broken → no claim.
3. The registry maps names→crates only. It must NEVER contain facts (IDs, layouts) —
   those live in the catalog. A registry entry without a catalog-backed concept is a
   compile-error-level bug (enforced by test, Step 2).
4. Registry entries for crates available as (dev-)deps are parity-verified against
   the real crate. Entries for crates we don't depend on are activated only by the
   full resolution chain — they make no standalone claims.

## Non-Goals

- No `cargo metadata` subprocess at diagnostic time (latency). Manifest parsing via
  the `cargo_toml` crate is the (C) source; full resolved-graph fidelity is not needed
  because the chain also requires the user's own `use` statement.
- No replacement of the anchor-syn constraint catalog. Anchor constraints are parsed
  from anchor-syn's own parser source; that pipeline is untouched.
- No full semver resolution. Version ranges are checked against the declared
  requirement in the manifest (best-effort string/semver-req match), not the lockfile.
- No new diagnostics. This plan re-bases *recognition*; behavior change is strictly
  "fewer unknown-type FPs".
- Do not remove the `solana-program` pin or existing parity tests.

## Steps

### Step 0 — Baseline

```bash
cargo test --workspace 2>&1 | tail -3   # 0 failed required
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
```

Green + committed tree required. If plan-01/02/03 work is in flight on this clone,
coordinate: this plan touches `src/core/document/imports.rs` (also touched by plan-04
Step 1). Execute after plan-04 lands, or rebase carefully.

### Step 1 — Record import origins (extend `imports.rs`)

`collect_imported_names` currently records *that* a name is imported and its alias,
but not *which crate root* it came from. Add an origins map.

- In `src/core/document/imports.rs`: while walking `UseTree::Path`, capture the first
  path segment (the crate root, e.g. `solana_clock`, `pinocchio`, `anchor_lang`,
  `crate`, `super`, `self`). For every leaf name/alias collected, record
  `name → crate_root` into a new `import_origins: HashMap<String, String>`.
- Add the field to `AnchorSymbols` in `src/core/document/mod.rs`; populate in
  `from_items`. `crate`/`super`/`self` origins are recorded as-is (they will simply
  never match a registry crate — correct behavior).
- Tests (inline, `src/core/document/mod.rs` test module):
  - `import_origin_recorded_for_plain_use` — `use solana_clock::Clock;` →
    origin("Clock") == "solana_clock"
  - `import_origin_recorded_for_alias` — `use solana_program::clock::Clock as Klock;`
    → origin("Klock") == "solana_program"
  - `import_origin_local_crate_not_external` — `use crate::state::Clock;` →
    origin("Clock") == "crate"

### Step 2 — The registry (`src/solana/capability_registry.rs`, NEW)

A `const` table, not a parsed file (no new deps, reviewed like code, exhaustively
testable):

```rust
/// A capability concept, keyed into the runtime catalog.
/// Variants must correspond 1:1 to catalog-backed facts.
pub enum Concept {
    SysvarClock,
    SysvarRent,
    SysvarInstructions,
    SysvarEpochSchedule,
    SysvarSlotHashes,
    // ... one per catalog sysvar
    AccountInfo,       // raw account handle (native / pinocchio)
}

pub struct CrateCapability {
    /// Crate name as it appears in Cargo.toml (kebab-case).
    pub crate_name: &'static str,
    /// Minimal declared version requirement that provides these concepts.
    pub min_version: &'static str,
    /// Concepts this crate exposes, with the type identifier it uses.
    pub provides: &'static [(Concept, &'static str)], // (concept, type ident)
}

pub const CAPABILITY_REGISTRY: &[CrateCapability] = &[
    CrateCapability { crate_name: "solana-program", min_version: "1.0", provides: &[
        (Concept::SysvarClock, "Clock"), (Concept::SysvarRent, "Rent"), /* … */
    ]},
    CrateCapability { crate_name: "solana-clock",  min_version: "0.1",
        provides: &[(Concept::SysvarClock, "Clock")] },
    CrateCapability { crate_name: "solana-rent",   min_version: "0.1",
        provides: &[(Concept::SysvarRent, "Rent")] },
    CrateCapability { crate_name: "solana-sysvar", min_version: "0.1", provides: &[/*…*/] },
    CrateCapability { crate_name: "pinocchio",     min_version: "0.1", provides: &[
        (Concept::SysvarClock, "Clock"), (Concept::SysvarRent, "Rent"),
        (Concept::AccountInfo, "AccountInfo"),
    ]},
    CrateCapability { crate_name: "anchor-lang",   min_version: "0.29", provides: &[
        (Concept::SysvarClock, "Clock"), (Concept::SysvarRent, "Rent"), // via prelude re-export
    ]},
];
```

Resolution entry point:

```rust
/// Resolve an identifier to a runtime concept, requiring the full chain:
/// import origin (A) → declared dep in range (C) → registry → concept.
/// Returns None whenever any link is missing — silence, never a guess.
pub fn resolve_concept(
    ident: &str,
    import_origins: &HashMap<String, String>,
    declared_deps: &CargoManifestDeps,   // from src/solana/project parse
) -> Option<Concept>
```

Note crate-name vs path-root normalization: `solana-clock` in Cargo.toml is
`solana_clock` in source. Normalize with `str::replace('-', "_")` at comparison time.

Enforcement tests in the same file:
- `registry_concepts_all_backed_by_catalog` — every `Concept` variant maps to a
  catalog fact (exhaustive match against `runtime_catalog` entries; doctrine rule 3).
- `registry_crate_names_unique_and_sorted`.
- `local_struct_never_resolves` — ident "Clock" with origin "crate" → None.
- `dep_missing_never_resolves` — origin "solana_clock" but no such declared dep → None.

### Step 3 — Parity-verify registry entries we can afford as dev-deps

Add as **dev-dependencies only** (they must not ship in the published binary deps):
`solana-clock`, `solana-rent` at the versions currently published.

In `runtime_catalog.rs` tests, extend the existing parity-test pattern:
- `modular_crate_clock_matches_catalog` — destructure `solana_clock::Clock` fields,
  compare against catalog layout (compile error on upstream rename = the point).
- Same for `solana_rent::Rent` and rent constants.

For registry crates NOT taken as dev-deps (pinocchio, future quasar): document in a
comment that their entries are chain-activated only (doctrine rule 4) and reviewed
manually at addition time.

### Step 4 — Rewire sysvar recognition through the chain

Consumers of sysvar knowledge switch from hard-coded `solana_program` path checks to
`resolve_concept`:

1. `SysvarAddressVisitor` / `SecuritySysvar` in `src/lsp/diagnostics/security/mod.rs`.
2. The sysvar-type validation feeding `invalid_sysvar_diagnostic` (locate via
   `rg -n "sysvar" src/anchor/types/ src/lsp/diagnostics/ -g '*.rs'`).
3. Sysvar completions (`sysvar_generic_candidates` — locate via
   `rg -n "sysvar_generic_candidates" src/`).

Each call site needs `import_origins` (from `AnchorSymbols`, Step 1) and the parsed
manifest deps (already plumbed for `AnchorCheckCfg`; after plan-04, workspace-inherited
deps too). Where a call site has no manifest available, pass an empty dep set — the
chain then resolves nothing new and behavior is unchanged (silence, not regression).

Behavior delta to test: a document importing `solana_clock::Clock` with
`solana-clock` declared in its manifest is recognized exactly like one importing
`solana_program::clock::Clock` today.

### Step 5 — Fixture + regression tests

`tests/fixtures/capability_resolution/` (NEW, text-only, not a cargo workspace member):

- `modular/` — program using `use solana_clock::Clock;` + manifest declaring
  `solana-clock`. Expected: zero unknown-sysvar diagnostics; completions/hover treat
  `Clock` as the sysvar.
- `no_dep/` — same source, manifest WITHOUT `solana-clock`. Expected: no sysvar
  claim made (and no new diagnostic invented about the missing dep).
- `pinocchio/` — `use pinocchio::sysvars::clock::Clock;` + pinocchio dep. Expected:
  recognized via the pinocchio registry entry.

Wire as unit tests with `include_str!`, mirroring the plan-04 fixture approach.

### Step 6 — Document the model

Append to `docs/diagnostics-strategy.md`, section `## Capability Registry (Plan 05)`:
facts-vs-naming split, the resolution chain diagram, the rule for adding a new
crate/framework (one registry entry + optional dev-dep parity test), and the explicit
statement that registry entries are never facts.

## Acceptance Criteria

1. `cargo test --workspace` — 0 failed; count strictly greater than baseline.
2. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
3. `cargo test capability_registry` — all enforcement tests pass, including
   `registry_concepts_all_backed_by_catalog`, `local_struct_never_resolves`,
   `dep_missing_never_resolves`.
4. `cargo test modular_crate` — dev-dep parity tests pass.
5. Fixture tests: `modular` recognized, `no_dep` silent, `pinocchio` recognized.
6. `grep -n "solana-clock\|solana-rent" Cargo.toml` shows them under
   `[dev-dependencies]` only.
7. `grep -n "Plan 05" docs/diagnostics-strategy.md` — section present.

## Risks / Edge Cases

- **Anchor prelude glob**: `use anchor_lang::prelude::*;` imports `Clock` without a
  named `use`, so `import_origins` has no entry. The existing Anchor-path recognition
  must remain as the fallback for Anchor projects (do not delete it in Step 4) —
  the chain is additive, not a replacement, for the anchor-lang case.
- **Re-exports**: `solana_program` itself re-exports the modular crates internally;
  a user importing through `solana_program::clock` keeps working via the existing
  entry. Both entries coexist; resolution is per-import-origin.
- **Workspace-inherited deps**: a member manifest with `solana-clock.workspace = true`
  needs plan-04's `nearest_workspace_manifest` plumbing. Until plan-04 lands, such
  projects resolve nothing new (silent, not wrong). Sequencing: execute after plan-04.
- **Version-range false negatives**: declared req `"2"` vs registry min `"0.1"` —
  use `semver::VersionReq` intersection only if `semver` is already a dep
  (`rg '^semver' Cargo.lock`); otherwise compare leniently (presence beats precision;
  a wrong-version match still required the user to import the name from that crate).
- **Plan-04 interaction**: extractors (plan-06 Stages 2/4) should consume
  `resolve_concept` for runtime types instead of growing their own path lists. Add a
  pointer in plan-06 at execution time if plan-05 lands first.
