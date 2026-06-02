# Assistant And Automation Commands

Tools that do not speak LSP can use the CLI diagnostics mode. This is the
headless path: no editor UI, no prompts, JSON over stdout, and stable exit
codes.

```sh
cargo run -p seagrass -- diagnostics programs/demo/src/lib.rs --json
```

Agents can also pipe one Rust file through stdin:

```sh
cat programs/demo/src/lib.rs | cargo run -p seagrass -- diagnostics --stdin --stdin-path programs/demo/src/lib.rs --json
```

It prints a JSON array:

```json
[
  {
    "file": "/workspace/programs/demo/src/lib.rs",
    "range": {
      "start": { "line": 5, "character": 15 },
      "end": { "line": 5, "character": 26 }
    },
    "code": "anchor-security-signer",
    "severity": "WARNING",
    "topic": "seagrass/security.signer.authorization",
    "confidence": "authoritative",
    "message": "`authority` is used as a signer account without Anchor signer validation; use `Signer<'info>` or add `#[account(signer)]`."
  }
]
```

The command accepts a Rust file or directory, recurses over `.rs` files for
directories, and exits with code `1` when any diagnostic has ERROR severity.
Usage and input errors exit with code `2` and include a retryable example
invocation. Run `cargo run -p seagrass -- diagnostics --help` for the layered
CLI help.

Use these `workspace/executeCommand` endpoints when an agent needs a compact
semantic snapshot instead of many granular LSP requests.

## Instruction Summary

Command:

```json
{
  "command": "seagrass/instructionSummary",
  "arguments": [
    {
      "uri": "file:///workspace/programs/demo/src/lib.rs",
      "function": "initialize"
    }
  ]
}
```

The `instruction` key is also accepted for compatibility with
`seagrass/analyze`.

Returns:

```json
{
  "name": "initialize",
  "found": true,
  "context": "Create",
  "accounts": [
    {
      "name": "payer",
      "ty": "Signer",
      "constraints": ["account(mut)"],
      "mutability": true,
      "signer": true
    }
  ],
  "args": [{ "name": "amount", "ty": "u64" }],
  "mutates": ["payer"],
  "cpisCalled": ["token_program"],
  "errorsReturned": []
}
```

## Program Report

Command:

```json
{
  "command": "seagrass/programReport",
  "arguments": []
}
```

Returns:

```json
{
  "workspaceRoots": ["file:///workspace"],
  "programs": [],
  "instructions": [],
  "pdas": [],
  "errors": [],
  "idlHash": null
}
```

`programs` is populated from local build artifacts when they exist.
`instructions`, `pdas`, and `errors` are populated from open documents.

## Propose Assists

Command:

```json
{
  "command": "seagrass/proposeAssists",
  "arguments": [
    {
      "uri": "file:///workspace/programs/demo/src/lib.rs"
    }
  ]
}
```

Returns proactive, semantic suggestions that do not require diagnostics:

Current v1 assists:

- `add-system-program-field` adds `system_program` when init-like constraints
  need the System program account.
- `add-token-program-field` adds `token_program` or the explicitly referenced
  token program field for token or mint initialization.
- `add-associated-token-program-field` adds `associated_token_program` for
  associated-token initialization.
- `add-pda-bump-constraint` adds `bump` when PDA seeds lack canonical bump
  validation.
- `add-canonical-seeds-struct` appends a reusable `*Seeds<'a>` helper for PDA
  accounts with supported literal, account-key, and string instruction seeds.
- `add-mut-constraint` adds `mut` when instruction code mutates an account field.
- `add-instruction-args-attribute` adds `#[instruction(...)]` when account
  constraints reference instruction arguments.
- `use-typed-cpi-program-account` replaces an unchecked known CPI program
  account type with the typed Anchor program account.
- `add-cpi-program-executable-constraint` adds `executable` to unknown
  unchecked CPI program accounts.

```json
{
  "uri": "file:///workspace/programs/demo/src/lib.rs",
  "assists": [
    {
      "id": "add-system-program-field",
      "title": "Add Anchor system program account",
      "kind": "refactor",
      "applicability": "machineApplicable",
      "range": {
        "start": { "line": 4, "character": 0 },
        "end": { "line": 8, "character": 1 }
      },
      "hasEdit": true,
      "edit": {},
      "evidence": {
        "accountsStruct": "Create",
        "field": "system_program",
        "reason": "init-like account constraints require the System program account"
      }
    }
  ]
}
```

Use `edit` directly when `applicability` is `machineApplicable`. Treat
`evidence` as the source of truth for why the assist was offered; do not infer
Anchor requirements from account names when the payload has explicit evidence.

Scope and limits:

- Assists are normal LSP `refactor` code actions and also appear through
  `seagrass/proposeAssists`.
- `add-canonical-seeds-struct` only offers helpers for supported seed shapes:
  byte literals, direct account keys, and string instruction arguments.
- CPI safety assists do not generate client artifacts. Known program fields are
  upgraded to typed Anchor program accounts; unknown unchecked CPI programs get
  executable validation.
- Client-side helper generation is intentionally out of v1 until there is a
  dedicated artifact ownership model.

## Example Agent Prompts

Claude Code:

```text
Use the Seagrass executeCommand endpoint seagrass/instructionSummary for
file:///workspace/programs/demo/src/lib.rs and function initialize. Use the JSON
accounts, args, mutates, and cpisCalled fields as the source of truth before
editing the instruction.
```

Cursor:

```text
Ask the LSP for seagrass/programReport, then use the instructions and pdas
arrays to plan account changes. Do not infer Anchor account mutability from
names when the report has explicit constraints.
```

```text
Ask the LSP for seagrass/proposeAssists on the target Rust file before editing.
If it returns a machineApplicable assist, apply the provided edit instead of
hand-writing the structural change.
```

Aider:

```text
Before changing this Anchor program, query seagrass/instructionSummary for the
target function and paste the returned JSON into the edit context. Preserve the
reported account names and constraints unless the code change intentionally
updates them.
```

## Verification

Run:

```sh
bun scripts/protocol-smoke.ts
```

The smoke test verifies command advertisement, `textDocument/codeAction`
refactor assists, `seagrass/proposeAssists` payloads, focused cursor behavior,
and edit refresh. `bun scripts/verify-production.ts` also covers the VS Code and
Zed adapter checks plus the release wasm freshness check.

## Tool-Specific Setup

### Claude Code

Claude Code should load the Seagrass `SKILL.md` folders:

```sh
mkdir -p ~/.claude/skills
for skill in skills/seagrass-*; do
  ln -sfn "$(pwd)/$skill" "$HOME/.claude/skills/$(basename "$skill")"
done
```

Use `skills/README.md` as the catalog. The skills route normal requests to the
CLI diagnostics path above and route false-positive work to minimal repro
fixtures.

### OpenCode

OpenCode should be started at the repository root. The tracked template is
`editors/opencode/opencode.json`; activate it in a checkout with:

```sh
ln -sfn editors/opencode/opencode.json opencode.json
```

The activated root file points OpenCode at `AGENTS.md`, this file, and
`skills/README.md`. That is enough for project rules and Seagrass command
discovery; do not install a separate OpenCode package unless you are building a
custom OpenCode plugin.

### Cursor

Cursor should read `.cursor/rules/seagrass.mdc` as a project rule. The tracked
template is `editors/cursor/rules/seagrass.mdc`; activate it in a checkout with:

```sh
mkdir -p .cursor/rules
ln -sfn ../../editors/cursor/rules/seagrass.mdc .cursor/rules/seagrass.mdc
```

Cursor also supports root `AGENTS.md`, but the `.mdc` rule gives scoped
activation for Rust and Solana manifest files.
