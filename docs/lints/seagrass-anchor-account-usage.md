# Anchor Account Usage

Topic: `seagrass/anchor.account.usage`

Source: `seagrass`

## What It Catches

Anchor account mutability, account-data member usage, and shallow handler scope
diagnostics. `AccountLoader<'info, T>` fields are treated as loader values until
Anchor's zero-copy `load`, `load_mut`, or `load_init` methods produce `T`.

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
| `seagrass/anchor.account.usage` appears only in a comment, doc comment, or string literal | no diagnostic |
| an unrelated identifier contains words from this topic | no diagnostic |
| the parsed semantic evidence for this invariant is absent | no diagnostic |
| a bare lowercase value in an Anchor handler is not an argument, local binding, import, or item | diagnostic |
| a call name is unresolved but the arguments are valid | no diagnostic |
| parsed evidence satisfies anchor account usage | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/anchor.account.usage
```

File suppression:

```rust
// seagrass-allow-file: seagrass/anchor.account.usage
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/anchor.account.usage"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/anchor.account.usage"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
