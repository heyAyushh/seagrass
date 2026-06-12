---
name: seagrass-suppress
description: Add a Seagrass diagnostic suppression at the narrowest correct scope. Use when the user says "suppress this seagrass warning", "ignore this finding", "silence seagrass for this line/file/project", "add seagrass-allow", or "this is a false positive, suppress it". Picks the right suppression form (line, item/block, file, workspace, or Cargo project switch) and writes the comment or config.
user-invocable: true
license: MIT
compatibility: Requires write access to the file, `Seagrass.toml`, or `Cargo.toml`.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-suppress

Suppress a Seagrass diagnostic at the narrowest scope that resolves the false positive without hiding genuine future findings.

## When to use

The user wants Seagrass to stop emitting a specific topic at a specific location, OR they've confirmed a finding is a false positive (or accepted-risk).

**Before suppressing, ask once:** "Is this a real finding you're accepting, or a Seagrass false positive?" Route accordingly:

- **Real finding, accepted** → suppress, and recommend leaving a comment line explaining why.
- **False positive** → suppress AND route to `seagrass-debug-fp` so the upstream rule gets a regression fixture.

## The seven suppression forms

Seagrass supports exactly these. No others.

### 1. Line / next-line

```rust
let x = maybe_value().unwrap(); // seagrass-allow: seagrass/solana.code-quality.unsafe-unwrap
```

Or on the preceding line:

```rust
// seagrass-allow: seagrass/solana.code-quality.unsafe-unwrap
let x = maybe_value().unwrap();
```

**Use when:** one specific expression triggers the FP. Narrowest scope.

### 2. Line / next-line, all topics

```rust
// seagrass-ignore
let x = a - b;
```

**Use when:** a local, documented exception intentionally accepts every Seagrass diagnostic on one line. Prefer `seagrass-allow:` when the topic is known.

### 3. Item / block attribute

```rust
#[seagrass(allow("seagrass/solana.code-quality.unsafe-unwrap"))]
fn required_value(input: Option<u64>) -> u64 { input.unwrap() }
```

```rust
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[seagrass(allow("seagrass/anchor.constraint.shape"))]
    #[account(has_one = authority)]
    pub config: Account<'info, Config>,
}
```

**Use when:** the FP is structural to one function, struct, or field. Multiple lines, single semantic unit.

### 4. File

```rust
// seagrass-allow-file: seagrass/security.account.unchecked
```

Place at the top of the file (before the first item).

**Use when:** the file is intentionally exempt — e.g., a fixture, an unsafe-but-audited helper, a generated module.

### 5. Whole file, all topics

```rust
// seagrass-ignore-file
```

Place in the leading file comment/header before the first Rust item.

**Use when:** the whole file is outside Seagrass's useful scope. Prefer `seagrass-allow-file:` when the topic is known.

### 6. Workspace topic allow-list

`Seagrass.toml` at the workspace root:

```toml
[lints]
allow = [
  "unsafe-unwrap",
  "security.account.unchecked",
]
```

Topic names in `Seagrass.toml` use the suffix after `seagrass/`. Trailing `.<issue>` only is also accepted (legacy rule code).

**Use when:** an entire crate or workspace genuinely doesn't want a topic — rare, and almost always wrong for security-category topics.

### 7. Cargo project switch, all topics

Package-level `Cargo.toml`:

```toml
[package.metadata.seagrass]
suppress = true
```

Workspace-root `Cargo.toml`:

```toml
[workspace.metadata.seagrass]
suppress = true
```

**Use when:** a package or workspace is intentionally out of Seagrass scope. This turns off all Seagrass diagnostics for files under that manifest, so push back unless the user explicitly asks for project-wide silence.

## Choosing scope (always pick the smallest)

```
Line/next-line       →  one expression / one statement
Line all-topic       →  one documented exception where every topic is accepted
Item/block           →  one function / one field / one struct
File                 →  fixture file / generated module / explicit exemption
Whole file all-topic →  generated or external file intentionally out of scope
Workspace topic      →  whole crate or workspace topic allow-list
Cargo project        →  whole package or workspace all-topic disable
```

If the user asks for a broader scope than the FP actually requires, push back: "This will silence Seagrass everywhere — are you sure? A line-scoped allow would also resolve this finding."

## What to write

1. Locate the diagnostic (line + topic from `seagrass diagnostics`).
2. Pick the scope per above.
3. Insert the comment or attribute. Show the diff before writing.
4. Re-run `seagrass diagnostics <file>` and confirm the topic no longer fires at that location.

### Diff before writing — example

```
+ // seagrass-allow: seagrass/anchor.constraint.shape
  #[account(has_one = authority)]
  pub config: Account<'info, Config>,
```

Ask: "Apply?"

## After suppression

Add a `WHY` comment line if it isn't obvious from context:

```rust
// seagrass-allow: seagrass/solana.code-quality.unsafe-unwrap
// reason: initialization guarantees this option is present
let x = maybe_value().unwrap();
```

This is for the next reader. Seagrass itself only needs the directive.

## Anti-patterns — refuse these

- Adding a file-scope suppression because "there are many findings" — that hides future regressions. Run `seagrass-debug-fp` for each instead.
- Adding a workspace-scope suppression for a security topic. Always push back.
- Adding `[package.metadata.seagrass] suppress = true` when a topic allow-list or file suppression would work.
- Editing `Seagrass.toml` to suppress when only one line triggers.
- Adding a suppression without confirming Seagrass actually emits there — re-check with `seagrass diagnostics` first.

## Stop conditions

- If the user can't tell you whether the finding is a true positive or false positive, route to `seagrass-explain` to read the contract, then `seagrass-debug-fp` to inspect.
- Never suppress more than the user authorized.
