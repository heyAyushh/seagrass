# Contributing

Status: Active

Work from the repository root unless a command explicitly changes directory.
Preserve unrelated dirty state and keep server changes scoped to `src/`, editor
adapters, workflows, or generated support files unless the task requires parent
Anchor changes.

## Development Setup

```sh
cargo build -p seagrass
cargo test -p seagrass
cargo install --path crates/seagrass
bun scripts/verify-production.ts
```

VS Code adapter:

```sh
cd editors/vscode
bun install
bun run check
```

Zed adapter:

```sh
cd editors/zed
cargo test
cargo build --target wasm32-wasip2 --release
```

## Generated Anchor Support

Generated support lives in `src/generated/`. Normal builds do not scrape
Anchor sources.

Regenerate v1 support:

```sh
bun scripts/regen-support.ts --anchor-path . --family v1
```

Check that generated files are current:

```sh
bun scripts/regen-support.ts --anchor-path . --family v1 --check
```

Preview a v2 checkout:

```sh
bun scripts/regen-support.ts \
  --anchor-path ../anchor-next \
  --family v2-preview \
  --out-dir crates/seagrass-anchor-v2-preview/src/generated \
  --check
```

When bumping Anchor support:

- update the root `anchor-syn` git revision
- regenerate `src/generated/*.rs`
- inspect support-matrix and fingerprint changes
- commit the dependency pin, generated files, and semantic LSP updates together

## Quality Gates

Use targeted tests while iterating, then run the production gate before review:

```sh
cargo test -p seagrass
bun scripts/check-rule-hygiene.ts
bun scripts/check-source-size.ts
bun scripts/verify-production.ts
```

See `docs/quality-kit.md` for fuzzing, property tests, and hotpath replay.

## Commit Messages

Use Conventional Commits:

```text
<prefix>: <summary>

- Change item 1
- Change item 2
```

Allowed prefixes: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`,
`build`, `ci`, `chore`, `style`, and `revert`.

## DCO

Sign off commits:

```sh
git commit -s
```

The sign-off certifies that you have the right to submit the contribution under
the project license.

## Review SLA

Security fixes get first review. Normal pull requests should receive an initial
maintainer response within two business days. Release-blocking LSP regressions
should include a failing fixture or protocol transcript when possible.
