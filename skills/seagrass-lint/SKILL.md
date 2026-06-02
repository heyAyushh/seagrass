---
name: seagrass-lint
description: Run Seagrass diagnostics on a Solana program file or workspace and surface findings ranked by severity and topic. Use when the user says "lint my program", "run seagrass", "check my anchor code", "show seagrass issues", "find security problems", or "what's wrong with this account struct". Parses JSON output, groups by topic, and recommends suppression vs fix per finding.
user-invocable: true
license: MIT
compatibility: Requires the `seagrass` binary on PATH and `jq`.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-lint

Run Seagrass against a file, directory, or whole workspace, and present a useful triaged report.

## When to use

The user wants to know what Seagrass says about their code. The current file, the current directory, or the whole workspace.

## What to lint

Default scope when the user doesn't specify:

1. If `pwd` is inside a crate (Cargo.toml present), lint that crate's `src/`.
2. If `pwd` is at a workspace root, lint every `programs/*/src/` (Anchor layout) or every `src/` of every workspace member.
3. If the user is asking about a specific file (visible in the conversation or selected in their editor), lint just that file.

Always ask before linting more than the user implied — running on a 50-file workspace produces a lot of output.

## Run

```bash
seagrass diagnostics <path> --json | jq .
```

Path can be a single file or a directory. Directory walks `.rs` files.
For piped source, preserve workspace context with `--stdin-path`:

```bash
cat programs/demo/src/lib.rs | seagrass diagnostics --stdin --stdin-path programs/demo/src/lib.rs --json | jq .
```

If the command is missing an input, points at a non-Rust file, or finds an empty
directory, it exits with usage code `2` and prints a retryable example command.

## Output shape

The CLI emits a JSON array (not an object wrapper):

```json
[
  {
    "file": "/path/to/file.rs",
    "range": { "start": { "line": 12, "character": 4 }, "end": { "line": 12, "character": 28 } },
    "severity": "WARNING",
    "code": "solana-code-quality.unchecked-arithmetic",
    "message": "Use checked arithmetic for balance, lamport, token, or amount math in Solana program code.",
    "topic": "seagrass/solana.code-quality.unchecked-arithmetic",
    "confidence": "heuristic",
    "applicability": "Unspecified",
    "docsUrl": "https://github.com/heyAyushh/seagrass/blob/main/docs/lints/seagrass-solana-code-quality-unchecked-arithmetic.md"
  }
]
```

`severity` is uppercase (ERROR / WARNING / INFORMATION / HINT). `confidence`
is lower-case (`authoritative`, `derived`, `heuristic`). `topic`, `confidence`,
`applicability`, and `docsUrl` are the primary fields for triage. `code` is the
legacy short identifier.

For GitHub code scanning or review surfaces, use SARIF:

```bash
seagrass diagnostics <path> --sarif > seagrass.sarif
```

## How to present results

Group and rank:

1. **By severity** — errors first, then warnings, then information, then hints.
2. **Within severity, by topic** — collapse duplicates of the same topic into "N findings of <topic>".
3. **Confidence-aware** — call out `confidence: "authoritative"` findings before `"heuristic"` ones.

Sample summary:

```
seagrass found 7 findings in programs/escrow/

ERRORS
  seagrass/security.cpi.program  (1)
    src/lib.rs:118  — CPI to a Pubkey not validated against a known program ID

WARNINGS
  seagrass/solana.code-quality.unchecked-arithmetic  (3)
    src/state.rs:42, 47, 53
  seagrass/anchor.constraint.shape              (1)
    src/lib.rs:201  — has_one target field type mismatch

INFORMATION
  seagrass/anchor.init.missing-payer  (2)
    src/lib.rs:88, 134  — #[account(init, ...)] without payer
```

## Recommend next actions per finding

For each finding, suggest one of three paths:

- **Genuine bug** → quickfix or manual fix. Show the diff if it's a one-line change.
- **False positive** → route to `seagrass-debug-fp` skill (or, if user accepts the FP, route to `seagrass-suppress`).
- **Unknown topic** → route to `seagrass-explain` with the topic string.

Do not propose code changes blindly. Show the offending line first.

## Filtering

Common asks the user may make (note: top-level array, so use `jq '.[] | ...'` or `jq 'map(select(...))'`):

- "only security" → `jq '.[] | select(.topic | startswith("seagrass/security."))'`
- "only errors" → `jq '.[] | select(.severity == "ERROR")'`
- "exclude X topic" → `jq '.[] | select(.topic != "<topic>")'`
- "just this file" → run `seagrass diagnostics <single-file>.rs`
- "lint piped source" → run `cat <single-file>.rs | seagrass diagnostics --stdin --stdin-path <single-file>.rs --json`

## Stop conditions

- If the binary isn't found, route to `seagrass-install`.
- If JSON parsing fails, re-run with `RUST_LOG=debug` and surface raw output. Do not retry blindly.
- If the user asks to "fix everything," refuse and propose triage. Auto-fix without per-finding review is unsafe.
