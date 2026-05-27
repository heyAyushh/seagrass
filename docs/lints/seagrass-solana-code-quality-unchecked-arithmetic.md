# Unchecked Arithmetic

Topic: `seagrass/solana.code-quality.unchecked-arithmetic`

Rule: `unchecked-arithmetic`

Source: `seagrass`

## What It Catches

Unchecked `+`, `-`, and `*` binary expressions where either side contains a
balance-like identifier such as `amount`, `fee`, `lamport`, `stake`, `supply`,
`total`, `value`, or `withdraw`.

Example:

```rust
fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
```

## What It Does Not Catch

- unary deref, such as `*ptr`
- `#[account(...)]` attribute arguments
- doc comments and string literals
- identifiers that only contain `token`, such as `token_mint_a`
- checked, saturating, or wrapping arithmetic
- broken Rust documents where `syn` cannot prove an expression

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `#[account(address = *token_mint_a.to_account_info().owner)]` | no diagnostic |
| `Ok(*ptr)` | no diagnostic |
| `let message = "token - fee";` | no diagnostic |
| `token_mint_a - other` | no diagnostic |
| `amount.checked_sub(fee)` | no diagnostic |
| `amount - fee` | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.unchecked-arithmetic
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.unchecked-arithmetic
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.unchecked-arithmetic"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.unchecked-arithmetic"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
