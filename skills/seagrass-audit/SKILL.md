---
name: seagrass-audit
description: Run a full Seagrass quality audit on a Solana project — diagnostics by severity and topic, suppression review, and an actionable summary. Use when the user says "audit my program", "full seagrass check", "is my anchor code clean", "review for production", "production readiness check", or "what does seagrass say about all of this". Produces a triaged report and a punch list.
user-invocable: true
license: MIT
compatibility: Requires the `seagrass` binary on PATH and `jq`. Optional: `cargo` for cross-check with `cargo check`.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-audit

Single-pass quality audit. Output is a ranked punch list, not a wall of JSON.

## When to use

- User is about to deploy / publish a Solana program.
- User wants a "production readiness" review of their codebase.
- A new contributor is ramping up and wants to see what's already known.

## What this skill does NOT do

- It does not replace external security review (Trail of Bits / Otter / Halborn etc.).
- It does not run fuzz harnesses.
- It does not run formal verification.
- It does not promise zero false positives — the user must still review.

This skill is the **fast-feedback layer** sitting under a real audit, not a substitute for one.

## Steps

### 1. Scope

Default: the current workspace. Ask before expanding to vendored deps, generated code, or test fixtures.

```bash
# Anchor-style workspace:
LINT_PATHS="programs"

# Single-crate:
LINT_PATHS="src"

# Custom:
LINT_PATHS="<paths>"
```

### 2. Run diagnostics

```bash
seagrass diagnostics "$LINT_PATHS" --json > /tmp/seagrass-audit.json
```

### 3. Summarize

The CLI outputs a top-level JSON *array*, not `{ "diagnostics": [...] }`:

```bash
jq '
  group_by(.severity)
  | map({severity: .[0].severity, count: length, by_topic: (group_by(.topic) | map({topic: .[0].topic, count: length}))})
' /tmp/seagrass-audit.json
```

Build the report as plain text:

```
=== seagrass audit — <project-name> @ <commit-sha-short> ===

ERRORS                       <n>
  topic                                                    count    sample
  seagrass/security.cpi.program                      1    programs/escrow/src/lib.rs:118
  seagrass/security.owner-check                                2    programs/escrow/src/state.rs:42, 67

WARNINGS                     <n>
  ...

INFORMATION                  <n>
  ...

SUPPRESSIONS IN TREE        <n>
  seagrass-allow line directives                              <n>
  seagrass-allow-file directives                              <n>
  Seagrass.toml [lints] entries                               <n>

PUNCH LIST
  1. <topic>: <action> — <file:line>
  2. ...
```

### 4. Audit existing suppressions

Suppressions can drift — what was a true FP may now be fixed upstream, leaving the allow as a permanent blind spot.

```bash
# Count line/next-line suppressions in tree
grep -rnE "seagrass-allow:" --include="*.rs" "$LINT_PATHS" | wc -l

# Count file suppressions
grep -rnE "seagrass-allow-file:" --include="*.rs" "$LINT_PATHS" | wc -l

# Workspace suppressions
test -f Seagrass.toml && cat Seagrass.toml
```

For each suppression, surface the topic and prompt the user: "Is this still needed?" Cold suppressions are bug habitats.

### 5. Build the punch list

For each finding, pick exactly one of:

- **Fix** — code edit. Show diff.
- **Suppress** — route to `seagrass-suppress`.
- **Debug FP** — route to `seagrass-debug-fp`.
- **Defer** — log with rationale.

Order by:
1. Severity (errors first).
2. Confidence (`Authoritative` first).
3. Security topics before code-quality topics.
4. Within the same topic, by file path for locality.

### 6. Cross-check

Optionally run:

```bash
cargo check --workspace          # baseline compile
cargo test --workspace --no-run  # build tests
```

A Seagrass clean run on code that doesn't compile is meaningless.

### 7. Report and stop

Produce the report. Do **not** auto-apply fixes. Hand the punch list back to the user and let them decide what to act on.

## Outputs

Save the report to `seagrass-audit-<YYYY-MM-DD>.md` if the user asks for a persistent artifact. Otherwise just print to the chat.

## Anti-patterns — refuse these

- Running on `target/`, `node_modules/`, or vendored deps — those are not the user's code.
- "Pass everything in CI" gate without triaging — pushes the user toward blanket suppressions.
- Auto-applying any code change. Audit is read-only output until the user picks an item.

## Stop conditions

- If `seagrass diagnostics` returns a non-JSON error, route to `seagrass-install` and stop.
- If the count of findings is enormous (> 200), do NOT print all of them — print the top 20 by severity/confidence and offer to drill into specific topics.
- Never edit `Seagrass.toml` from this skill. That belongs to `seagrass-suppress` with explicit user consent.
