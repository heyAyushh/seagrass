---
name: seagrass-debug-fp
description: Investigate a Seagrass false positive — confirm it's actually false, identify the rule and region that misfired, and produce a minimal regression fixture for upstream. Use when the user says "seagrass is wrong here", "this is a false positive", "why is seagrass flagging valid code", "debug this seagrass diagnostic", or "file a seagrass bug". Outputs a reproducible fixture and a draft upstream issue body.
user-invocable: true
license: MIT
compatibility: Requires the `seagrass` binary on PATH and write access to a scratch file.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-debug-fp

When the user is convinced Seagrass is wrong, run a disciplined check: confirm the FP, minimize the reproducer, identify which rule and which region misfired, and produce a fixture suitable for an upstream regression test.

## When to use

- A Seagrass diagnostic fires on code the user is certain is correct.
- A diagnostic fires inside `#[account(...)]`, doc comments, string literals, or other non-executable regions.
- A diagnostic fires because an identifier *contains* a sensitive substring (e.g., `Tokenizer` flagged for "token").
- A diagnostic fires twice at the same span/topic.

These are the four historical FP root causes Seagrass was designed to eliminate. Each maps to a fixture pattern.

## Workflow

### 1. Confirm the FP is real

Ask: "Did you read the topic's false-positive matrix?" If no, route to `seagrass-explain` first. The matrix is the contract — if the row says "no diagnostic" but Seagrass emits, that is a confirmed FP.

If the row says "diagnostic" for the user's pattern, it's NOT an FP; route to `seagrass-suppress` (accepted risk) or fix.

### 2. Reproduce in isolation

Copy the smallest snippet that reproduces the FP into a scratch file:

```bash
cat > /tmp/seagrass-fp-repro.rs <<'EOF'
<minimal code>
EOF
seagrass diagnostics /tmp/seagrass-fp-repro.rs --json | jq '.[] | {topic, message, range}'
```

Iterate by deleting code until the FP disappears, then add back the minimum that brings it back. This is the regression fixture.

### 3. Classify the failure mode

Ask which historical bug class this is:

| Class | Signal | Example |
|---|---|---|
| Region | Fires inside attribute / comment / string | `#[account(address = *x.owner)]` flagged as arithmetic |
| Substring | Fires because identifier contains keyword | `tokenizer.next()` flagged as token math |
| AST shape | Fires on syntactic shape that isn't the semantic shape | unary deref `*x` mistaken for `x * y` |
| Duplicate | Two diagnostics at the same (span, topic) | two findings for one Anchor field |

Most FPs are Region or Substring.

### 4. Identify the rule and region

From the JSON, the `code` field tells you which rule. From the source code, the rule lives under `src/lsp/diagnostics/` — typically in the `code_quality/` or `security/` module. Identify which `Visit` method the rule implements (`visit_expr_binary`, `visit_expr_method_call`, etc.).

If the FP is region-based, the rule should be skipping `visit_attribute`, `visit_lit_str`, `doc_comment`, etc. — check whether it is.

### 5. Produce the fixture for upstream

The fixture is a Rust test in the rule's test file. Convention:

```rust
#[test]
fn ignores_<scenario>_<context>() {
    let source = r#"
<minimal repro>
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    assert!(!diagnostics.iter().any(|d| d.code.as_deref() == Some("<offending-code>")));
}
```

Place it in `src/lsp/diagnostics/code_quality/tests/` (for code_quality rules) or the equivalent module. The existing fixtures `ignores_deref_in_account_attribute`, `ignores_token_word_inside_unrelated_identifier`, etc. are the model.

### 6. Suppress locally so the user is unblocked

While the upstream fix is in flight, route to `seagrass-suppress` to add the narrowest local allow with a comment linking to the issue:

```rust
// seagrass-allow: <topic>
// FP: tracking at https://github.com/heyAyushh/seagrass/issues/<N>
```

### 7. (Optional) Draft an upstream issue body

If the user is in VS Code, `Seagrass: Report False Positive` already copies the
diagnostic topic, confidence, applicability, quickfix, docs URL, source range,
and source line. Use that report as the starting point, then add the minimized
fixture below.

```markdown
**Topic:** seagrass/<topic>
**Class:** Region | Substring | AST shape | Duplicate
**Repro:**

```rust
<minimal source>
```

**Expected:** no diagnostic — false-positive matrix row says <which row>.
**Actual:** Seagrass emits at <range>: "<message>".

**Suggested fixture** (drop into `src/lsp/diagnostics/code_quality/tests/` or the rule's test module):

```rust
<the assert above>
```
```

Print this for the user to paste into a GitHub issue at `heyAyushh/seagrass`. Do not open the issue automatically.

## Don't do these

- Don't ask the user to "just fix the code" if it really is a Seagrass FP. The contract is the matrix.
- Don't propose a code change to Seagrass itself from this skill. Producing a fixture and issue body is the boundary. Upstream review handles the rule fix.
- Don't minimize away the framework context. A fixture that drops `#[derive(Accounts)]` may stop reproducing because the rule depends on Anchor parse state — keep enough scaffolding that the rule's visitor actually runs.

## Stop conditions

- If three minimization attempts don't shrink the repro, stop and present what you have. Over-minimization is worse than a 30-line fixture.
- If the FP only reproduces on the user's full project, say so — workspace-dependent FPs need different reproduction (paste the relevant `Cargo.toml`, `Anchor.toml`, and module structure).
