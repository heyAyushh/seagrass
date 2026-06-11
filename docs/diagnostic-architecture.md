# Diagnostic Architecture

Status: Active

## Region Model

Rules should emit only from regions they own.

```rust
pub enum Region {
    InstructionBody,
    HelperFnBody,
    AccountsStructField,
    AttributeArguments,
    DocComment,
    StringLiteral,
    UseItem,
    ItemDecl,
    Other,
}

pub struct RegionMap {
    spans: Vec<RegionSpan>,
}

impl RegionMap {
    pub fn from_document(document: &ParsedDocument) -> Self;
    pub fn region_at(&self, byte_offset: usize) -> Region;
    pub fn allows_executable_lints(&self, byte_offset: usize) -> bool;
    pub fn allows_constraint_lints(&self, byte_offset: usize) -> bool;
}
```

AST-backed rules get region from the visitor position. Text fallback rules must
prove region before emitting; if they cannot, they return no diagnostic.

## Rule Shape

Required shape for newly ported diagnostic rules:

```rust
pub trait LintVisitor<'ast>: syn::visit::Visit<'ast> {
    const SCOPE: &'static [Region];
    const CONFIDENCE: Confidence;
    const APPLICABILITY: Applicability;
    const TOPIC: &'static str;
    fn finish(self) -> Vec<Diagnostic>;
}

pub enum Confidence {
    Heuristic,
    Derived,
    Authoritative,
}

pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}
```

Implemented entry point: `src/lsp/diagnostics/lint.rs`. The shared module owns
`Region`, `RegionMap`, `Confidence`, `Applicability`, `LintVisitor`, and
`run_lint_visitor`. The unsafe-unwrap exemplar is wired through this
contract; remaining rule ports should migrate incrementally instead of creating
rule-local metadata enums.

Rules that do not lint attributes must override `visit_attribute` and avoid
recursing into attribute token streams. Exemplar ports now include unsafe
unwrap, manual close/reinit, native raw account invariants, native account
validation, stale CPI, non-canonical PDA bump, and instruction data bounds.

## Framework Applicability

The diagnostics engine receives a precomputed framework context for the current
document. Rules declare their supported framework set in the registry, and the
engine skips inapplicable collectors before running rule logic. Anchor-only
rules stay scoped to Anchor v1/v2-preview contexts; shared Solana program rules
can run for Anchor, Pinocchio, and native Solana.

`docs/framework-parity.md` is the user-facing boundary for these framework
claims. Native Solana and Pinocchio use applicable parity: shared Solana
diagnostics and artifact evidence run through the same LSP path, while Anchor
constraint, Accounts, IDL, and generated type surfaces are non-applicable.

Hot-path code may derive only cheap parse-level framework facts. Richer project
or artifact evidence belongs in the cold/project diagnostic lane.

## Diagnostic Metadata

Every diagnostic emitted through `diagnostic_from_range` /
`diagnostic_from_span` carries LSP `source: seagrass` plus a data object. The
data object carries:

- `code`: stable LSP diagnostic code
- `rule`: rule id when `AnchorDiagnosticKind::rule()` has one, preserving
  rule-local overrides when present
- `confidence`: `heuristic`, `derived`, or `authoritative`; legacy
  `low`/`medium`/`high` values remain accepted only for compatibility with old
  diagnostics crossing the arbitration boundary
- `topic`: issue-class key used by arbitration
- `applicability`: default `Unspecified`, stronger values allowed by rules

## Arbitration

Arbitration groups diagnostics by span and topic, keeps the highest-confidence
finding, and dedupes exact duplicates. Related-information enrichment attaches
semantic companions for structured diagnostic data.

```rust
let key = (diagnostic.range, diagnostic_topic(&diagnostic));
let survivor = highest_confidence_for(key);
append_related_diagnostic(survivor, demoted);
```

## Partial Input

When `ParsedDocument::syntax().items` is empty, AST-only rules stay quiet by
default. A diagnostic may use tree-sitter only when the rule owns an explicit
recovery path and false-positive tests for broken input.

```rust
if document.syntax().items.is_empty() {
    return document
        .tree_sitter()
        .map(|syntax| recovered_rule_diagnostics(document.source(), syntax))
        .unwrap_or_default();
}
```

Current shared runner behavior is conservative: `run_lint_visitor` returns an
empty diagnostic set on zero-item `syn` parses. Cursor features may use
`RustSyntax` more broadly because completions and hover are request-scoped and
non-diagnostic.

## Suppression Syntax

Active user syntax:

- line: `// seagrass-allow: seagrass/solana.code-quality.unsafe-unwrap`
- line, all Seagrass diagnostics: `// seagrass-ignore`
- file: `// seagrass-allow-file: seagrass/solana.code-quality.unsafe-unwrap`
- item or block: `#[seagrass(allow("seagrass/solana.code-quality.unsafe-unwrap"))]`
  on top-level items, nested items, impl/trait/foreign items, local bindings, or
  block expressions
- workspace: `Seagrass.toml`

  ```toml
  [lints]
  allow = ["seagrass/solana.code-quality.unsafe-unwrap"]
  ```

Suppression matches full topics, topic suffixes, diagnostic codes, or rule ids.
For code-scoped rules, `code.rule` patterns such as
`solana-code-quality.unsafe-unwrap` are accepted too.

Line comments apply to the current line and the next source line so users can
place a suppression above an attribute or expression without appending a trailing
comment.

```rust
struct SuppressionIndex {
    file_patterns: Vec<String>,
    line_patterns: HashMap<u32, Vec<String>>,
    range_patterns: Vec<RangeSuppression>,
}

impl SuppressionIndex {
    fn suppresses(&self, diagnostic: &Diagnostic) -> bool;
}
```

## Lint-The-Linter

`bun scripts/check-rule-hygiene.ts` fails raw-source production diagnostic
scans:

- `source.lines().`
- `.match_indices(`
- raw `.contains("...")` on source-like variables
- delimiter `.rfind("...")`
- token-stream helper `.contains("...")` checks such as
  `expr_text(...).contains("program_id")`

The checker has no legacy allowlist. If a rule needs token text for display, it
may serialize the expression, but issue detection must use parsed predicates or
explicit helpers.

```ts
const BANNED_PATTERNS = [
  "raw-source-lines",
  "raw-match-indices",
  "raw-contains",
  "token-stream-contains",
  "raw-rfind",
];
```

The checker intentionally skips `*_tests.rs`, `tests.rs`, `tests/`, and
`#[cfg(test)]` modules so fixtures can assert message text and string payloads
without weakening production lint hygiene.
