# Bump Seed Canonicalization

Topic: `seagrass/solana.code-quality.bump-seed-canonicalization`

Attack: `bump-seed-canonicalization`

Source: `seagrass`

## What It Catches

Manual PDA derivation through `Pubkey::create_program_address(...)`. That API
accepts any valid bump for a seed tuple, so manual callers must prove they are
using the canonical bump.

Example:

```rust
let address = Pubkey::create_program_address(&[seed, &[bump]], program_id)?;
```

## What It Does Not Catch

- comments, strings, and `#[account(...)]` attribute text
- project helper calls named `create_program_address`
- canonical `Pubkey::find_program_address(...)`
- broken Rust documents where `syn` cannot prove an expression call

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `// Pubkey::create_program_address(...)` | no diagnostic |
| `"Pubkey::create_program_address(...)"` | no diagnostic |
| `#[account(constraint = note == "create_program_address")]` | no diagnostic |
| `helpers::create_program_address(seed)` | no diagnostic |
| `Pubkey::find_program_address(...)` | no diagnostic |
| `Pubkey::create_program_address(...)` | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.bump-seed-canonicalization
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.bump-seed-canonicalization
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.bump-seed-canonicalization"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.bump-seed-canonicalization"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
