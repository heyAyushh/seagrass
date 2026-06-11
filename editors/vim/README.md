# Seagrass for Vim

This is a Vim package that registers the `seagrass` language server with
[`vim-lsp`](https://github.com/prabirshrestha/vim-lsp). It does not implement a
second LSP client. Vim users need an LSP transport plugin, and Seagrass owns the
Anchor/Solana intelligence.

## Requirements

- Vim 8.2+ with package support.
- [`vim-lsp`](https://github.com/prabirshrestha/vim-lsp).
- A `seagrass` binary on `PATH`, or `g:seagrass_command` pointing to it.

## Install

With native Vim packages from a Seagrass checkout:

```bash
mkdir -p ~/.vim/pack/seagrass/start
ln -sfn /path/to/seagrass/editors/vim ~/.vim/pack/seagrass/start/seagrass
```

With vim-plug:

```vim
Plug 'prabirshrestha/vim-lsp'
Plug '/path/to/seagrass/editors/vim'
```

The package registers on vim-lsp's `User lsp_setup` event. Open a Rust file in a
workspace containing `Anchor.toml`, `Seagrass.toml`, or `Cargo.toml`; vim-lsp
starts `seagrass` over stdio.

## Settings

Defaults:

```vim
let g:seagrass_command = 'seagrass'
let g:seagrass_args = []
let g:seagrass_root_markers = ['Anchor.toml', 'Seagrass.toml', 'Cargo.toml']
let g:seagrass_settings = {
      \ 'diagnostics.security.enabled': v:true,
      \ 'diagnostics.experimental.enabled': v:true,
      \ 'security.strictNative.enabled': v:true,
      \ 'diagnostics.transport': 'push',
      \ 'diagnostics.coldPath': 'idle',
      \ 'editor.client': 'vim',
      \ 'editor.inlineValues.enabled': v:false,
      \ 'telemetry.completion.enabled': v:true,
      \ 'telemetry.diagnostics.enabled': v:true,
      \ 'inlayHints.enabled': v:true,
      \ 'workspaceIndex.enabled': v:true,
      \ 'trace.server': v:false,
      \ }
```

Use `:SeagrassInfo` to inspect the package command and root markers.

## CoC

If you use CoC instead of vim-lsp, copy
[`coc-settings.json`](./coc-settings.json) into your Vim or project CoC settings.
Do not load `plugin/seagrass.vim` at the same time; choose one LSP transport.
