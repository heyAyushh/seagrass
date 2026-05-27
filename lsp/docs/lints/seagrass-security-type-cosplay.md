# Security Type Cosplay

Topic: `seagrass/security.type-cosplay`

Code: `anchor-security-type-cosplay`

Rule: `sealevel-attacks/type-cosplay`

Source: `seagrass`

## What It Catches

Unchecked Anchor, native Solana, or Pinocchio accounts whose raw account bytes
are deserialized without visible discriminator or type validation.

Example:

```rust
let user = UserState::try_from_slice(&ctx.accounts.user.data.borrow())?;
```

## What It Does Not Catch

- comments, strings, and attributes containing discriminator text
- safe `try_deserialize` in the same handler
- raw data deserialization after checking `DISCRIMINATOR`
- unrelated safe deserialization in another handler
- typed Anchor accounts that already validate discriminators

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `try_from_slice(&ctx.accounts.user.data.borrow())` | diagnostic |
| `let data = ...; try_from_slice(&data)` without type check | diagnostic |
| same handler checks `UserState::DISCRIMINATOR` | no diagnostic |
| different handler uses `try_deserialize` | diagnostic |
| string/comment containing `discriminator` | diagnostic still fires for real unchecked deserialize |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/security.type-cosplay
```

File suppression:

```rust
// seagrass-allow-file: seagrass/security.type-cosplay
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/security.type-cosplay"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/security.type-cosplay"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
