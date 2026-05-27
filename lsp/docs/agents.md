# Agent Commands

Agents that do not speak LSP can use the CLI diagnostics mode:

```sh
cargo run -p seagrass -- diagnostics --json programs/demo/src/lib.rs
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
bun lsp/scripts/protocol-smoke.ts
```

The smoke test verifies both commands are advertised and return agent-usable
JSON.
