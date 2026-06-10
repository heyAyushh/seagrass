# Plan 07 — Ecosystem Distribution: Prebuilt Binaries, VS Code Extension, Zero-Config, Editor Docs

## Context

**What seagrass is**: An LSP for Solana programs (Anchor v1/v2-preview, Pinocchio, native).
Tower-lsp + syn/tree-sitter + salsa 0.26 stack. Binary name: `seagrass` (crate: `seagrass-cli`).

**Current state of distribution**:

- GitHub Releases CI (`/.github/workflows/release.yaml`) already builds and uploads 5
  binary targets (aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu,
  x86_64-unknown-linux-musl, x86_64-pc-windows-msvc) plus a VSIX and a Zed wasm tarball.
  Each asset gets a `.sha256` companion and build-provenance attestation.
- Packaging scripts exist: `scripts/package-release.ts` handles `--server`, `--vscode`,
  `--zed`, `--fuzz-corpus` modes.
- VS Code extension (`editors/vscode/`) is a thin LSP client. It resolves the server via
  `seagrass.serverCommand` (default `"seagrass"` on PATH). It does NOT contain a bundled
  server binary or a download-on-activate flow. Publisher is `seagrass-local`; not yet
  published to the VS Code Marketplace.
- Zed extension (`editors/zed/`) is dev-extension only; not in the Zed registry.
- `skills/seagrass-install/SKILL.md` documents source/crates.io install; Marketplace
  install is marked conditional ("when published").
- Framework auto-detection is fully implemented server-side in
  `src/solana/project/mod.rs`: `SolanaProjectKind` enum (Anchor / Pinocchio /
  NativeSolana), detection via `Anchor.toml` presence (`detect_anchor_programs`) and
  Cargo.toml dependency inspection (`classify_program`). No gap here for detection itself.

**Sequencing gate**: plan-07 builds on plan-01 (FP fixes for corpus programs) and plan-03
(heuristic lint prune). Do NOT widen distribution before both plans are merged to main
and their acceptance criteria pass. Widening install base before FP fixes multiplies user
exposure to the ~38 known false positives.

**Current version**: `0.1.2` (in `VERSION`, `Cargo.toml` workspace, `editors/vscode/package.json`,
`editors/zed/Cargo.toml`, `editors/zed/extension.toml`). Release readiness evidence in
`docs/release-readiness.json` is all-pending; the release gate in `release.yaml` will
block a GitHub release until that evidence is populated.

**Key file paths** (all verified to exist unless marked NEW):

| Path | Status | Purpose |
|---|---|---|
| `/.github/workflows/release.yaml` | exists | Tag-triggered release pipeline |
| `/.github/workflows/pr.yaml` | exists | PR gate; runs cargo test, LSP smoke, readiness shape check |
| `/scripts/package-release.ts` | exists | Packages server tarballs, VSIX, Zed wasm, fuzz corpus |
| `/scripts/bump-version.ts` | exists | Atomic version bump across all manifests |
| `/scripts/check-release-readiness.ts` | exists | Validates `docs/release-readiness.json` |
| `/docs/release-readiness.json` | exists | Evidence record; currently all-pending |
| `/editors/vscode/src/extension.ts` | exists | VS Code extension entry point |
| `/editors/vscode/package.json` | exists | Extension manifest; publisher=seagrass-local |
| `/editors/vscode/.vscodeignore` | exists | VSIX exclusion list |
| `/editors/zed/extension.toml` | exists | Zed extension metadata |
| `/skills/seagrass-install/SKILL.md` | exists | Agent install skill |
| `/src/solana/project/mod.rs` | exists | Server-side framework auto-detection |
| `/.github/workflows/publish-vscode.yaml` | **NEW** | Separate workflow to publish VSIX to Marketplace |
| `/editors/vscode/src/serverInstall.ts` | **NEW** | Binary download/cache logic for bundled-server mode |
| `/docs/plans/plan-07-distribution.md` | this file | Execution plan |

---

## Non-Goals

- Full crates.io publish flow (owned by release-plz; do not modify `release-plz.toml`).
- Neovim plugin packaging (document config only; no lua plugin repo).
- Zed extension registry submission (out of scope until registry accepts it; document
  dev-extension path only).
- ARM Linux binary (not in existing CI matrix; do not add without cross-compile validation).
- Automatic update mechanism inside the VS Code extension (VS Code handles updates when
  published to Marketplace; no in-extension auto-updater needed).
- Changing the existing release gate shape (`check-release-readiness.ts` logic is correct;
  evidence must be earned, not bypassed).
- Any diagnostic logic, FP fixes, or lint changes (those are plan-01 and plan-03).

---

## Steps

### Step 0 — Verify baseline before starting

**Mandatory. Do not proceed if this fails.**

```bash
cd /path/to/seagrass  # repo root
cargo test --workspace 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings 2>&1 | tail -5
```

Both must exit 0. The working tree has ~50 modified files (uncommitted at plan write time).
Commit or stash all current changes as a baseline commit before starting any step below.
Commit message format: `chore: baseline before plan-07 distribution work`.

Verify the baseline test count matches ≥ 1485 passing tests.

---

### Step 1 — Fix VS Code extension publisher ID

**Files**: `editors/vscode/package.json`

Current publisher is `seagrass-local`. For Marketplace publishing the publisher must match
the registered VS Code Marketplace publisher account. The correct publisher ID must be
decided by the project owner (the publisher account on marketplace.visualstudio.com).

1. The project owner registers or uses an existing Marketplace publisher (e.g. `seagrass`
   or `makrozoia`). Record the chosen publisher ID.
2. Edit `editors/vscode/package.json`: change `"publisher": "seagrass-local"` to
   `"publisher": "<chosen-id>"`.
3. The `activationEvents` list already includes `"workspaceContains:Anchor.toml"` — keep
   it. Do not add other activation events.
4. Confirm `"engines": { "vscode": "^1.90.0" }` still holds (minimum VS Code version;
   do not lower it).
5. Run `bun install && bun run check` in `editors/vscode/` — must exit 0.

**Doctrine respected**: No detection logic changed. No new magic numbers. If publisher
account does not exist yet, do NOT change the file — mark this step blocked.

---

### Step 2 — Add binary download/cache to VS Code extension (bundled-server mode)

**Files**: `editors/vscode/src/extension.ts`, `editors/vscode/src/serverInstall.ts` (NEW),
`editors/vscode/package.json`

The extension currently resolves `seagrass` from PATH only. For zero-Rust-toolchain install,
it must download a matching prebuilt binary on first activation if the command is not found.

**What to implement in `editors/vscode/src/serverInstall.ts`** (NEW file):

```
Exported function: resolveServerBinary(context: vscode.ExtensionContext, version: string): Promise<string>

Logic:
1. If seagrass.dev.useCargoFromCheckout is true → return "cargo" (existing behaviour, no change).
2. If seagrass.serverCommand is explicitly set (non-empty, non-default) → return that value.
3. Check if `seagrass` is on PATH and its --version output contains `version`. If yes → return "seagrass".
4. Check extension global storage for a cached binary at:
     <globalStoragePath>/bin/seagrass-<version>[.exe on Windows]
   If it exists and is executable → return that path.
5. Download from GitHub Releases:
     https://github.com/heyAyushh/seagrass/releases/download/v<version>/<archive-name>
   where archive-name is derived from the current platform:
     - darwin + arm64  → seagrass-<version>-aarch64-apple-darwin.tar.gz
     - darwin + x64    → seagrass-<version>-x86_64-apple-darwin.tar.gz
     - linux + x64     → seagrass-<version>-x86_64-unknown-linux-gnu.tar.gz
     - win32 + x64     → seagrass-<version>-x86_64-pc-windows-msvc.zip
   Show a VS Code progress notification: "Seagrass: downloading server v<version>..."
   Extract to <globalStoragePath>/bin/.
   Verify sha256 against the matching .sha256 file from the same release.
   If sha256 mismatch → delete downloaded file, show error, throw.
   Return extracted binary path.
6. If platform is not in the matrix above → show error "Seagrass: prebuilt binary not
   available for this platform. Install from source: cargo install seagrass-cli --locked"
   and throw.
```

**Constants** (use named constants, no magic strings inline):

```ts
const GITHUB_RELEASE_BASE = "https://github.com/heyAyushh/seagrass/releases/download";
const SERVER_BINARY_NAME = "seagrass";
const SERVER_VERSION = "<injected at build time from VERSION file>";
```

`SERVER_VERSION` must be injected at build time. The `bun build` CLI `--define` flag
only accepts literal string substitutions — it does NOT evaluate JS expressions such as
`readFileSync(...)`. Use a shell wrapper to read `VERSION` before invoking `bun build`.

Update `editors/vscode/package.json`'s `build` script to:

```json
"build": "VERSION=$(cat ../../VERSION | tr -d '[:space:]') && bun build ./src/extension.ts --target=node --format=cjs --external=vscode --outdir ./dist --define \"SEAGRASS_VERSION='\\\"$VERSION'\\\"\""
```

This reads `VERSION` from the repo root at build time (shell `cat`), then passes it as a
quoted string literal to `--define` so `bun build` replaces every occurrence of the bare
identifier `SEAGRASS_VERSION` with the version string (e.g. `"0.1.2"`).

In `serverInstall.ts`, declare:
```ts
declare const SEAGRASS_VERSION: string;
const SERVER_VERSION = SEAGRASS_VERSION;
```
The `declare const` keeps TypeScript happy at type-check time; `bun build` replaces the
identifier with the literal string at bundle time.

Verify injection after building:
```bash
cd editors/vscode && bun run build
grep '0\.1\.' dist/extension.js | head -3
```
If the version string does not appear, the `--define` substitution failed.

Do NOT hard-code the version string — it must come from `VERSION` via this build step so
`scripts/bump-version.ts` keeps it current automatically.

**Wire into `extension.ts`** — there are TWO call sites that must both be updated:

**Call site 1 — `startClient` (line ~211)**:

`startClient` calls `readServerLaunchConfig(context)` which delegates to
`readServerLaunchConfigFromWorkspace(context)`. In
`readServerLaunchConfigFromWorkspace`, replace the line:
```ts
const command = useCargo ? "cargo" : config.get<string>("serverCommand") || "seagrass";
```
with an `await` on `resolveServerBinary`. Because `readServerLaunchConfigFromWorkspace`
is currently synchronous, you must make both it and `readServerLaunchConfig` async:

```ts
async function readServerLaunchConfig(context: vscode.ExtensionContext): Promise<ServerLaunchConfig> {
  return readServerLaunchConfigFromWorkspace(context);
}

async function readServerLaunchConfigFromWorkspace(
  context?: vscode.ExtensionContext,
): Promise<ServerLaunchConfig> {
  const config = vscode.workspace.getConfiguration("seagrass");
  const useCargo = config.get<boolean>("dev.useCargoFromCheckout", false);
  // resolveServerBinary handles cargo-checkout, explicit serverCommand, PATH check,
  // cached binary, and download — see serverInstall.ts
  const command = await resolveServerBinary(context, SERVER_VERSION);
  // ... rest unchanged
```

Update the `startClient` call site:
```ts
const launch = await readServerLaunchConfig(context);
```

**Call site 2 — `scanWorkspace` handler (line ~456)**:

`scanWorkspace` calls `readServerLaunchConfigFromWorkspace()` (without `context`) and
uses its `command` directly for a one-shot `execFileAsync` invocation:
```ts
const launch = readServerLaunchConfigFromWorkspace();   // <-- must also be updated
const command = launch.useCargo ? "cargo" : launch.command;
```
Update this to:
```ts
const launch = await readServerLaunchConfigFromWorkspace(context);
const command = launch.useCargo ? "cargo" : launch.command;
```
where `context` is the `vscode.ExtensionContext` captured in the activation closure.
`scanWorkspace` must receive `context` as a parameter (or close over it from the
`activate` function scope — the existing code already has `context` in scope at the
point where `scanWorkspace` is registered as a command handler, so passing it via
closure is the simpler approach).

**Why this matters**: `scanWorkspace` runs independently of `startClient`. Without this
fix, a user who scans a workspace on a fresh VS Code install (no `seagrass` on PATH, no
running client) will fail to find the downloaded binary and the scan will error out.
The resolution of the server binary must go through the same `resolveServerBinary` path
in both call sites.

**`editors/vscode/package.json` node dependencies**: add `node-fetch` or use the built-in
`https` module (Node.js 18+ has `fetch` globally; use that, no new dependency). Add
`adm-zip` (for `.zip` extraction on Windows) and `tar` (for `.tar.gz` on non-Windows) as
dependencies if not already present, OR use `child_process.execFile` to shell out to
`tar`/`unzip` (simpler; fewer dependencies). Shell-out approach is preferred; add no new
npm dependencies.

**`.vscodeignore`**: already excludes `src/**`, `node_modules/**`. No change needed
for the downloaded binary (it goes to globalStorage, not extension directory).

After implementing, run:
```bash
cd editors/vscode && bun run check
```
Must exit 0.

---

### Step 3 — Verify framework auto-detection coverage and smoke-test it

**Files**: `src/solana/project/mod.rs` (read-only verification), `src/solana/project/tests/` (if gaps found)

Server-side auto-detection is already fully implemented:
- `detect_anchor_programs`: walks up from file looking for `Anchor.toml`; classifies as Anchor.
- `detect_cargo_for_document` + `classify_program`: checks Cargo.toml for pinocchio
  dependencies (see `PINOCCHIO_DEPENDENCIES` constant), then native solana deps; falls
  back to `None` if neither.

**Verify the following tests exist and pass** (search, do not write if already present):

```bash
cargo test -p seagrass -- detects_pinocchio_from_manifest_dependency 2>&1 | grep -E 'test .* ok|1 passed'
cargo test -p seagrass -- detects_native_solana_from_manifest_dependency 2>&1 | grep -E 'test .* ok|1 passed'
cargo test -p seagrass -- detects_anchor_from_toml_presence 2>&1 | grep -E 'test .* ok|1 passed'
```

Note: always pipe through `grep -E 'test .* ok|1 passed'` — `cargo test` exits 0 even
when a name filter matches zero tests (it prints "running 0 tests"), which silently masks
a missing test. The `grep` makes a zero-test run fail the acceptance check.

The tests `detects_pinocchio_from_manifest_dependency` and
`detects_native_solana_from_manifest_dependency` already exist (verified in
`src/solana/project/mod.rs` at lines ~449 and ~530 respectively).

The test `detects_anchor_from_toml_presence` does NOT yet exist. Add it in
`src/solana/project/mod.rs` inside the `#[cfg(test)] mod tests` block, following the
exact pattern of `detects_pinocchio_from_manifest_dependency`:

```rust
#[test]
fn detects_anchor_from_toml_presence() {
    let root = unique_temp_dir("seagrass-anchor-project");
    std::fs::create_dir_all(root.join("src")).unwrap();
    // Anchor.toml presence is sufficient to classify as Anchor
    std::fs::write(root.join("Anchor.toml"), "[workspace]\nmembers = []\n").unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"anchor-demo\"\n\n[lib]\ncrate-type = [\"cdylib\", \"lib\"]\n\n[dependencies]\nanchor-lang = \"0.31\"\n",
    )
    .unwrap();
    let source = r#"
use anchor_lang::prelude::*;
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
#[program]
mod anchor_demo {}
"#;
    let lib = root.join("src/lib.rs");
    std::fs::write(&lib, source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let program = detect_for_document(&uri, &document).unwrap();

    assert_eq!(program.kind, SolanaProjectKind::Anchor);
    assert_eq!(program.name, "anchor_demo");
    assert_eq!(program.root, root);
}
```

After adding, run:
```bash
cargo test -p seagrass -- detects_anchor_from_toml_presence 2>&1 | grep -E 'test .* ok|1 passed'
```
Must print a line matching `test .* ok` or `1 passed`.

If a test for native-only detection (`solana-program` dep + `entrypoint!` macro, no
Anchor.toml) does not exist, add one in `src/solana/project/mod.rs` following the pattern
of `detects_pinocchio_from_manifest_dependency` (lines ~449–479 of that file).
`detects_native_solana_from_manifest_dependency` already covers this case — confirm it
passes before proceeding.

**Gap check**: The `classify_program` function requires `crate-type = ["cdylib"]` in
`[lib]` to classify any project. This correctly rejects bin-only crates (correct
behaviour). Confirm this is intentional by reading the function — no change needed.

**Do not change** detection logic. Doctrine: silence on failure, never a guess.

---

### Step 4 — Marketplace publish workflow (NEW CI file)

**File**: `.github/workflows/publish-vscode.yaml` (NEW)

This workflow must be manually triggered (`workflow_dispatch`) only. It must NOT run
automatically on every push to main (that is the job of the release gate). It runs AFTER
a GitHub Release is published (the release.yaml workflow uploads the VSIX as a release
asset). The operator downloads the VSIX from the GitHub Release, verifies it, and triggers
this workflow to push it to the Marketplace.

```yaml
name: Publish VS Code Extension

on:
  workflow_dispatch:
    inputs:
      tag:
        description: "Release tag to publish, e.g. v0.1.2"
        required: true
        type: string

jobs:
  publish:
    name: Publish VSIX to VS Code Marketplace
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2
        with:
          ref: ${{ inputs.tag }}

      - uses: actions/setup-node@6044e13b5dc448c55e2357c09f80417699197238 # v6.2.0
        with:
          node-version: "24"
          package-manager-cache: false

      - name: Install Bun
        run: node scripts/install-bun-ci.mjs

      - name: Extract version from tag
        id: meta
        run: |
          tag="${{ inputs.tag }}"
          echo "version=${tag#v}" >> "$GITHUB_OUTPUT"

      - name: Download VSIX from GitHub Release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          VERSION: ${{ steps.meta.outputs.version }}
          TAG: ${{ inputs.tag }}
        run: |
          set -euo pipefail
          gh release download "$TAG" \
            --pattern "seagrass-vscode-${VERSION}.vsix" \
            --pattern "seagrass-vscode-${VERSION}.vsix.sha256" \
            --dir artifacts/
          (cd artifacts && sha256sum -c "seagrass-vscode-${VERSION}.vsix.sha256")

      - name: Publish to VS Code Marketplace
        env:
          VSCE_PAT: ${{ secrets.VSCE_PAT }}
          VERSION: ${{ steps.meta.outputs.version }}
        run: |
          npm exec --yes "@vscode/vsce@3.9.1" -- \
            publish \
            --packagePath "artifacts/seagrass-vscode-${VERSION}.vsix" \
            --pat "$VSCE_PAT"
```

**Secret required**: `VSCE_PAT` must be added to the repository secrets by the project
owner (token from marketplace.visualstudio.com → publisher → Personal Access Tokens).
Document this in the workflow as a comment.

**Do not add** an OpenVSX (ovsx) publish step unless explicitly requested. Keep scope minimal.

---

### Step 5 — Install script for headless / non-Rust environments

**File**: `scripts/install.sh` (NEW)

A POSIX shell script that downloads, verifies, and installs the matching prebuilt binary.
No Rust toolchain required.

```sh
#!/bin/sh
# Usage: curl -fsSL https://raw.githubusercontent.com/heyAyushh/seagrass/main/scripts/install.sh | sh
# Or: sh scripts/install.sh [--version v0.1.2] [--install-dir /usr/local/bin]
```

Logic:
1. Detect OS (`uname -s`) and arch (`uname -m`). Map to target triple:
   - Linux + x86_64 → `x86_64-unknown-linux-gnu`
   - macOS + arm64/aarch64 → `aarch64-apple-darwin`
   - macOS + x86_64 → `x86_64-apple-darwin`
   - Anything else → print error and exit 1.
2. Parse `--version` arg; if absent, fetch latest tag from GitHub API:
   `https://api.github.com/repos/heyAyushh/seagrass/releases/latest` (requires `curl` or `wget`).
3. Construct download URL:
   `https://github.com/heyAyushh/seagrass/releases/download/<tag>/seagrass-<version>-<target>.tar.gz`
4. Download archive and `.sha256` file to a temp directory (`mktemp -d`).
5. Verify sha256 (`sha256sum -c` on Linux; `shasum -a 256 -c` on macOS).
6. Extract binary. Install to `--install-dir` (default `/usr/local/bin` if writable, else
   `$HOME/.local/bin`; create the dir if needed).
7. Print: `Seagrass <version> installed to <path>. Run: seagrass --version`.
8. Clean up temp dir.

Use named shell variables for all URL components. No heredocs for binary content.
POSIX-compatible: no bashisms. Tested with `sh` (not just `bash`).

After writing the file: `chmod +x scripts/install.sh`. Verify with shellcheck if available:
```bash
shellcheck scripts/install.sh
```

---

### Step 6 — Update `skills/seagrass-install/SKILL.md`

**File**: `skills/seagrass-install/SKILL.md`

Replace the "Install the binary" section (Step 1) to document three install paths in
priority order:

1. **Prebuilt binary (recommended, no Rust required)**:
   ```bash
   curl -fsSL https://raw.githubusercontent.com/heyAyushh/seagrass/main/scripts/install.sh | sh
   # or manually from https://github.com/heyAyushh/seagrass/releases
   ```
2. **From crates.io** (requires Rust 1.89.0):
   ```bash
   cargo install seagrass-cli --locked
   ```
3. **From source checkout** (requires Rust 1.89.0):
   ```bash
   rustup toolchain install 1.89.0 --profile minimal
   cargo install --path crates/seagrass --locked
   ```

Replace the VSCode section ("Configure the editor → VSCode") to reflect two extension
install paths:

1. **VS Code Marketplace** (primary once published):
   ```
   code --install-extension <publisher>.seagrass-vscode
   # or: Extensions panel → search "Seagrass"
   ```
   Replace `<publisher>` with the confirmed publisher ID from Step 1.
   **GATE**: If Step 1 is blocked (publisher account does not yet exist), do NOT replace
   the existing `seagrass-local.seagrass-vscode` identifier in this file with
   `<publisher>.seagrass-vscode`. Instead, keep the current text and add a TODO comment:
   ```
   # TODO: replace seagrass-local with confirmed publisher ID once Step 1 is unblocked
   code --install-extension seagrass-local.seagrass-vscode
   ```
   Replacing it with a placeholder `<publisher>.seagrass-vscode` introduces a broken
   install command that is worse than the existing working (local) ID.

   The extension downloads the matching server binary automatically on first activation;
   no manual server install required when using the Marketplace extension.

2. **VSIX from GitHub Release** (for offline or pinned-version installs):
   ```bash
   # Download seagrass-vscode-<version>.vsix from the GitHub Release page
   code --install-extension seagrass-vscode-<version>.vsix
   ```
   Same auto-download behaviour applies.

3. **Local dev** (from a seagrass checkout):
   ```bash
   cd editors/vscode && bun install && bun run build
   # Then F5 in VS Code to launch extension host
   ```
   Set `seagrass.serverCommand` to point at the local binary if the PATH binary is stale.

Keep the Helix and Neovim sections unchanged. Keep the smoke-check section unchanged.
Keep the "Stop conditions" section unchanged.

**Do not invent** new settings keys or commands not present in `editors/vscode/package.json`.

---

### Step 7 — Editor docs: Neovim and Helix verification pass

**Files**: `skills/seagrass-install/SKILL.md` (already covers Helix/Neovim configs),
any README files in `editors/` that reference manual LSP config.

1. Read the Helix config block in the skill. Verify `command = "seagrass"` matches the
   installed binary name. No change required if correct.
2. Read the Neovim config block. Verify `root_pattern("Anchor.toml", "Cargo.toml", "Seagrass.toml")`
   is correct — this must trigger when any of those files exist. Cross-check with server-side
   detection in `src/solana/project/mod.rs` (already verified: Anchor.toml triggers Anchor
   detection; Cargo.toml triggers pinocchio/native detection). Pattern is correct.
3. Verify that `nvim-lspconfig` custom config (NOT upstream lspconfig, since seagrass is
   not upstream) correctly uses `configs.seagrass = { default_config = { ... } }`. This
   pattern is correct for custom servers.
4. If any path in the docs references `seagrass-server` instead of `seagrass`, correct it
   (binary name is `seagrass`, built from `seagrass-cli` crate).
5. Verify Zed instructions: `editors/zed/` is dev-extension only; the skill correctly
   documents dev-extension install. The Zed extensions registry path is NOT documented
   as available (correct — not yet registered). Confirm no change needed.

No new files for this step unless corrections are found.

---

### Step 8 — Integration smoke test (manual gate before publishing to Marketplace)

**This step is manual and must be run on a clean machine without Rust toolchain installed.**

Procedure:
1. On a fresh macOS or Linux VM with no Rust toolchain, no `seagrass` binary on PATH:
   ```bash
   curl -fsSL https://raw.githubusercontent.com/heyAyushh/seagrass/main/scripts/install.sh | sh
   seagrass --version   # must print version matching the release
   ```
2. Clone a known Anchor workspace (e.g. `solana-playground` examples or any repo with
   `Anchor.toml`). Open it in VS Code with the VSIX installed:
   ```bash
   code --install-extension seagrass-vscode-<version>.vsix /path/to/anchor-workspace
   ```
3. Open any `.rs` file from the workspace. Observe:
   - Status bar shows "$(sync~spin) Seagrass" then "$(check) Seagrass" (not "stopped").
   - At least one diagnostic appears in a file with a known violation.
4. Run the smoke-check fixture from `skills/seagrass-install/SKILL.md` Step 3.
   Confirm output contains `"topic": "seagrass/solana.code-quality.unchecked-arithmetic"`.
5. If any step above fails: DO NOT publish to Marketplace. Debug first.

---

## Acceptance Criteria

All must pass before the plan is complete:

```bash
# 1. Baseline tests green
cargo test --workspace
# Expected: ≥ 1485 tests pass, 0 failures

# 2. Clippy clean
cargo clippy --all-targets -- -D warnings
# Expected: exit 0

# 3. Framework detection tests pass (pipe through grep to catch zero-test runs)
cargo test -p seagrass -- detects_pinocchio_from_manifest_dependency 2>&1 | grep -E 'test .* ok|1 passed'
cargo test -p seagrass -- detects_native_solana_from_manifest_dependency 2>&1 | grep -E 'test .* ok|1 passed'
cargo test -p seagrass -- detects_anchor_from_toml_presence 2>&1 | grep -E 'test .* ok|1 passed'
# Expected: each grep matches; a "running 0 tests" line (zero-match filter) causes grep to exit non-zero

# 4. VS Code extension typecheck + build pass
cd editors/vscode && bun install && bun run check
# Expected: exit 0

# 5. VSIX packages successfully
bun scripts/package-release.ts --vscode --version 0.1.2
# Expected: artifacts/seagrass-vscode-0.1.2.vsix created, sha256 written

# 6. Install script shellcheck
shellcheck scripts/install.sh
# Expected: exit 0 (no errors; warnings acceptable if intentional)

# 7. Server binary packages for a target
bun scripts/package-release.ts --server --version 0.1.2 \
  --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/seagrass \
  --skip-build
# Expected: artifacts/seagrass-0.1.2-aarch64-apple-darwin.tar.gz created

# 8. PR gate passes (simulates CI)
# In CI: .github/workflows/pr.yaml runs cargo test + LSP smoke + readiness shape check
# Locally verify readiness shape:
bun scripts/check-release-readiness.ts --allow-pending --version 0.1.2
# Expected: shape valid (fields present), gate not blocked on shape errors

# 9. Manual smoke test (see Step 8 above)
# Expected: binary installs without Rust, extension activates, diagnostic fires
```

**Observable end state**:
- `scripts/install.sh` exists, is POSIX-compatible, shellcheck-clean.
- `editors/vscode/src/serverInstall.ts` exists; `extension.ts` calls it.
- `.github/workflows/publish-vscode.yaml` exists; triggers only on `workflow_dispatch`.
- `skills/seagrass-install/SKILL.md` documents three server install paths and two
  extension install paths.
- Publisher ID in `editors/vscode/package.json` is updated (or step is marked blocked
  if publisher account does not yet exist).
- Zero new test failures vs baseline.
- Zero new clippy warnings.

---

## Risks and Edge Cases

**R1 — Release gate blocks Marketplace publish** (HIGH): `docs/release-readiness.json`
is all-pending; the existing `release.yaml` gate will block a GitHub Release today,
which means no VSIX exists on a GitHub Release to download for the publish-vscode
workflow. Resolution: populate evidence (fuzz hours + external review) before triggering
any publish workflow. This plan does NOT bypass the gate.

**R2 — SHA256 mismatch on downloaded binary** (MEDIUM): The verify step in
`serverInstall.ts` and `install.sh` must fail loudly (not silently skip). Implement
sha256 check as a hard gate in both. Delete partially-downloaded or corrupt files on
failure.

**R3 — Extension cwd for binary resolution** (MEDIUM): The existing extension resolves
`seagrass` relative to `process.cwd()` (line 732 of `extension.ts`) as fallback CWD.
The new `resolveServerBinary` uses `globalStoragePath` for the cached binary, which is
stable. Confirm `context.globalStorageUri.fsPath` is available in the activation context
(it is for VS Code 1.90+; the existing engine requirement `^1.90.0` covers this).

**R4 — VERSION injection into VSIX build** (MEDIUM): The `bun build` CLI `--define` flag
only accepts literal string substitutions — it does NOT evaluate JS expressions at build
time. Do NOT attempt to use `readFileSync` inside a `--define` value; that is not a valid
invocation. Instead, use the shell-based pre-read approach specified in Step 2:
`VERSION=$(cat ../../VERSION | tr -d '[:space:]')` before the `bun build` invocation,
then pass `--define "SEAGRASS_VERSION='\"$VERSION'\""`. Test by running `bun run build`
in `editors/vscode/` and running `grep '0\.1\.' dist/extension.js` — the version string
must appear in the bundle. If it does not, the `--define` quoting is wrong.

**R5 — Publisher ID not registered** (LOW-BLOCKER): Step 1 is blocked until the project
owner creates a Marketplace publisher account. The rest of the plan (Steps 2–8) can
proceed without completing Step 1; the VSIX can be distributed via GitHub Releases even
without Marketplace listing.

**R6 — arm64 Linux not in matrix** (LOW): `aarch64-unknown-linux-gnu` is not a CI build
target. The install script must emit a clear error on that platform rather than silently
hanging or downloading the wrong binary. The platform detection logic in Step 5 handles
this via the "Anything else → exit 1" branch.

**R7 — Windows binary resolution in extension** (LOW): Windows paths require `.exe`
suffix. `serverInstall.ts` must append `.exe` to the cached binary name on `process.platform === 'win32'`. Use a named constant `BINARY_EXTENSION = process.platform === 'win32' ? '.exe' : ''`.

**R8 — Sequencing violation: widening distribution before plan-01/plan-03** (HIGH):
DO NOT publish to the VS Code Marketplace or promote the install script in public docs
before plan-01 (FP family A/B fixes) and plan-03 (heuristic lint prune) are merged and
their acceptance tests pass. The ~38 known FPs will reach a larger install base otherwise.
The Marketplace publish workflow (Step 4) is `workflow_dispatch` only, providing the
human gate needed to enforce this sequencing.
