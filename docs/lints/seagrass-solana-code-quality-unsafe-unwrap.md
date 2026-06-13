# Unsafe Unwrap

Topic: `seagrass/solana.code-quality.unsafe-unwrap`

Rule: `unsafe-unwrap`

Source: `seagrass`

## What It Catches

Solana program code that calls `.unwrap()` or `.expect()` in executable Rust
expressions. Panics abort the instruction instead of returning a typed program
error.

Example:

```rust
let value = maybe_value.unwrap();
```

## What It Does Not Catch

- comments, docs, strings, and attributes containing unwrap text
- fixed byte-slice conversions such as `data[0..8].try_into().unwrap()`
- broken Rust documents where `syn` cannot prove the method call

## False-Positive Matrix

| fixture | expected |
| --- | --- |
| executable `maybe_value.unwrap()` | diagnostic |
| executable `maybe_value.expect("value")` | diagnostic |
| doc/comment/string containing `.unwrap()` | no diagnostic |
| attribute containing `.unwrap()` text | no diagnostic |
| fixed slice `try_into().unwrap()` | no diagnostic |

## Suppression

Use the narrowest suppression form that matches the false positive.

Line or next-line suppression:

```rust
// seagrass-allow: seagrass/solana.code-quality.unsafe-unwrap
```

File suppression:

```rust
// seagrass-allow-file: seagrass/solana.code-quality.unsafe-unwrap
```

Whole-file suppression (all Seagrass diagnostics, leading file comment only):

```rust
// seagrass-ignore-file
```

Item or block suppression:

```rust
#[seagrass(allow("seagrass/solana.code-quality.unsafe-unwrap"))]
{
  // diagnostic scope
}
```

Workspace suppression in `Seagrass.toml`:

```toml
[lints]
allow = ["seagrass/solana.code-quality.unsafe-unwrap"]
```

Project suppression in `Cargo.toml` (all Seagrass diagnostics):

```toml
[package.metadata.seagrass]
suppress = true
```

Prefer fixing the underlying Anchor or Solana invariant when the diagnostic has
enough evidence to point at a concrete issue.
