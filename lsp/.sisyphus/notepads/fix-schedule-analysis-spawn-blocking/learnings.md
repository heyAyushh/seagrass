# Fix: schedule_analysis_and_publish spawn_blocking

## Problem

1. **Compilation**: `ParsedDocument` (contains `tree_sitter::Tree` + `syn::File` via `proc-macro2` `span-locations`) is `!Send`. The `debounce.schedule()` closure requires `Send + 'static`.

2. **Runtime race**: `workspace/symbol` query returns `null` because the workspace index isn't populated until the debounce fires (100ms delay). Index population must happen BEFORE diagnostics+publish so synchronous queries (workspace/symbol, hover, goto-def) see results.

## Solution

Split into two phases:

1. **Immediate parse+index** via `tokio::task::spawn_blocking` — outside the debounce, inside a scoped block. This ensures the workspace index is populated before `did_open` returns.

2. **Debounced diagnostics+publish** — heavyweight analysis deferred by debounce to avoid running on every keystroke.

Both phases use `spawn_blocking` for `ParsedDocument` (`!Send`) creation. The immediate phase only does index population; the debounced phase runs diagnostics and publishes.

## Key Pattern

- Clone `this`, `uri_c`, `text` before `spawn_blocking` to avoid move-after-use errors
- `spawn_blocking` owns `!Send` types entirely — drop happens inside the blocking thread
- Only `Send + Clone` values (`OpenDocument`, `Vec<Diagnostic>`) are returned to the async context

## Removed

- `index_document_on_blocking_thread` — dead code, never called. Inlined into the immediate parse+index phase.
