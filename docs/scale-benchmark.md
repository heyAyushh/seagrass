# Scale Benchmark

Status: Active

Generate a synthetic Anchor workspace:

```sh
bun scripts/synth-workspace.ts --programs 200
```

Run the measured LSP initialization benchmark:

```sh
bun scripts/scale-benchmark.ts --programs 200
```

Run repeated fresh server starts and publish p99:

```sh
bun scripts/scale-benchmark.ts --programs 10 --samples 3 --report target/seagrass-scale-10.json
bun scripts/scale-benchmark.ts --programs 50 --samples 3 --report target/seagrass-scale-50.json
bun scripts/scale-benchmark.ts --programs 200 --samples 3 --report target/seagrass-scale-200.json
```

Default outputs:

```text
target/seagrass-synth-workspace-<timestamp>-<pid>
target/seagrass-scale-report.json
```

Current scale target:

- N = 200 programs
- cold start < 5s
- RSS < 500 MB

Latest local p99 run, 2026-05-26:

| programs | samples | indexed files | p99 cold start | p99 RSS |
| --- | --- | --- | --- | --- |
| 10 | 3 | 10 | 1575.5 ms | 23.0 MB |
| 50 | 3 | 50 | 1541.6 ms | 23.8 MB |
| 200 | 3 | 200 | 1765.9 ms | 26.5 MB |

The benchmark drives `initialize` and `initialized` over LSP stdio, polls
`seagrass/status` until the workspace index reaches the requested program
count, records process RSS with `ps` when the host allows process listing,
writes the JSON report, and fails when the measured budgets are exceeded. If
the host blocks `ps`, RSS fields are written as `null` and the cold-start/index
budget remains enforced. With `--samples`, `coldStartMillis` and
`rssMegabytes` in the report are the p99 values, and the raw observations remain
available under `samples`.
