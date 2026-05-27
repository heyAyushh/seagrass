# Security Signer Authorization

Topic: `seagrass/security.signer.authorization`

Source: `seagrass`

## What It Catches

Native Solana or Pinocchio handlers that mark an account as a signer in
`AccountMeta::new(..., true)` without visible signer authorization in the same
function.

## What It Does Not Catch

- comments, docs, strings, attributes, or macro bodies containing signer text
- signer metadata followed by `invoke_signed`
- `AccountMeta::new(..., false)` account metadata
- validation that lives in a different function and is not visible at the callsite
- Anchor programs using typed account validation

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| comments and strings containing `AccountMeta::new` | no diagnostic |
| helper function validates signer, handler does not | diagnostic |
| same function calls signer validator before CPI | no diagnostic |
| `AccountMeta::new(..., false)` with unrelated `true` variable | no diagnostic |
| signer PDA metadata used with `invoke_signed` | no diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/security.signer.authorization
```

File suppression:

```rust
// seagrass-allow-file: seagrass/security.signer.authorization
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/security.signer.authorization"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/security.signer.authorization"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
