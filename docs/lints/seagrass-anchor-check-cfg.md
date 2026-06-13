# Anchor Check Cfg

Topic: `seagrass/anchor.check-cfg`

Source: `seagrass`

## What It Catches

Anchor Cargo feature and Solana `target_os` diagnostics for Rust check-cfg
compatibility.

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
| `seagrass/anchor.check-cfg` appears only in a comment, doc comment, or string literal | no diagnostic |
| an unrelated identifier contains words from this topic | no diagnostic |
| the parsed semantic evidence for this invariant is absent | no diagnostic |
| Cargo.toml already allows `cfg(target_os, values("solana"))` | no diagnostic |
| Solana program evidence exists and Cargo.toml lacks the Solana `target_os` check-cfg entry | diagnostic |
| parsed evidence satisfies anchor check cfg | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/anchor.check-cfg
```

File suppression:

```rust
// seagrass-allow-file: seagrass/anchor.check-cfg
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/anchor.check-cfg"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/anchor.check-cfg"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
