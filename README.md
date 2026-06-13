<p align="center">
  <img src="docs/assets/readme-banner.png" alt="Seagrass — static analysis for Solana programs" />
</p>

<p align="center">
  <strong>Static analysis for Solana programs — catches the Anchor and account-model mistakes that compile cleanly and break on-chain.</strong><br/>
  Codebase intelligence for Solana programs, in your editor, on the command line, and in CI.
</p>

<p align="center">
  <a href="LICENSE"><img alt="MIT" src="https://img.shields.io/badge/license-MIT-blue.svg" /></a>
  <img alt="version 0.1.2" src="https://img.shields.io/badge/version-0.1.2-green.svg" />
  <img alt="rust 1.89.0" src="https://img.shields.io/badge/rust-1.89.0-orange.svg" />
</p>

---

## Install

```sh
cargo install --git https://github.com/heyAyushh/seagrass seagrass-cli --locked
```

Needs Rust 1.89+ (pinned in `rust-toolchain.toml`). Now point it at your code:

```sh
seagrass diagnostics programs/ --json
```

Run `seagrass` with **no arguments** and it speaks LSP over stdio — that's how the editors below connect.

---

## See it work

This struct compiles. It won't run — an `init` account needs a `payer`:

```rust
#[derive(Accounts)]
pub struct CreateVault<'info> {
    #[account(init, space = 8 + 64)]   // no payer =
    pub vault: Account<'info, Vault>,
    pub user: Signer<'info>,
}
```

```sh
seagrass diagnostics src/lib.rs --json
```

```json
{
  "code": "anchor-init-constraints",
  "topic": "seagrass/anchor.init.missing-payer",
  "severity": "ERROR",
  "message": "Anchor `init` constraint is missing `payer = ...`; add the account that funds initialization.",
  "docsUrl": "..."
}
```

Exit code is `1` when something's wrong, `0` when clean. The same finding shows up **inline in your editor with a one-click fix** — before you ever run `anchor build`.

Why generic Rust tools miss this: rust-analyzer and clippy understand Rust, not Solana's account model. A missing `payer`, an unchecked account `owner`, a PDA seeded only from byte literals — all valid Rust, all on-chain bugs. Seagrass reads the same source your compiler does and reasons about that layer.

---

## What it catches

40 rules in four families. Every rule has a stable ID, a doc page, and a suppression form — browse them all in [`docs/lints/`](docs/lints/).

- **Anchor constraints** — `init` missing payer or space, constraints that reference accounts that don't exist, malformed constraint expressions.
- **Security** — unchecked account owners and signers, CPIs to unvalidated program IDs, collidable PDA seeds, type cosplay. (Several carry [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks) references.)
- **Code quality** — unchecked arithmetic on lamports, non-canonical bump seeds, accounts read stale after a CPI, lamport loss on close.
- **Artifacts** — IDL drift, deploy keypair vs declared program ID, stale generated types.

Silence a finding with the narrowest scope that fits:

```rust
// seagrass-allow: seagrass/security.owner-check     // this line only
// seagrass-ignore-file                              // whole file, all rules
```

---

## What it won't do

> Seagrass never invents runtime facts from code shape.

Runtime intelligence is an evidence-ingestion boundary.

Every finding tells you which evidence layer it came from:

- **Static** — your source, manifests, IDLs, and binaries. Always on; the basis for all 40 rules.
- **Preflight** — validates 12 Anchor runtime errors from an invocation JSON file, no validator required. Try it: `seagrass preflight fixtures/preflight/anchor-errors.json --json`.
- **Runtime** — live compute, traffic, account state. Stays unavailable unless you feed in telemetry.

More in [`docs/product-framing.md`](docs/product-framing.md).

---

## In your editor

Seagrass runs alongside rust-analyzer — rust-analyzer keeps doing generic Rust, Seagrass adds the Anchor/Solana layer: constraint-aware completion, account-space hovers, quick fixes, and account/CPI navigation.

- **VS Code** — local dev extension: `cd editors/vscode && bun install && bun run check && code .`, then run **Run Seagrass Extension**. → [`editors/vscode/README.md`](editors/vscode/README.md)
- **Zed** — `zed: extensions` → **Install Dev Extension** → pick `editors/zed`. → [`editors/zed/README.md`](editors/zed/README.md)
- **Vim** — install `vim-lsp`, then load the package at `editors/vim`. → [`editors/vim/README.md`](editors/vim/README.md)
- **Neovim** — load the runtime package at `editors/nvim`; it uses built-in LSP config or `nvim-lspconfig`. → [`editors/nvim/README.md`](editors/nvim/README.md)
- **vi** — classic POSIX `vi` is CLI-only. → [`editors/vi/README.md`](editors/vi/README.md)
- **Any LSP client** — command `seagrass`, language `rust`, root markers `Anchor.toml` / `Seagrass.toml` / `Cargo.toml`.

---

## For agents & CI

In CI, lint and upload SARIF for GitHub code scanning:

```yaml
- run: cargo install --git https://github.com/heyAyushh/seagrass seagrass-cli --locked
- run: seagrass diagnostics programs/ --sarif > seagrass.sarif
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: seagrass.sarif
```

Ready-made workflow: [`docs/templates/seagrass-diagnostics-sarif.yml`](docs/templates/seagrass-diagnostics-sarif.yml).

For AI agents, Seagrass ships six skills (`install`, `lint`, `explain`, `suppress`, `debug-fp`, `audit`) and serves version-matched instructions straight from the binary:

```sh
seagrass skills list --json
seagrass skills get audit --full
```

Full agent guide: [`docs/agents.md`](docs/agents.md). The installed-CLI
discovery contract is documented in
[`docs/agent-skill-help.md`](docs/agent-skill-help.md).

---

## Framework support

| Framework | Deep Anchor analysis | Security diagnostics |
|---|---|---|
| Anchor v1 | ✅ | ✅ |
| Anchor v2 (preview) | ✅ | ✅ |
| Native Solana | — | ✅ |
| Pinocchio | — | ✅ |

Anchor gets the deep treatment — constraints, completions, IDL and keypair checks. Native and Pinocchio share the same security engine (owner, type, signer, CPI) where the semantics line up. Findings are account-scoped: a guard on one account never silences another in the same handler. Details in [`docs/framework-parity.md`](docs/framework-parity.md).

---

## Develop

```sh
cargo test -p seagrass              # core tests
bun scripts/verify-production.ts    # full gate: LSP, editors, versions, hygiene
```

[`CONTRIBUTING.md`](CONTRIBUTING.md) has the full workflow and quality gates. Driving an agent on Seagrass itself? [`AGENTS.md`](AGENTS.md) is its guide to the repo.

---

## License

MIT — see [`LICENSE`](LICENSE). Seagrass builds on generated Anchor catalogs derived from [otter-sec/anchor](https://github.com/otter-sec/anchor) (Apache-2.0); [`NOTICE`](NOTICE) carries the attribution.

---

<p align="center">
  <img src="docs/assets/readme-footer.png" alt="" />
</p>
