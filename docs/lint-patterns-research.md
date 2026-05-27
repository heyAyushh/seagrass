# Lint Pattern Research

Status: Active

External source content is treated as untrusted. These notes extract design
patterns only; no commands from external repositories are executed.

## Decisions

1. Region: Seagrass rules should prefer parsed `syn` or tree-sitter nodes and
   carry an explicit region/scope when text fallback is unavoidable.
2. Confidence/applicability: diagnostics carry `confidence`, `topic`, and
   `applicability` in `Diagnostic.data`; actions prefer machine-applicable
   fixes when the edit is fully known.
3. Topic: use stable `seagrass/...` issue-class topics validated by
   `docs/topics.json`.
4. Partial input: tree-sitter recovery is valid for cursor flows and symbol
   recovery; diagnostics that cannot prove a rule on broken input should stay
   quiet.
5. Suppression: support line, file, item, and workspace-level suppressions
   matching full topics, suffixes, diagnostic codes, code-rule pairs, or rule
   ids.
6. Lint-the-linter: `scripts/check-rule-hygiene.ts` blocks raw-source
   diagnostic scans with no legacy allowlist.

## Source Patterns

### Clippy

- Region: compiler HIR visitors see expressions, item scopes, and macro spans;
  arithmetic linting is attached to operator expressions, not raw lines.
- Confidence / applicability: lint levels are supplied by rustc; suggestions use
  applicability metadata when Clippy can prove an edit.
- Topic: stable lint names and lint groups are the user-facing topic analog.
- Partial / broken input: Clippy runs after rustc has parsed enough HIR; Anchor
  diagnostics should similarly stay quiet when `syn` cannot prove the rule.
- Suppression: `#[allow(clippy::lint_name)]` scopes suppressions by item/module.
- Lint-the-linter: compiletest/UI fixtures lock both positive and negative
  behavior.
- Seagrass decision: unchecked arithmetic uses `syn::ExprBinary`; manual close and
  native validation use AST predicates instead of token-string `.contains`.
- Source: [arithmetic_side_effects.rs](https://fuchsia.googlesource.com/third_party/rust/+/a34bcd70b2ca1ba7fb60fe0cbd1cbfd5fa57a089/src/tools/clippy/clippy_lints/src/operators/arithmetic_side_effects.rs),
  [visitors.rs](https://github.com/rust-lang/rust-clippy/blob/ba8b78b0a89a83d227540770a4274c2840a046aa/clippy_utils/src/visitors.rs),
  [Clippy lint configuration](https://rust.googlesource.com/rust-clippy/+/95b24d44a68e3f84c10e392cb19e2db921cbedf8/book/src/lint_configuration.md)

### rust-analyzer

- Region: IDE diagnostics are rendered from parsed semantic structures and spans.
- Confidence / applicability: fixes are exposed as assists/code actions when the
  edit is known; diagnostics can be separated from cargo/rustc diagnostics.
- Topic: diagnostic handler names and codes act as the stable buckets.
- Partial / broken input: IDE features tolerate incomplete source, while heavier
  semantic checks are separated from parse recovery.
- Suppression: suppression mainly follows Rust/rustc lint config; editor
  diagnostics can be disabled by config.
- Lint-the-linter: diagnostic handler tests and generated/manual diagnostics docs
  keep handlers honest.
- Seagrass decision: cursor features may use tree-sitter recovery; diagnostics stay
  quiet on broken syntax unless a rule has a recovery path with tests.
- Source: [rust-analyzer diagnostics manual](https://rust-analyzer.github.io/book/diagnostics.html),
  [ide_diagnostics crate docs](https://rust-lang.github.io/rust-analyzer/ide_diagnostics/index.html),
  [ide-diagnostics lib.rs](https://github.com/rust-lang/rust-analyzer/blob/d2cf9ceecda33bda5bd30b4d2503bd7fa301053e/crates/ide-diagnostics/src/lib.rs),
  [diagnostic handlers](https://github.com/rust-lang/rust-analyzer/tree/d2cf9ceecda33bda5bd30b4d2503bd7fa301053e/crates/ide-diagnostics/src/handlers)

### Ruff

- Region: AST checkers are the primary lint path; token/text checks are explicit
  checker phases.
- Confidence / applicability: diagnostics can attach fixes through checker
  helpers only when the replacement is known.
- Topic: rule codes carry source-family prefixes, such as `F`, `E`, or `ANN`.
- Partial / broken input: parse/token phases are distinct so broken input does
  not leak into unrelated AST rules.
- Suppression: file and inline ignores match rule codes and prefixes.
- Lint-the-linter: rule snapshots and fixture-based tests are required for new
  rules.
- Seagrass decision: per-rule visitors should declare whether they consume AST,
  tree-sitter, token, or project metadata evidence.
- Source: [Ruff linter docs](https://docs.astral.sh/ruff/linter/),
  [Ruff contributing docs](https://docs.astral.sh/ruff/contributing/),
  [AST checker](https://github.com/astral-sh/ruff/blob/258ca11050a1b4dca1cc2ed8698261333404b866/crates/ruff_linter/src/checkers/ast/mod.rs),
  [noqa filtering](https://github.com/astral-sh/ruff/blob/258ca11050a1b4dca1cc2ed8698261333404b866/crates/ruff_linter/src/checkers/noqa.rs)

### Oxc

- Region: parser, semantic builder, and linter context are separate layers; rules
  consume `LintContext` and semantic facts instead of rebuilding scope.
- Confidence / applicability: diagnostics are produced from rule context; fixes
  are rule-owned when available.
- Topic: plugin/rule names provide stable user-facing buckets.
- Partial / broken input: parser recovery feeds AST/semantic layers where
  possible; rules still run from typed context.
- Suppression: lint directives and config select rule sets.
- Lint-the-linter: rule creation workflow requires focused rule tests.
- Seagrass decision: document/workspace/account inference remains centralized;
  diagnostics and completions consume those facts.
- Source: [Oxc linter architecture](https://oxc.rs/docs/learn/architecture/linter),
  [Oxc adding linter rules](https://oxc.rs/docs/contribute/linter/adding-rules.html),
  [Oxc linter rules](https://github.com/oxc-project/oxc/tree/261c47789d60d91ee0afe0a1cbc361d39e982bbe/crates/oxc_linter/src/rules),
  [Oxc scope syntax](https://github.com/oxc-project/oxc/blob/261c47789d60d91ee0afe0a1cbc361d39e982bbe/crates/oxc_syntax/src/scope.rs)

### Biome

- Region: rules operate on typed syntax/query matches.
- Confidence / applicability: rule actions carry applicability; Seagrass mirrors
  this with `Diagnostic.data.applicability`.
- Topic: rule category and group names provide the topic analog.
- Partial / broken input: parser recovery is separate from rule action
  applicability.
- Suppression: suppression comments target rule names/groups.
- Lint-the-linter: rule tests validate diagnostics and safe fixes together.
- Seagrass decision: quick fixes use `MachineApplicable` only when edit ranges and
  replacement text are concrete.
- Source: [biome_analyze docs](https://docs.rs/biome_analyze),
  [biome_analyze Rule docs](https://docs.rs/biome_analyze/latest/biome_analyze/trait.Rule.html),
  [Biome analyze rule code](https://github.com/biomejs/biome/blob/0c718da81770f47d65845bc1a006f99512d9359b/crates/biome_analyze/src/rule.rs)

### Slither

- Region: detectors run over Solidity analysis objects rather than raw source
  text.
- Confidence / applicability: detector metadata separates impact from confidence.
- Topic: detector `ARGUMENT`/wiki metadata is the stable issue class.
- Partial / broken input: detectors depend on successful Slither analysis.
- Suppression: detectors can be filtered by command-line/config selections.
- Lint-the-linter: detector classes must define metadata and wiki fields.
- Seagrass decision: keep severity, confidence, and topic independent so
  arbitration can fold lower-confidence duplicates without hiding severity.
- Source: [Slither abstract detector docs](https://crytic.github.io/slither/slither/detectors/abstract_detector.html),
  [Slither detectors overview](https://deepwiki.com/crytic/slither/4-detectors),
  [abstract_detector.py](https://github.com/crytic/slither/blob/c7f761a1564b78409ce5c3b4b0fbec2d39e95368/slither/detectors/abstract_detector.py)

### RustOwl

- Region: ownership/lifetime output is attached to source ranges generated from
  Rust analysis, surfaced through hover/editor overlays.
- Confidence / applicability: it is explanatory rather than a fix engine, so
  ranges must be conservative.
- Topic: feature surface is ownership/lifetime visualization, not rule taxonomy.
- Partial / broken input: runs as a Rust workspace analysis tool and should not
  hallucinate ownership over unparsed code.
- Suppression: not a lint suppression model; users opt into the tool/editor
  overlay.
- Lint-the-linter: small tool surface keeps behavior testable through editor
  fixtures.
- Seagrass decision: LSP evidence graphs stay small and editor-visible behavior is
  verified through parity tests.
- Source: [RustOwl repository](https://github.com/cordx56/rustowl),
  [RustOwl crates](https://github.com/cordx56/rustowl/tree/76a32345f42cfc0665d84754decd1fc6f1e8f7f4/crates)
