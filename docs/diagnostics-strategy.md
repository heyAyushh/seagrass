# Diagnostics Strategy: from AST linter to Solana semantic analyzer

> Status: **plan** — except two false-positive fixes already landed (noted below).
> Trigger: a clean checkout of blueshift's `anchor_escrow` (a canonical, compiling Anchor
> program) produced 7 false positives, 3 of them ERROR-severity.

## The one idea that ties this whole document together

**Defense (never be wrong) and offense (be genuinely valuable) are the same problem.** Both are
capped by one thing: *how much semantic context the analyzer has above the syntax tree.*

- Single-file AST can't see across modules → false "this doesn't exist" errors. **(defense gap)**
- Single-file AST can't follow data across instructions → can't catch the deep bugs that would make
  seagrass matter. **(offense gap)**

There is one ladder underneath both, the **analysis foundation**:

| Rung | Foundation | Where seagrass is | Defense it unlocks | Offense it unlocks |
|---|---|---|---|---|
| **a** | Single-file AST | ✅ today | (the floor) | Tier A: signer/owner/discriminator/CPI/arith pattern checks |
| **b** | Workspace symbol + type resolution | partial (`WorkspaceIndex`) | kills cross-module "absent" FPs (Family 2) | Tier B: PDA/discriminator collision, IDL drift, upgrade authority |
| **c** | Inter-procedural call graph | ✗ | sound cross-fn account/usage reasoning | **Moat**: stale-after-CPI, oracle staleness, authority reachability, signer propagation |
| **d** | Dataflow / taint (Anchor-sanitizer-aware) | ✗ | suppresses "missing check" FPs via sanitizer recognition | **Moat**: cross-instruction state machine, economic-invariant/rounding bugs |

Every rung you climb *simultaneously* removes a class of false positive **and** unlocks a tier of
diagnostics. That is the strategy in one sentence: **invest in the foundation, not in individual
lints.** The single highest-leverage rung is **(c) the call graph** — it unlocks the largest set of
moat diagnostics and the soundest cross-file reasoning at once.

---

# Part 1 — Defense: stop being wrong

The 3 confirmed false positives (plus ~45 audited) collapse into three families. The architecture to
prevent them mostly *already exists* in seagrass (`Confidence`, `Region`/`SCOPE`, `FrameworkSet`,
`arbitration`, `DiagnosticPhase`, an `experimental` flag) — the defect is **missing enforcement**.

### The three failure families

1. **Incomplete AST traversal.** A visitor declares binders only for free fns (`visit_item_fn`), so
   when syn recurses into an `impl` method (`ImplItemFn`), closure, or nested mod, params are never
   scoped → names appear "unresolved." Replicated in `handler_members`, `security/raw_account`
   (owner-check, type-cosplay), `spl_semantics`, `code_quality/stale_cpi` (hardcodes `"ctx"`), …
2. **Closed-world absence under partial context.** A lint asserts "X is absent" at ERROR severity
   from one file, no workspace index, behind `use …::*` globs it can't follow (globs dropped at
   `core/document/imports.rs:34`). Replicated across `constraint_expressions`, `account_references`
   (ERROR), `account_usage` (ERROR), `constraint_shape/has_one` (ERROR), `anchor_syn`, …
3. **Analyzer config/heuristics leak as verdicts about user code.** `init_if_needed` fires because
   *seagrass's* `anchor-syn` dep lacks the `init-if-needed` feature; `check_cfg` line-scans the
   manifest instead of parsing it; artifact "stale" warnings are mtime-based; security heuristics
   over-fire on idiomatic Anchor.

### Five enforced invariants + one durable guard

| # | Invariant | Enforced once in | Closes |
|---|---|---|---|
| **A** | One complete scope-modeling traversal (free fns + `ImplItemFn` + closures + nested mods) shared by all body-walking lints | `lint.rs` / `lsp/scope.rs` | Family 1 |
| **B** | **Open-world absence** — no "X is absent" without whole-program evidence (`workspace_index` populated, or symbol is necessarily file-local and no glob could supply it). Glob roots tracked once in `imports.rs`. **Decision: when absence can't be proven, emit nothing** | shared `can_prove_absence` gate | Family 2 |
| **C** | Confidence drives default severity (`Authoritative→Error`, `Derived→Warning`, `Heuristic→Hint`); flip `experimental_diagnostics` default `true→false` | `arbitration.rs`, `server_types` | Family 3 heuristics + over-promotion |
| **D** | Analyzer config matches the real-world feature superset: build `anchor-syn` with `init-if-needed`; parse `Cargo.toml` via `cargo_toml::Manifest` incl. `workspace=true` inheritance; never surface anchor-syn feature-gated parse errors as user verdicts | `Cargo.toml`, `check_cfg` | manifest/feature FPs |
| **E** | **Filesystem/mtime staleness is opt-in (off by default).** A source diagnostic must not depend on whether the user rebuilt. Prefer content-hash over mtime if enabled | `artifacts.rs`, settings | the artifact noise from the incident |

**Durable guard — golden corpus + compile-truth.** Vendor real, *compiling* Anchor programs into
`fixtures/corpus/` (blueshift escrow, official `anchor/examples/*`, a few mainnet programs). One CI
test runs the full engine and asserts **zero ERROR-severity diagnostics** — *if `anchor build`
succeeds, nothing is genuinely absent.* This single test would have caught all three ERROR-level
false positives, and gives an objective definition of "false positive" that doesn't depend on us
imagining edge cases.

### Decisions taken
- **Absence policy:** suppress entirely without whole-program evidence (zero FPs; genuine typos still
  caught once the workspace index is populated — the LSP default).
- **Artifact diagnostics:** off by default, opt-in.

### Already landed (tested, clippy-clean)
1. `handler_scope` now scopes `impl`-method params (`visit_impl_item_fn`).
2. `context_accounts` suppresses the missing-struct ERROR under a *local* glob (`use crate|super|self::*`
   / `use <local_mod>::*`) when the workspace index has no cross-file visibility;
   `use anchor_lang::prelude::*;` is excluded.

---

# Part 2 — Offense: be the tool Solana engineers can't work without

seagrass's moat is **Solana/Anchor domain semantics** — the protocol-correctness and security checks
that a generic Rust tool (rust-analyzer, clippy) *structurally cannot* do, delivered **inline in the
editor** (which batch auditors/CLIs don't). Research basis: Sec3 X-Ray/Soteria, VRust (CCS '22),
SseRex ('25 symbolic), Radar/Auditware, L3X, Anchor lints, Ackee's VS Code detectors, and the
`coral-xyz/sealevel-attacks` taxonomy.

### Tier A — commoditized (ship for credibility, expect no differentiation)
Single-file AST. Every Solana tool has these; their *absence* is what looks amateur.

| Check | Class |
|---|---|
| Missing `is_signer` on a privileged `AccountInfo` | signer bypass |
| Missing owner check on raw `AccountInfo` | account confusion |
| Missing 8-byte discriminator on raw deserialize | type cosplay |
| Arbitrary CPI with constant unverified `program_id` | arbitrary CPI |
| Unchecked arithmetic on lamport/token math | integer overflow |
| `create_program_address` with user-supplied bump | bump canonicalization |
| `remaining_accounts` iterated without owner/type check | account injection |
| `UncheckedAccount` without `/// CHECK:` | doc enforcement |
| Account close without zeroing + discriminator poison | closing/revival attack |
| Duplicate mutable accounts without key-inequality check | state corruption |

> seagrass already implements much of this band (`security/*`, `code_quality/*`) — Part 1's invariants
> are mostly about making these *not over-fire*.

### Tier B — differentiating (workspace context makes these qualitatively better, inline)
Foundation **(b) workspace symbol/type resolution** — the same rung that closes Family 2 FPs.

| Check | Class | Foundation |
|---|---|---|
| Division-before-multiplication in token math | precision loss | AST |
| `init` instruction without an authority constraint | privilege escalation | AST |
| `invoke_signed` bump seeds not matching the stored/derived bump | PDA validation gap | b |
| **PDA seed collision across instruction types** (hash all seed byte-seqs workspace-wide) | account confusion | b |
| **Account discriminator collision** (`sha256("account:Type")[..8]` across all structs) | type confusion | b |
| **IDL ↔ source struct layout drift** (parse `target/idl/*.json`, diff fields/types/order/size) | silent client corruption | b |
| Upgrade authority set to a non-multisig key | centralization risk | b |

### Tier C — moat (seagrass-only territory)
Foundations **(c) call graph** and **(d) dataflow/taint**. No generic Rust tool can reach these; no
existing Solana tool delivers them in an editor.

| Check | Class | Foundation |
|---|---|---|
| **Stale account read after CPI** (mutable account passed to `invoke*`, then read without `.reload()`) | data integrity | c |
| **Oracle staleness** (Pyth/Switchboard read without a `max_staleness` slot check) | price manipulation | c |
| **Authority reachability** (authority field set-site → use-site across instructions) | access control (#1 loss class, 53% of 2025 losses) | c |
| **Signer privilege propagation through CPI** (PDA signer delegated to a user-controlled program) | confused deputy | c |
| Dynamic/user-controlled CPI target | arbitrary CPI | c |
| **Cross-instruction state-machine exploit surface** (valid predecessor-state gating) | protocol logic | d |
| CPI lamport delta not asserted pre/post | fund manipulation | d |
| Economic-invariant / rounding taint (AMM/lending solvency) | protocol solvency | d |
| Compute-unit exhaustion paths | DoS griefing | c |

### The 7 highest-leverage moat diagnostics
Ranked for "high value × uniquely seagrass":
1. **Authority reachability graph** — model Anchor `has_one`/`constraint`/`address` as a typed
   authority graph. Addresses the largest real-world loss category. Generic tools see opaque macros.
2. **Cross-instruction state machine** — model which account fields encode state and which transitions
   are legal. Catches "deposit-before-initialize" logic bugs. No generic/EVM tool has the concept.
3. **Stale-after-CPI reload** — CPI-boundary dataflow unique to Solana; clippy has no concept of CPI.
4. **Workspace PDA seed-collision** — produce concrete colliding-seed examples (subtle enough to evade
   human review).
5. **IDL-vs-struct drift** — silent integration corruption every workspace accumulates; nobody checks it.
6. **Oracle staleness** — pure DeFi domain knowledge; trivial with an oracle type-signature catalog.
7. **Signer propagation through CPIs** — confused-deputy tracing; needs call graph + signer-set flow.

### Keeping offense from re-introducing false positives
Deep analysis must not regress Part 1. Two rules:
- **Positive evidence, not absence.** Look for the *presence* of `is_signer`/owner/discriminator
  assertions (blocklist "flag all missing" is what gave VRust an ~89% false-alarm rate).
- **Sanitizer-aware taint.** Recognize that `Account<'info,T>` sanitizes owner+discriminator,
  `Signer<'info>` sanitizes `is_signer`, `has_one` sanitizes key equality, typed program accounts
  sanitize CPI targets — so framework-safe code isn't flagged. Reserve ERROR for symbolically/
  graph-proven findings; heuristics stay at WARNING/HINT (Invariant C). Offer an auditable
  `// seagrass-ignore(reason)` escape hatch, mirroring Anchor's `/// CHECK:` UX.

---

# Part 3 — Sequencing (defense and offense on one timeline)

1. **Quick relief (hours):** Invariants D, E, and the `experimental→false` flip. Removes the visible
   noise from the incident.
2. **Foundation rung (a→b) — structural (days):** Invariants A + B (shared scope visitor, absence
   gate, glob fix). *Same work unlocks Tier B offense* (collision/drift/authority-centralization).
3. **Durable guard:** golden corpus + compile-truth CI. Ship early — it locks in every later change.
4. **Foundation rung (b→c) — the call graph (weeks):** the highest-leverage infrastructure. Unlocks
   moat diagnostics #1–#7 region (stale-after-CPI, oracle staleness, authority reachability, signer
   propagation) *and* makes cross-file account/usage reasoning sound (retiring the remaining Family 2
   ERROR-level lints).
5. **Foundation rung (c→d) — dataflow/taint (research horizon):** cross-instruction state machines and
   economic-invariant checks — the long-term identity of the tool.

> If only one structural thing ships first: the **call graph (step 4)**. It is the single investment
> that pays both defense (sound cross-file reasoning) and offense (the moat tier) at once.

---

## Appendix A — full lint FP-risk inventory (defense)

Audited across all ~30 diagnostic modules. Risk = likelihood of a *false* positive on idiomatic,
compiling Anchor code.

| Risk | Lint | Sev | Claim | Fix principle |
|---|---|---|---|---|
| HIGH | handler_members / unknown-handler-member (impl-method) | WARNING | absence | `visit_impl_item_fn` + scope params (Inv. A) |
| HIGH | constraint_expressions / UnresolvedIdentifier (glob) | WARNING | absence | track glob roots; suppress under glob+no-index (Inv. B) |
| HIGH | constraint_shape/has_one — field resolution | ERROR | absence | suppress when struct found but field list empty |
| HIGH | check_cfg / init-if-needed | WARNING | absence | parse manifest incl. `workspace=true` (Inv. D) |
| HIGH | check_cfg / anchor-debug | WARNING | absence | walk to workspace `Cargo.toml` (Inv. D) |
| HIGH | artifacts / SbfArtifact stale | WARNING | heuristic | off by default / content-hash (Inv. E) |
| HIGH | ecosystem / TestHarness MissingTests | WARNING | absence | scan workspace `tests/`; same-crate dev-deps only |
| HIGH | security.owner-check (raw_account) | WARNING | absence | `visit_impl_item_fn` on `RawAccountFileVisitor` (Inv. A) |
| HIGH | security.type-cosplay (raw_account) | WARNING | absence | same as owner-check (Inv. A) |
| HIGH | code_quality.stale-account-after-cpi | WARNING | heuristic | use real ctx param name, not `"ctx"` (Inv. A) |
| HIGH | code_quality.initialization (reinit) | WARNING | heuristic | require co-absence of init guard; or INFO (Inv. C) |
| HIGH | code_quality.pda-seed-collision | WARNING | heuristic | threshold ≥3 dynamic seeds; or real collision only |
| MED | account_references / missing-account-reference | ERROR | absence | EvidenceGraph w/ reachable fns + workspace (Inv. B) |
| MED | account_references / missing-instruction-argument | ERROR | absence | gate on workspace-indexed mapping |
| MED | account_usage / unknown-ctx-account-field | ERROR | absence | union cross-file reachable-fn usages |
| MED | constraint_expressions / UnknownMember | WARNING | absence | index freshness marker; skip if coverage partial |
| MED | handler_members / unknown-handler-member (text) | WARNING | absence | strip block comments + raw strings first |
| MED | context_accounts / missing derive(Accounts) | ERROR | absence | glob+workspace guard (partially fixed) |
| MED | context_accounts / empty `Context<>` fill | WARNING | shape | same guard; `visit_impl_item_fn` in EmptyContextVisitor |
| MED | anchor_syn / unresolved-generic-account-type | ERROR | absence | suppress when index None and type non-local (Inv. B) |
| MED | constraint_shape/initialization — MissingInit | ERROR | absence | fire only with positive initializer evidence |
| MED | check_cfg / anchor-lang multi-line table | WARNING | absence | parse full manifest (Inv. D) |
| MED | artifacts / ProgramKeypair mismatch | WARNING | existence | matching-cluster id only; or INFO |
| MED | artifacts / IdlArtifact missing | WARNING | absence | require Anchor kind AND known IDL producer |
| MED | ecosystem / SurfpoolWorkspace missing artifact | WARNING | absence | distinguish local-deploy vs remote-cluster |
| MED | ecosystem / SolanaIdlArtifact (codama, shank) | WARNING | absence | resolve IDL paths relative to `codama.json` |
| MED | spl_semantics / token-program-type | WARNING | existence | `resolve_type_alias()` before comparing |
| MED | spl_semantics / mint-decimals-type | WARNING | existence | walk `ImplItemFn` in `#[program]` (Inv. A) |
| MED | security.signer.authorization | ERROR | existence | index signer/CPI usages from `ImplItemFn` (Inv. A) |
| MED | security.cpi.program | WARNING | existence | include impl-method bodies in usage graph (Inv. A) |
| MED | security.token-account | WARNING | existence | no fix — gap is under-reporting |
| MED | code_quality.stale-cpi — invoke_signed blind spot | WARNING | absence | add `invoke_signed` to `is_cpi_call` |
| MED | code_quality.unsafe-unwrap | WARNING | heuristic | recognize integer-widening `try_into`; escape hatch |
| MED | code_quality.account-closing | WARNING | heuristic | discriminator write on assignment LHS, not any mention |
| MED | code_quality.instruction-data-bounds | WARNING | absence | inspect `require!`/`assert!` for `.len()` checks |
| LOW | handler_scope / unresolved-handler-identifier | WARNING | absence | **fixed**; verify nested-mod impl blocks |
| LOW | account_usage / missing-mut | WARNING | absence | document proc-macro boundary |
| LOW | constraint_expressions / bump UnexpectedType | WARNING | shape | resolve type aliases before compare |
| LOW | instruction_attributes / MissingInstructionArg | ERROR | shape | require index; skip if instruction not in file |
| LOW | constraint_shape/program_account — UncheckedAccount | WARNING | existence | cover cross-file bodies, or document scope |
| LOW | anchor_syn/program — handler return type | ERROR | shape | recurse nested `mod` in `#[program]` |
| LOW | constraint_shape/catalog — companion diagnostics | ERROR | absence | generate exclusion list programmatically |
| LOW | constraint_shape/token — mint reference | ERROR | shape | restrict mint lookup to same struct level |
| LOW | project_identity / AnchorProjectId | ERROR | existence | allow per-cluster id overrides; or WARNING |
| LOW | code_quality.bump-seed-canonicalization | WARNING | existence | suppress when bump from `bump`/`*_bump` field |
| LOW | security.sysvar.address | WARNING | absence | require `AccountInfo`/`Unchecked` + no `address =` |

## Appendix B — external principles (how mature tooling avoids FPs)
rust-analyzer (`{unknown}` propagation, experimental gating, salsa cancellation), clippy
(correctness/nursery tiers, `Applicability`), typescript-eslint (open-world; `no-undef` disabled).
Ten principles: (1) unresolved entities *silence* downstream diagnostics; (2) open-world absence;
(3) phase by confidence; (4) experimental opt-in; (5) confidence→severity; (6) no emission from
partial state; (7) region-scoped rules; (8) framework gating; (9) classify fix applicability
(seagrass has only `Unspecified` today); (10) arbitrate overlaps by topic, keep highest confidence.
**seagrass already has scaffolding for all ten — the work is enforcement.**
