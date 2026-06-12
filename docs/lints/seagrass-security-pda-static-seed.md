# Security Pda Static Seed

Topic: `seagrass/security.pda.static-seed`

Source: `seagrass`

## What It Catches

Fully static Anchor PDA seed lists such as `seeds = [b"vault"]`.

This is a design-review hint, not a definite bug: static seeds are valid for
singleton/global PDAs, but they deserve review because they do not include a
user, authority, mint, or other scoped component.

## What It Does Not Catch

- comments, doc comments, string literals, or unrelated attribute text
- PDA seed lists that include at least one dynamic component such as
  `authority.key().as_ref()`
- claims that the account is exploitable; Seagrass only reports the static seed
  shape

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `seagrass/security.pda.static-seed` appears only in a comment, doc comment, or string literal | no diagnostic |
| an unrelated identifier contains words from this topic | no diagnostic |
| the parsed seed list includes an account key or instruction argument | no diagnostic |
| all parsed seeds are byte/string literals | HINT diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/security.pda.static-seed
```

File suppression:

```rust
// seagrass-allow-file: seagrass/security.pda.static-seed
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/security.pda.static-seed"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/security.pda.static-seed"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
