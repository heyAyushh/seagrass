# Seagrass Diagnostic Audit

Status: Active

This audit tracks whether user-visible providers are parsed, region-aware, and
covered by false-positive fixtures. It is the source of truth for the rule-port
queue across `src/lsp/diagnostics/`, `lsp/completions/`, `lsp/hover/`, and `lsp/actions/`.

| file | provider | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `lsp/diagnostics/code_quality/mod.rs` | `unchecked_balance_arithmetic` | yes, `syn::visit::Visit` over `ExprBinary` | yes, skips attributes and non-syn fallback | heuristic | `seagrass/solana.code-quality.unchecked-arithmetic` | fixed: `token_mint`/unary deref | `ignores_deref_in_account_attribute`, `editor_ux_does_not_flag_account_attribute_deref_as_arithmetic` |
| `lsp/diagnostics/code_quality/mod.rs` | `unsafe_unwrap` | yes, `syn::visit::Visit` over `ExprMethodCall` | yes, skips attributes/comments/strings | heuristic | `seagrass/solana.code-quality.unsafe-unwrap` | fixed: comments/strings/attributes | `ignores_unwrap_text_in_comments_strings_and_attributes` |
| `lsp/diagnostics/code_quality/mod.rs` | `non_canonical_pda_bump` | yes, `syn::visit::Visit` over `ExprCall` | yes, skips attributes and non-syn fallback | heuristic | `seagrass/solana.code-quality.bump-seed-canonicalization` | fixed: comments/strings/helper calls | `ignores_create_program_address_text_in_comments_strings_and_attributes`, `ignores_project_helper_named_create_program_address` |
| `lsp/diagnostics/code_quality/mod.rs` | `native_raw_account_invariant` | yes, `syn::visit::Visit` over native functions | yes, skips attributes/macros/non-account methods | heuristic | `seagrass/security.owner-check`, `seagrass/security.type-cosplay` | fixed: comments/strings/macros/helper methods | `ignores_raw_account_text_in_comments_and_strings`, `ignores_raw_account_text_inside_dead_macro`, `ignores_non_account_try_borrow_data_method` |
| `lsp/diagnostics/code_quality/mod.rs` | `manual_close_reinit` | yes, `syn::visit::Visit` over function bodies | yes, skips attributes/comments/strings/macros | heuristic | `seagrass/solana.code-quality.account-closing`, `seagrass/solana.code-quality.initialization` | fixed: global `close =` suppression and text templates | `ignores_manual_close_and_reinit_text_in_comments_strings_and_attributes` |
| `lsp/diagnostics/code_quality/mod.rs` | `stale_account_after_cpi` | yes, `syn::visit::Visit` over function bodies | yes, per-function CPI/reload/read ordering | heuristic | `seagrass/solana.code-quality.stale-account-after-cpi` | fixed: comments/strings and global reload suppression | `ignores_stale_cpi_text_in_comments_and_strings`, `ignores_stale_cpi_when_non_context_struct_has_accounts_field` |
| `lsp/diagnostics/code_quality/mod.rs` | `native_account_validation` | yes, `syn::visit::Visit` over native functions | yes, per-function signer/program-id evidence | heuristic | `seagrass/security.signer.authorization`, `seagrass/security.cpi.program` | fixed: comments/strings/global helpers/static program IDs | `ignores_native_account_validation_text_in_comments_and_strings`, `ignores_known_program_id_instruction` |
| `lsp/diagnostics/code_quality/mod.rs` | `instruction_data_bounds` | yes, `syn::visit::Visit` over native functions | yes, per-function data access and bounds evidence | heuristic | `seagrass/solana.code-quality.instruction-data-bounds` | fixed: comments/strings/global helper suppression/local vectors | `ignores_instruction_bounds_text_in_comments_strings_and_attributes`, `ignores_unrelated_local_vector_named_data_index` |
| `lsp/diagnostics/code_quality/mod.rs` | `pda_seed_collision` | yes, parsed Anchor PDA projection | yes, account-constraint range only | heuristic | `seagrass/solana.code-quality.pda-seed-collision` | fixed: comments/strings and static domain seeds | `ignores_pda_seed_collision_text_in_comments_and_strings` |
| `lsp/diagnostics/security/mod.rs` | signer/token/CPI/sysvar/native checks | yes, parsed symbols plus `syn::visit::Visit` raw-account evidence | yes, per-account field and per-handler evidence | authoritative/heuristic | `seagrass/security.cpi.program`, `seagrass/security.owner-check`, `seagrass/security.signer.authorization`, `seagrass/security.sysvar.address`, `seagrass/security.token-account`, `seagrass/security.type-cosplay` | fixed: owner/type comments, strings, and unrelated helpers | `ignores_unchecked_cpi_program_in_unreachable_split_helper`, `does_not_flag_plain_cpi_account_infos_as_program_accounts` |
| `lsp/diagnostics/security/duplicates.rs` | duplicate mutable account checks | yes, parsed document symbols/workspace | accounts struct fields | heuristic | `seagrass/security.account.duplicate-mutable` | low | duplicate tests |
| `lsp/diagnostics/context_accounts.rs` | missing `Context<T>` accounts | yes, parsed context references plus `syn::visit::Visit` for empty `Context<>` | yes, function-signature only | authoritative | `seagrass/anchor.context.accounts` | fixed: comments/strings/attributes | `ignores_empty_context_text_in_comments_strings_and_attributes` |
| `lsp/diagnostics/account_references/mod.rs` | missing account/instruction references | yes, symbols plus generated constraint catalog | account attribute references | authoritative | `seagrass/anchor.constraint.account-reference` | low | account reference tests |
| `lsp/diagnostics/account_usage/mod.rs` | missing `mut`/usage checks | yes, parsed account usage model | accounts struct fields | authoritative | `seagrass/anchor.account.usage` | low | account usage tests |
| `lsp/diagnostics/handler_scope.rs` | unresolved handler value identifiers | yes, parsed handler bodies plus shallow local scopes, block item values, assertion macro expressions, const-like program-module values, and bounded parse-error recovery | hot lane, handler/helper bodies only | derived | `seagrass/anchor.account.usage` | low | `hot_engine_flags_unresolved_handler_identifiers`, `editor_ux_flags_unresolved_anchor_handler_identifier`, `editor_ux_flags_unresolved_const_like_handler_identifier`, `parse_error_reports_unresolved_identifier_inside_anchor_handler`, `recovers_generated_parse_error_identifiers`, `reports_unresolved_identifier_inside_require_macro`, `reports_unresolved_const_like_identifier_inside_require_macro`, `accepts_block_item_values_declared_after_use` |
| `lsp/diagnostics/handler_members.rs` | handler local member resolution | yes, parsed handler bodies plus shallow local/argument/alias types, typed `const`/`static` block items, assertion macro expressions, transparent `as_ref`/deref account aliases, `Context<T>::accounts` aliases, `AccountLoader<T>` loaded-value evidence, `Context<T>::bumps` PDA evidence, parsed struct fields, inherent-method separation, pattern-scoped match/if-let branch output inference, diverging branch output inference, block-local tail inference, typed tuple destructuring, and bounded tree-sitter recovery for broken handlers | hot lane, handler/helper bodies only | derived | `seagrass/anchor.account.usage` | fixed: unknown_external_types | `ignores_unknown_external_handler_local_type`, `ignores_unknown_handler_alias_type`, `reports_generated_unknown_member_through_typed_alias`, `reports_generated_unknown_member_through_accounts_alias`, `reports_generated_unknown_member_after_as_ref_alias`, `reports_generated_account_loader_direct_alias_member`, `reports_generated_context_bump_member`, `reports_generated_field_called_as_method`, `reports_generated_unknown_block_item_members`, `reports_generated_unknown_members_after_match_pattern_output`, `reports_generated_unknown_members_after_match_return_arm`, `reports_generated_unknown_members_after_block_local_tail`, `reports_unknown_member_after_typed_tuple_handler_arg`, `reports_unknown_member_inside_require_macro` |
| `lsp/diagnostics/handler_members/*` | handler member helper modules | yes, parsed field-access, method-call, iterator-closure, pattern-scope, and typed-scope evidence | handler/helper body spans and member diagnostic ranges | derived | `seagrass/anchor.account.usage` | low | handler member module tests |
| `lsp/diagnostics/handler_struct_literals.rs` | handler struct literal and pattern field resolution | yes, `syn::visit::Visit` over `ExprStruct`/`PatStruct` plus shared struct/member resolver | hot lane, Anchor handler bodies only | derived | `seagrass/anchor.account.usage` | low | `reports_unknown_handler_struct_literal_field`, `reports_unknown_handler_struct_pattern_field`, `ignores_unknown_external_struct_pattern_type`, `editor_ux_resolves_handler_struct_literal_fields` |
| `lsp/diagnostics/mod.rs` | missing-semicolon parse range recovery | yes, parser span plus bounded source-line recovery | parse-error span recovery near syntax tokens | authoritative | `seagrass/anchor.syntax` | fixed: previous_account_attribute_close_lines | `parse_error_expected_semicolon_does_not_jump_to_previous_account_attribute` |
| `lsp/diagnostics/anchor_syn/mod.rs` | Anchor parser diagnostics | yes, parser-backed | parser diagnostic spans | authoritative | `seagrass/anchor.syntax` / `seagrass/anchor.constraint.shape` | low | parser tests |
| `lsp/diagnostics/anchor_syn/program.rs` | Anchor program parser diagnostics | yes, parser-backed program module inspection | yes, program module and handler signature spans | authoritative | `seagrass/anchor.syntax` | low | anchor parser tests |
| `lsp/diagnostics/constraint_expressions.rs` | account constraint expression resolution | yes, parsed expressions plus account/member/method/workspace symbols | account attribute expression spans | authoritative | `seagrass/anchor.constraint.expression` | low | constraint expression tests |
| `lsp/diagnostics/constraint_shape/mod.rs` | constraint-shape diagnostic router | evidence graph over parsed account constraints | yes, account field and constraint spans | authoritative/heuristic | `seagrass/anchor.constraint.shape`, `seagrass/security.pda.static-seed`, `seagrass/security.account.unchecked` | low | generated constraint shape tests |
| `lsp/diagnostics/constraint_shape/*` | generated constraint shape rules | yes, catalog and symbol ranges | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | generated constraint tests |
| `lsp/diagnostics/instruction_attributes.rs` | `#[instruction]` arg checks | yes, parsed instruction args | instruction attribute spans | authoritative | `seagrass/anchor.instruction.argument` | low | instruction attribute tests |
| `lsp/diagnostics/pda.rs` | PDA seed resolution | yes, parsed PDA projection | account attribute spans | derived | `seagrass/anchor.pda.seed-resolution` | low | PDA diagnostics tests |
| `lsp/diagnostics/spl_semantics.rs` | SPL interface semantics | yes, parsed fields/constraints | account attribute spans | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/artifacts.rs` | artifact/project diagnostics | project metadata | n/a | authoritative/derived | `seagrass/anchor.artifact.idl`, `seagrass/anchor.artifact.program-keypair`, `seagrass/anchor.artifact.sbf`, `seagrass/anchor.artifact.types` | low | artifact tests |
| `lsp/diagnostics/check_cfg/mod.rs` | Cargo check-cfg project diagnostics | manifest parser | manifest only | authoritative | `seagrass/anchor.check-cfg` | low | check-cfg tests |
| `lsp/diagnostics/project_identity.rs` | Anchor project id diagnostics | Anchor.toml + parsed source | manifest/source bridge | authoritative | `seagrass/anchor.project-id` | low | project identity tests |
| `lsp/diagnostics/ecosystem.rs` | Solana ecosystem artifact/test diagnostics | project metadata | project files | derived | `seagrass/solana.artifact.idl`, `seagrass/solana.program-metadata`, `seagrass/solana.surfpool-workspace`, `seagrass/solana.test-harness` | low | ecosystem tests |
| `lsp/completions/mod.rs` | completion router + resolve | yes, `CursorContext` and `RankingContext` | cursor and completion item data | n/a | n/a | low | completion gate and resolve tests |
| `lsp/completions/account_aliases.rs` | local account alias member completions | cursor-line parser over local `let` assignments | cursor position before member access | n/a | n/a | low | `resolves_direct_ctx_account_alias_variants`, `resolves_intermediate_accounts_alias_variants` |
| `lsp/completions/account_constraints.rs` | constraint key completions | generated constraint catalog | account attribute only | n/a | n/a | low | generated catalog completion tests |
| `lsp/completions/constraint_values/mod.rs` | constraint value completions | `CursorContext`, account semantics, workspace symbols | account attribute values | n/a | n/a | low | constraint value core/catalog tests |
| `lsp/completions/constraint_values/associated_values.rs` | associated const/function completions | parsed impl items + workspace symbols | account attribute expression values | n/a | n/a | low | semantic constraint value tests |
| `lsp/completions/constraint_values/expression_scope.rs` | constraint expression scope completions | parsed account fields, instruction args, consts, and workspace symbols | account attribute expression values | n/a | n/a | low | value expression completion tests |
| `lsp/completions/constraint_values/members.rs` | constraint expression member completions | account/member resolver plus workspace symbols | account attribute expression values | n/a | n/a | low | semantic constraint value tests |
| `lsp/completions/constraint_values/recovery.rs` | incomplete constraint recovery | tree-sitter recovery | account attribute values | n/a | n/a | low | recovery + incomplete attribute tests |
| `lsp/completions/constraint_values/slots.rs` | constraint value slot classifier and prefix matching | generated constraint catalog + `AccountAttributeCursor` | account attribute values | n/a | n/a | low | constraint value core/catalog tests |
| `lsp/completions/account_fields/mod.rs` | account field/type completions | account semantics + workspace index | accounts structs | n/a | n/a | low | account field tests |
| `lsp/completions/account_fields/context.rs` | account field completion context classifier | parsed document, tree-sitter fallback, and account semantics | accounts structs | n/a | n/a | low | account field context tests |
| `lsp/completions/account_paths.rs` | `ctx.accounts.*` path completions | parsed account path + workspace index + same-file text recovery for incomplete handlers | instruction body | n/a | n/a | low | account path tests |
| `lsp/completions/handler_members.rs` | handler member completions | parsed handler local/argument types, typed `const`/`static` block items with bounded text recovery, transparent `as_ref`/deref account aliases, `Context<T>` account aliases, `AccountLoader<T>` load-state types, intermediate `ctx.accounts` aliases, generated `Bumps` PDA fields, inherent impl methods, assertion macro argument text recovery, and workspace symbols | handler body cursor position and member access tokens | n/a | n/a | low | handler member tests + generated proptest |
| `lsp/completions/handler_struct_fields.rs` | handler struct literal and pattern field completions | parsed struct/member resolver plus bounded text recovery for incomplete record literals and patterns | Anchor handler record field cursor slots | n/a | n/a | low | handler struct literal/pattern completion tests + editor parity |
| `lsp/completions/handler_values.rs` | handler expression value completions | parsed handler scope, account context fields, imports, constants, block item values, program-module values, assertion macro empty argument slots, and empty Rust expression slots | handler body cursor expression values | n/a | n/a | low | handler value tests + generated proptest + editor parity |
| `lsp/completions/instruction_attributes.rs` | `#[instruction(...)]` completions | parsed handler args | instruction attribute | n/a | n/a | low | instruction attribute completion tests |
| `lsp/completions/ranking.rs` | global completion ranking | `RankingContext` + item data | cross-provider | n/a | n/a | low | global ranking tests |
| `lsp/completions/cursor_context.rs` | cursor classification | parsed document + tree-sitter fallback | cursor position and syntax tokens | n/a | n/a | low | cursor context tests |
| `lsp/completions/proptest_support.rs` | completion property-test identifier strategies | generated test identifiers via regex plus Rust keyword denylist | completion property test inputs | authoritative | n/a | low | generated account alias property tests |
| `lsp/hover/mod.rs` | hover router + account/context hovers | `CursorContext`, symbols, workspace | cursor position and symbol spans | n/a | n/a | low | hover tests + editor parity |
| `lsp/hover/account_constraints.rs` | constraint hover docs/evidence | generated catalog + PDA evidence | account attribute only | n/a | n/a | low | generated constraint hover tests |
| `lsp/actions/mod.rs` | quick-fix router + resolve | diagnostic data + parsed document | diagnostic span | n/a | n/a | low | action routing tests |
| `lsp/actions/accounts/mod.rs` | account type/field quick fixes | parsed structs, semantic diagnostics, workspace candidates | account field spans | n/a | n/a | low | account action tests |
| `lsp/actions/accounts/context_structs.rs` | create accounts struct fixes | parsed instruction/context evidence | instruction signature | n/a | n/a | low | context action tests |
| `lsp/actions/accounts/field_edits.rs` | inferred account field edit builder | diagnostic data + parsed account semantics | accounts structs | n/a | n/a | low | field edit tests |
| `lsp/actions/constraint_expressions.rs` | constraint expression member replacement actions | structured diagnostic data with resolved member candidates | account attribute expression spans | n/a | n/a | low | constraint expression action tests |
| `lsp/actions/constraints.rs` | generated constraint quick fixes | generated parser catalog + parsed constraint ranges | account attributes | n/a | n/a | low | generated constraint action tests |
| `lsp/actions/features.rs` | manifest feature quick fixes | manifest diagnostics | Cargo manifest | n/a | n/a | low | feature action tests |
| `lsp/actions/init_constraints.rs` | init placeholder/fix-all actions | parsed init diagnostics | account attributes | n/a | n/a | low | init constraint action tests |
| `lsp/actions/instructions.rs` | instruction argument quick fixes | parsed handler args + instruction attrs | instruction attrs | n/a | n/a | low | instruction action tests |
| `lsp/actions/missing_init.rs` | missing init companion actions | parsed accounts structs | account field spans | n/a | n/a | low | missing init action tests |
| `lsp/actions/pda.rs` | PDA limitation/copy actions | diagnostic seed metadata | diagnostic span | n/a | n/a | low | PDA action tests |
| `lsp/actions/security.rs` | security/code-quality remediation actions | structured diagnostic data + parsed edits | diagnostic span | n/a | n/a | low | security action tests |
| `lsp/code_lens/mod.rs` | accounts/program lens | parsed symbols + workspace index | accounts/program declarations | n/a | n/a | low | code lens tests |
| `lsp/document_links/mod.rs` | Anchor docs links | generated constraint catalog + constraint ranges | account attributes | n/a | n/a | low | document link tests |
| `lsp/folding/mod.rs` | account constraint folding | parsed constraint ranges | account attributes | n/a | n/a | low | folding tests |
| `lsp/inlay_hints/mod.rs` | implied mut / PDA hints | parsed constraints + PDA evidence | account attributes | n/a | n/a | low | inlay hint tests |
| `lsp/signature_help/mod.rs` | constraint signature help | generated constraint catalog + cursor position | account attributes | n/a | n/a | low | signature help tests |
| `lsp/semantic_tokens/mod.rs` | Anchor semantic tokens | tree-sitter query overlay + generated catalog | syntax tokens | n/a | n/a | low | semantic token tests |
| `lsp/navigation/mod.rs` | definition/type/references/implementation | parsed symbols + workspace index | declarations/usages | n/a | n/a | low | navigation tests |
| `lsp/navigation/associated_values.rs` | associated const/function definition routing | parsed impl items and generated InitSpace ownership | account attribute expression values | n/a | n/a | low | navigation tests |
| `lsp/renaming/mod.rs` | semantic rename edits | parsed declarations/usages + workspace index | declarations/usages | n/a | n/a | low | rename tests |
| `lsp/selection_ranges/mod.rs` | hierarchical selections | parsed accounts/fields/constraint ranges | cursor position | n/a | n/a | low | selection range tests |
| `core/workspace/mod.rs` | workspace symbols and cross-file index | parsed document symbols + transactional index | workspace files | n/a | n/a | low | workspace tests |
| `server/reports.rs` | agent-facing reports | parsed symbols/evidence/diagnostics | request-scoped document/workspace | n/a | n/a | low | server report tests |

## Branch-Level Expansion

These rows expand providers that have multiple user-visible branches behind one
router. Keep these rows branch-level so Phase D false-positive fixtures do not
hide behind file-level coverage.

| file | provider branch | AST-aware | region-aware | confidence | topic | substring risk | fixture |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `lsp/diagnostics/initialization.rs` | missing init companion evidence | evidence graph over parsed accounts + instructions | accounts struct field spans | authoritative | `seagrass/anchor.init.missing-companion` | low | initialization tests |
| `lsp/diagnostics/constraint_expressions/assignments.rs` | generated expression assignment value resolution | parsed assignment values plus generated catalog value kinds | account attribute expression spans | authoritative | `seagrass/anchor.constraint.expression` | low | expression value tests |
| `lsp/diagnostics/constraint_expressions/bump.rs` | explicit PDA bump expression resolution | parsed expression values plus account-data member type checks | account attribute expression spans | authoritative | `seagrass/anchor.constraint.expression` | low | expression value tests |
| `lsp/diagnostics/constraint_expressions/resolution.rs` | constraint identifier/path scope resolution | parsed document values, imports, account fields, and instruction args | account attribute expression spans | authoritative | `seagrass/anchor.constraint.expression` | low | expression value tests |
| `lsp/diagnostics/constraint_shape/pda.rs` | init with foreign seeds program | parsed constraint evidence | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | `constraint_shape::pda` tests |
| `lsp/diagnostics/constraint_shape/pda.rs` | seed/bump pairing and seed limits | parsed PDA projection | account attributes | authoritative/heuristic | `seagrass/anchor.constraint.shape`, `seagrass/security.pda.static-seed` | low | PDA seed fixtures |
| `lsp/diagnostics/constraint_shape/init_lifecycle.rs` | payer/system/realloc/close/zero lifecycle checks | parsed constraint and field evidence | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | init lifecycle fixtures |
| `lsp/diagnostics/constraint_shape/catalog.rs` | generated catalog companions and keyword values | generated catalog + parsed constraints | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | catalog fixtures |
| `lsp/diagnostics/constraint_shape/token.rs` | token/mint init and required program checks | parsed SPL constraint evidence | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | token constraint fixtures |
| `lsp/diagnostics/constraint_shape/has_one.rs` | `has_one` field compatibility | parsed account data + workspace index | account attributes | authoritative | `seagrass/anchor.constraint.shape` | low | has-one fixtures |
| `lsp/diagnostics/constraint_shape/program_account.rs` | unchecked program account validation | parsed account field constraints | account fields | heuristic | `seagrass/security.account.unchecked` | low | program account fixtures |
| `lsp/diagnostics/security/mod.rs` | typed signer authorization | `Signer<T>` account visitor | accounts struct fields | authoritative | `seagrass/security.signer.authorization` | low | security core tests |
| `lsp/diagnostics/security/mod.rs` | typed sysvar address checks | sysvar account visitor | accounts struct fields | authoritative | `seagrass/security.sysvar.address` | low | security core tests |
| `lsp/diagnostics/security/mod.rs` | token account unpacking checks | token account visitor | accounts struct fields | heuristic | `seagrass/security.token-account` | low | security core tests |
| `lsp/diagnostics/security/mod.rs` | raw owner checks | visitor + raw account evidence | handler body + accounts field | heuristic | `seagrass/security.owner-check` | low | raw account tests |
| `lsp/diagnostics/security/mod.rs` | raw type cosplay checks | visitor + discriminator evidence | handler body + accounts field | heuristic | `seagrass/security.type-cosplay` | low | raw account tests |
| `lsp/diagnostics/security/mod.rs` | arbitrary CPI program checks | visitor + workspace program evidence | accounts struct fields | authoritative/heuristic | `seagrass/security.cpi.program` | low | CPI program tests |
| `lsp/diagnostics/security/duplicates.rs` | duplicate mutable account checks | `LintVisitor` over accounts structs | accounts struct fields | heuristic | `seagrass/security.account.duplicate-mutable` | low | duplicate tests |
| `lsp/diagnostics/security/raw_account.rs` | raw data/deserialize evidence collector | `syn::visit::Visit` over functions and macros | handler bodies only | n/a | `seagrass/security.owner-check`, `seagrass/security.type-cosplay` | fixed: comments/strings/dead macros | `ignores_raw_account_text_in_comments_strings_and_attributes`, `ignores_security_evidence_inside_dead_macro_body` |
| `lsp/diagnostics/code_quality/native_raw.rs` | native owner/discriminator validation | AST visitors and expression predicates | native helper bodies | heuristic | `seagrass/solana.code-quality`, `seagrass/security.owner-check`, `seagrass/security.type-cosplay` | fixed: helpers/comments/strings | `ignores_raw_account_text_in_comments_and_strings`, `ignores_deserialize_text_without_raw_account_data_read` |
| `lsp/diagnostics/code_quality/manual_close.rs` | account closing and reinitialization | AST visitor over assignments and transfers | function bodies | heuristic | `seagrass/solana.code-quality.account-closing`, `seagrass/solana.code-quality.initialization` | fixed: attributes/comments/strings | `ignores_manual_close_and_reinit_text_in_comments_strings_and_attributes` |
| `lsp/diagnostics/code_quality/stale_cpi.rs` | stale account after CPI | AST visitor over CPI/reload/read ordering | function bodies | heuristic | `seagrass/solana.code-quality.stale-account-after-cpi` | fixed: comments/strings | `ignores_stale_cpi_text_in_comments_and_strings` |
| `lsp/diagnostics/code_quality/native_validation.rs` | native signer and CPI validation | AST visitor over AccountInfo guards | native helper bodies | heuristic | `seagrass/security.signer.authorization`, `seagrass/security.cpi.program` | fixed: unrelated helpers/static IDs | `ignores_native_account_validation_text_in_comments_and_strings`, `ignores_known_program_id_instruction` |
| `lsp/diagnostics/mod.rs` | parser init constraint topic split | parser messages with structured diagnostic data | parse-error span | authoritative | `seagrass/anchor.init.constraints`, `seagrass/anchor.init.missing-payer`, `seagrass/anchor.init.missing-space` | low | parser init constraint diagnostics tests |
| `lsp/diagnostics/spl_semantics.rs` | mint reference type checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/spl_semantics.rs` | token/mint target container checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/spl_semantics.rs` | token program override checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/spl_semantics.rs` | Token-2022 extension checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/spl_semantics.rs` | mint decimals argument checks | parsed instruction args | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/spl_semantics.rs` | transfer-hook program checks | evidence graph constraints | account attributes | derived | `seagrass/anchor.spl-token-interface` | low | SPL semantic tests |
| `lsp/diagnostics/artifacts.rs` | SBPF artifact missing/invalid/stale | project artifact metadata | project file span | derived | `seagrass/anchor.artifact.sbf` | low | artifact tests |
| `lsp/diagnostics/artifacts.rs` | program keypair missing/invalid/mismatch | keypair metadata + declared id | project file span | derived | `seagrass/anchor.artifact.program-keypair` | low | artifact tests |
| `lsp/diagnostics/artifacts.rs` | IDL artifact missing/invalid/stale/mismatch | artifact metadata | project file span | derived | `seagrass/anchor.artifact.idl` | low | artifact tests |
| `lsp/diagnostics/artifacts.rs` | generated TypeScript artifact checks | artifact metadata | project file span | derived | `seagrass/anchor.artifact.types` | low | artifact tests |
| `lsp/diagnostics/ecosystem.rs` | non-Anchor IDL ecosystem checks | ecosystem metadata | project file span | derived | `seagrass/solana.artifact.idl` | low | ecosystem tests |
| `lsp/diagnostics/ecosystem.rs` | program metadata checks | ecosystem metadata | project file span | derived | `seagrass/solana.program-metadata` | low | ecosystem tests |
| `lsp/diagnostics/ecosystem.rs` | test harness checks | ecosystem metadata | project file span | derived | `seagrass/solana.test-harness` | low | ecosystem tests |
| `lsp/diagnostics/ecosystem.rs` | surfpool workspace checks | ecosystem metadata | project file span | derived | `seagrass/solana.surfpool-workspace` | low | ecosystem tests |
| `lsp/completions/mod.rs` | context type completions | `CursorContext` + parsed account structs | function signatures | n/a | n/a | low | context completion tests |
| `lsp/completions/mod.rs` | account constraint snippets | generated catalog + pending-fix data | account attributes | n/a | n/a | low | constraint completion tests |
| `lsp/completions/mod.rs` | completion resolve docs | generated catalog docs | completion item data | n/a | n/a | low | resolve tests |
| `lsp/completions/constraint_values/candidates.rs` | signer/account/program/space/seed candidates | account semantics + workspace symbols | account attribute values | n/a | n/a | low | constraint value tests |
| `lsp/actions/accounts/mod.rs` | account name replacement, add field, mut, duplicate actions | diagnostic data + parsed accounts | diagnostic span | n/a | n/a | low | account action tests |
| `lsp/actions/constraints.rs` | duplicate/order/generated constraint actions | generated parser catalog + parsed ranges | account attributes | n/a | n/a | low | constraint action tests |
| `lsp/actions/features.rs` | feature/Cargo/declaration sync actions | manifest diagnostic data | manifest/project spans | n/a | n/a | low | feature action tests |
| `lsp/actions/pda.rs` | IDL limitation and PDA derivation actions | diagnostic seed metadata | diagnostic span | n/a | n/a | low | PDA action tests |
| `lsp/actions/security.rs` | owner/type/sysvar/native/security remediation actions | structured diagnostic data + parsed edits | diagnostic span | n/a | n/a | low | security action tests |
| `lsp/actions/init_constraints.rs` | placeholder and fix-all init actions | init diagnostic data + attribute ranges | account attributes | n/a | n/a | low | init action tests |
| `lsp/actions/instructions.rs` | add/replace/remove instruction args | parsed handler args + instruction attrs | instruction attrs | n/a | n/a | low | instruction action tests |
| `lsp/actions/missing_init.rs` | add/fix-all missing init companion actions | parsed accounts structs | accounts struct fields | n/a | n/a | low | missing init action tests |

## Quickfix Coverage Matrix

Coverage values: `covered` means every emitted quickfix branch has a routed code
action; `partial` means some branches still lack machine-applicable repair;
`guidance` means at least one branch is intentionally no-edit advice; `gap`
means the diagnostic currently has no code action.

| diagnostic code | quickfix coverage | action emitter | quickfix tags | gaps |
| --- | --- | --- | --- | --- |
| `anchor-account-usage` | covered | `lsp/actions/accounts/mod.rs` | `add-mut-constraint`, `code-routed` | none |
| `anchor-check-cfg` | covered | `lsp/actions/features.rs` | `add-anchor-debug-feature`, `add-init-if-needed-feature`, `add-solana-target-os-check-cfg` | none |
| `anchor-constraint-expression` | partial | `lsp/actions/constraint_expressions.rs` | `replace-constraint-expression-member`, `replace-constraint-expression-identifier` | Import suggestions remain deferred; unresolved local typos use scoped replacement candidates. |
| `anchor-constraint-shape` | partial | `lsp/actions/constraints.rs`, `lsp/actions/accounts/mod.rs`, `lsp/actions/features.rs`, `lsp/actions/security.rs` | `parser-rule:duplicate`, `parser-rule:ordering`, `remove-conflicting-constraints`, `replace-keyword-value`, `replace-account-type`, `program-field-type`, `system-program-type`, `replace-has-one-target`, `add-missing-constraint`, `add-mut-constraint` | Generic shape diagnostics without parser-rule or quickfix metadata remain diagnostic-only. |
| `anchor-context-accounts` | covered | `lsp/actions/accounts/context_structs.rs` | `derive-accounts`, `create-accounts-struct`, `fill-context-type` | none |
| `anchor-idl-artifact` | gap | n/a | none | Add build/IDL-generation command action from `buildCommand`. |
| `anchor-init-constraints` | covered | `lsp/actions/init_constraints.rs` | `init-placeholders` | none |
| `anchor-missing-account-reference` | covered | `lsp/actions/accounts/mod.rs`, `lsp/actions/accounts/field_edits.rs` | `code-routed` | none |
| `anchor-missing-init-constraint` | covered | `lsp/actions/missing_init.rs` | `code-routed` | none |
| `anchor-missing-instruction-argument` | covered | `lsp/actions/instructions.rs` | `add-instruction-argument`, `replace-instruction-argument`, `remove-instruction-argument` | none |
| `anchor-pda-seed-resolution` | covered | `lsp/actions/pda.rs` | `code-routed` | none |
| `anchor-program-keypair` | gap | n/a | none | Add keypair sync/generation action from artifact metadata. |
| `anchor-project-id` | covered | `lsp/actions/features.rs` | `sync-declare-id` | none |
| `anchor-sbf-artifact` | gap | n/a | none | Add build command action from `buildCommand`. |
| `anchor-spl-token-interface` | gap | n/a | none | Add token-interface type/constraint conversion actions. |
| `anchor-syn` | partial | `lsp/actions/security.rs` | `replace-account-type`, `replace-invalid-sysvar` | Generic parser diagnostics without typed fix metadata remain diagnostic-only. |
| `anchor-types-artifact` | gap | n/a | none | Add TypeScript artifact generation action from `buildCommand`. |
| `solana-code-quality` | partial | `lsp/actions/security.rs` | `use-checked-data-access`, `add-static-pda-domain-seed`, `prefer-anchor-close`, `reject-reinit`, `insert-reload-after-cpi`, `add-signer-check`, `add-program-id-check`, `add-owner-check`, `add-discriminator-check` | Manual close/reinit/discriminator branches are guidance-only; arithmetic/unwrap/bump branches have no quickfix. |
| `solana-idl-artifact` | gap | n/a | none | Add IDL write/import command action for non-Anchor program metadata. |
| `solana-program-metadata` | gap | n/a | none | Expose `suggestedCommand` as a command/code action. |
| `solana-surfpool-workspace` | gap | n/a | none | Add Surfpool workspace config action. |
| `solana-test-harness` | gap | n/a | none | Add test harness scaffold action. |
| `anchor-security-cpi-program` | covered | `lsp/actions/security.rs`, `lsp/actions/constraints.rs` | `replace-account-type`, `add-missing-constraint` | none |
| `anchor-security-duplicate-account` | covered | `lsp/actions/accounts/mod.rs` | `duplicate-account-remediation` | none |
| `anchor-security-owner-check` | covered | `lsp/actions/security.rs` | `add-owner-constraint`, `add-owner-check` | none |
| `anchor-security-signer` | covered | `lsp/actions/security.rs` | `replace-account-type` | none |
| `anchor-security-static-pda` | covered | `lsp/actions/security.rs` | `add-scoped-pda-seed` | none |
| `anchor-security-sysvar` | covered | `lsp/actions/security.rs` | `replace-account-type` | none |
| `anchor-security-token-account` | covered | `lsp/actions/security.rs` | `replace-account-type` | none |
| `anchor-security-type-cosplay` | guidance | `lsp/actions/security.rs` | `typed-account-or-discriminator`, `add-discriminator-check` | Add concrete discriminator/type edit for raw native branches. |
| `anchor-security-unchecked-account` | covered | `lsp/actions/constraints.rs` | `add-missing-constraint` | none |

## Tracking Rows

- Keep `scripts/check-rule-hygiene.ts` at zero legacy diagnostic findings.
- Keep `docs/topics.json` synchronized with emitted diagnostic topics.
- Keep this table updated for editor providers, not only diagnostic emitters.
