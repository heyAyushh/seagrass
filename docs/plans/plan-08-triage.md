# Plan 08 Triage — External Corpus Promotion

Status: baseline triaged on 2026-06-11.

## Baseline Run

Command:

```sh
scripts/fetch-corpus.sh
CORPUS_ENABLED=1 cargo test -p seagrass external_corpus -- --nocapture 2>&1 | tee /tmp/corpus-baseline.txt
```

Result:

| metric | value |
| --- | ---: |
| external source roots scanned | 267 |
| ERROR-severity diagnostics | 0 |
| oversized source files skipped | 2 |

Oversized files reported by the scan:

| file | disposition |
| --- | --- |
| `light-protocol/sdk-libs/photon-api/src/codegen.rs` | reported only; no exclusion because oversized handling is a CLI file-size policy |
| `orca-whirlpools/programs/whirlpool/src/manager/swap_manager.rs` | reported only; no exclusion because oversized handling is a CLI file-size policy |

## FP-Family Table

No FP-family rows were found in the baseline run. No diagnostic rule changes or
`fixtures/corpus/fp_family_*` additions were required for Plan 08.

| topic | pattern | example file | count | bucket | disposition |
| --- | --- | --- | ---: | --- | --- |
| _none_ | _none_ | _none_ | 0 | _none_ | no fix commit required |

## Broken-Tree Exclusions

The hard gate starts with an empty external exclusion table.

| path prefix | reason | disposition |
| --- | --- | --- |
| _none_ | _none_ | `EXTERNAL_CORPUS_EXCLUSIONS` remains empty |

## Vendoring Decisions

Committed fixtures were selected for permissive licensing, Anchor-version
spread, and idiom coverage:

| fixture | upstream | SHA | license | framework version |
| --- | --- | --- | --- | --- |
| `tensor_amm` | `tensor-foundation/amm` | `f6aab5e5b7ad20b0085561321eb1ebf484fe076d` | Apache-2.0 | Anchor 0.29.0 |
| `spl_token_swap` | `solana-labs/solana-program-library` | `264ca72de06b0c2b45c0b15d298000fe3f82db2e` | Apache-2.0 | native Solana |
| `raydium_clmm` | `raydium-io/raydium-clmm` | `6dcdd5610492888b12ffe083c2943c0b8b0abe62` | Apache-2.0 | Anchor 0.32.1 |
| `drift` | `drift-labs/protocol-v2` | `0ae3e3b1db782a6765c3525b3dec38ad4d9d3a62` | Apache-2.0 | Anchor 0.29.0 |
| `marginfi` | `mrgnlabs/marginfi-v2` | `4d57e2c234a41a868c519b722a6cce1c0431a699` | Apache-2.0 | Anchor 0.31.1 |
| `program_examples_cpi_anchor` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Anchor 1.0.0 |
| `program_examples_realloc_anchor` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Anchor 1.0.0 |
| `program_examples_repository_layout_pinocchio` | `solana-developers/program-examples` | `203b246141d15d54ce8a782749b901d76a4e7c51` | MIT | Pinocchio |

Not vendored:

| source | reason |
| --- | --- |
| `kamino-lending` | fetched tree is Business Source License 1.1, not MIT/Apache-2.0 |
| `squads-v4` | AGPL-3.0 |
| `openbook-v2` | mixed MIT/GPL source tree under the program instructions directory |
| `mango-v4` | mixed MIT/GPL source tree under the program instructions directory |
| `mpl-token-metadata` | custom Metaplex NFT Open Source License, not MIT/Apache-2.0 |
| manifest Pinocchio submodules | marked `unlicensed`; replaced by the MIT Pinocchio repository-layout example from `program-examples` |
