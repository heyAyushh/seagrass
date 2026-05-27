# Seagrass Diagnostic Audit

Status: Active

This audit tracks whether user-visible providers are parsed, region-aware, and
covered by false-positive fixtures. It is the source of truth for the rule-port
queue across `lsp/src/diagnostics/`, `completions/`, `hover/`, and `actions/`.

| file | provider | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `diagnostics/code_quality.rs` | `unchecked_balance_arithmetic` | yes, `syn::visit::Visit` over `ExprBinary` | yes, skips attributes and non-syn fallback | heuristic | `seagrass/solana.code-quality.unchecked-arithmetic` | fixed: `token_mint`/unary deref | `ignores_deref_in_account_attribute`, `editor_ux_does_not_flag_account_attribute_deref_as_arithmetic` |
| `diagnostics/code_quality.rs` | `unsafe_unwrap` | yes, `syn::visit::Visit` over `ExprMethodCall` | yes, skips attributes/comments/strings | heuristic | `seagrass/solana.code-quality.unsafe-unwrap` | fixed: comments/strings/attributes | `ignores_unwrap_text_in_comments_strings_and_attributes` |
| `diagnostics/code_quality.rs` | `non_canonical_pda_bump` | yes, `syn::visit::Visit` over `ExprCall` | yes, skips attributes and non-syn fallback | heuristic | `seagrass/solana.code-quality.bump-seed-canonicalization` | fixed: comments/strings/helper calls | `ignores_create_program_address_text_in_comments_strings_and_attributes`, `ignores_project_helper_named_create_program_address` |
| `diagnostics/code_quality.rs` | `native_raw_account_invariant` | yes, `syn::visit::Visit` over native functions | yes, skips attributes/macros/non-account methods | heuristic | `seagrass/security.owner-check`, `seagrass/security.type-cosplay` | fixed: comments/strings/macros/helper methods | `ignores_raw_account_text_in_comments_and_strings`, `ignores_raw_account_text_inside_dead_macro`, `ignores_non_account_try_borrow_data_method` |
| `diagnostics/code_quality.rs` | `manual_close_reinit` | yes, `syn::visit::Visit` over function bodies | yes, skips attributes/comments/strings/macros | heuristic | `seagrass/solana.code-quality.account-closing`, `seagrass/solana.code-quality.initialization` | fixed: global `close =` suppression and text templates | `ignores_manual_close_and_reinit_text_in_comments_strings_and_attributes` |
| `diagnostics/code_quality.rs` | `stale_account_after_cpi` | yes, `syn::visit::Visit` over function bodies | yes, per-function CPI/reload/read ordering | heuristic | `seagrass/solana.code-quality.stale-account-after-cpi` | fixed: comments/strings and global reload suppression | `ignores_stale_cpi_text_in_comments_and_strings`, `ignores_stale_cpi_when_non_context_struct_has_accounts_field` |
| `diagnostics/code_quality.rs` | `native_account_validation` | yes, `syn::visit::Visit` over native functions | yes, per-function signer/program-id evidence | heuristic | `seagrass/security.signer.authorization`, `seagrass/security.cpi.program` | fixed: comments/strings/global helpers/static program IDs | `ignores_native_account_validation_text_in_comments_and_strings`, `ignores_known_program_id_instruction` |
| `diagnostics/code_quality.rs` | `instruction_data_bounds` | yes, `syn::visit::Visit` over native functions | yes, per-function data access and bounds evidence | heuristic | `seagrass/solana.code-quality.instruction-data-bounds` | fixed: comments/strings/global helper suppression/local vectors | `ignores_instruction_bounds_text_in_comments_strings_and_attributes`, `ignores_unrelated_local_vector_named_data_index` |
| `diagnostics/code_quality.rs` | `pda_seed_collision` | yes, parsed Anchor PDA projection | yes, account-constraint range only | heuristic | `seagrass/solana.code-quality.pda-seed-collision` | fixed: comments/strings and static domain seeds | `ignores_pda_seed_collision_text_in_comments_and_strings` |
| `diagnostics/security.rs` | signer/token/CPI/sysvar/native checks | yes, parsed symbols plus `syn::visit::Visit` raw-account evidence | yes, per-account field and per-handler evidence | authoritative/heuristic | `seagrass/security.cpi.program`, `seagrass/security.owner-check`, `seagrass/security.signer.authorization`, `seagrass/security.sysvar.address`, `seagrass/security.token-account`, `seagrass/security.type-cosplay` | fixed: owner/type comments, strings, and unrelated helpers | `ignores_unchecked_cpi_program_in_unreachable_split_helper`, `does_not_flag_plain_cpi_account_infos_as_program_accounts` |
| `diagnostics/security/duplicates.rs` | duplicate mutable account checks | yes, parsed document symbols/workspace | accounts struct fields | heuristic | `seagrass/security.account.duplicate-mutable` | low | duplicate tests |
| `diagnostics/context_accounts.rs` | missing `Context<T>` accounts | yes, parsed context references plus `syn::visit::Visit` for empty `Context<>` | yes, function-signature only | authoritative | `seagrass/anchor.context.accounts` | fixed: comments/strings/attributes | `ignores_empty_context_text_in_comments_strings_and_attributes` |
| `diagnostics/account_references.rs` | missing account/instruction references | yes, symbols plus generated constraint catalog | account attribute references | authoritative | `seagrass/anchor.constraint.account-reference` | low | account reference tests |
| `diagnostics/account_usage.rs` | missing `mut`/usage checks | yes, parsed account usage model | accounts struct fields | authoritative | `seagrass/anchor.account.usage` | low | account usage tests |
| `diagnostics/anchor_syn.rs` | Anchor parser diagnostics | yes, parser-backed | parser diagnostic spans | authoritative | `seagrass/anchor.syntax` / `seagrass/anchor.constraint.shape` | low | parser tests |
| `diagnostics/anchor_syn/program.rs` | Anchor program parser diagnostics | yes, parser-backed program module inspection | yes, program module and handler signature spans | authoritative | `seagrass/anchor.syntax` | low | anchor parser tests |
| `diagnostics/constraint_shape.rs` | constraint-shape diagnostic router | evidence graph over parsed account constraints | yes, account field and constraint spans | authoritative/heuristic | `seagrass/anchor.constraint.shape`, `seagrass/security.pda.static-seed`, `seagrass/security.account.unchecked` | low | generated constraint shape tests |
| `diagnostics/constraint_shape/*` | generated constraint shape rules | yes, catalog and symbol ranges | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | generated constraint tests |
| `diagnostics/instruction_attributes.rs` | `#[instruction]` arg checks | yes, parsed instruction args | instruction attribute spans | authoritative | `seagrass/anchor.instruction.argument` | low | instruction attribute tests |
| `diagnostics/pda.rs` | PDA seed resolution | yes, parsed PDA projection | account attribute spans | derived | `seagrass/anchor.pda.seed-resolution` | low | PDA diagnostics tests |
| `diagnostics/spl_semantics.rs` | SPL interface semantics | yes, parsed fields/constraints | account attribute spans | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/artifacts.rs` | artifact/project diagnostics | project metadata | n/a | authoritative/derived | `seagrass/anchor.artifact.idl`, `seagrass/anchor.artifact.program-keypair`, `seagrass/anchor.artifact.sbf`, `seagrass/anchor.artifact.types` | low | artifact tests |
| `diagnostics/check_cfg.rs` | Cargo check-cfg project diagnostics | manifest parser | manifest only | authoritative | `seagrass/anchor.check-cfg` | low | check-cfg tests |
| `diagnostics/project_identity.rs` | Anchor project id diagnostics | Anchor.toml + parsed source | manifest/source bridge | authoritative | `seagrass/anchor.project-id` | low | project identity tests |
| `diagnostics/ecosystem.rs` | Solana ecosystem artifact/test diagnostics | project metadata | project files | derived | `seagrass/solana.artifact.idl`, `seagrass/solana.program-metadata`, `seagrass/solana.surfpool-workspace`, `seagrass/solana.test-harness` | low | ecosystem tests |
| `completions/mod.rs` | completion router + resolve | yes, `CursorContext` and `RankingContext` | cursor and completion item data | n/a | n/a | low | completion gate and resolve tests |
| `completions/account_constraints.rs` | constraint key completions | generated constraint catalog | account attribute only | n/a | n/a | low | generated catalog completion tests |
| `completions/constraint_values.rs` | constraint value completions | `CursorContext`, account semantics, workspace symbols | account attribute values | n/a | n/a | low | constraint value core/catalog tests |
| `completions/constraint_values/recovery.rs` | incomplete constraint recovery | tree-sitter recovery | account attribute values | n/a | n/a | low | recovery + incomplete attribute tests |
| `completions/constraint_values/slots.rs` | constraint value slot classifier and prefix matching | generated constraint catalog + `AccountAttributeCursor` | account attribute values | n/a | n/a | low | constraint value core/catalog tests |
| `completions/account_fields.rs` | account field/type completions | account semantics + workspace index | accounts structs | n/a | n/a | low | account field tests |
| `completions/account_fields/context.rs` | account field completion context classifier | parsed document, tree-sitter fallback, and account semantics | accounts structs | n/a | n/a | low | account field context tests |
| `completions/account_paths.rs` | `ctx.accounts.*` path completions | parsed account path + workspace index | instruction body | n/a | n/a | low | account path tests |
| `completions/instruction_attributes.rs` | `#[instruction(...)]` completions | parsed handler args | instruction attribute | n/a | n/a | low | instruction attribute completion tests |
| `completions/ranking.rs` | global completion ranking | `RankingContext` + item data | cross-provider | n/a | n/a | low | global ranking tests |
| `completions/cursor_context.rs` | cursor classification | parsed document + tree-sitter fallback | cursor position and syntax tokens | n/a | n/a | low | cursor context tests |
| `hover/mod.rs` | hover router + account/context hovers | `CursorContext`, symbols, workspace | cursor position and symbol spans | n/a | n/a | low | hover tests + editor parity |
| `hover/account_constraints.rs` | constraint hover docs/evidence | generated catalog + PDA evidence | account attribute only | n/a | n/a | low | generated constraint hover tests |
| `actions/mod.rs` | quick-fix router + resolve | diagnostic data + parsed document | diagnostic span | n/a | n/a | low | action routing tests |
| `actions/accounts.rs` | account type/field quick fixes | parsed structs, semantic diagnostics, workspace candidates | account field spans | n/a | n/a | low | account action tests |
| `actions/accounts/context_structs.rs` | create accounts struct fixes | parsed instruction/context evidence | instruction signature | n/a | n/a | low | context action tests |
| `actions/accounts/field_edits.rs` | inferred account field edit builder | diagnostic data + parsed account semantics | accounts structs | n/a | n/a | low | field edit tests |
| `actions/constraints.rs` | generated constraint quick fixes | generated parser catalog + parsed constraint ranges | account attributes | n/a | n/a | low | generated constraint action tests |
| `actions/features.rs` | manifest feature quick fixes | manifest diagnostics | Cargo manifest | n/a | n/a | low | feature action tests |
| `actions/init_constraints.rs` | init placeholder/fix-all actions | parsed init diagnostics | account attributes | n/a | n/a | low | init constraint action tests |
| `actions/instructions.rs` | instruction argument quick fixes | parsed handler args + instruction attrs | instruction attrs | n/a | n/a | low | instruction action tests |
| `actions/missing_init.rs` | missing init companion actions | parsed accounts structs | account field spans | n/a | n/a | low | missing init action tests |
| `actions/pda.rs` | PDA limitation/copy actions | diagnostic seed metadata | diagnostic span | n/a | n/a | low | PDA action tests |
| `actions/security.rs` | security/code-quality remediation actions | structured diagnostic data + parsed edits | diagnostic span | n/a | n/a | low | security action tests |
| `code_lens.rs` | accounts/program lens | parsed symbols + workspace index | accounts/program declarations | n/a | n/a | low | code lens tests |
| `document_links.rs` | Anchor docs links | generated constraint catalog + constraint ranges | account attributes | n/a | n/a | low | document link tests |
| `folding.rs` | account constraint folding | parsed constraint ranges | account attributes | n/a | n/a | low | folding tests |
| `inlay_hints.rs` | implied mut / PDA hints | parsed constraints + PDA evidence | account attributes | n/a | n/a | low | inlay hint tests |
| `signature_help.rs` | constraint signature help | generated constraint catalog + cursor position | account attributes | n/a | n/a | low | signature help tests |
| `semantic_tokens.rs` | Anchor semantic tokens | tree-sitter query overlay + generated catalog | syntax tokens | n/a | n/a | low | semantic token tests |
| `navigation/mod.rs` | definition/type/references/implementation | parsed symbols + workspace index | declarations/usages | n/a | n/a | low | navigation tests |
| `renaming.rs` | semantic rename edits | parsed declarations/usages + workspace index | declarations/usages | n/a | n/a | low | rename tests |
| `selection_ranges.rs` | hierarchical selections | parsed accounts/fields/constraint ranges | cursor position | n/a | n/a | low | selection range tests |
| `workspace.rs` | workspace symbols and cross-file index | parsed document symbols + transactional index | workspace files | n/a | n/a | low | workspace tests |
| `server/reports.rs` | agent-facing reports | parsed symbols/evidence/diagnostics | request-scoped document/workspace | n/a | n/a | low | server report tests |

## Branch-Level Expansion

These rows expand providers that have multiple user-visible branches behind one
router. Keep these rows branch-level so Phase D false-positive fixtures do not
hide behind file-level coverage.

| file | provider branch | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `diagnostics/initialization.rs` | missing init companion evidence | evidence graph over parsed accounts + instructions | accounts struct field spans | authoritative | `seagrass/anchor.init.missing-companion` | low | initialization tests |
| `diagnostics/constraint_shape/pda.rs` | init with foreign seeds program | parsed constraint evidence | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | `constraint_shape::pda` tests |
| `diagnostics/constraint_shape/pda.rs` | seed/bump pairing and seed limits | parsed PDA projection | account attributes | authoritative/heuristic | `seagrass/anchor.constraint.shape`, `seagrass/security.pda.static-seed` | low | PDA seed fixtures |
| `diagnostics/constraint_shape/init_lifecycle.rs` | payer/system/realloc/close/zero lifecycle checks | parsed constraint and field evidence | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | init lifecycle fixtures |
| `diagnostics/constraint_shape/catalog.rs` | generated catalog companions and keyword values | generated catalog + parsed constraints | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | catalog fixtures |
| `diagnostics/constraint_shape/token.rs` | token/mint init and required program checks | parsed SPL constraint evidence | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | token constraint fixtures |
| `diagnostics/constraint_shape/has_one.rs` | `has_one` field compatibility | parsed account data + workspace index | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | has-one fixtures |
| `diagnostics/constraint_shape/program_account.rs` | unchecked program account validation | parsed account field constraints | account fields | heuristic | `seagrass/security.account.unchecked` | low | program account fixtures |
| `diagnostics/security.rs` | typed signer authorization | `Signer<T>` account visitor | accounts struct fields | authoritative | `seagrass/security.signer.authorization` | low | security core tests |
| `diagnostics/security.rs` | typed sysvar address checks | sysvar account visitor | accounts struct fields | authoritative | `seagrass/security.sysvar.address` | low | security core tests |
| `diagnostics/security.rs` | token account unpacking checks | token account visitor | accounts struct fields | heuristic | `seagrass/security.token-account` | low | security core tests |
| `diagnostics/security.rs` | raw owner checks | visitor + raw account evidence | handler body + accounts field | heuristic | `seagrass/security.owner-check` | low | raw account tests |
| `diagnostics/security.rs` | raw type cosplay checks | visitor + discriminator evidence | handler body + accounts field | heuristic | `seagrass/security.type-cosplay` | low | raw account tests |
| `diagnostics/security.rs` | arbitrary CPI program checks | visitor + workspace program evidence | accounts struct fields | authoritative/heuristic | `seagrass/security.cpi.program` | low | CPI program tests |
| `diagnostics/security/duplicates.rs` | duplicate mutable account checks | `LintVisitor` over accounts structs | accounts struct fields | heuristic | `seagrass/security.account.duplicate-mutable` | low | duplicate tests |
| `diagnostics/security/raw_account.rs` | raw data/deserialize evidence collector | `syn::visit::Visit` over functions and macros | handler bodies only | n/a | `seagrass/security.owner-check`, `seagrass/security.type-cosplay` | fixed: comments/strings/dead macros | `ignores_raw_account_text_in_comments_strings_and_attributes`, `ignores_security_evidence_inside_dead_macro_body` |
| `diagnostics/code_quality/native_raw.rs` | native owner/discriminator validation | AST visitors and expression predicates | native helper bodies | heuristic | `seagrass/solana.code-quality`, `seagrass/security.owner-check`, `seagrass/security.type-cosplay` | fixed: helpers/comments/strings | `ignores_raw_account_text_in_comments_and_strings`, `ignores_deserialize_text_without_raw_account_data_read` |
| `diagnostics/code_quality/manual_close.rs` | account closing and reinitialization | AST visitor over assignments and transfers | function bodies | heuristic | `seagrass/solana.code-quality.account-closing`, `seagrass/solana.code-quality.initialization` | fixed: attributes/comments/strings | `ignores_manual_close_and_reinit_text_in_comments_strings_and_attributes` |
| `diagnostics/code_quality/stale_cpi.rs` | stale account after CPI | AST visitor over CPI/reload/read ordering | function bodies | heuristic | `seagrass/solana.code-quality.stale-account-after-cpi` | fixed: comments/strings | `ignores_stale_cpi_text_in_comments_and_strings` |
| `diagnostics/code_quality/native_validation.rs` | native signer and CPI validation | AST visitor over AccountInfo guards | native helper bodies | heuristic | `seagrass/security.signer.authorization`, `seagrass/security.cpi.program` | fixed: unrelated helpers/static IDs | `ignores_native_account_validation_text_in_comments_and_strings`, `ignores_known_program_id_instruction` |
| `diagnostics/mod.rs` | parser init constraint topic split | parser messages with structured diagnostic data | parse-error span | authoritative | `seagrass/anchor.init.constraints`, `seagrass/anchor.init.missing-payer`, `seagrass/anchor.init.missing-space` | low | parser init constraint diagnostics tests |
| `diagnostics/spl_semantics.rs` | mint reference type checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/spl_semantics.rs` | token/mint target container checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/spl_semantics.rs` | token program override checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/spl_semantics.rs` | Token-2022 extension checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/spl_semantics.rs` | mint decimals argument checks | parsed instruction args | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/spl_semantics.rs` | transfer-hook program checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `diagnostics/artifacts.rs` | SBPF artifact missing/invalid/stale | project artifact metadata | project file span | derived | `seagrass/anchor.artifact.sbf` | low | artifact tests |
| `diagnostics/artifacts.rs` | program keypair missing/invalid/mismatch | keypair metadata + declared id | project file span | derived | `seagrass/anchor.artifact.program-keypair` | low | artifact tests |
| `diagnostics/artifacts.rs` | IDL artifact missing/invalid/stale/mismatch | artifact metadata | project file span | derived | `seagrass/anchor.artifact.idl` | low | artifact tests |
| `diagnostics/artifacts.rs` | generated TypeScript artifact checks | artifact metadata | project file span | derived | `seagrass/anchor.artifact.types` | low | artifact tests |
| `diagnostics/ecosystem.rs` | non-Anchor IDL ecosystem checks | ecosystem metadata | project file span | derived | `seagrass/solana.artifact.idl` | low | ecosystem tests |
| `diagnostics/ecosystem.rs` | program metadata checks | ecosystem metadata | project file span | derived | `seagrass/solana.program-metadata` | low | ecosystem tests |
| `diagnostics/ecosystem.rs` | test harness checks | ecosystem metadata | project file span | derived | `seagrass/solana.test-harness` | low | ecosystem tests |
| `diagnostics/ecosystem.rs` | surfpool workspace checks | ecosystem metadata | project file span | derived | `seagrass/solana.surfpool-workspace` | low | ecosystem tests |
| `completions/mod.rs` | context type completions | `CursorContext` + parsed account structs | function signatures | n/a | n/a | low | context completion tests |
| `completions/mod.rs` | account constraint snippets | generated catalog + pending-fix data | account attributes | n/a | n/a | low | constraint completion tests |
| `completions/mod.rs` | completion resolve docs | generated catalog docs | completion item data | n/a | n/a | low | resolve tests |
| `completions/constraint_values/candidates.rs` | signer/account/program/space/seed candidates | account semantics + workspace symbols | account attribute values | n/a | n/a | low | constraint value tests |
| `actions/accounts.rs` | account name replacement, add field, mut, duplicate actions | diagnostic data + parsed accounts | diagnostic span | n/a | n/a | low | account action tests |
| `actions/constraints.rs` | duplicate/order/generated constraint actions | generated parser catalog + parsed ranges | account attributes | n/a | n/a | low | constraint action tests |
| `actions/features.rs` | feature/Cargo/declaration sync actions | manifest diagnostic data | manifest/project spans | n/a | n/a | low | feature action tests |
| `actions/pda.rs` | IDL limitation and PDA derivation actions | diagnostic seed metadata | diagnostic span | n/a | n/a | low | PDA action tests |
| `actions/security.rs` | owner/type/sysvar/native/security remediation actions | structured diagnostic data + parsed edits | diagnostic span | n/a | n/a | low | security action tests |
| `actions/init_constraints.rs` | placeholder and fix-all init actions | init diagnostic data + attribute ranges | account attributes | n/a | n/a | low | init action tests |
| `actions/instructions.rs` | add/replace/remove instruction args | parsed handler args + instruction attrs | instruction attrs | n/a | n/a | low | instruction action tests |
| `actions/missing_init.rs` | add/fix-all missing init companion actions | parsed accounts structs | accounts struct fields | n/a | n/a | low | missing init action tests |

## Tracking Rows

- Keep `lsp/scripts/check-rule-hygiene.ts` at zero legacy diagnostic findings.
- Keep `lsp/docs/topics.json` synchronized with emitted diagnostic topics.
- Keep this table updated for editor providers, not only diagnostic emitters.
