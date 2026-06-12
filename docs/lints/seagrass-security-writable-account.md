# Security Writable Account

Topic: `seagrass/security.writable-account`

Source: `seagrass`

## What It Catches

Native Solana or Pinocchio handlers that pass an account as writable CPI
metadata with `AccountMeta::new(...)`, `InstructionAccount::writable(...)`, or
`InstructionAccount::writable_signer(...)` without a visible writable-account
check in the same function or impl method.

## What It Does Not Catch

- comments, docs, strings, attributes, or macro bodies containing writable text
- readonly CPI metadata such as `AccountMeta::new_readonly(...)`
- validation that lives in a different function and is not visible at the callsite
- Anchor programs using typed account constraints

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| comments and strings containing writable metadata | no diagnostic |
| `AccountMeta::new(...)` without a writable check | diagnostic |
| same function checks `account.is_writable` before CPI metadata | no diagnostic |
| Pinocchio `InstructionAccount::writable_signer(...)` without `is_writable()` | diagnostic |
| Pinocchio same function calls `is_writable()` | no diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/security.writable-account
```

File suppression:

```rust
// seagrass-allow-file: seagrass/security.writable-account
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/security.writable-account"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/security.writable-account"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
