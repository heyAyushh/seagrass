# Solana Code Quality Initialization

Topic: `seagrass/solana.code-quality.initialization`

Source: `seagrass`

## What It Catches

Executable unchecked initialization paths, especially
`try_deserialize_unchecked`, when the code can reopen or decode account state
without the normal discriminator guarantees.

## What It Does Not Catch

- strings, comments, attributes, or macro templates containing initialization text
- checked deserialization such as `try_deserialize`
- initialization paths that are already constrained by Anchor account types
- broken Rust documents where `syn` cannot prove the function body

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| string containing `try_deserialize_unchecked` | no diagnostic |
| doc comment mentioning unchecked initialization | no diagnostic |
| checked `try_deserialize` call | no diagnostic |
| executable `try_deserialize_unchecked` call | diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.initialization
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.initialization
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.initialization"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.initialization"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
