# PDA Seed Collision

Topic: `seagrass/solana.code-quality.pda-seed-collision`

Attack: `pda-seed-collision`

Source: `seagrass`

## What It Catches

Anchor account constraints with PDA seed lists that use at least two dynamic
seed expressions and no fixed byte-string namespace seed.

Example:

```rust
#[account(seeds = [user.key().as_ref(), mint.key().as_ref()], bump)]
state: AccountInfo<'info>,
```

## What It Does Not Catch

- comments and string literals containing `seeds = [`
- non-Anchor helper text outside parsed account constraints
- seed lists with a static byte-string domain seed such as `b"vault"`
- PDA seed expressions that are not represented as Anchor seed lists
- broken Rust documents where no parsed accounts projection exists

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `// seeds = [user.key().as_ref(), mint.key().as_ref()]` | no diagnostic |
| `"seeds = [user.key().as_ref(), mint.key().as_ref()]"` | no diagnostic |
| `#[account(seeds = [b"vault", user.key().as_ref(), mint.key().as_ref()], bump)]` | no diagnostic |
| `#[account(seeds = [user.key().as_ref(), mint.key().as_ref()], bump)]` | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.pda-seed-collision
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.pda-seed-collision
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.pda-seed-collision"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.pda-seed-collision"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
