# Diagnostic Corpus — Committed Zero-ERROR Regression Fixtures

Each sub-directory here is a **committed** Anchor program used as a hard
false-positive regression guard: seagrass must emit **zero ERROR-severity
diagnostics** against every file in every program here.  If a seagrass change
causes any program in this directory to receive an ERROR diagnostic that was
absent before, the corpus test fails and the change must be reviewed before it
can land.

> **Only `blueshift_anchor_escrow` is committed here.**  The three synthetic
> programs that previously lived here (`anchor_pda_counter`, `anchor_token_vault`,
> `anchor_multisig`) have been removed.  Broader real-world coverage is provided
> by the *external* corpus described below.

## Programs

### `blueshift_anchor_escrow`

A token-escrow tutorial program written by Blueshift
(<https://www.blueshift.gg/> / <https://github.com/Blueshift-Finance>).  The
program is publicly available as a teaching example for Anchor-based escrow
patterns and is reproduced here under the MIT licence that accompanies the
original repository.

This program was the first corpus entry because it triggered false-positive
ERROR diagnostics in earlier seagrass releases (raw-account and signer checks
fired against accounts that are correctly constrained via `has_one`, `seeds`,
and `bump`).

**Idioms covered**: `init`/`close`, `associated_token`, `has_one`, `seeds` +
`bump`, `init_if_needed`, `CpiContext::new_with_signer`, multi-file instruction
modules with `use instructions::*;`.

## External corpus (not committed)

Real-world programs from public, permissively-licensed Anchor repositories are
pinned in [`corpus/manifest.toml`](../../corpus/manifest.toml) at the workspace
root.  Their source trees are **never committed**; they live in
`corpus/programs/` which is gitignored.

To materialise the external corpus locally:

```sh
scripts/fetch-corpus.sh
```

To run the external corpus test after fetching:

```sh
CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture
```

The nightly CI workflow (`.github/workflows/corpus.yml`) fetches and exercises
the external corpus automatically.

## Adding new committed programs

1. Copy only the minimal source tree: `src/**/*.rs`, the program `Cargo.toml`,
   and the workspace `Anchor.toml`.  Do **not** copy `target/`, `node_modules/`,
   `.git/`, build artefacts, or lock files.
2. The program source must be real, publicly available, and under an Apache-2.0
   or MIT licence.
3. The corpus test in `src/lsp/diagnostics/tests/corpus.rs` picks up the new
   program automatically — no manual registration required.

For broader real-world programs, prefer adding an entry to `corpus/manifest.toml`
instead of committing the source here.
