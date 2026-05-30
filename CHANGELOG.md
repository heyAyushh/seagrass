# Changelog

All notable Seagrass LSP changes are tracked here. Release automation should
keep future entries aligned with release-plz output.

## [Unreleased]

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
