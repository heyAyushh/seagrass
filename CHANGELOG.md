# Changelog

All notable Seagrass LSP changes are tracked here. Release automation should
keep future entries aligned with release-plz output.

## [Unreleased]

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
