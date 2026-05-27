# Learnings: Error Boundary Improvements

## Decisions

- **Lines 1662/1670 kept as `InternalError`**: Both in `with_catch_unwind` — handler panics and task join failures are genuine server bugs, not invalid client input. Added comments explaining the distinction from `InvalidParams`.
- **Workspace root validation warns, never rejects**: LSP spec says `rootUri` is optional and may not map to real paths. Warning gives visibility without breaking clients.
- **Server version log**: Added `emit_log` during initialize with `env!("CARGO_PKG_VERSION")` for visibility in LSP client logs.

## Patterns

- `with_catch_unwind` uses `AssertUnwindSafe` + `spawn_blocking` to isolate handler panics from crashing the entire LSP server.
- Server already emits `MessageType::INFO` logs during `initialized` — we added one during `initialize` too.
