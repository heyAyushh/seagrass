# Seagrass Skills

Bundled `SKILL.md` workflows for Claude Code, plus CLI/LSP routing notes for
OpenCode, Cursor, Codex, Aider, CI, and plain-shell automation.

## What's here

| Skill | Trigger | Purpose |
|---|---|---|
| [`seagrass-install`](./seagrass-install/SKILL.md) | "install seagrass", "set up seagrass", "configure seagrass in vscode/zed/helix/neovim" | Install binary + per-editor config + smoke check |
| [`seagrass-lint`](./seagrass-lint/SKILL.md) | "lint my program", "run seagrass", "show seagrass issues" | Run CLI, parse JSON, triage by severity/topic |
| [`seagrass-explain`](./seagrass-explain/SKILL.md) | "what does this seagrass topic mean", "explain seagrass/...", paste of error message | Look up the lint's authoritative doc |
| [`seagrass-suppress`](./seagrass-suppress/SKILL.md) | "suppress this seagrass warning", "ignore this finding" | Pick narrowest correct suppression scope |
| [`seagrass-debug-fp`](./seagrass-debug-fp/SKILL.md) | "this is a false positive", "seagrass is wrong here" | Reproduce, minimize, write upstream fixture |
| [`seagrass-audit`](./seagrass-audit/SKILL.md) | "audit my program", "production readiness check" | Full project triage + punch list |

## Claude Code

Symlink the skills into Claude's global skills directory:

```bash
mkdir -p ~/.claude/skills
for skill in skills/seagrass-*; do
  ln -sfn "$(pwd)/$skill" "$HOME/.claude/skills/$(basename $skill)"
done
```

Claude Code picks them up on next session. Verify:

```bash
ls ~/.claude/skills/seagrass-*
```

Each will then trigger via `/seagrass-<name>` or natural-language phrasing
matching the skill's description.

You can also install them project-locally under `.claude/skills`.

## OpenCode

Use the tracked template at `editors/opencode/opencode.json`. Activate it from
the repository root:

```bash
ln -sfn editors/opencode/opencode.json opencode.json
```

The activated root file points OpenCode at `AGENTS.md`, `docs/agents.md`, and
this catalog. That gives OpenCode the CLI commands and workflow entry points
without copying every skill body into the default prompt.

Create a custom OpenCode package only if you need plugin behavior beyond
instructions, such as new tools, permissions, or slash commands.

## Cursor

Cursor should use the tracked template at `editors/cursor/rules/seagrass.mdc`.
Activate it from the repository root:

```bash
mkdir -p .cursor/rules
ln -sfn ../../editors/cursor/rules/seagrass.mdc .cursor/rules/seagrass.mdc
```

Cursor's current project-rule format is `.cursor/rules/*.mdc`; root `AGENTS.md`
remains a simple fallback, but the Seagrass rule is scoped to Rust and Solana
manifest files.

## Codex, Aider, CI, And Plain Shells

Use the CLI directly:

```bash
seagrass diagnostics <path> --json
cat <file>.rs | seagrass diagnostics --stdin --stdin-path <file>.rs --json
```

Use `docs/agents.md` when an LSP bridge can call Seagrass execute-command
endpoints.

## Workflow

A typical user session uses the skills in order:

1. **`seagrass-install`** — once, on setup.
2. **`seagrass-lint`** or **`seagrass-audit`** — to surface findings.
3. **`seagrass-explain`** — when a topic is unfamiliar.
4. **`seagrass-suppress`** or **`seagrass-debug-fp`** — per finding.

Skills route to each other where useful — for example, `seagrass-debug-fp`
routes back to `seagrass-suppress` once a fixture is produced, so the user is
unblocked while the upstream rule fix lands.

## Not provided

These skills cover end-user workflows. They do **not** cover:

- Internal contributor workflows (rule porting, catalog regen, release-plz
  publishing). Those live in `CONTRIBUTING.md` and `scripts/`.
- Auto-fix of diagnostics. Seagrass quickfixes flow through the LSP
  `textDocument/codeAction` channel; agents should drive those through the
  editor's LSP integration, not through these CLI-oriented skills.

## License

MIT, same as the rest of Seagrass.
