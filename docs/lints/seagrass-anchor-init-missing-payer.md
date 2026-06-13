# Anchor Init Missing Payer

Topic: `seagrass/anchor.init.missing-payer`

Source: `seagrass`

## What It Catches

Anchor init constraints missing payer evidence.

Seagrass should emit this topic only when parsed Anchor, Solana, workspace, or
artifact evidence proves this specific invariant. The diagnostic must not be
derived from raw substring matches.

## What It Does Not Catch

- comments, doc comments, string literals, or unrelated attribute text
- examples where the required semantic evidence is absent or ambiguous
- project states outside this topic's diagnostic contract

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `seagrass/anchor.init.missing-payer` appears only in a comment, doc comment, or string literal | no diagnostic |
| an unrelated identifier contains words from this topic | no diagnostic |
| the parsed semantic evidence for this invariant is absent | no diagnostic |
| parsed evidence satisfies anchor init missing payer | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/anchor.init.missing-payer
```

File suppression:

```rust
// seagrass-allow-file: seagrass/anchor.init.missing-payer
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/anchor.init.missing-payer"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/anchor.init.missing-payer"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
