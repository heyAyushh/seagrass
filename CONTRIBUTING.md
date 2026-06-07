# Contributing

Status: Active

Work from the repository root unless a command explicitly changes directory.
Keep changes scoped to `src/`, editor adapters, workflows, or generated support
files unless your task explicitly involves other areas.

## Development Setup

Seagrass pins checkout builds to Rust `1.89.0` via `rust-toolchain.toml`, and
all Rust packages declare the same MSRV through workspace metadata.

```sh
rustup toolchain install 1.89.0 --profile minimal --component clippy rustfmt
cargo build -p seagrass-cli
cargo test -p seagrass
cargo install --path crates/seagrass --locked
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

Generated support lives in `src/anchor/generated/`. Normal builds do not scrape
Anchor sources.

Regenerate v1 support from a checkout that contains Anchor's `lang/syn` source
tree:

```sh
bun scripts/regen-support.ts --anchor-path "$SEAGRASS_ANCHOR_PATH" --family v1
```

Check that generated files are current:

```sh
bun scripts/regen-support.ts --anchor-path "$SEAGRASS_ANCHOR_PATH" --family v1 --check
```

Standalone Seagrass checkouts can run the production gate against a separate
Anchor checkout by setting `SEAGRASS_ANCHOR_PATH`. If the variable is unset, the
gate falls back to the pinned Anchor git dependency from Cargo's cache (fetching
it with `cargo fetch --locked` if needed), and fails with a clear error if no
supported Anchor source can be found.

Preview a v2 checkout:

```sh
bun scripts/regen-support.ts \
  --anchor-path ../anchor-next \
  --family v2-preview \
  --out-dir crates/seagrass-anchor-v2-preview/src/generated \
  --check
```

Run the real Anchor v2 preview example-corpus check before claiming v2 preview
coverage:

```sh
bun scripts/check-anchor-v2-preview-corpus.ts --anchor-path ../anchor-next
```

When bumping Anchor support:

- update the root `anchor-syn` git revision
- regenerate `src/anchor/generated/*.rs`
- inspect support-matrix and fingerprint changes
- commit the dependency pin, generated files, and semantic LSP updates together

## Quality Gates

Use targeted tests while iterating, then run the production gate before review:

```sh
cargo test -p seagrass
bun scripts/check-rule-hygiene.ts
bun scripts/check-source-size.ts
bash scripts/smoke-install.sh
bun scripts/verify-production.ts
```

`bun scripts/verify-production.ts` is the full gate: formatting, editor UX
parity, full LSP tests, protocol smoke, shared editor UI contract, diagnostic
rule hygiene, VS Code, Zed, version alignment, generated support, and checked-in
Zed wasm freshness. See `docs/quality-kit.md` for fuzzing, property tests, and
hotpath replay.

## Releasing

Cut a signed release tag only after `VERSION` and every LSP/editor manifest
match the intended version:

```sh
git tag -s v0.1.2
git push origin v0.1.2
```

The release workflow builds and attaches:

- `seagrass-<version>-<target>.tar.gz` for macOS/Linux server binaries
- `seagrass-<version>-x86_64-pc-windows-msvc.zip` for the Windows server binary
- `seagrass-zed-<version>.tar.gz`
- `seagrass-vscode-<version>.vsix`
- matching `.sha256` files

Pre-release tags such as `v0.1.2-rc.1` create GitHub pre-releases.

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

## Agent skills

The `skills/seagrass-*` directory contains markdown-driven skills for Claude
Code, Cursor, and other coding agents. These are the maintained automation docs
for the diagnostics CLI, suppression forms, and lint topics.

When you:
- add or rename a lint topic
- change CLI output shape or flags in `src/app/cli/`
- modify suppression syntax in `src/lsp/diagnostics/suppression.rs`
- add/remove docs in `docs/lints/`

...also update the affected `SKILL.md` files and `skills/README.md` in the same
change. See `AGENTS.md` (Verification Matrix + Maintenance) for the contract.
