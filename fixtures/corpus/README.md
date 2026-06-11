# Diagnostic Corpus — Committed Zero-ERROR Regression Fixtures

Each sub-directory here is a committed Solana program used as a hard
false-positive regression guard: seagrass must emit zero ERROR-severity
diagnostics against every file in every program here. If a seagrass change
causes any program in this directory to receive an ERROR diagnostic that was
absent before, the corpus test fails and the change must be reviewed before it
can land.

The committed set deliberately covers Anchor 0.29, 0.31, 0.32, and 1.0-era
syntax, plus one native SPL program and one Pinocchio program. The external
corpus remains the broader nightly gate.

## Real Programs

| Fixture | Upstream | SHA | License | Framework version | Idioms covered |
| --- | --- | --- | --- | --- | --- |
| `blueshift_anchor_escrow` | Blueshift Anchor escrow tutorial | upstream teaching repo | MIT | Anchor 0.31.1 | `init`/`close`, `associated_token`, `has_one`, `seeds` + `bump`, `init_if_needed`, signer CPI |
| `tensor_amm` | `tensor-foundation/amm` | `f6aab5e5b7ad20b0085561321eb1ebf484fe076d` | Apache-2.0 | Anchor 0.29.0 | NFT AMM pools, Token-2022, legacy + MPL Core instruction families |
| `spl_token_swap` | `solana-labs/solana-program-library` | `264ca72de06b0c2b45c0b15d298000fe3f82db2e` | Apache-2.0 | Native Solana program | SPL token-swap processor, curve math, packed state |
| `raydium_clmm` | `raydium-io/raydium-clmm` | `6dcdd5610492888b12ffe083c2943c0b8b0abe62` | Apache-2.0 | Anchor 0.32.1 | Concentrated liquidity, Token-2022 metadata/memo, dense account constraints |
| `drift` | `drift-labs/protocol-v2` | `0ae3e3b1db782a6765c3525b3dec38ad4d9d3a62` | Apache-2.0 | Anchor 0.29.0 | Perpetuals, oracle and order math, large instruction/state surface |
| `marginfi` | `mrgnlabs/marginfi-v2` | `4d57e2c234a41a868c519b722a6cce1c0431a699` | Apache-2.0 | Anchor 0.31.1 | Lending, Token-2022 hooks, nested account validation |
| `program_examples_cpi_anchor` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Anchor 1.0.0 | Cross-program invocation between Anchor programs |
| `program_examples_realloc_anchor` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Anchor 1.0.0 | PDA/realloc flow and Anchor 1.0 toolchain metadata |
| `program_examples_repository_layout_pinocchio` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Pinocchio workspace example | Repository-layout Pinocchio processor, state modules, instruction routing |

## Synthetic Regressions

The `fp_family_*` fixtures are minimal reproductions for false-positive
families fixed during earlier plan work. They are kept alongside the real
programs so regressions fail in the same committed sweep.

## External Corpus

Real-world programs from public repositories are pinned in
[`corpus/manifest.toml`](../../corpus/manifest.toml) at the workspace root.
Their full source trees live in `corpus/programs/`, which is gitignored.

To materialise the external corpus locally:

```sh
scripts/fetch-corpus.sh
```

To run the external corpus hard gate after fetching:

```sh
CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture
```

The nightly CI workflow (`.github/workflows/corpus.yml`) fetches and exercises
the external corpus automatically.

## Adding New Committed Programs

1. Copy only the minimal source tree: `src/**/*.rs`, the program `Cargo.toml`,
   the workspace `Anchor.toml` when one exists, and the upstream license file.
   Do not copy `target/`, `node_modules/`, `.git/`, build artifacts, or lock
   files.
2. The program source must be real, publicly available, and under an
   Apache-2.0 or MIT license.
3. Record the upstream repository, pinned SHA, license, and framework version in
   this README.
4. The corpus test in `src/lsp/diagnostics/tests/corpus.rs` picks up the new
   program automatically; no manual test registration is required.
