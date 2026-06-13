# Seagrass for vi

Classic POSIX `vi` does not expose an LSP client, quickfix list, plugin package
runtime, or JSON parser that Seagrass can target as an editor extension.

Use the CLI directly:

```sh
seagrass diagnostics programs/demo/src/lib.rs --json
seagrass analyze programs/demo/src/lib.rs --json
```

If your `vi` command is actually Vim, use [`../vim`](../vim). If it is Neovim,
use [`../nvim`](../nvim). Those packages provide LSP-backed Seagrass commands
plus quickfix diagnostics.
