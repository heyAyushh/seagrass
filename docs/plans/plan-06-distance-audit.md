# Plan 06 Distance Audit

Status: Stage 0 snapshot after plan 05.

This classifies the 19 entries in `src/lsp/diagnostics/rules.rs` by their current
diagnostic shape and the semantic model nodes they should eventually query.

| Rule | Current shape | Required node kinds | Priority | Notes |
| --- | --- | --- | --- | --- |
| `anchor-syn` | visitor | `AccountsStruct`, `AccountField` | medium | Parser-backed structural checks stay mostly syntactic; sysvar recognition now has plan-05 capability data. |
| `context-accounts` | visitor | `Instruction`, `AccountsStruct` | medium | Missing context claims need instruction-to-context edges and workspace evidence. |
| `instruction-attributes` | visitor | `Instruction`, `InstructionParam` | low | Single-file argument shape checks already map cleanly to instruction params. |
| `initialization` | visitor | `Instruction`, `AccountsStruct`, `AccountField`, `Constraint` | medium | Init intent and mutation evidence can query constraints and instruction usage. |
| `handler-scope` | visitor | `Instruction`, `InstructionParam` | low | Handler lexical scope remains mostly syntactic; no HIR replacement planned. |
| `handler-members` | visitor | `Instruction`, `AccountField` | low | Member checks benefit from account fields but still require local expression evidence. |
| `handler-struct-literals` | visitor | `Instruction`, `AccountField` | low | Local struct literal fields can later read model account data fields. |
| `account-references` | visitor | `AccountsStruct`, `AccountField`, `CompositeRef`, `Constraint` | high | Blocks nested composite payer false positives; should use `all_fields_for_struct`. |
| `constraint-expressions` | visitor | `AccountsStruct`, `AccountField`, `InstructionParam`, `Constraint` | medium | Constraint identifier and member references need model-visible accounts and args. |
| `constraint-shape` | visitor | `AccountsStruct`, `AccountField`, `Constraint`, `PdaSeedSet` | medium | Catalog-backed shape checks can migrate incrementally. |
| `spl-semantics` | visitor | `AccountsStruct`, `AccountField`, `Constraint` | high | Blocks token-interface false positives; should query `token_interface_candidate`. |
| `account-usage` | visitor | `Instruction`, `AccountsStruct`, `AccountField`, `Check` | medium | Mutability and account usage claims need instruction usage edges. |
| `security` | visitor + call graph substrate | `Instruction`, `AccountsStruct`, `AccountField`, `Check` | high | Signer propagation now has bounded reachable-helper defense; `security.cpi.program` has one reachable-helper offense proof. Remaining owner/type-cosplay ports can consume `CallGraph` instead of adding local walks. |
| `pda` | visitor | `AccountsStruct`, `AccountField`, `Constraint`, `PdaSeedSet` | medium | PDA visibility and seed claims map directly to model seed nodes. |
| `code-quality` | visitor | `Instruction`, `CpiCall`, `Check` | medium | Arithmetic/manual-close/stale-CPI rules can share instruction and CPI facts. |
| `check-cfg` | model | none | low | Manifest/toolchain rule; keep outside semantic model. |
| `project-identity` | model | `Program` | low | Toolchain and Anchor.toml backed; can optionally read declared program node. |
| `artifacts` | model | `Program` | low | Artifact and SBF checks are project metadata/toolchain paths. |
| `ecosystem` | model | `Program` | low | Ecosystem recommendations remain manifest/artifact based. |

High-priority migration targets for this plan are `account-references`,
`spl-semantics`, and the remaining model-based security queries. Plan 10 landed
the cross-function substrate for `security`; future migrations should reuse the
workspace `CallGraph` and framework-free reachable-check query rather than
creating rule-local traversal.
