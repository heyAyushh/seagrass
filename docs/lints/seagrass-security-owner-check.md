# Security Owner Check

Topic: `seagrass/security.owner-check`

Code: `anchor-security-owner-check`

Rule: `sealevel-attacks/owner-checks`

Source: `seagrass`

## What It Catches

Unchecked Anchor, native Solana, or Pinocchio accounts whose raw account data is
read without visible owner validation near the read. Native `AccountInfo`
borrows, Pinocchio `AccountInfo`/`AccountView` collections, and Pinocchio
`borrow_data_unchecked()` reads are included.

Example:

```rust
let data = ctx.accounts.user.data.borrow();
```

## What It Does Not Catch

- comments, strings, and attributes containing raw account text
- raw data reads guarded by `#[account(owner = ...)]`
- raw data reads guarded by same-handler owner comparisons
- Pinocchio raw data reads guarded by same-handler `is_owned_by(...)` checks
- unrelated owner helper calls in other handlers
- typed Anchor accounts that already validate ownership

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `ctx.accounts.user.data.borrow()` without owner check | diagnostic |
| alias `let user = &ctx.accounts.user; user.data.borrow()` | diagnostic |
| same handler compares `ctx.accounts.user.owner` | no diagnostic |
| Pinocchio `account.borrow_data_unchecked()` without `is_owned_by(...)` | diagnostic |
| Pinocchio `let [authority, config] = accounts else` then unchecked `config` data | diagnostic |
| Pinocchio `account.is_owned_by(&crate::ID)` before unchecked borrow | no diagnostic |
| different handler calls `assert_owner` | diagnostic |
| string/comment containing `ctx.accounts.user.data.borrow()` | no diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/security.owner-check
```

File suppression:

```rust
// seagrass-allow-file: seagrass/security.owner-check
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/security.owner-check"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/security.owner-check"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
