# Seagrass agent skills

Bundled skills for Claude Code (and compatible agents) to invoke when working
in a project that uses Seagrass.

## What's here

| Skill | Trigger | Purpose |
|---|---|---|
| [`seagrass-install`](./seagrass-install/SKILL.md) | "install seagrass", "set up seagrass", "configure seagrass in vscode/zed/helix/neovim" | Install binary + per-editor config + smoke check |
| [`seagrass-lint`](./seagrass-lint/SKILL.md) | "lint my program", "run seagrass", "show seagrass issues" | Run CLI, parse JSON, triage by severity/topic |
| [`seagrass-explain`](./seagrass-explain/SKILL.md) | "what does this seagrass topic mean", "explain seagrass/...", paste of error message | Look up the lint's authoritative doc |
| [`seagrass-suppress`](./seagrass-suppress/SKILL.md) | "suppress this seagrass warning", "ignore this finding" | Pick narrowest correct suppression scope |
| [`seagrass-debug-fp`](./seagrass-debug-fp/SKILL.md) | "this is a false positive", "seagrass is wrong here" | Reproduce, minimize, write upstream fixture |
| [`seagrass-audit`](./seagrass-audit/SKILL.md) | "audit my program", "production readiness check" | Full project triage + punch list |

## Install for Claude Code

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

## Install for Cursor / other agents

Most agents accept a `skills/` or `agents/` folder at the project root. The
files in this directory follow the standard frontmatter + markdown skill format,
so most clients should discover them with no changes. If your client requires a
specific path, symlink or copy as documented above.

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
