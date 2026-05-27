## Learnings

### Macro placement in Rust
- `macro_rules!` cannot be defined inside `impl` blocks — must be at module level
- When a macro needs to reference `self`, pass `$self:expr` as an explicit parameter
- The macro body then uses `$self.field` instead of `self.field`

### Pattern used
- `prune_uri_entries!` with two match arms: `location` (for `entry.location.uri`) and `direct` (for `entry.uri`)
- Reduces 4x (retain + prune) pairs = 22 lines → 6 lines of macro invocations
