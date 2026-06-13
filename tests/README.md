# Seagrass integration tests

This directory is reserved for black-box LSP and CLI fixtures that must run
against the installable `seagrass` entrypoint instead of crate-internal unit
test helpers.

`jsonrpc_lsp.rs` launches `cargo run -p seagrass-cli --quiet`, speaks LSP
JSON-RPC over stdio, and verifies root-server parity for editor-visible
surfaces: formatting plus native Solana and Pinocchio framework diagnostics and
code actions. The JSON-RPC cases serialize their cargo-backed subprocesses
inside the test binary so platform runners do not race LSP startup, rustfmt, or
diagnostic publication. The harness builds file URIs with `Url::from_file_path`
so path separators and percent-encoding match the server's canonical URI form
on every runner OS.
