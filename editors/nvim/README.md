# Seagrass for Neovim

This is a Neovim runtime package for the `seagrass` language server. It
registers Seagrass for Rust buffers through Neovim's built-in LSP config when
available, and falls back to `nvim-lspconfig` on older Neovim installs.

## Requirements

- Neovim with the built-in LSP client.
- Either Neovim `vim.lsp.config` support or
  [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig).
- A `seagrass` binary on `PATH`, or a custom command passed to
  `require("seagrass").setup`.

## Install

With native packages from a Seagrass checkout:

```sh
mkdir -p ~/.local/share/nvim/site/pack/seagrass/start
ln -sfn /path/to/seagrass/editors/nvim ~/.local/share/nvim/site/pack/seagrass/start/seagrass
```

With a plugin manager, point it at `editors/nvim` in this checkout.

The package auto-registers on startup. To configure it manually, set
`vim.g.seagrass_nvim` before the plugin loads:

```lua
vim.g.seagrass_nvim = {
  command = "seagrass",
  args = {},
  cli_command = "seagrass",
  cli_args = {},
  root_markers = { "Anchor.toml", "Seagrass.toml", "Cargo.toml" },
  settings = {
    ["diagnostics.transport"] = "push",
    ["diagnostics.coldPath"] = "idle",
    ["diagnostics.security.enabled"] = true,
    ["diagnostics.experimental.enabled"] = true,
    ["security.strictNative.enabled"] = true,
    ["workspaceIndex.enabled"] = true,
    ["trace.server"] = false,
  },
}
```

Or disable auto-start and call setup yourself:

```lua
vim.g.seagrass_nvim = { auto_start = false }
require("seagrass").setup({
  command = "seagrass",
  settings = {
    ["agent.mode"] = false,
  },
})
```

## Commands

LSP-backed commands require an attached Seagrass client:

```vim
:SeagrassInfo
:SeagrassStatus
:SeagrassAnalyze
:SeagrassAnalyze instruction=initialize context=Create
:SeagrassArtifacts
:SeagrassProgramReport
:SeagrassErrorCoverage
:SeagrassSupportMatrix
:SeagrassGeneratorProfile
:SeagrassLogs
:SeagrassProjectCoverage
:SeagrassFeedback
:SeagrassRestart
```

CLI-backed commands work without an attached LSP client:

```vim
:SeagrassDiagnostics
:SeagrassDiagnostics programs/demo/src/lib.rs
:SeagrassAnalyzeCli
:SeagrassAnalyzeCli programs/demo/src/lib.rs
```

`:SeagrassDiagnostics` runs `seagrass diagnostics <path> --json`, converts
Seagrass ranges into quickfix items, and opens the quickfix list when findings
exist. `:SeagrassAnalyzeCli` opens the static codebase-intelligence JSON in a
scratch buffer.

## Relationship To rust-analyzer

Seagrass should run beside rust-analyzer unless you intentionally want only
Solana-specific analysis. Rust-analyzer owns generic Rust behavior; Seagrass
owns Anchor/Solana diagnostics, completions, hovers, quick fixes, symbols,
artifact reports, and command reports.
