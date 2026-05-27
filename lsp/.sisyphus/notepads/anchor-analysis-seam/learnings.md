# Anchor Analysis Seam — Learnings

## Salsa tracked function return type requirements
- Return type must implement `PartialEq` (for change detection) 
- Return type must implement `salsa::Update` (unsafe trait — use `unsafe impl` with `true` for opaque types)
- These propagate through all struct fields, including Maps and Vecs

## Domain model observations
- `ParsedDocument` already carries rich constraint info via `AnchorSymbols.accounts_structs`
- Each `SymbolRange` field has `account_constraints: Vec<AccountConstraint>` with raw attribute text
- `PdaConstraint` (parser-backed) provides structured seeds/bump info from `anchor-syn`
- `constraint_text` module provides `has_key`, `has_flag`, `has_flag_or_key`, `value_after_key`, `bracket_value` — reused here
- `evidence.rs` has comprehensive `ConstraintEvidence` but doesn't expose structured constraint kinds — fills different need (diagnostic generation)

## Key design decisions
- Model extracts from `ParsedDocument.symbols()` only — no direct tree-sitter access needed
- Constraint text parsed into `ConstraintKind` enum from raw `#[account(...)]` attribute text using `constraint_text` primitives
- PDA seeds from two sources: parser-backed `field.pda_constraint` (preferred) and text-based `extract_pda_seeds` (fallback for `seeds = [...]` in raw constraint text)
- `extract_simple_value` extracts the first identifier after `key = `, stopping at `,`, `)`, `]`, `.`, `:` — prevents path-qualified types from leaking into account references

## Pre-existing issues
- `src/server.rs` has compilation error: `schedule_analysis_and_publish` defined inside `impl Backend` but referenced from `impl LanguageServer for Backend` — needs to be moved or trait definition updated. Not caused by this change.
