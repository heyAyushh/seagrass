# Anchor Support Generator

Seagrass checks in its generated Anchor support catalogs under
`src/anchor/generated/`. Normal `cargo build -p seagrass` does not scrape an
Anchor source tree; `build.rs` only watches the checked-in generated directory.

Regeneration is an explicit maintainer workflow that requires a separate Anchor
source checkout. Point `$SEAGRASS_ANCHOR_PATH` at that checkout (or pass it
directly via `--anchor-path`):

```sh
bun scripts/regen-support.ts --anchor-path "$SEAGRASS_ANCHOR_PATH" --family v1
```

For a preview Anchor v2 checkout:

```sh
bun scripts/regen-support.ts --anchor-path ../anchor-next --family v2-preview --dry-run
```

To validate the checked-in v2 preview catalog against real `anchor-next` example
programs:

```sh
bun scripts/check-anchor-v2-preview-corpus.ts --anchor-path ../anchor-next
```

Before committing generated changes:

```sh
bun scripts/regen-support.ts --anchor-path "$SEAGRASS_ANCHOR_PATH" --family v1 --check
cargo test -p seagrass constraint_catalog anchor_support anchor_errors anchor_types
```

The generator reads from the Anchor source checkout:

- `Cargo.toml` for the Anchor workspace version
- `lang/syn/src/parser/accounts/constraints.rs` for accepted account
  constraints and parser rules
- `lang/syn/src/parser/accounts/mod.rs` for account wrapper and sysvar parser
  evidence
- `lang/error/src/lib.rs` for Anchor `ErrorCode` values
- `lang/src/lib.rs` and
  `spl/src/{associated_token,token,token_2022,token_interface}.rs` for account
  field completions
- `examples/**/programs/**/src/**/*.rs` and
  `tests/**/programs/**/src/**/*.rs` for real program corpus coverage, unless
  `--corpus-path` is supplied

The generated profile is available through `workspace/executeCommand`:

- `seagrass/supportMatrix`
- `seagrass/generatorProfile`

## Anchor v1 Policy

The checked-in v1 profile is valid when its source fingerprints match the
intended Anchor v1 checkout and the generated support matrix has no unaudited
gaps for changed parser, error, field completion, or corpus inputs.

For a newer Anchor v1 checkout:

1. Update the pinned `anchor-syn` revision in the root workspace.
2. Run `bun scripts/regen-support.ts --anchor-path "$SEAGRASS_ANCHOR_PATH" --family v1`.
3. Inspect fingerprint and support-matrix changes.
4. Commit the dependency pin, generated Rust files, and any semantic LSP updates
   together.

## Anchor v2 Preview Policy

Anchor v2 remains preview-only until a v2 checkout generates a stable profile
and every semantic analyzer has been audited against the v2 parser, error, and
corpus output. Generate v2 output with `--family v2-preview` and keep it out of
the default v1 catalog until that audit is complete.
