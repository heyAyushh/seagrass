# Learnings: Extract Accounts Family from mod.rs

## Pattern Confirmed
- Thin router + per-family `code_actions` seam works identically to `instructions.rs` pattern
- 26 accounts functions + 4 local helpers extracted from 5520→4130 LOC in mod.rs
- 487 tests all passing with no signature changes

## Shared Helpers Moved to common.rs
- `eof_range`, `add_constraint_edit`, `add_constraint_to_field_edit`, `constraint_action`, `edit_distance`, `field_indent`, `account_struct_closing_line`
- `edit_distance` changed from `pub(crate)` to `pub` and re-exported via `mod.rs` for `constraint_shape.rs` external consumer
- `field_indent` needed type annotation `|line: &str|` after move to avoid inference issues

## Duplicate Code Removed
- `instruction_argument_*` function duplicates (8 functions at lines 893-1085) were already in `instructions.rs` - removed from mod.rs
- `struct_line` local copy (line 2681) removed - now only in common.rs

## Issues
- Python heredoc approach fails for large Rust files due to JSON escaping - used Python scripts instead
- Extra `}` left on line 1330 after extraction - needed manual fix
