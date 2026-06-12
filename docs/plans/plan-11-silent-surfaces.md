# Plan 11 — Silent Surfaces: Control-Plane Parity Guards + Install Smoke Matrix

## Context

The `// seagrass-ignore` directive shipped broken (fixed in d5e1b2e: one diagnostic
pipeline path bypassed the shared suppression filter) and nothing caught it — because
suppression is a **silent control surface**: when it breaks, it says nothing. The
corpus gates prove diagnostics don't *lie*; nothing proves the knobs *work*. This
plan generalizes the d5e1b2e lesson into enforced parity guards, and closes the
sibling gap: the install path, which also fails silently — on the user's machine,
before they've seen a single diagnostic.

Silent surfaces in this codebase, inventoried (verified at write time):

1. **Suppression directives** — `// seagrass-allow:`, `// seagrass-allow-file:`,
   `// seagrass-ignore`, `#[seagrass(allow("…"))]`, and `Seagrass.toml`
   `[lints] allow` (`src/lsp/diagnostics/suppression.rs:11–16, 31–41, 126–150`).
   Post-d5e1b2e tests cover the *parsing* (:355–468); what remains unguarded is
   that **every diagnostic emission path** applies them (the actual d5e1b2e bug).
2. **Settings keys** — `ServerSettings::apply` (`src/runtime/server_types/mod.rs:177–204`)
   **silently ignores unknown keys** (`setting_value` :281–286). ~30 keys in
   `editors/vscode/package.json` `contributes.configuration` (:209–436), a parallel
   set in `editors/vim/coc-settings.json` (:8–47), `SECURITY_LEVEL_SETTINGS`
   (:9–41), and the prose list in `editors/UI_CONTRACT.md` (:104–132). Any drift
   between these five surfaces = a knob that does nothing, undetected.
3. **Install artifacts** — three independent encodings of the artifact name
   contract: `release.yaml` build matrix (:219–284, 5 targets),
   `scripts/install.sh` platform map (:83–102, 3 targets),
   `editors/vscode/src/serverInstallModel.ts` (:6–12, :54–71, 4 targets).
   **Known live drift found during recon: `release.yaml` builds
   `x86_64-unknown-linux-musl` but neither installer can ever select it.**

### Existing scaffolding (extend, don't duplicate)

| File | What it already does |
|---|---|
| `editors/check-ui-contract.ts` | Commands/titles, startup output tokens, status bar, Zed slash commands, the 10 security-family enums, Vim/CoC root markers + `editor.client` |
| `scripts/check-lint-catalog.ts` | Every lint doc page must show all 4 suppression forms (:94–108); topic uniqueness |
| `scripts/protocol-smoke.ts` | Full LSP handshake + open/change/diagnostics/completions/hover/actions over real JSON-RPC against fixtures |
| `scripts/smoke-install.sh` | Finds a built binary, runs `seagrass diagnostics fixtures/smoke-broken.rs --json`, asserts expected pattern |
| `.github/workflows/release.yaml` | verify-tag cross-checks VERSION/Cargo.toml/package.json/zed versions; builds 5 targets; asserts all assets + checksums present |

### Architecture doctrine (must hold in every step)

1. **One canonical source per contract, everything else verified against it.**
   Settings keys: the server's parser is canon (it's what executes). Artifact
   names: the release matrix is canon (it's what exists). Guards compare surfaces
   *to canon*, never to each other pairwise.
2. A silent surface earns a guard that fails loudly on drift — a test or CI job,
   not a doc note.
3. Guards live next to the existing gate of the same species: TS contract checks
   extend `check-ui-contract.ts` patterns; Rust enforcement mirrors the registry
   invariant tests; CI mirrors `corpus.yml` structure.

## Non-Goals

- No new settings, directives, or platforms (the musl/ARM-linux decisions in
  Step 6 are scoped to *reconciling existing* surfaces, not adding support).
- No real VS Code instance in CI (the extension's download logic is exercised at
  the module level; full-editor e2e stays manual).
- No re-test of suppression *parsing* (d5e1b2e's tests own that); this plan owns
  *application* coverage across pipeline paths.
- No Windows support in `install.sh` (documented Unix-only; Windows is covered by
  the extension path and the smoke matrix).

## Steps

### Step 0 — Baseline

```bash
cargo test 2>&1 | tail -3          # 0 failed (1587+ at plan time)
cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
bun editors/check-ui-contract.ts   # passes
bun scripts/check-lint-catalog.ts  # passes
```

### Step 1 — Settings-key canon + exhaustive recognition test (Rust)

`ServerSettings::apply` consults keys ad hoc; make the key set a first-class
constant so it can be exported and verified:

1. In `src/runtime/server_types/mod.rs`, add
   `pub const RECOGNIZED_SETTING_KEYS: &[&str]` listing every key `apply()` reads
   (the ~14 scalar keys + the 10 `SECURITY_LEVEL_SETTINGS` entries' `setting`
   values + aliases like the two `strictNative` spellings).
2. Enforcement test `apply_consumes_every_recognized_key`: for each key, build a
   JSON settings object containing only that key with a non-default value, call
   `apply()`, and assert the resulting `ServerSettings` differs from default.
   This makes "key listed but parser dropped it" a test failure — the settings
   equivalent of the registry's `all_variants_are_covered_by_all`.
3. Test `recognized_keys_are_sorted_and_unique` (cheap, prevents append rot).

### Step 2 — Cross-editor settings parity (extend `check-ui-contract.ts`)

Emit canon for the TS side: a small build step or checked-in generated file
`editors/recognized-settings.json` produced from `RECOGNIZED_SETTING_KEYS`
(follow the existing generated-support drift-gate pattern from plan-02: a script
regenerates it; a test fails if it's stale). Then in `check-ui-contract.ts`:

- Every `seagrass.*` key in `editors/vscode/package.json` `contributes.configuration`,
  after stripping the `seagrass.` prefix, must be in canon — **and vice versa**:
  every canon key must appear in VS Code's contributes (the flagship surface
  exposes everything). Explicit allowlist for extension-local keys the server
  never sees (`serverCommand`, `serverArgs`, `serverCwd`, `serverEnv`,
  `dev.useCargoFromCheckout`, `tridentCoverage.*`, `inlayHints.enabled` if
  client-side — each with a one-line reason in the script).
- Every key in `editors/vim/coc-settings.json` (both the
  `initializationOptions.seagrass.*` nest and the flat `settings.seagrass.*`
  block) must be in canon.
- Every key named in `editors/UI_CONTRACT.md` :104–132 must be in canon.

### Step 3 — Suppression application across every emission path (Rust)

The d5e1b2e generalization. First enumerate the paths: in
`src/server/diagnostic_pipeline.rs` and `src/server/helpers.rs`, list every
site that converts collected diagnostics into published/pulled LSP diagnostics
(push on open/change, pull (`textDocument/diagnostic`), the cold path
(`diagnostics.coldPath` idle/save/manual), and the parse-pause path d5e1b2e
fixed). Then:

1. Add a doc comment at the suppression filter call site naming it the **single
   choke point**; if enumeration finds paths that don't route through it, that is
   the bug — route them (this is how d5e1b2e was fixed; finish the job for all).
2. Integration tests (extend the harness behind `scripts/protocol-smoke.ts` or
   the Rust JSON-RPC test in `tests/jsonrpc_lsp.rs`): one fixture with a
   guaranteed diagnostic + `// seagrass-ignore`, asserted suppressed via **push**
   and via **pull** transport, and after a parse-pause edit sequence (type broken
   code, pause, fix). One directive form suffices per transport — the parsing
   matrix is owned by suppression.rs tests; this matrix is (transport × one form).
3. `Seagrass.toml` end-to-end: a workspace fixture with `[lints] allow` and a
   matching diagnostic, asserted suppressed through the real
   `nearest_seagrass_toml` discovery (`src/core/project/mod.rs:72`), not by
   passing the TOML string directly.

### Step 4 — `Seagrass.toml` documented-vs-parsed parity

`SeagrassConfig` parses exactly `lints.allow` (`suppression.rs:31–41`). Add to
`check-lint-catalog.ts` (or a sibling script): scan `docs/`, `skills/`,
`editors/` for fenced TOML blocks attributed to `Seagrass.toml`; every key
path used in them must be in a canon list exported next to `SeagrassConfig`
(start: `["lints.allow"]`). Docs promising un-parsed keys = check failure.
(Recon found `Seagrass.toml` referenced as a root marker in coc-settings,
helpers.rs watchers, and Neovim docs — the risk of doc-invented keys is real.)

### Step 5 — Artifact-name parity check (TS, runs in `pr.yaml`)

New `scripts/check-release-parity.ts`:

- Parse the `release.yaml` build matrix (targets + archive extensions) — canon.
- Parse `scripts/install.sh`'s case-arm targets (regex on the `case` block
  :83–102) and assert ⊆ canon with matching extensions.
- Import `editors/vscode/src/serverInstallModel.ts` directly (it's dependency-free)
  and assert its `supportedReleasePlatform` outputs ⊆ canon; assert
  `releaseArchiveName(...)` reproduces the exact `seagrass-{v}-{target}{ext}`
  string for every supported platform.
- Assert the **converse with an exceptions list**: every canon target is
  installable by at least one installer, or named in
  `UNINSTALLABLE_TARGETS: { target, reason }[]`. Seed it with
  `x86_64-unknown-linux-musl` and force Step 6's decision to empty or justify it.
- Wire into `pr.yaml` next to the existing contract checks.

### Step 6 — Reconcile the musl drift (decision + small change)

Pick one (smallest correct change wins):

- **(a)** Teach `install.sh` musl detection (`ldd --version` mentions musl →
  select the musl target) — Alpine/container users get a working install; or
- **(b)** Drop musl from the release matrix and the exceptions list.

Default to (a): the artifact already builds; ~10 lines of shell makes it real.
Either way the Step-5 exceptions list ends empty.

### Step 7 — Install smoke matrix (`.github/workflows/install-smoke.yaml`, NEW)

Two jobs:

1. **`smoke-build`** (on `pull_request` touching `scripts/install*.sh`,
   `editors/vscode/src/serverInstall*`, `release.yaml`, `Cargo.toml`, plus
   `workflow_dispatch`): matrix over `macos-latest` (arm64), `ubuntu-latest`,
   `windows-latest`. Build `seagrass-cli` for the host, then run
   `scripts/smoke-install.sh` and assert `seagrass --version` output matches
   `VERSION` (reuse the token logic of `versionOutputMatches`,
   `serverInstall.ts:44–48`), then run `scripts/protocol-smoke.ts` (or its
   documented smoke subset) for a real LSP handshake. Windows: skip
   `install.sh`, run the binary + handshake only.
2. **`smoke-release`** (on `release: published`, + `workflow_dispatch` with a tag
   input): same OS matrix, **no checkout-built binary** — run `install.sh`
   (Unix) against the *actual published release*, and on all three OSes run a
   small Node script driving `serverInstallModel.ts` + the download/verify/extract
   functions from `serverInstall.ts` against the real release URL, then execute
   the resulting binary with `--version`. This is the guard that the published
   artifacts, names, checksums, and binaries actually work on user machines —
   the first-contact gate.

Module-level: add unit tests for `serverInstallModel.ts` (archive naming per
platform, checksum-line parsing, unsupported-platform error) runnable under
`bun test` in the extension's existing test setup, so name regressions fail in
PRs, not at release time.

### Step 8 — Document

- `editors/UI_CONTRACT.md`: add a "Silent surfaces" section: the canon locations
  (settings keys, artifact names, suppression choke point) and which check guards
  each.
- `docs/diagnostics-strategy.md`: one paragraph — corpus gates guard claims;
  plan-11 gates guard controls; both required for "doesn't embarrass you".

## Acceptance Criteria

1. `cargo test` — 0 failed; count strictly greater than baseline; includes
   `apply_consumes_every_recognized_key` passing.
2. `cargo clippy --all-targets -- -D warnings` — clean.
3. `bun editors/check-ui-contract.ts` fails when (test it by mutation, then
   revert): a key is added to VS Code contributes but not canon; a canon key is
   removed from VS Code contributes; a CoC key drifts.
4. Suppression transport matrix passes: ignored diagnostic absent via push, pull,
   and post-parse-pause; `Seagrass.toml` allow honored through real file discovery.
5. `bun scripts/check-release-parity.ts` — passes with an **empty** exceptions
   list (Step 6 resolved), and fails on a mutated target string (mutation-test,
   then revert).
6. `install-smoke.yaml` green on all three OSes for the `smoke-build` job
   (trigger via `workflow_dispatch` after push); `smoke-release` job exists and
   is wired to `release: published` (it can only fully run at next release —
   dry-run it via `workflow_dispatch` against the latest existing release tag,
   if one exists; otherwise its first real run is the next release).
7. `grep -rn "single choke point" src/server/` — the suppression call-site doc
   comment exists; the emission-path enumeration is recorded in the commit body.

## Risks / Edge Cases

- **Canon export staleness.** `recognized-settings.json` is generated-and-checked-in;
  the drift gate (same pattern as the generated support catalog) is what keeps it
  honest. Without the gate, the file *is* a new silent surface — do not skip it.
- **Allowlist creep in Step 2.** Extension-local keys need reasons, and the
  reviewer's question for every addition is "why does the server never need
  this?" An allowlist without reasons re-creates the hole the check exists to
  close.
- **Parse-pause test flakiness.** The cold-path/typing-suppression timing
  (`DiagnosticsColdPath::Idle`) involves debounce; drive the test via the
  `manual`/`save` cold-path setting or explicit request sequencing rather than
  wall-clock sleeps.
- **`smoke-release` can't be fully proven until a release happens.** Mitigate by
  dry-running against the most recent existing tag; if the repo has no published
  release yet, the job's first execution is the next release — acceptable, since
  it gates *that* release's announcement, not the merge.
- **Windows runner PowerShell extraction** (`Expand-Archive`,
  `serverInstall.ts:170–183`) differs across runner images; pin behavior by
  asserting on the extracted binary's existence and `--version` output only,
  not on intermediate paths.
- **Parsing YAML/shell with regex in Step 5** is brittle by nature; keep the
  parsers anchored to the exact structures (`case` arms, matrix entries) and
  fail loudly on parse-shape changes — a parity checker that silently parses
  nothing would itself be a silent surface.
- **`linux-aarch64` has no release target at all** (Graviton/Raspberry
  Pi/Asahi users). Out of scope here (Non-Goals), but record it in the Step-5
  checker as a comment so the next person who asks "why no ARM Linux" finds the
  decision point. Adding it later = one matrix row + parity check passes
  automatically.
