![Seagrass](docs/assets/readme-banner.png)

# Seagrass

Codebase intelligence for Solana programs — accounts, CPIs, PDAs, artifacts.
LSP + CLI.
rust-analyzer → Rust. Seagrass → Solana. Output: editor · JSON · SARIF.
Unofficial · [`otter-sec/anchor`](https://github.com/otter-sec/anchor) pin.

## Install

`rust-toolchain.toml` · MSRV `1.89.0`

```sh
cargo install --path crates/seagrass --locked && seagrass --version
bash scripts/smoke-install.sh
```

## CLI

```sh
seagrass diagnostics <path> --json
seagrass diagnostics <path> --sarif > out.sarif
seagrass analyze <path> --json
```

`1`=ERROR · `2`=usage · checkout: `cargo run -p seagrass-cli -- diagnostics …`

Anchor-first (v1/v2-preview full). Native/Pinocchio: stable applicable parity for shared Solana security/artifact evidence; Anchor-only surfaces are `non-applicable`. Runtime intelligence is an evidence-ingestion boundary, not inferred from static code. [`framework-parity`](docs/framework-parity.md) · static=`analyze` · runtime=explicit telemetry only [`product-framing`](docs/product-framing.md)

## Editors

VS Code `editors/vscode` · Zed [`editors/zed/README.md`](editors/zed/README.md) · +rust-analyzer · Zed `diagnostics.transport: push`

## Agents

Paste. Agent fetches raw `SKILL.md` or reads `skills/*/SKILL.md`. Symlink optional.

**Edit Solana** — copy:

```text
Before accounts/constraints/CPIs/security:
seagrass diagnostics <path> --json
seagrass analyze <path> --json
https://raw.githubusercontent.com/heyAyushh/seagrass/main/AGENTS.md
https://raw.githubusercontent.com/heyAyushh/seagrass/main/docs/agents.md
No Anchor from rust-analyzer. Seagrass quick fixes if machineApplicable.
```

**Task** — copy one (`Follow ` + URL if agent needs verb):

```text
lint     https://raw.githubusercontent.com/heyAyushh/seagrass/main/skills/seagrass-lint/SKILL.md
audit    https://raw.githubusercontent.com/heyAyushh/seagrass/main/skills/seagrass-audit/SKILL.md
explain  https://raw.githubusercontent.com/heyAyushh/seagrass/main/skills/seagrass-explain/SKILL.md
suppress https://raw.githubusercontent.com/heyAyushh/seagrass/main/skills/seagrass-suppress/SKILL.md
debug-fp https://raw.githubusercontent.com/heyAyushh/seagrass/main/skills/seagrass-debug-fp/SKILL.md
install  https://raw.githubusercontent.com/heyAyushh/seagrass/main/skills/seagrass-install/SKILL.md
```

Flow: install → lint\|audit → explain → suppress\|debug-fp · [`skills/README.md`](skills/README.md)

**Optional slash/rules:**

```bash
mkdir -p ~/.claude/skills && for s in skills/seagrass-*; do ln -sfn "$(pwd)/$s" ~/.claude/skills/$(basename "$s"); done
ln -sfn ../../editors/cursor/rules/seagrass.mdc .cursor/rules/seagrass.mdc
ln -sfn editors/opencode/opencode.json opencode.json
```

[`AGENTS.md`](AGENTS.md) · [`docs/agents.md`](docs/agents.md)

## Docs

[`docs/lints/`](docs/lints/) · [`CONTRIBUTING.md`](CONTRIBUTING.md) · `mkdocs build --strict` · MIT · [`NOTICE`](NOTICE)
