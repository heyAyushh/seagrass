---
name: seagrass-install
description: Install and configure the Seagrass language server for the user's editor. Use when the user says "install seagrass", "set up seagrass", "configure seagrass in vscode/zed/helix/neovim", "get seagrass running", or "wire seagrass into my project". Walks through binary install (`cargo install seagrass`), per-editor configuration, and a smoke check that real diagnostics fire on a Solana program.
user-invocable: true
license: MIT
compatibility: Requires Rust toolchain (1.78+), cargo, and one of VSCode, Zed, Helix, or a Neovim LSP client.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-install

Install Seagrass and verify it actually fires diagnostics on a real Solana program.

## When to use

The user wants to start using Seagrass in their editor or in CI. They have a Solana program project (Anchor, Pinocchio, or native) and want diagnostics, completions, and hovers.

## What Seagrass is

A language server for Solana framework programs:

- **Anchor v1** — stable catalog (constraint shapes, init/payer/space rules, account references)
- **Anchor v2 preview** — pre-generated `anchor-next` catalog
- **Pinocchio** — native invariants
- **Native Solana** — owner checks, type cosplay, signer authorization

40 lint topics today. All diagnostics carry `(source, code, confidence, topic, applicability)` metadata.

## Steps

### 1. Install the binary

```bash
cargo install seagrass
```

Confirm:

```bash
seagrass --version
```

If `cargo install` fails:

- `seagrass` may not yet be on crates.io. Fall back:
  ```bash
  cargo install --git https://github.com/heyAyushh/seagrass --locked
  ```

### 2. Configure the editor

Pick the user's editor and apply the relevant block. Default to VSCode if unknown.

#### VSCode

Install the extension. Two paths:

- **Marketplace** (once published):
  ```
  ext install seagrass-local.seagrass-vscode
  ```

- **Local development** (from a checkout of the seagrass repo — recommended while the extension is private):
  ```bash
  cd editors/vscode
  bun install
  bun run build
  ```
  Then point the server at the local binary (no .vsix packaging script is currently defined; run the language server directly):

Settings (`.vscode/settings.json`):

```json
{
  "seagrass.serverCommand": "seagrass",
  "seagrass.serverArgs": [],
  "seagrass.diagnostics.security.enabled": true,
  "seagrass.inlayHints.enabled": true,
  "seagrass.workspaceIndex.enabled": true,
  "seagrass.trace.server": "off"
}
```

#### Zed

`~/.config/zed/extensions.json` — add the seagrass extension. Or install via Zed's extensions panel ("Seagrass").

Project settings (`.zed/settings.json`):

```json
{
  "lsp": {
    "seagrass": {
      "binary": { "path": "seagrass" }
    }
  },
  "languages": {
    "Rust": {
      "language_servers": ["seagrass", "..."]
    }
  }
}
```

#### Helix

`~/.config/helix/languages.toml`:

```toml
[language-server.seagrass]
command = "seagrass"

[[language]]
name = "rust"
language-servers = [{ name = "seagrass" }, { name = "rust-analyzer" }]
```

#### Neovim (nvim-lspconfig)

```lua
local configs = require("lspconfig.configs")
if not configs.seagrass then
  configs.seagrass = {
    default_config = {
      cmd = { "seagrass" },
      filetypes = { "rust" },
      root_dir = require("lspconfig.util").root_pattern("Anchor.toml", "Cargo.toml", "Seagrass.toml"),
      settings = {},
    },
  }
end
require("lspconfig").seagrass.setup({})
```

### 3. Smoke check

Generate a known-bad file and confirm Seagrass surfaces the diagnostic:

```bash
cat > /tmp/seagrass-smoke.rs <<'EOF'
use anchor_lang::prelude::*;

#[program]
mod smoke {
    use super::*;
    pub fn debit(ctx: Context<Debit>, amount: u64) -> Result<()> {
        let acc = &mut ctx.accounts.balance;
        acc.value = acc.value - amount;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Debit<'info> {
    #[account(mut)]
    pub balance: Account<'info, BalanceAccount>,
}

#[account]
pub struct BalanceAccount { pub value: u64 }
EOF

seagrass diagnostics /tmp/seagrass-smoke.rs --json | jq '.[] | {topic, severity, message}'
```

Expected: at least one diagnostic with `topic: "seagrass/solana.code-quality.unchecked-arithmetic"`.

If empty:

- Re-run with `RUST_LOG=debug seagrass diagnostics /tmp/seagrass-smoke.rs --json 2>&1 | head -40` and report.
- Verify `seagrass --version` matches the latest release.

### 4. (Optional) Wire into CI

Add to the project's CI:

```yaml
- name: Seagrass lint
  run: |
    cargo install seagrass --locked
    seagrass diagnostics ./programs --json > seagrass-report.json
    test "$(jq '[.[] | select(.severity == "ERROR")] | length' seagrass-report.json)" = "0"
```

## What to do AFTER install

If the user's project has existing diagnostics:

- For genuine issues, surface them and ask whether to fix.
- For false positives, route to the `seagrass-suppress` and `seagrass-debug-fp` skills.
- For unknown topics, route to the `seagrass-explain` skill.

## Stop conditions

- If `cargo install` fails twice with the same error, stop and report the error verbatim. Do not loop.
- If the smoke check produces zero diagnostics on the deliberately-broken fixture, the install is broken — stop and report. Do not try to "fix" by editing the user's code.
- Never modify the user's editor config without showing the diff first.
