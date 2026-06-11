# Solana Code Quality Unchecked Arithmetic

Topic: `seagrass/solana.code-quality.unchecked-arithmetic`

Source: `seagrass`

## What It Catches

Unchecked `+`, `-`, `*`, `/`, `%`, `+=`, `-=`, `*=`, `/=`, and `%=` operations when at least one
operand is proven from parsed syntax to be a lamports value or an SPL token
account amount.

The diagnostic requires semantic evidence from the current function:

- `ctx.accounts.<account>.to_account_info().lamports()` or another parsed
  lamports accessor on a current `Context<T>` account
- `ctx.accounts.<token_account>.amount` where `<token_account>` is a parsed
  `Account<'info, TokenAccount>` or `InterfaceAccount<'info, TokenAccount>`
  field on the current `Context<T>`
- local aliases derived from either of the above

## What It Does Not Catch

- arithmetic on identifiers that merely contain words such as `amount`,
  `balance`, `fee`, or `total`
- `.amount` fields on non-token account data types
- arithmetic expressed through checked methods such as `checked_add`,
  `checked_sub`, `checked_mul`, `checked_div`, or `checked_rem`
- comments, doc comments, string literals, or unrelated attribute text

The editor quick fix rewrites supported expressions to the matching checked
method and returns `ProgramError::ArithmeticOverflow` through a fully qualified
path, without adding imports.

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `total_amount + step` where both values are ordinary instruction arguments | no diagnostic |
| `ctx.accounts.counter.amount + step` where `counter` is `Account<CounterState>` | no diagnostic |
| `counter_ctx.accounts.vault.amount + step` where another context has a token `vault` | no diagnostic |
| `ctx.accounts.vault.amount.checked_sub(fee)` | no diagnostic |
| `ctx.accounts.vault.amount - fee` where `vault` is `Account<TokenAccount>` | diagnostic |
| `ctx.accounts.vault.amount / divisor` where `vault` is `Account<TokenAccount>` | diagnostic |
| `ctx.accounts.vault.amount % divisor` where `vault` is `Account<TokenAccount>` | diagnostic |
| `let amount = vault.amount; amount - fee` where `vault` aliases `ctx.accounts.vault: Account<TokenAccount>` | diagnostic |
| `ctx.accounts.vault.to_account_info().lamports() + extra` | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
let next = ctx.accounts.vault.amount - fee; // seagrass-allow: seagrass/solana.code-quality.unchecked-arithmetic
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.unchecked-arithmetic
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.unchecked-arithmetic"))]
pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
    let next = ctx.accounts.vault.amount - fee;
    Ok(())
}
```

Workspace suppression:

```toml
[lints]
allow = ["seagrass/solana.code-quality.unchecked-arithmetic"]
```
