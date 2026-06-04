# Security CPI Program

Topic: `seagrass/security.cpi.program`

Source: `seagrass`

## What It Catches

Native Solana or Pinocchio handlers that build a CPI `Instruction` or
Pinocchio `InstructionView` from a dynamic `program_id` without visible
program-id validation before `invoke`.

## What It Does Not Catch

- comments, docs, strings, attributes, or macro bodies containing CPI text
- `Instruction` values using static SDK ids such as `system_program::ID`
- SDK-built instructions passed to `invoke`
- validation that lives in a different function and is not visible at the callsite
- Anchor programs using typed `Program<'info, T>` or `Interface<'info, T>` accounts

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| comments and strings containing `Instruction { program_id }` | no diagnostic |
| helper function validates program id, handler does not | diagnostic |
| Pinocchio `InstructionView { program_id, ... }` without validation | diagnostic |
| same function validates dynamic program id before invoke | no diagnostic |
| `Instruction { program_id: system_program::ID, ... }` | no diagnostic |
| SDK-built instruction passed to `invoke` | no diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/security.cpi.program
```

File suppression:

```rust
// seagrass-allow-file: seagrass/security.cpi.program
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/security.cpi.program"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/security.cpi.program"]
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
