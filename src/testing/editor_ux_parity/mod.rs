use {
    crate::{
        actions, completions,
        constraint_catalog::{self, ConstraintValueKind},
        diagnostics,
        document::ParsedDocument,
        hover,
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{Diagnostic, NumberOrString, Position, Url},
};

mod account_member_wakeup;
mod account_usage;
mod constraint_expressions;
mod handler_constructor_wrappers;
mod handler_macro_expressions;
mod handler_match_outputs;
mod handler_return_usage;
mod handler_scope;
mod handler_struct_literals;
mod handler_tuple_patterns;
mod handler_value_completions;
mod handler_wrapper_outputs;
mod proactive_assists;

struct EditorCase<'a> {
    name: &'a str,
    source: &'a str,
    completions: &'a [CompletionCheck<'a>],
    diagnostics: &'a [DiagnosticCheck<'a>],
    actions: &'a [ActionCheck<'a>],
}

struct CompletionCheck<'a> {
    marker: &'a str,
    first_label: &'a str,
    preselect: bool,
}

struct DiagnosticCheck<'a> {
    expected: &'a str,
    evidence: &'a str,
    message_contains: &'a str,
}

struct ActionCheck<'a> {
    diagnostic_expected: &'a str,
    title_contains: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct SemanticVariant<'a> {
    name: &'a str,
    state: &'a str,
    data_struct: &'a str,
    mint_member: &'a str,
    token_account: &'a str,
    mint_account: &'a str,
    token_property: &'a str,
    token_property_check: &'a str,
    token_before_mint: bool,
    compact: bool,
}

#[test]
fn editor_ux_parity_for_anchor_semantic_account_shapes() {
    for case in editor_cases() {
        assert_editor_case(case);
    }
}

#[test]
fn semantic_account_shape_inference_survives_name_order_and_format_variants() {
    let variants = [
        SemanticVariant {
            name: "ledger receipt before mint",
            state: "ledger",
            data_struct: "LedgerState",
            mint_member: "asset_mint",
            token_account: "receipt",
            mint_account: "asset_mint_account",
            token_property: "amount",
            token_property_check: "1",
            token_before_mint: true,
            compact: false,
        },
        SemanticVariant {
            name: "position mint before token",
            state: "position",
            data_struct: "PositionState",
            mint_member: "position_mint",
            token_account: "nft_escrow",
            mint_account: "nft_mint",
            token_property: "delegated_amount",
            token_property_check: "0",
            token_before_mint: false,
            compact: true,
        },
        SemanticVariant {
            name: "reward vault compact relation",
            state: "reward_state",
            data_struct: "RewardState",
            mint_member: "reward_mint",
            token_account: "reward_vault",
            mint_account: "reward_mint_account",
            token_property: "amount",
            token_property_check: "pending_rewards",
            token_before_mint: true,
            compact: true,
        },
    ];

    for variant in variants {
        assert_semantic_variant(variant);
    }
}

#[test]
fn anchor_docs_constraint_surface_has_catalog_entries() {
    let docs_surface = [
        ("signer", false, ConstraintValueKind::None),
        ("mut", false, ConstraintValueKind::None),
        ("init", false, ConstraintValueKind::None),
        ("init_if_needed", false, ConstraintValueKind::None),
        ("seeds", true, ConstraintValueKind::Seeds),
        ("bump", false, ConstraintValueKind::None),
        (
            "seeds::program",
            true,
            ConstraintValueKind::ProgramReference,
        ),
        ("has_one", true, ConstraintValueKind::AccountReference),
        ("address", true, ConstraintValueKind::AnyExpression),
        ("owner", true, ConstraintValueKind::AnyExpression),
        ("executable", false, ConstraintValueKind::None),
        ("zero", false, ConstraintValueKind::None),
        ("close", true, ConstraintValueKind::AccountReference),
        ("constraint", true, ConstraintValueKind::AnyExpression),
        ("realloc", true, ConstraintValueKind::Space),
        ("realloc::payer", true, ConstraintValueKind::SignerReference),
        ("realloc::zero", true, ConstraintValueKind::Boolean),
        ("token::mint", true, ConstraintValueKind::AccountReference),
        (
            "token::authority",
            true,
            ConstraintValueKind::SignerReference,
        ),
        (
            "token::token_program",
            true,
            ConstraintValueKind::ProgramReference,
        ),
        (
            "mint::authority",
            true,
            ConstraintValueKind::SignerReference,
        ),
        (
            "mint::freeze_authority",
            true,
            ConstraintValueKind::SignerReference,
        ),
        (
            "mint::decimals",
            true,
            ConstraintValueKind::InstructionArgument,
        ),
        (
            "associated_token::mint",
            true,
            ConstraintValueKind::AccountReference,
        ),
        (
            "associated_token::authority",
            true,
            ConstraintValueKind::SignerReference,
        ),
        (
            "associated_token::token_program",
            true,
            ConstraintValueKind::ProgramReference,
        ),
    ];

    for (key, has_assignment, value_kind) in docs_surface {
        let spec = constraint_catalog::by_key_with_assignment(key, has_assignment)
            .unwrap_or_else(|| panic!("missing Anchor docs constraint `{key}`"));
        assert_eq!(
            spec.value_kind, value_kind,
            "unexpected value kind for Anchor docs constraint `{key}`"
        );
    }
}

#[test]
fn editor_ux_hover_routes_account_attribute_values_through_cursor_context() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, token_name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [token_name.as_bytes()], bump)]
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let position = position_after(source, "seeds = [token_name");
    let hover = hover::hover(&document, position).expect("instruction argument hover");
    let tower_lsp::lsp_types::HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup hover");
    };

    assert!(markup
        .value
        .contains("Anchor instruction argument for `Context<Create>`"));
    assert!(markup.value.contains("Type: `String`"));
}

#[test]
fn editor_ux_pda_seed_resolution_keeps_nested_account_fields_visible() {
    let source = r#"
#[program]
pub mod demo {
    pub fn collect_fees_v2(ctx: Context<CollectFeesV2>, bundle_index: u16) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(
        seeds = [
            b"bundled_position",
            position_bundle.position_bundle_mint.key().as_ref(),
            bundle_index.to_string().as_bytes(),
        ],
        bump,
    )]
    pub bundled_position: Account<'info, BundledPosition>,
    pub position_bundle: Account<'info, PositionBundle>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);
    let pda_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| is_pda_seed_resolution_diagnostic(diagnostic))
        .collect::<Vec<_>>();

    assert!(
        pda_diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("bundle_index.to_string().as_bytes()")
                && diagnostic
                    .message
                    .contains("cannot be represented in the IDL")
        }),
        "expected transformed argument seed warning; got {pda_diagnostics:#?}"
    );
    assert!(
        pda_diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("position_bundle.position_bundle_mint")
        }),
        "nested account field seed should stay IDL-visible; got {pda_diagnostics:#?}"
    );

    let transformed_seed_warning = pda_diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("bundle_index.to_string().as_bytes()")
        })
        .unwrap_or_else(|| {
            panic!("expected transformed seed diagnostic; got {pda_diagnostics:#?}")
        });
    let related_information = transformed_seed_warning
        .related_information
        .as_ref()
        .expect("expected related information for PDA seed warning");
    let invisible_related = related_information
        .iter()
        .filter(|info| info.message.starts_with("IDL-invisible seed"))
        .collect::<Vec<_>>();

    assert_eq!(
        invisible_related.len(),
        1,
        "expected one invisible-seed hint; got {related_information:?}"
    );
    assert!(invisible_related[0]
        .message
        .contains("bundle_index.to_string().as_bytes()"));
    assert!(
        related_information.iter().any(|info| {
            info.message
                .contains("mixed IDL-visible and IDL-invisible seeds")
        }),
        "expected mixed seed context as related information; got {related_information:?}"
    );
    assert!(
        related_information.iter().any(|info| {
            info.message.contains("IDL-visible seed")
                && info
                    .message
                    .contains("position_bundle.position_bundle_mint.key().as_ref()")
        }),
        "expected visible related hint for nested account field seed; got {related_information:?}"
    );
}

#[test]
fn init_constraint_diagnostics_are_low_noise_and_link_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);
    let init_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_data_str(diagnostic, "topic").is_some_and(|topic| {
                matches!(
                    topic,
                    "seagrass/anchor.init.missing-payer" | "seagrass/anchor.init.missing-space"
                )
            })
        })
        .collect::<Vec<_>>();

    assert!(
        init_diagnostics.len() <= 2,
        "init diagnostics should stay low-noise; got {init_diagnostics:#?}"
    );
    for diagnostic in init_diagnostics {
        assert_related_information(
            diagnostic,
            &[
                "requires this companion",
                "`state` is the account being initialized",
                "Add the missing companion",
            ],
        );
    }
}

#[test]
fn editor_ux_does_not_flag_account_attribute_deref_as_arithmetic() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Interface, TokenInterface};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(address = *token_mint_a.to_account_info().owner)]
    pub token_program_a: Interface<'info, TokenInterface>,
    #[account(address = *token_mint_b.to_account_info().owner)]
    pub token_program_b: Interface<'info, TokenInterface>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics.is_empty(),
        "Whirlpools token owner account attributes should not produce diagnostics: {diagnostics:#?}"
    );
}

fn editor_cases<'a>() -> Vec<EditorCase<'a>> {
    vec![
        EditorCase {
            name: "address alias plus token mint equality infers mint generic",
            source: r#"
#[derive(Accounts)]
pub struct SettlePosition<'info> {
    pub pool_state: Box<Account<'info, PoolState>>,
    #[account(mut, constraint = owner_receipt.mint == pool_state.asset_mint)]
    pub owner_receipt: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address = pool_state.asset_mint)]
    pub asset_mint: InterfaceAccount<'info, /*caret:mint_generic*/>,
}

#[account]
pub struct LocalAccountData {
    pub value: u64,
}
"#,
            completions: &[CompletionCheck {
                marker: "mint_generic",
                first_label: "Mint",
                preselect: true,
            }],
            diagnostics: &[DiagnosticCheck {
                expected: "InterfaceAccount<'info, Mint>",
                evidence: "account-reference",
                message_contains: "Missing account data type for `asset_mint`",
            }],
            actions: &[ActionCheck {
                diagnostic_expected: "InterfaceAccount<'info, Mint>",
                title_contains: "InterfaceAccount<'info, Mint>",
            }],
        },
        EditorCase {
            name: "custom SPL token-account property constraints infer token account",
            source: r#"
#[derive(Accounts)]
pub struct SettlePosition<'info> {
    pub position_record: Account<'info, PositionRecord>,
    #[account(
        constraint = receipt_account.mint == position_record.position_mint,
        constraint = receipt_account.amount == 1,
    )]
    pub receipt_account: InterfaceAccount<'info, /*caret:token_generic*/>,
}

#[account]
pub struct PositionRecord {
    pub position_mint: Pubkey,
}
"#,
            completions: &[CompletionCheck {
                marker: "token_generic",
                first_label: "TokenAccount",
                preselect: true,
            }],
            diagnostics: &[DiagnosticCheck {
                expected: "InterfaceAccount<'info, TokenAccount>",
                evidence: "token-account-property",
                message_contains: "Missing account data type for `receipt_account`",
            }],
            actions: &[ActionCheck {
                diagnostic_expected: "InterfaceAccount<'info, TokenAccount>",
                title_contains: "InterfaceAccount<'info, TokenAccount>",
            }],
        },
        EditorCase {
            name: "constraint value completions consume semantic mint inference",
            source: r#"
#[derive(Accounts)]
pub struct SettlePosition<'info> {
    pub pool_state: Box<Account<'info, PoolState>>,
    #[account(mut, constraint = owner_receipt.mint == pool_state.asset_mint)]
    pub owner_receipt: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address = pool_state.asset_mint)]
    pub asset_mint: InterfaceAccount<'info, M>,
    #[account(token::mint = asset_/*caret:token_mint_value*/)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub payer: Signer<'info>,
}
"#,
            completions: &[CompletionCheck {
                marker: "token_mint_value",
                first_label: "asset_mint",
                preselect: true,
            }],
            diagnostics: &[DiagnosticCheck {
                expected: "InterfaceAccount<'info, Mint>",
                evidence: "account-reference",
                message_contains: "`M` is unresolved",
            }],
            actions: &[ActionCheck {
                diagnostic_expected: "InterfaceAccount<'info, Mint>",
                title_contains: "InterfaceAccount<'info, Mint>",
            }],
        },
    ]
}

fn assert_semantic_variant(variant: SemanticVariant<'_>) {
    let token_block = if variant.compact {
        format!(
            "#[account(constraint={token}.mint=={state}.{mint_member},constraint={token}.{property}=={check})]\n    pub {token}: InterfaceAccount<'info, /*caret:token_generic*/>,",
            token = variant.token_account,
            state = variant.state,
            mint_member = variant.mint_member,
            property = variant.token_property,
            check = variant.token_property_check,
        )
    } else {
        format!(
            "#[account(\n        constraint = {token}.mint == {state}.{mint_member},\n        constraint = {token}.{property} == {check},\n    )]\n    pub {token}: InterfaceAccount<'info, /*caret:token_generic*/>,",
            token = variant.token_account,
            state = variant.state,
            mint_member = variant.mint_member,
            property = variant.token_property,
            check = variant.token_property_check,
        )
    };
    let mint_block = if variant.compact {
        format!(
            "#[account(address={state}.{mint_member})]\n    pub {mint}: InterfaceAccount<'info, /*caret:mint_generic*/>,",
            state = variant.state,
            mint_member = variant.mint_member,
            mint = variant.mint_account,
        )
    } else {
        format!(
            "#[account(address = {state}.{mint_member})]\n    pub {mint}: InterfaceAccount<'info, /*caret:mint_generic*/>,",
            state = variant.state,
            mint_member = variant.mint_member,
            mint = variant.mint_account,
        )
    };
    let (first_block, second_block) = if variant.token_before_mint {
        (token_block.as_str(), mint_block.as_str())
    } else {
        (mint_block.as_str(), token_block.as_str())
    };
    let source = format!(
        r#"
#[derive(Accounts)]
pub struct GeneratedUxCase<'info> {{
    pub {state}: Account<'info, {data_struct}>,
    pub pending_rewards: u64,
    {first_block}
    {second_block}
}}

#[account]
pub struct {data_struct} {{
    pub {mint_member}: Pubkey,
}}
"#,
        state = variant.state,
        data_struct = variant.data_struct,
        mint_member = variant.mint_member,
        first_block = first_block,
        second_block = second_block,
    );

    let marked = strip_markers(&source);
    let document = ParsedDocument::parse_or_empty(&marked.source);
    let diagnostics = diagnostics::collect(&document);
    let uri = Url::parse("file:///editor-ux-parity-variant.rs").unwrap();

    assert_first_completion(
        variant.name,
        &document,
        &marked,
        "mint_generic",
        "Mint",
        true,
    );
    assert_first_completion(
        variant.name,
        &document,
        &marked,
        "token_generic",
        "TokenAccount",
        true,
    );

    let expected_mint = "InterfaceAccount<'info, Mint>";
    let expected_token = "InterfaceAccount<'info, TokenAccount>";
    assert_diagnostic(
        variant.name,
        &diagnostics,
        expected_mint,
        "account-reference",
        &format!("Missing account data type for `{}`", variant.mint_account),
    );
    assert_diagnostic(
        variant.name,
        &diagnostics,
        expected_token,
        "token-account-property",
        &format!("Missing account data type for `{}`", variant.token_account),
    );
    assert_action(
        variant.name,
        &document,
        uri.clone(),
        &diagnostics,
        expected_mint,
        expected_mint,
    );
    assert_action(
        variant.name,
        &document,
        uri,
        &diagnostics,
        expected_token,
        expected_token,
    );
}

fn assert_editor_case(case: EditorCase<'_>) {
    let marked = strip_markers(case.source);
    let document = ParsedDocument::parse_or_empty(&marked.source);

    for check in case.completions {
        assert_first_completion(
            case.name,
            &document,
            &marked,
            check.marker,
            check.first_label,
            check.preselect,
        );
    }

    let diagnostics = diagnostics::collect(&document);
    for check in case.diagnostics {
        assert_diagnostic(
            case.name,
            &diagnostics,
            check.expected,
            check.evidence,
            check.message_contains,
        );
    }

    let uri = Url::parse("file:///editor-ux-parity.rs").unwrap();
    for check in case.actions {
        assert_action(
            case.name,
            &document,
            uri.clone(),
            &diagnostics,
            check.diagnostic_expected,
            check.title_contains,
        );
    }
}

fn assert_first_completion(
    case_name: &str,
    document: &ParsedDocument,
    marked: &MarkedSource,
    marker: &str,
    first_label: &str,
    preselect: bool,
) {
    let position = *marked
        .positions
        .get(marker)
        .unwrap_or_else(|| panic!("{case_name}: missing caret marker `{marker}`"));
    let items = completions::completions(document, position)
        .unwrap_or_else(|| panic!("{case_name}: no completions at marker `{marker}`"));
    let first = items
        .first()
        .unwrap_or_else(|| panic!("{case_name}: empty completions at marker `{marker}`"));
    assert_eq!(
        first.label,
        first_label,
        "{case_name}: wrong first completion at marker `{marker}`; labels: {:?}",
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        first.preselect,
        Some(preselect),
        "{case_name}: wrong preselect for first completion `{}`",
        first.label
    );
}

fn assert_diagnostic(
    case_name: &str,
    diagnostics: &[Diagnostic],
    expected: &str,
    evidence: &str,
    message_contains: &str,
) {
    let diagnostic = diagnostic_with_expected(diagnostics, expected).unwrap_or_else(|| {
        panic!("{case_name}: missing diagnostic for `{expected}`; diagnostics: {diagnostics:#?}")
    });
    assert!(
        diagnostic.message.contains(message_contains),
        "{case_name}: diagnostic for `{expected}` had wrong message: {}",
        diagnostic.message
    );
    assert_eq!(
        diagnostic_data_str(diagnostic, "evidence"),
        Some(evidence),
        "{case_name}: diagnostic for `{expected}` had wrong evidence",
    );
}

fn assert_action(
    case_name: &str,
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
    diagnostic_expected: &str,
    title_contains: &str,
) {
    let diagnostic =
        diagnostic_with_expected(diagnostics, diagnostic_expected).unwrap_or_else(|| {
            panic!("{case_name}: missing action diagnostic `{diagnostic_expected}`")
        });
    let actions = actions::code_actions(document, uri, diagnostic.range, diagnostics);
    assert!(
        actions
            .iter()
            .any(|action| action.title.contains(title_contains)),
        "{case_name}: missing action containing `{title_contains}`; actions: {:?}",
        actions
            .iter()
            .map(|action| action.title.as_str())
            .collect::<Vec<_>>()
    );
}

fn diagnostic_with_expected<'a>(
    diagnostics: &'a [Diagnostic],
    expected: &str,
) -> Option<&'a Diagnostic> {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic_data_str(diagnostic, "expected") == Some(expected))
}

fn is_pda_seed_resolution_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.code.as_ref().is_some_and(
        |code| matches!(code, NumberOrString::String(code) if code == "anchor-pda-seed-resolution"),
    )
}

fn diagnostic_data_str<'a>(diagnostic: &'a Diagnostic, key: &str) -> Option<&'a str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
}

fn assert_related_information(diagnostic: &Diagnostic, expected_messages: &[&str]) {
    let related_information = diagnostic
        .related_information
        .as_ref()
        .unwrap_or_else(|| panic!("missing related information for {diagnostic:?}"));

    for expected_message in expected_messages {
        assert!(
            related_information
                .iter()
                .any(|info| info.message.contains(expected_message)),
            "missing related information containing {expected_message:?}; got {related_information:?}"
        );
    }
}

struct MarkedSource {
    source: String,
    positions: HashMap<String, Position>,
}

fn strip_markers(marked: &str) -> MarkedSource {
    let mut source = String::with_capacity(marked.len());
    let mut positions = HashMap::new();
    let mut tail = marked;

    while let Some(start) = tail.find("/*caret:") {
        source.push_str(&tail[..start]);
        let marker_start = start + "/*caret:".len();
        let marker_tail = &tail[marker_start..];
        let marker_end = marker_tail.find("*/").expect("caret marker closes");
        let marker = marker_tail[..marker_end].to_string();
        let position = position_at(&source, source.len());
        let previous = positions.insert(marker.clone(), position);
        assert!(previous.is_none(), "duplicate caret marker `{marker}`");
        tail = &marker_tail[marker_end + "*/".len()..];
    }

    source.push_str(tail);
    MarkedSource { source, positions }
}

fn position_at(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.chars().filter(|ch| *ch == '\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    Position {
        line,
        character: prefix[line_start..].chars().count() as u32,
    }
}

fn position_after(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle present") + needle.len();
    position_at(source, offset)
}
