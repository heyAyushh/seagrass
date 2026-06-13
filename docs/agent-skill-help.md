# Agent Skill Help Process

Seagrass serves agent workflow help from the installed `seagrass` binary so
agents can ask the local tool for the guidance that matches the binary they are
using. This keeps the workflow close to the Vercel-style pattern: discover
topics from the CLI, fetch compact help by default, request full reference text
only when needed, and keep raw repository files as a fallback.

## Contract

The installed CLI is the source a running agent should prefer:

```sh
seagrass skills list --json
seagrass skills get lint
seagrass skills get lint --full
seagrass skills path lint --json
```

The JSON output must include:

- `schemaVersion`
- `binaryVersion`
- `catalogPath`
- topic names, full skill names, descriptions, local paths, and raw fallback URLs
- compact and full commands for every topic

Compact `skills get <name>` output should be short enough to fit in an agent's
active context. `--full` is the progressive-disclosure path for the bundled
`SKILL.md` body.

## When To Update

Update the skill help surface when any of these change:

- a file under `skills/seagrass-*`
- `skills/README.md`
- diagnostic JSON, SARIF, suppression syntax, or exit-code behavior
- suppression surfaces, including comments, attributes, `Seagrass.toml`, or
  `Cargo.toml` metadata such as `[package.metadata.seagrass] suppress = true`
- lint topics, docs URLs, or `docs/lints/`
- `seagrass analyze`, `seagrass diagnostics`, or other agent-facing CLI output
- editor workflow commands that a skill tells an agent to use

## Manual Update Steps

1. Edit the source `skills/seagrass-*/SKILL.md` file first.
2. Update `skills/README.md` so the checkout catalog matches the topic list.
3. Update `src/app/cli/skills.rs` so `SKILL_TOPICS` embeds the same topics and
   exposes accurate compact descriptions.
4. Update `docs/agents.md` if the command shape or agent routing changes.
5. Update this document if the process or output contract changes.
6. Add a `CHANGELOG.md` entry when user-visible guidance or CLI output changes.

Do not add a new topic to only one surface. A topic is complete only when the
checked-in skill, installed CLI output, checkout catalog, and agent docs agree.

## Verification

Run the narrow checks while editing:

```sh
cargo fmt -p seagrass -p seagrass-cli -- --check
cargo test -p seagrass app::cli::skills -- --nocapture
seagrass skills list --json
seagrass skills get lint --full --json
bun test scripts/product-gate.test.ts
```

From a checkout before install, use `cargo run -p seagrass-cli --` before the
`skills` commands.

Before reporting the work done, run the production gate:

```sh
bun scripts/verify-production.ts
```

`scripts/product-gate.test.ts` intentionally checks the installed-CLI skill
discovery contract and this process doc. If a manual doc update drifts from the
binary surface, tighten that guardrail instead of relying on review memory.
