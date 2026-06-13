---
name: seagrass-install
description: Install and configure the Seagrass language server for the user's editor. Use when the user says "install seagrass", "set up seagrass", "configure seagrass in vscode/zed/vim/helix/neovim", "get seagrass running", or "wire seagrass into my project". Walks through source or release binary install, per-editor configuration, and a smoke check that real diagnostics fire on a Solana program.
user-invocable: true
license: MIT
compatibility: Uses a prebuilt Seagrass binary, or Rust 1.89.0 and cargo for source installs, plus one of VS Code, Zed, Vim with vim-lsp or CoC, the checked-in Neovim package, Helix, or CLI-only classic vi.
metadata:
  author: Seagrass Maintainers
  version: 1.0.0
---

# seagrass-install

Install Seagrass and verify it actually fires diagnostics on a real Solana program.

## When to use

The user wants to start using Seagrass in their editor or in CI. They have a
Solana program project (Anchor, Pinocchio, or native) and want supported
diagnostics plus the applicable editor surfaces for that framework.

## What Seagrass is

Codebase intelligence for Solana framework programs. The shipped static layer is
a language server and CLI for:

- **Anchor v1** — stable catalog (constraint shapes, init/payer/space rules, account references)
- **Anchor v2 preview** — pre-generated `anchor-next` catalog
- **Pinocchio** — native-style owner/type, signer, CPI, bounds, PDA, and
  arithmetic invariants where parsed evidence is available
- **Native Solana** — owner checks, type cosplay, signer authorization, CPI
  program validation, instruction bounds, PDA, and arithmetic invariants

Anchor v1/v2-preview get constraint completions, hovers, signatures, IDL/types
artifact checks, and generated account-rule coverage. Pinocchio and native
Solana do not use Anchor account constraints, so those Anchor-only surfaces are
non-applicable. Use `docs/framework-parity.md` for the exact boundary.

Diagnostics carry `(source, code, confidence, topic, applicability)` metadata.
Read the current topic catalog from `docs/topics.json` instead of hardcoding a
count.

Runtime compute-unit and traffic claims require explicit imported evidence such
as Trident, validator logs, indexer output, or application telemetry. Do not
present production CU usage, CU regressions, or cold-path deletion evidence as
available unless the user has provided or configured that runtime data source.

## Steps

### 1. Install the binary

Recommended prebuilt install, no Rust required:

```bash
curl -fsSL https://raw.githubusercontent.com/heyAyushh/seagrass/main/scripts/install.sh | sh
# or manually download from https://github.com/heyAyushh/seagrass/releases
```

From an already-published crates.io CLI package, requires Rust 1.89.0:

```bash
cargo install seagrass-cli --locked
```

From a Seagrass checkout, requires Rust 1.89.0:

```bash
rustup toolchain install 1.89.0 --profile minimal
cargo install --path crates/seagrass --locked
```

All paths install the user-facing binary as `seagrass`.

Confirm:

```bash
seagrass --version
```

If crates.io install fails because the CLI package is not published yet, use the
prebuilt binary or source-checkout install above.

### 2. Configure the editor

Pick the user's editor and apply the relevant block. Default to VSCode if unknown.

#### VSCode

Install the extension. Two paths:

- **Marketplace** (primary once published):
  ```bash
  # TODO: replace seagrass-local with confirmed publisher ID once Step 1 is unblocked
  code --install-extension seagrass-local.seagrass-vscode
  ```
  The extension downloads the matching server binary automatically on first
  activation; no manual server install is required when using the Marketplace
  extension.

- **VSIX from GitHub Release** (offline or pinned-version installs):
  ```bash
  # Download seagrass-vscode-<version>.vsix from the GitHub Release page
  code --install-extension seagrass-vscode-<version>.vsix
  ```
  The same automatic server download applies.

- **Local development** (from a checkout of the seagrass repo):
  ```bash
  cd editors/vscode && bun install && bun run build
  ```
  Then launch the VS Code extension host with F5. Set
  `seagrass.serverCommand` to point at the local binary if the PATH binary is
  stale. Release VSIX packaging is owned by the root
  `scripts/package-release.ts --vscode` command; do not invent an editor-local
  package script.

Settings (`.vscode/settings.json`):

```json
{
  "seagrass.serverCommand": "seagrass",
  "seagrass.serverArgs": [],
  "seagrass.diagnostics.security.enabled": true,
  "seagrass.diagnostics.confidenceDecorations": true,
  "seagrass.inlayHints.enabled": true,
  "seagrass.workspaceIndex.enabled": true,
  "seagrass.tridentCoverage.reportPath": "",
  "seagrass.tridentCoverage.showExecutionCount": true,
  "seagrass.trace.server": false
}
```

#### Zed

Zed registry submission is not done yet. Install as a dev extension from a
Seagrass checkout: run `zed: extensions`, choose `Install Dev Extension`, and
select `editors/zed`.

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

#### Vim

Vim does not ship a built-in LSP client. Use the repo package with `vim-lsp`:

```bash
mkdir -p ~/.vim/pack/seagrass/start
ln -sfn /path/to/seagrass/editors/vim ~/.vim/pack/seagrass/start/seagrass
```

Then install `vim-lsp` with the user's Vim plugin manager. The package registers
`seagrass` for Rust buffers on vim-lsp's setup event. For a custom binary:

```vim
let g:seagrass_command = '/path/to/seagrass'
```

For CoC users, copy `editors/vim/coc-settings.json` into the user's CoC settings
and do not load the vim-lsp package at the same time.

The Vim package also exposes Seagrass report commands and a CLI quickfix scan:

```vim
:SeagrassStatus
:SeagrassAnalyze
:SeagrassDiagnostics
```

#### Neovim

Use the checked-in Neovim runtime package:

```bash
mkdir -p ~/.local/share/nvim/site/pack/seagrass/start
ln -sfn /path/to/seagrass/editors/nvim ~/.local/share/nvim/site/pack/seagrass/start/seagrass
```

It uses Neovim's built-in LSP config when available and falls back to
`nvim-lspconfig`. Configure it before startup when needed:

```lua
vim.g.seagrass_nvim = {
  command = "seagrass",
  cli_command = "seagrass",
  settings = {
    ["diagnostics.transport"] = "push",
    ["diagnostics.coldPath"] = "idle",
  },
}
```

Run `:SeagrassInfo`, `:SeagrassAnalyze`, or `:SeagrassDiagnostics` in a Rust
buffer to verify the editor path.

#### Classic vi

Classic POSIX `vi` has no LSP client or plugin package surface. Use the CLI:

```bash
seagrass diagnostics programs/demo/src/lib.rs --json
seagrass analyze programs/demo/src/lib.rs --json
```

If the user's `vi` is actually Vim or Neovim, use the package sections above.

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

fn must_have_value(value: Option<u64>) -> u64 {
    value.unwrap()
}
EOF

seagrass diagnostics /tmp/seagrass-smoke.rs --json | jq '.[] | {topic, severity, message}'
```

Expected: at least one diagnostic with `topic: "seagrass/solana.code-quality.unsafe-unwrap"`.

If empty:

- Re-run with `RUST_LOG=debug seagrass diagnostics /tmp/seagrass-smoke.rs --json 2>&1 | head -40` and report.
- Verify `seagrass --version` matches the intended checkout or release version.

### 4. (Optional) Wire into CI

Add to the project's CI:

```yaml
- name: Seagrass lint
  run: |
    cargo install seagrass-cli --locked
    seagrass diagnostics ./programs --json > seagrass-report.json
    test "$(jq '[.[] | select(.severity == "ERROR")] | length' seagrass-report.json)" = "0"
```

For GitHub code scanning, use SARIF:

```yaml
- name: Seagrass SARIF
  run: |
    cargo install seagrass-cli --locked
    seagrass diagnostics ./programs --sarif > seagrass.sarif
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: seagrass.sarif
```

If the project has Trident coverage JSON, point VS Code at it with
`seagrass.tridentCoverage.reportPath` or leave that empty and let the extension
discover reports under `trident-tests/`. Keep
`seagrass.tridentCoverage.showExecutionCount` enabled when using coverage to
decide whether heuristic lint gaps are ready to promote.

## What to do AFTER install

If the user's project has existing diagnostics:

- For genuine issues, surface them and ask whether to fix.
- For false positives, route to the `seagrass-suppress` and `seagrass-debug-fp` skills.
- For unknown topics, route to the `seagrass-explain` skill.

## Stop conditions

- If `cargo install` fails twice with the same error, stop and report the error verbatim. Do not loop.
- If the smoke check produces zero diagnostics on the deliberately-broken fixture, the install is broken — stop and report. Do not try to "fix" by editing the user's code.
- Never modify the user's editor config without showing the diff first.
