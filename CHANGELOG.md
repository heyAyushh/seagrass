# Changelog

All notable Seagrass LSP changes are tracked here. Release automation should
keep future entries aligned with release-plz output.

## [Unreleased]

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
- lsp: add agent-friendly JSON diagnostics CLI mode.
- ci: add hotpath, property-test, and fuzz workflow gates for the LSP overlay.
- docs: add bundled agent skills (`skills/seagrass-*`) for Claude Code, Cursor,
  and compatible agents. Covers install, lint, explain, suppress, debug-fp, and
  full audit workflows. Includes fixes to skill docs for accurate CLI JSON shapes.

## [0.1.0] - 2026-05-26

- lsp: initial public launch baseline for Anchor, Pinocchio, and native Solana
  Rust editor support.
