# Diagnostic Corpus

This directory contains small committed regression fixtures. They run during
normal `cargo test` and must emit zero ERROR-severity diagnostics.

Large real-program corpora are intentionally not committed here. They are pinned
in [`corpus/manifest.toml`](../../corpus/manifest.toml), fetched into the
gitignored `corpus/programs/` directory, and exercised by the external corpus
hard gate.

## Committed Fixtures

| Fixture | Upstream | SHA | License | Framework version | Idioms covered |
| --- | --- | --- | --- | --- | --- |
| `blueshift_anchor_escrow` | Blueshift Anchor escrow tutorial | upstream teaching repo | MIT | Anchor 0.31.1 | `init`/`close`, `associated_token`, `has_one`, `seeds` + `bump`, `init_if_needed`, signer CPI |
| `program_examples_cpi_anchor` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Anchor 1.0.0 | Cross-program invocation between Anchor programs |
| `program_examples_realloc_anchor` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Anchor 1.0.0 | PDA/realloc flow and Anchor 1.0 toolchain metadata |
| `program_examples_repository_layout_pinocchio` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Pinocchio workspace example | Repository-layout Pinocchio processor, state modules, instruction routing |

The broad Anchor 0.29, 0.31, 0.32, native Solana, Token-2022, and mainnet
protocol coverage lives in the external corpus so the PR and repository stay
reviewable.

## External Real Programs

The external hard gate fetches these large upstream programs on demand instead
of committing their source trees:

| Program | Manifest entry | Notes |
| --- | --- | --- |
| Drift protocol v2 | `drift` | Anchor 0.29 perpetuals and oracle/order math |
| marginfi v2 | `marginfi-v2` | Anchor 0.31 lending and Token-2022 hooks |
| Raydium CLMM | `raydium-clmm` | Anchor 0.32 concentrated liquidity |
| SPL token-swap | `spl-token` | Native Solana token-swap processor |
| Tensor AMM | `tensor-amm` | Anchor 0.29 NFT AMM and Token-2022 surfaces |

## Synthetic Regressions

The `fp_family_*` fixtures are minimal reproductions for false-positive
families fixed during earlier plan work. They are kept in the committed corpus
so regressions fail in the normal local sweep.

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

## Adding New Committed Fixtures

1. Prefer a minimal regression fixture over copying a full upstream repository.
2. Keep committed fixtures small enough for normal `cargo test` review.
3. Put large real-program coverage in `corpus/manifest.toml` instead.
4. Record upstream repository, pinned SHA, license, and framework version here
   when a real source fixture is intentionally committed.
5. The corpus test in `src/lsp/diagnostics/tests/corpus.rs` picks up committed
   fixture directories automatically.
