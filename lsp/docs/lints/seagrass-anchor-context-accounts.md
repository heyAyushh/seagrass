# Anchor Context Accounts

Topic: `seagrass/anchor.context.accounts`

Source: `seagrass`

## What It Catches

Anchor Context<T> account struct diagnostics.

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
| `seagrass/anchor.context.accounts` appears only in a comment, doc comment, or string literal | no diagnostic |
| an unrelated identifier contains words from this topic | no diagnostic |
| the parsed semantic evidence for this invariant is absent | no diagnostic |
| parsed evidence satisfies anchor context accounts | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/anchor.context.accounts
```

File suppression:

```rust
// seagrass-allow-file: seagrass/anchor.context.accounts
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/anchor.context.accounts"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/anchor.context.accounts"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
