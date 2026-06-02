---
name: seagrass-explain
description: Explain a Seagrass diagnostic topic — what it catches, what it doesn't, false-positive matrix, and suppression syntax. Use when the user says "what does this seagrass topic mean", "explain seagrass/<...>", "why is seagrass flagging this", "what's the topic for unchecked arithmetic", or pastes a Seagrass error message. Pulls authoritative content from `docs/lints/<topic>.md`.
user-invocable: true
license: MIT
compatibility: Requires access to the seagrass repo's `docs/lints/` directory, or the installed seagrass package's documentation.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-explain

Resolve a Seagrass topic or diagnostic message to its authoritative documentation.

## When to use

The user has hit a Seagrass diagnostic and wants to understand:

- What it catches
- What it does NOT catch (false-positive matrix)
- How to suppress it
- Whether the finding is likely real

## Input

Accept any of:

- A full topic: `seagrass/security.cpi.program`
- A topic suffix: `security.cpi.program` or `cpi.program`
- A diagnostic code: `solana-code-quality.unchecked-arithmetic`
- A natural-language ask: "the one about unchecked arithmetic"
- A raw error message pasted in: `"Use checked arithmetic for balance ..."`

## Resolution

### 1. Canonicalize to topic

If user gave a topic, use it directly. If they gave a fragment, look it up from `docs/topics.json`:

```bash
jq -r --arg q "<query>" '.topics[] | select(.name | test($q; "i")) | .name' docs/topics.json
```

If user gave a free-text description, grep the lint catalog:

```bash
grep -l -i "<phrase>" docs/lints/*.md
```

### 2. Locate the doc

Lint doc filename convention: replace `seagrass/` prefix and dots with dashes.

- `seagrass/security.cpi.program` → `docs/lints/seagrass-security-cpi-program.md`
- `seagrass/solana.code-quality.unchecked-arithmetic` → `docs/lints/seagrass-solana-code-quality-unchecked-arithmetic.md`

The mapping is 1:1: every topic in `docs/topics.json` should have one matching
lint doc in `docs/lints/`. If a topic does not have a matching doc, run
`bun scripts/check-lint-catalog.ts` to detect drift.

If exact filename doesn't exist, fuzzy match:

```bash
ls docs/lints/ | grep -i "<topic-fragment>"
```

### 3. Read and present

Each lint doc has a stable structure:

- **Topic** — full topic string
- **Source** — always `seagrass`
- **What It Catches** — the diagnostic contract
- **What It Does Not Catch** — explicit non-targets (e.g. comments, doc comments, string literals, attribute text)
- **False-Positive Matrix** — fixture-by-fixture table
- **Suppression** — line / file / item-block / workspace forms

Present in that order. Don't paraphrase the matrix — copy the table verbatim. It is the contract.

## Confidence and applicability

Seagrass tags every diagnostic with two extra axes that influence the explanation:

- **Confidence**
  - `Authoritative` — derived from parsed syntax + context; act on it
  - `Derived` — multi-step inference; usually correct
  - `Heuristic` — pattern-based; check before fixing

- **Applicability**
  - `MachineApplicable` — quickfix can be auto-applied
  - `MaybeIncorrect` — quickfix needs review
  - `HasPlaceholders` — needs user input
  - `Unspecified` — no fix offered

If the user's diagnostic carries those metadata fields, surface them — they tell the user whether to act blindly or investigate.

## Topic taxonomy

`seagrass/<framework>.<rest>` — depth varies.

- Some topics are two segments: `seagrass/anchor.syntax`, `seagrass/solana.code-quality`, `seagrass/security.owner-check`.
- Some are three segments: `seagrass/anchor.constraint.shape`, `seagrass/security.account.unchecked`, `seagrass/solana.code-quality.unchecked-arithmetic`.

Top-level frameworks: `anchor`, `solana`, `security` (security cuts across the others).

Authoritative list: `jq -r '.topics[].name' docs/topics.json`. Count it with
`jq '.topics | length' docs/topics.json`; do not hardcode the number.

If the user asks "what topics exist?", read from `docs/topics.json` rather than guessing.

## When the doc is missing or stale

- If `docs/lints/<topic>.md` doesn't exist but the topic does emit, the catalog has drift — flag it. Run `bun scripts/check-lint-catalog.ts` to confirm.
- If the doc exists but reads boilerplate ("this topic catches X" with no specifics), the lint may still be in port. Tell the user the contract is the false-positive matrix, not the prose.

## Stop conditions

- If the topic can't be resolved (no doc, no match in `topics.json`), say so. Don't fabricate a description.
- Don't summarize the false-positive matrix into "it catches X." That's the bug class Seagrass was built to fix.
