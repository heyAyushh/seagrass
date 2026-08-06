# Changelog

All notable Seagrass LSP changes are tracked here. Release automation should
keep future entries aligned with release-plz output.

## [Unreleased]

- ci: add a PR fuzz build smoke so install/MSRV and ASAN link regressions fail
  before merge, instead of only on the scheduled long fuzz run.
- ci: disable cargo-fuzz AddressSanitizer (`-s none`) so nightly builds avoid
  the broken `__sancov_gen_*` ASAN link path, and build only each shard's target.
- ci: install and run cargo-fuzz through deterministic nightly scripts so fuzz
  CI no longer fails on the cargo-platform 1.91 MSRV floor, and package corpus
  artifacts even when crash dirs are empty.
- build: pin Solana and Anchor catalog inputs, add runtime catalog parity checks,
  and keep generated Anchor support guarded against hand-edited drift.
- lsp: derive diagnostic severity from registry provability, enforce
  truth-source audit coverage, and align emitted topics with the lint catalog.
- lsp: add framework semantic modeling and capability-registry resolution so
  Anchor, native Solana, and Pinocchio diagnostics rely on parsed workspace
  facts instead of name-only guesses.
- lsp: add trie-backed completion wakeups and program-id completion gating so
  Anchor-aware completions stay fast without polluting normal Rust contexts.
- client: add first-class Vim-family command surfaces with Vim quickfix CLI
  scans, a Neovim runtime package, and explicit classic vi CLI-only guidance.
- client: add Zed-native completion and symbol labels, language-server
  installation status reporting, and file/instruction arguments for document
  slash commands.
- client: open Zed slash-command path arguments in the throwaway LSP session so
  `/seagrass-analyze <path>` returns the same report as an active document.
- client: parse raw Zed slash-command argument text before opening analysis
  paths, so path plus filter forms dispatch the intended document URI.
- client: add VS Code server-binary resolution for matching released Seagrass
  versions and document install paths for agent/editor users.
- client: select the VS Code `x86_64-unknown-linux-musl` server release on
  musl-based Linux extension hosts instead of caching the glibc binary.
- ci: add installer and release packaging workflows, release parity checks,
  generated-support freshness checks, and local production-gate coverage for the
  release/install surfaces.
- ci: guard silent editor and installer control surfaces with settings,
  suppression, `Seagrass.toml`, release-parity, and install-smoke checks.
- ci: allow slower Windows server-portability runs to finish the full Seagrass
  library test suite instead of canceling at the previous 30-minute cap, and
  install rustfmt explicitly for cross-platform protocol formatting smoke.
- ci: serialize black-box JSON-RPC LSP integration tests so platform runners do
  not race cargo-backed server startup or diagnostic publication.
- ci: construct JSON-RPC LSP test file URIs through the canonical URL parser so
  Windows push-diagnostic assertions compare the same URI form the server emits.
- ci: retry the protocol-smoke formatting request after `didOpen` so slower
  runners do not race document registration before checking rustfmt output.
- test: keep large real-program corpus trees fetch-only instead of committing
  their source mirrors under `fixtures/corpus`.
- test: add fetch-on-demand external corpus infrastructure, vendor permissively
  licensed corpus programs, and promote the corpus scan into a hard gate.
- lsp: apply `seagrass-ignore` and workspace lint allowances to open-document
  syntax diagnostics before publishing editor Problems.
- lsp: require static program-id evidence before suppressing arbitrary-CPI
  program-account diagnostics from reachable runtime checks.
- lsp: add a bounded workspace call graph for reachable-helper signer defense
  and unambiguous reachable-helper CPI program diagnostics.
- lsp: update trie-backed module-path resolution during incremental workspace
  file upserts and removals, not only during full startup scans.
- lsp: add Anchor account-space estimation for hovers and code lenses, with
  InitSpace/max_len quick fixes for `space = T::INIT_SPACE` constraints.
- lsp: offer checked-arithmetic quick fixes for lamport/token math and cover
  division and remainder operators alongside add/sub/mul.
- lsp: accept compatible SPL token program accounts for Token-2022/interface
  token initialization, resolve token-interface import aliases, and silence
  unprovable composite payer member-path checks found by the external corpus.
- test: add committed corpus regressions for SPL token-program compatibility
  and composite payer references, and make external-corpus setup fail early
  when Python is too old for `tomllib`.

## [0.1.2] - 2026-06-05

- client: expose Seagrass analysis, program report, error coverage, support
  matrix, generator profile, and logs as Zed slash commands, and advertise
  refactor code actions in the Zed manifest with concise report summaries before
  raw JSON detail.
- lsp: complete account-data methods on `ctx.accounts` paths and local account
  aliases while Rust syntax is temporarily incomplete.
- lsp: recognize Pinocchio split-crate projects and native
  `next_account_info`/`accounts.get(..)` account aliases in Solana security
  diagnostics.
- cli: extend `seagrass analyze` compute analysis with SBF `.text`
  instruction-count floors when local deploy artifacts are available, while
  keeping runtime CU measurements evidence-gated.
- cli: add `seagrass skills list/get/path` so installed binaries can expose
  version-matched agent workflow instructions with JSON output and `--full`
  progressive disclosure.
- lsp: mark statically emitted Anchor account error metadata as
  `static-covered` and guard it in protocol smoke.
- lsp: split Anchor error coverage into `static-covered`,
  `preflight-covered`, and `runtime-only` tiers with a no-build invocation
  evidence model for instruction/account input failures.
- cli: add `seagrass preflight` with a checked-in Anchor evidence fixture that
  exercises every `preflight-covered` error while leaving runtime evidence
  unconfigured.
- ci: add Linux and Windows server portability tests to PR guardrails, ship a
  static `x86_64-unknown-linux-musl` server release artifact, and package
  Windows server releases as `.zip`.
- docs: add the installed skill-help maintenance process so checked-in
  `SKILL.md` files, `seagrass skills` output, and fallback docs do not drift.
- lsp: mark native Solana and Pinocchio framework crates as stable applicable
  parity and keep owner/type/signer/writable/CPI validations scoped to the
  specific account or program expression being checked.
- docs: frame Seagrass as Solana codebase intelligence while separating shipped
  static analysis from runtime compute/traffic evidence requirements.
- cli: add `seagrass analyze` for headless codebase-intelligence reports with
  static account/CPI/PDA evidence and explicit runtime evidence boundaries.
- build: pin Seagrass Rust MSRV and CI stable toolchains to Rust 1.89.0 while
  preserving nightly-only fuzz target execution.
- lsp: add document formatting support backed by `rustfmt` for open editor
  buffers.
- lsp: bound definition-bridge Rust source discovery with explicit depth and
  file-count budgets.
- test: extend the LSP protocol smoke path to verify document formatting over
  JSON-RPC.
- test: add a black-box JSON-RPC integration test for root LSP formatting plus
  native Solana and Pinocchio diagnostic and quick-fix parity.
- lsp: return framework-correct code-action edits for native discriminator
  guards plus Pinocchio owner, signer, and CPI guards.
- lsp: avoid reusing cached code actions across diagnostic-scoped requests.
- lsp: diagnose writable CPI account metadata without visible writable checks
  for native Solana and Pinocchio, with editor quick fixes.
- docs: document applicable native Solana and Pinocchio parity boundaries so
  Anchor-only surfaces are marked non-applicable instead of overclaimed.
- fuzz: add semantic diagnostic collection coverage beyond parse-only targets.
- ci: verify pinned Bun npm package integrity before workflow installs and align
  PR property-test depth with release/property workflows.
- lsp: route native Solana and Pinocchio security diagnostics through their
  framework crates while keeping shared lint/range/diagnostic helpers in
  `seagrass-framework`.
- lsp: detect Pinocchio `borrow_data_unchecked()` raw account reads, accept
  `is_owned_by(...)` owner checks, and flag dynamic `InstructionView` CPI
  program ids in native/Pinocchio security diagnostics.
- lsp: run native/Pinocchio code-quality checks over impl methods, track
  destructured account collections, and detect Pinocchio
  `InstructionAccount::*_signer(...)` signer usage.
- client: align editor startup/status docs with incremental sync and
  native/Pinocchio security coverage.
- docs: make contributor guardrails repository-relative, fix dead editor setup
  references, document Cursor/OpenCode templates, and clarify current
  Anchor-first framework coverage.
- ci: add Rust formatting and Clippy checks to PR guardrails.
- ci: run the LSP protocol smoke test in PR guardrails.
- build: pin the Anchor dependency with its full git revision.
- security: replace the unreachable noreply security fallback with a safe
  no-details escalation path.
- security: bound project-file reads used by diagnostics, workspace indexing,
  and definition bridge collection.
- lsp: advertise incremental document sync and apply ranged text changes before
  live diagnostics.
- lsp: make syntax-error diagnostics state that semantic diagnostics are paused
  until the Rust file parses again.
- lsp: classify real `anchor-lang` / `anchor-spl` 2.x dependencies as Anchor v2
  preview instead of relying only on experimental alias names.
- perf: remove an unused Salsa diagnostics query that was executed and discarded
  during analysis reports.
- perf: update workspace indexes incrementally for watched Rust file changes
  instead of rebuilding every root on each file event.
- client: add VS Code lint-doc, suppression-copy, and false-positive reporting
  commands backed by Seagrass diagnostic metadata and the server feedback
  manifest.
- client: summarize Trident coverage gaps with lint-promotion hints in VS Code.
- lsp: include diagnostic applicability and quickfix preview metadata in editor
  related information for richer Problems hovers.
- docs: add a MkDocs entrypoint, consumer SARIF workflow example, and refreshed
  agent skill guidance for confidence, Trident coverage, and SARIF output.
- docs: add OpenCode and Cursor editor templates under `editors/` plus
  activation guidance for assistant and automation workflows.
- docs: remove stale agent-skill paths, hardcoded topic counts, and old install
  references from onboarding docs.
- build: make `seagrass-cli` the package-level CLI identity while keeping the
  installed binary named `seagrass`.
- product: add golden-path `scripts/smoke-install.sh`, `fixtures/smoke-broken.rs`,
  README choose-your-path table, and `docs/lints/index.html` generator.
- cli: add `--sarif` output plus `docsUrl` and `applicability` fields in JSON
  diagnostics for CI and GitHub code scanning.
- ci: add `seagrass-diagnostics` workflow, composite GitHub Action, and SARIF
  upload for the smoke fixture.
- client: VS Code defaults to the installed `seagrass` binary, adds explain /
  suppress / scan-workspace commands, and `seagrass.dev.useCargoFromCheckout`.
- lsp: prefer Seagrass lint catalog URLs in `codeDescription` for `seagrass/...`
  topics.
- lsp: attach `confidence` and `topic` as related information so editors can show
  trust tiers in Problems peek and hovers.
- client: VS Code Trident llvm-cov JSON bridge with gutter coverage and derived /
  heuristic diagnostic underlines.
- ci: wire golden-path smoke and lint index freshness into `verify-production.ts`.
- lint: ignore non-catalog pages such as `docs/lints/README.md` in the lint catalog
  checker.
- docs: document rust-analyzer coexistence, SARIF/agents CLI paths, and VS Code
  recommended settings for checkout development.
- build: add a strict `verify-production --release` mode for today-of-release
  preflight checks that reject pending fuzz and review evidence.
- ci: count release fuzz proof as 40 aggregate fuzz-hours across sharded
  targets so the fuzz workflow can run in parallel for release-day evidence.
- ci: run fuzz, property, and release workflows from the standalone repository
  root and fetch the pinned Anchor source in fresh CI clones.
- lsp: keep imported helpers, file constants/statics, and program handlers
  resolved during broken-buffer handler-scope diagnostic recovery.
- lsp: reduce false positives for trait-style handler method calls, qualified
  struct literals and patterns, and bogus module-qualified associated values in
  Anchor constraint expressions.
- lsp: wake account constraint value completions after `seeds = [` and include
  same-file Rust values plus const-like imports in constraint expression
  completions.
- lsp: ignore `seagrass-ignore` and `seagrass-allow` text inside Rust string
  literals when filtering diagnostics.
- lsp: resolve typed instruction-argument member access in account constraint
  expressions and completions.
- lsp: infer typed members from handler-local `const` and `static` block items,
  including mid-edit member completions before the item declaration.
- lsp: resolve and complete Rust block item values inside Anchor handlers.
- lsp: complete handler expression values declared inside Anchor
  `#[program]` modules.
- lsp: flag unresolved const-like handler identifiers such as
  `EXPECTED_LIMIT` typos using the shared handler scope model.
- lsp: complete and diagnose Rust struct literal and pattern fields in Anchor
  handlers from resolved struct members, with typo quick fixes.
- lsp: offer handler value completions in empty Rust expression slots such as
  assignment RHS, call arguments, and `return` values.
- lsp: diagnose account-data fields called as methods and mistyped
  `AccountLoader` load methods inside Anchor constraint expressions.
- lsp: resolve handler identifiers and account-data members inside common Rust
  and Anchor assertion macros such as `assert!` and `require!`.
- lsp: keep raw-account security evidence limited to runtime assertions so
  `debug_assert!` checks do not mask missing owner checks.
- lsp: offer handler value completions in empty assertion macro argument slots
  without waking inside format-style macros such as `msg!`.
- lsp: infer handler member types from typed Rust tuple destructuring in
  handler arguments and local bindings.
- lsp: carry handler member types through standard Iterator item-preserving
  adapters and `next`/`nth`/`find` unwrapping.
- lsp: infer Iterator `map` closure inputs and mapped output member types in
  handler diagnostics and completions.
- lsp: carry handler member types through `Option`/`Result` map, and_then, ok,
  and unwrap combinator chains.
- lsp: infer handler member types from `Option`/`Result` `Some` and `Ok`
  pattern bindings in if-let, let-else, while-let, and match arms.
- lsp: infer handler member types from standard `Some` and `Ok`
  constructor values before pattern matching and unwraps.
- lsp: infer handler member types through `Some`/`None` and `Ok`/`Err`
  control-flow expressions before pattern matching and unwraps.
- lsp: infer handler member types from `Option`/`Result` `map_or` and
  `map_or_else` output values.
- lsp: infer handler member types through standard `Option`/`Result` fallback
  and error-mapping wrapper methods.
- lsp: infer handler member types from match and if-let branch outputs that use
  pattern-bound values.
- lsp: infer handler member types through `if`/`match` branches that diverge
  with `return`, `break`, `continue`, or standard panic-style macros.
- lsp: infer handler member types from block-local `let` bindings that feed a
  block expression tail value.
- lsp: infer handler member types from `if`, `match`, and block expression
  results when every branch resolves to the same shallow type.
- lsp: infer handler member types from assignment expressions that initialize
  existing local bindings.
- lsp: infer typed member access directly from indexed and unwrapped iterable
  expressions in Anchor handlers.
- lsp: infer typed member access from indexed and unwrapped iterable element
  aliases in Anchor handlers.
- lsp: infer typed member access and value completions from Rust for-loop
  item patterns over typed iterable handler values.
- lsp: infer typed member access and value completions from Rust closure
  parameter patterns inside Anchor handlers.
- lsp: infer typed member access from destructured Rust function-parameter
  patterns in Anchor handlers.
- lsp: infer field member types from resolved Rust struct-pattern
  destructuring in Anchor handlers.
- lsp: infer account-data member types from typed `let` patterns, including
  `let Some(account) = ... else` handler flows.
- lsp: infer shallow types for pattern-bound Anchor account values so `if let`
  bindings drive handler member diagnostics and completions.
- lsp: resolve Rust pattern bindings from `if let`, `while let`, and `match`
  arms in Anchor handler diagnostics and value completions.
- lsp: flag unresolved lower-case handler call identifiers using the same
  parsed scope as bare handler values, with typo quick fixes.
- lsp: diagnose unknown handler method calls on resolved account-data values
  and reuse candidate methods for typo quick fixes.
- lsp: infer associated function return types such as `Type::new()?` for
  handler member completions and diagnostics, including split-file impls.
- lsp: carry inherent method return types through workspace associated-value
  indexing so split-file method returns drive member completions and diagnostics.
- lsp: carry helper function return types through the workspace index so
  split-file handler member completions and diagnostics can resolve them.
- lsp: infer same-file inherent method return types for handler member
  completions and diagnostics, including `Result<T>` values after `?`.
- lsp: infer same-file helper function return types for handler member
  completions and diagnostics, including `Result<T>` values after `?`.
- lsp: carry handler account-data member types through transparent `as_ref`
  and deref aliases backed by Anchor account traits.
- lsp: resolve `AccountLoader<'info, T>` in handler member access as a loader
  until `load`, `load_mut`, or `load_init` produces the zero-copy account data.
- lsp: complete inherent account-data methods in handler member access from
  parsed `impl` blocks and flag known fields that are accidentally called as
  methods, with a quick fix to remove the call suffix.
- lsp: resolve account-data member access through intermediate
  `ctx.accounts` aliases such as `let accounts = &mut ctx.accounts`.
- lsp: complete and validate `ctx.bumps.*` from generated Anchor `Bumps`
  fields using parsed PDA constraints instead of account-name guesses.
- lsp: complete handler member access through `Context<T>` account aliases,
  including boxed Anchor accounts such as `let account = &mut ctx.accounts.x`.
- lsp: recover typed handler member diagnostics from syntax-broken Anchor
  handlers using shared text scope, `Context<T>` account aliases, and
  tree-sitter struct fields.
- lsp: recover unresolved handler identifiers from syntax-error spans so
  malformed Anchor handlers still flag random bare values semantically.
- lsp: keep missing-semicolon parser range recovery near the parse error so
  malformed handler statements do not get reported on previous account attrs.
- lsp: propagate shallow handler types through local aliases and resolved
  field aliases for member diagnostics and completions.
- lsp: offer quick fixes for misspelled in-scope handler values.
- lsp: offer quick fixes for misspelled typed handler member access.
- lsp: complete and validate handler member access for locals and arguments
  with shallow resolved struct types.
- lsp: complete in-scope values inside Anchor handlers using parsed local
  bindings, instruction args, imports, constants, and accounts fields.
- lsp: surface unresolved Anchor handler identifiers in the hot diagnostics
  lane so editor Problems update without waiting for full cold analysis.
- lsp: recover account-data member completions for `ctx.accounts.*` and local
  account aliases while the handler contains incomplete dot access.
- lsp: flag obvious unresolved lowercase identifiers in Anchor handler bodies
  using a shallow parsed local scope.
- lsp: use reachable split-file helper evidence for signer authorization
  diagnostics and manual signer checks.
- lsp: detect modular native Solana crates such as
  `solana_program_entrypoint` and `solana_account_info` so single-file
  diagnostics run Solana-wide semantic checks without a manifest.
- lsp: flag native signer authorization gaps for
  `AccountMeta::new_readonly(_, true)` as well as writable signer metas.
- lsp: flag native arbitrary-CPI gaps when dynamic program ids flow through
  `Instruction::new_with_*` constructors.
- lsp: resolve local account aliases when collecting signer checks, signer
  usages, CPI program usages, and token-account unpacking security evidence.
- lsp: wake Anchor completions and tree-sitter context recovery for Anchor v2
  preview imports such as `anchor_lang_v2::prelude`.
- lsp: add a framework context and rule-applicability seam for Anchor,
  Pinocchio, and native Solana diagnostics.
- cli: make diagnostics input errors retryable with exact examples, add layered
  help examples, and support stdin with `--stdin-path` workspace context.
- lsp: property-test account alias diagnostics and preserve the source alias in
  invalid account-data member messages.
- lsp: keep completion wakeups inside handlers whose local variables contain
  `fn`, such as short aliases next to `afn` accounts bindings.
- lsp: complete account-data members after local aliases of `ctx.accounts.*`
  fields, including while the member access is mid-edit.
- lsp: point missing-semicolon parser diagnostics at the unterminated
  statement instead of the following recovery token.
- lsp: resolve imported helper function calls in Anchor constraint expressions
  without accepting unbound call names.
- lsp: build a workspace index for single-file CLI diagnostics so split-file
  account data member mistakes are caught consistently with editor analysis.
- lsp: use declaration-specific related information labels instead of generic
  diagnostic back-reference text.
- lsp: offer scoped seed quick fixes for static-only Anchor PDA diagnostics
  when the accounts context has an account key candidate.
- lsp: resolve `Box<Account<'info, T>>` and nested optional boxed account
  fields to their inner Anchor wrapper so editor diagnostics catch invalid
  account-data member access through local bindings.
- editors: move the bundled feedback link into `editors/feedback.toml` and
  point editor feedback commands at the Seagrass Telegram.
- scripts: resolve generated-support Anchor sources from explicit
  `SEAGRASS_ANCHOR_PATH`, the current checkout, or Cargo's pinned git checkout
  instead of sibling mirrors.
- lsp: offer scoped quick fixes for misspelled Anchor constraint identifiers
  using resolved account fields, instruction args, and const-like values.
- lsp: validate account-data member access through local account bindings such
  as `let position_bundle = &mut ctx.accounts.position_bundle`.
- lsp: link diagnostics and constraint document links to exact Anchor reference
  fragments, including PDA seed/bump and Token-2022 extension anchors.
- lsp: suppress repeated PDA documentation quick fixes once an IDL limitation
  comment is present, and accept `// seagrass-ignore` for line-scoped all-topic
  suppression.
- lsp: navigate from associated constants/functions in Anchor constraint
  expressions to real impl items or generated `InitSpace` owners.
- lsp: include associated constraint constants/functions in references,
  highlights, and safe rename edits.
- lsp: complete in-scope account fields, instruction arguments, and constants
  after partial `constraint = value` expression prefixes.
- lsp: offer quick fixes for misspelled account-data members in Anchor
  constraint expressions.
- lsp: complete real associated constants/functions in Anchor constraint
  expressions, including `State::INIT_SPACE` and workspace impl values.
- lsp: validate associated constants/functions in constraint expressions so
  `Type::FAKE` no longer resolves just because `Type` exists.
- lsp: add semantic constraint expression diagnostics and member completions for
  account fields, instruction args, constants, and workspace account data.
- lsp: add proactive assists for missing companion program accounts, PDA bump
  constraints, canonical PDA seed helpers, safer CPI program account patterns,
  mutated account `mut` constraints, and instruction argument attributes.
- ci: keep the scale benchmark usable when sandboxed hosts block process RSS
  measurement.
- lsp: check in generated Anchor support catalogs and remove build-time parent
  source scraping.
- lsp: add automation-ready JSON diagnostics CLI mode.
- ci: add hotpath, property-test, and fuzz workflow gates for the LSP overlay.
- docs: add bundled agent skills (`skills/seagrass-*`) for Claude Code, Cursor,
  and compatible agents. Covers install, lint, explain, suppress, debug-fp, and
  full audit workflows. Includes fixes to skill docs for accurate CLI JSON shapes.

- build: establish the standalone Seagrass workspace version used by `VERSION`,
  Cargo package metadata, and local editor manifests.
- release: switch release evidence and package workflow tags to the standalone
  `v*` version line.

## [0.1.0] - 2026-05-26

- lsp: initial public launch baseline for Anchor, Pinocchio, and native Solana
  Rust editor support.
