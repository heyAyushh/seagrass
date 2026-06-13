# Instruction Data Bounds

Topic: `seagrass/solana.code-quality.instruction-data-bounds`

Attack: `instruction-data-bounds`

Source: `seagrass`

## What It Catches

Native Solana or Pinocchio handlers that index instruction data or borrowed
account data without a visible length or checked-access guard in the same
function.

Example:

```rust
let tag = instruction_data[0];
```

## What It Does Not Catch

- comments, docs, strings, and attributes containing indexed-data text
- helper validation that lives in another function
- unrelated local vectors named `data`
- range slicing used only for fixed-size `try_into().unwrap()` conversions
- Anchor programs using generated account and instruction validation

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| `instruction_data[0]` without a local guard | diagnostic |
| helper function validates instruction data, handler does not | diagnostic |
| same function calls `validate_instruction_data` first | no diagnostic |
| doc/comment/string containing `instruction_data[0]` | no diagnostic |
| local `let data = vec![..]; data[0]` | no diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.instruction-data-bounds
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.instruction-data-bounds
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.instruction-data-bounds"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.instruction-data-bounds"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
