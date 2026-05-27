---
name: seagrass-suppress
description: Add a Seagrass diagnostic suppression at the narrowest correct scope. Use when the user says "suppress this seagrass warning", "ignore this finding", "silence seagrass for this line/file/project", "add seagrass-allow", or "this is a false positive, suppress it". Picks the right of four suppression forms (line, file, item/block, workspace) and writes the comment or config.
user-invocable: true
license: MIT
compatibility: Requires write access to the file (or `Seagrass.toml` for workspace scope).
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

## The four suppression forms

Seagrass supports exactly these. No others.

### 1. Line / next-line

```rust
let x = a - b; // seagrass-allow: seagrass/solana.code-quality.unchecked-arithmetic
```

Or on the preceding line:

```rust
// seagrass-allow: seagrass/solana.code-quality.unchecked-arithmetic
let x = a - b;
```

**Use when:** one specific expression triggers the FP. Narrowest scope.

### 2. Item / block attribute

```rust
#[seagrass(allow("seagrass/solana.code-quality.unchecked-arithmetic"))]
fn debit(amount: u64, fee: u64) -> u64 { amount - fee }
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

### 3. File

```rust
// seagrass-allow-file: seagrass/security.account.unchecked
```

Place at the top of the file (before the first item).

**Use when:** the file is intentionally exempt — e.g., a fixture, an unsafe-but-audited helper, a generated module.

### 4. Workspace

`Seagrass.toml` at the workspace root:

```toml
[lints]
allow = [
  "unchecked-arithmetic",
  "security.account.unchecked",
]
```

Topic names in `Seagrass.toml` use the suffix after `seagrass/`. Trailing `.<issue>` only is also accepted (legacy rule code).

**Use when:** an entire crate or workspace genuinely doesn't want a topic — rare, and almost always wrong for security-category topics.

## Choosing scope (always pick the smallest)

```
Line/next-line   →  one expression / one statement
Item/block       →  one function / one field / one struct
File             →  fixture file / generated module / explicit exemption
Workspace        →  whole crate or workspace
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
// seagrass-allow: seagrass/solana.code-quality.unchecked-arithmetic
// reason: bounded by upstream `validate_amount` invariant
let x = a - b;
```

This is for the next reader. Seagrass itself only needs the directive.

## Anti-patterns — refuse these

- Adding a file-scope suppression because "there are many findings" — that hides future regressions. Run `seagrass-debug-fp` for each instead.
- Adding a workspace-scope suppression for a security topic. Always push back.
- Editing `Seagrass.toml` to suppress when only one line triggers.
- Adding a suppression without confirming Seagrass actually emits there — re-check with `seagrass diagnostics` first.

## Stop conditions

- If the user can't tell you whether the finding is a true positive or false positive, route to `seagrass-explain` to read the contract, then `seagrass-debug-fp` to inspect.
- Never suppress more than the user authorized.
