# Product Framing

Status: Active

Seagrass ships codebase intelligence for Solana programs. Static analysis is
available from source, manifest, IDL, SBF, keypair, and workspace evidence. Runtime
intelligence requires explicit telemetry ingestion before Seagrass can claim live
compute, traffic, or execution behavior.

## Static Layer

The static layer can report parsed program structure, accounts, CPIs, PDAs,
artifact freshness, SBF `.text` instruction-count floors, framework routing,
lint topics, quick fixes, JSON output, and SARIF output. These findings are valid
when they are backed by local source or artifact evidence.

## Preflight Layer

The preflight layer can report Anchor runtime errors only when concrete
invocation evidence is available, such as instruction data length, decoded
instruction status, expected/provided account counts, account owners,
initialization status, discriminator bytes, and realloc deltas. These findings do
not require compiling an Anchor program or running a validator, but they still
require explicit invocation/account evidence.

The checked-in demo is:

```sh
seagrass preflight fixtures/preflight/anchor-errors.json --json
```

It exercises every `preflight-covered` Anchor error from explicit evidence and
keeps `runtimeEvidence.status` set to `notConfigured`.

## Runtime Layer

Runtime signals are not inferred from code shape alone. Actual compute-unit usage per instruction, production traffic volume, observed CPI frequency, live account state, and cluster execution traces require telemetry, logs, or explicit runtime artifacts.

Required evidence before claim:

- Telemetry source and collection window.
- Program id, cluster, and build/artifact identity.
- Sampling or replay method.
- Freshness timestamp.
- Failure and missing-data handling.

If any edge case fails this contract, describe the runtime field as unavailable
instead of implying Seagrass measured it.
