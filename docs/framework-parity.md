# Framework Parity

Status: Stable applicable parity

Seagrass is Anchor-first. Native Solana and Pinocchio parity means the same LSP
transport, editor metadata, artifact reporting, and shared Solana diagnostic
families where the framework has equivalent semantics. It does not mean
inventing Anchor-only account constraint behavior for frameworks that do not use
Anchor's `#[derive(Accounts)]` or `#[account(...)]` syntax.

Native Solana and Pinocchio framework crates are stable for equivalent Solana
diagnostic coverage. Owner/type, signer, writable, and CPI program-id checks are
account-scoped or program-expression-scoped, so a visible guard for one account
or program id does not suppress diagnostics for unrelated accounts or programs.

## Supported Surfaces

| Surface | Anchor v1/v2 preview | Native Solana | Pinocchio |
| --- | --- | --- | --- |
| LSP initialization, diagnostics transport, formatting, code actions, command routing | supported | supported | supported |
| JSON/CLI/SARIF diagnostic metadata | supported | supported | supported |
| Local SBF/deploy artifact evidence | supported | supported for deployable Cargo programs | supported for deployable Cargo programs |
| Owner/type validation security diagnostics | supported through typed account and raw-data evidence | supported through account-scoped raw account reads and deserialization evidence | supported through account-scoped native-style raw account reads, including `borrow_data_unchecked()` |
| Signer/CPI/instruction-data/PDA/code-quality diagnostics | supported where parsed evidence exists | supported where parsed evidence exists, with account-scoped signer/writable and expression-scoped CPI program validation | supported where parsed evidence exists, with account-scoped signer/writable and expression-scoped CPI program validation |
| Anchor account constraints, init/payer/space/realloc/close parser checks | supported | not applicable | not applicable |
| Anchor constraint completions, hovers, signatures, links | supported | not applicable | not applicable |
| Anchor IDL/types artifact freshness | supported | not applicable | not applicable |
| Anchor workspace account-reference and `Context<T>` analysis | supported | not applicable | not applicable |

## Enforcement

The black-box integration test in `tests/jsonrpc_lsp.rs` launches the installable
`seagrass` server through `cargo run -p seagrass-cli --quiet`, speaks LSP
JSON-RPC over stdio, and verifies:

- `initialize` advertises pull diagnostics and document formatting.
- `textDocument/formatting` returns a `rustfmt` edit through the root LSP path.
- Pinocchio raw account owner/type diagnostics surface as `solana-code-quality`
  with `programKind: pinocchio`.
- Native Solana raw account owner/type diagnostics surface as
  `solana-code-quality` with `programKind: native-solana`.
- Framework crate and editor parity tests verify that validating account 0 or
  `program_id` does not mask unvalidated account 1 or another dynamic CPI
  program expression.

Framework crate unit tests still cover crate-local rules. Editor parity tests
cover user-visible diagnostic metadata and quickfix payloads. The JSON-RPC test
guards the root server path that editor adapters and agent harnesses actually
use.
