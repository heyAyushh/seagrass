# Solana Code Quality Account Closing

Topic: `seagrass/solana.code-quality.account-closing`

Source: `seagrass`

## What It Catches

Manual account close logic in executable code when lamports are drained and the
account is reassigned, reallocated to zero, or zero-filled without a closed
discriminator invariant in the same function body.

## What It Does Not Catch

- comments, strings, attributes, and macro templates containing close text
- Anchor `close = ...` attributes unrelated to executable close logic
- functions that write or check `CLOSED_ACCOUNT_DISCRIMINATOR`
- broken Rust documents where `syn` cannot prove the function body

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| doc comment with `try_borrow_mut_lamports` and `assign` | no diagnostic |
| `#[account(close = receiver)]` elsewhere in file | no diagnostic |
| function writes `CLOSED_ACCOUNT_DISCRIMINATOR` before close | no diagnostic |
| manual lamport drain plus `assign(&system_program::ID)` | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.account-closing
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.account-closing
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.account-closing"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.account-closing"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
