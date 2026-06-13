# Stale Account After CPI

Topic: `seagrass/solana.code-quality.stale-account-after-cpi`

Attack: `stale-account-after-cpi`

Source: `seagrass`

## What It Catches

Anchor function bodies that build or invoke a CPI and then read
`ctx.accounts.<account>.<field>` without reloading that account after the CPI.

## What It Does Not Catch

- comments or strings containing CPI/read examples
- reloads in unrelated functions
- account reads before the CPI marker
- account reads after `ctx.accounts.<account>.reload()?`
- broken Rust documents where `syn` cannot prove the function body

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| doc comment with `CpiContext::new` | no diagnostic |
| string with `ctx.accounts.vault.amount` | no diagnostic |
| unrelated helper with `.reload()` | does not suppress real stale CPI |
| same function reload before field read | no diagnostic |
| CPI followed by account field read | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.stale-account-after-cpi
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.stale-account-after-cpi
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.stale-account-after-cpi"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.stale-account-after-cpi"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
