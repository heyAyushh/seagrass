use {
    super::*,
    crate::{
        diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
    },
    std::{fs, path::Path},
    tower_lsp::lsp_types::{NumberOrString, Position, Range, Url},
};

#[test]
fn reports_anchor_account_diagnostics() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("payer")));
}

#[test]
fn bind_current_document_related_uri_rewrites_placeholder_only() {
    let target_uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let external_uri = Url::parse("file:///workspace/Cargo.toml").unwrap();
    let placeholder_uri = Url::parse(CURRENT_DOCUMENT_PLACEHOLDER_URI).unwrap();

    let mut diagnostics = vec![Diagnostic {
        range: Range::default(),
        severity: None,
        code: None,
        code_description: None,
        source: None,
        message: "msg".to_string(),
        related_information: Some(vec![
            DiagnosticRelatedInformation {
                location: tower_lsp::lsp_types::Location {
                    uri: placeholder_uri,
                    range: Range::default(),
                },
                message: "current document".to_string(),
            },
            DiagnosticRelatedInformation {
                location: tower_lsp::lsp_types::Location {
                    uri: external_uri.clone(),
                    range: Range::default(),
                },
                message: "external".to_string(),
            },
        ]),
        tags: None,
        data: None,
    }];

    bind_current_document_related_uri(&mut diagnostics, &target_uri);

    let related = diagnostics[0].related_information.as_ref().unwrap();
    assert_eq!(related[0].location.uri, target_uri);
    assert_eq!(related[1].location.uri, external_uri);
}

#[test]
fn generated_parser_rules_classify_anchor_constraint_shape_diagnostics() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, mut)]
    pub state: Account<'info, State>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let diagnostic = collect(&document)
        .into_iter()
        .find(|diagnostic| diagnostic.message.contains("`mut` is duplicated"))
        .unwrap();

    assert!(!diagnostic.message.contains("mut already provided"));
    assert!(matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == "anchor-constraint-shape"
    ));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("constraint"))
            .and_then(|value| value.as_str()),
        Some("mut")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("parserRule"))
            .and_then(|rule| rule.get("kind"))
            .and_then(|value| value.as_str()),
        Some("duplicate")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("parserRule"))
            .and_then(|rule| rule.get("message"))
            .and_then(|value| value.as_str()),
        Some("mut already provided")
    );
    let related = diagnostic
        .related_information
        .as_ref()
        .expect("constraint diagnostics should point at source evidence");
    assert!(
        related
            .iter()
            .any(|info| info.message.contains("`mut` constraint")),
        "expected related information for the duplicated `mut` constraint; got {related:?}"
    );
}

#[test]
fn semantic_related_information_enriches_structured_diagnostic_data() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let mut diagnostics = vec![diagnostic_from_range(
        Range {
            start: Position {
                line: 3,
                character: 15,
            },
            end: Position {
                line: 3,
                character: 19,
            },
        },
        AnchorDiagnosticKind::AnchorConstraintShape,
        "synthetic structured diagnostic".to_string(),
        Some(serde_json::json!({
            "account": "state",
            "accountsStruct": "Create",
            "constraint": "init",
        })),
    )];

    enrich_current_document_related_information(&document, &mut diagnostics);

    let related = diagnostics[0]
        .related_information
        .as_ref()
        .expect("structured diagnostic should be enriched");
    for expected in [
        "`init` constraint",
        "`state` account field",
        "`Create` accounts context",
    ] {
        assert!(
            related.iter().any(|info| info.message.contains(expected)),
            "missing related information containing {expected:?}; got {related:?}"
        );
    }
}

#[test]
fn every_diagnostic_kind_carries_metadata_axes() {
    let all_kinds = [
        AnchorDiagnosticKind::AnchorSyn,
        AnchorDiagnosticKind::AnchorInitConstraints,
        AnchorDiagnosticKind::AnchorMissingInitConstraint,
        AnchorDiagnosticKind::AnchorContextAccounts,
        AnchorDiagnosticKind::AnchorMissingAccountReference,
        AnchorDiagnosticKind::AnchorMissingInstructionArgument,
        AnchorDiagnosticKind::AnchorConstraintShape,
        AnchorDiagnosticKind::AnchorAccountUsage,
        AnchorDiagnosticKind::AnchorCheckCfg,
        AnchorDiagnosticKind::AnchorProjectId,
        AnchorDiagnosticKind::AnchorSbfArtifact,
        AnchorDiagnosticKind::AnchorProgramKeypair,
        AnchorDiagnosticKind::AnchorIdlArtifact,
        AnchorDiagnosticKind::AnchorTypesArtifact,
        AnchorDiagnosticKind::SolanaIdlArtifact,
        AnchorDiagnosticKind::SolanaProgramMetadata,
        AnchorDiagnosticKind::SolanaTestHarness,
        AnchorDiagnosticKind::SolanaSurfpoolWorkspace,
        AnchorDiagnosticKind::AnchorSplTokenInterface,
        AnchorDiagnosticKind::SecuritySigner,
        AnchorDiagnosticKind::SecurityTokenAccount,
        AnchorDiagnosticKind::SecurityCpiProgram,
        AnchorDiagnosticKind::SecuritySysvar,
        AnchorDiagnosticKind::SecurityDuplicateAccount,
        AnchorDiagnosticKind::SecurityUncheckedAccount,
        AnchorDiagnosticKind::SecurityStaticPda,
        AnchorDiagnosticKind::SecurityOwnerCheck,
        AnchorDiagnosticKind::SecurityTypeCosplay,
        AnchorDiagnosticKind::SolanaCodeQuality,
        AnchorDiagnosticKind::PdaSeedResolution,
    ];

    for kind in all_kinds {
        let diagnostic = diagnostic_from_range(
            Range::default(),
            kind,
            "synthetic diagnostic".to_string(),
            None,
        );
        let data = diagnostic.data.as_ref().expect("metadata should exist");
        for key in ["topic", "confidence", "applicability"] {
            assert!(
                data.get(key).and_then(|value| value.as_str()).is_some(),
                "{kind:?} missing {key}: {data:?}"
            );
        }
        assert!(
            data.get("confidence")
                .and_then(|value| value.as_str())
                .is_some_and(|confidence| matches!(
                    confidence,
                    "heuristic" | "derived" | "authoritative"
                )),
            "{kind:?} confidence must use the Seagrass taxonomy: {data:?}"
        );
        assert!(
            data.get("topic")
                .and_then(|value| value.as_str())
                .is_some_and(|topic| topic.starts_with("seagrass/")),
            "{kind:?} topic must be namespaced: {data:?}"
        );
    }
}

#[test]
fn suppression_comment_filters_next_line_diagnostic_by_topic_suffix() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee) // seagrass-allow: unchecked-arithmetic
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unchecked_arithmetic(diagnostic)),
        "line suppression should remove unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn file_suppression_filters_diagnostic_by_code() {
    let source = r#"
// seagrass-allow-file: solana-code-quality
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unchecked_arithmetic(diagnostic)),
        "file suppression should remove unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn seagrass_allow_attribute_filters_item_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

#[seagrass(allow("seagrass/solana.code-quality.unchecked-arithmetic"))]
fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unchecked_arithmetic(diagnostic)),
        "attribute suppression should remove unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn seagrass_allow_attribute_filters_nested_item_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

mod handlers {
    use super::*;

    #[seagrass(allow("seagrass/solana.code-quality.unchecked-arithmetic"))]
    fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
        Ok(amount - fee)
    }
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unchecked_arithmetic(diagnostic)),
        "nested item suppression should remove unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn seagrass_allow_attribute_filters_block_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    #[seagrass(allow("seagrass/solana.code-quality.unchecked-arithmetic"))]
    {
        Ok(amount - fee)
    }
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unchecked_arithmetic(diagnostic)),
        "block suppression should remove unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn workspace_config_filters_diagnostic_by_lints_allow() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let seagrass_toml_uri = Url::parse("file:///workspace/Seagrass.toml").unwrap();
    let seagrass_toml = r#"
[lints]
allow = ["solana-code-quality.unchecked-arithmetic"]
"#;

    let diagnostics = collect_with_input(DiagnosticInput {
        document: &document,
        uri: Some(&uri),
        workspace_index: None,
        manifest: None,
        anchor_toml: None,
        seagrass_toml: Some((&seagrass_toml_uri, seagrass_toml)),
        solana_program: None,
        settings: DiagnosticSettings::default(),
    });

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unchecked_arithmetic(diagnostic)),
        "workspace lint config should remove unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn parser_init_constraint_diagnostics_are_actionable() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;

    let syntax = syn::parse_file(source).unwrap();
    let item_struct = syntax
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Struct(item_struct) => Some(item_struct),
            _ => None,
        })
        .unwrap();
    let err = ::anchor_syn::parser::accounts::parse(item_struct).unwrap_err();
    let diagnostic = diagnostic_from_syn_error(err);

    assert!(
        diagnostic.message.contains("missing `payer = ...`")
            || diagnostic.message.contains("missing `space = ...`")
    );
    assert!(diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("parserMessage"))
        .and_then(|value| value.as_str())
        .is_some_and(|message| {
            message.contains("payer must be provided") || message.contains("space must be provided")
        }));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("topic"))
            .and_then(|value| value.as_str()),
        if diagnostic.message.contains("payer") {
            Some("seagrass/anchor.init.missing-payer")
        } else {
            Some("seagrass/anchor.init.missing-space")
        }
    );
    assert!(!matches!(
        diagnostic.message.as_str(),
        "payer must be provided" | "space must be provided"
    ));
}

#[test]
fn parser_ordering_and_conflict_diagnostics_are_actionable() {
    let ordering_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(payer = payer, init, space = 8)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;
    let conflict_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8, seeds = [b"state"], bump, seeds::program = other_program.key())]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;

    let ordering = collect(&ParsedDocument::parse(ordering_source).unwrap());
    assert!(ordering
        .iter()
        .any(|diagnostic| diagnostic.message.contains("place `init` before `payer`")));
    assert!(!ordering
        .iter()
        .any(|diagnostic| diagnostic.message == "init must be provided before payer"));

    let conflict = collect(&ParsedDocument::parse(conflict_source).unwrap());
    assert!(conflict.iter().any(|diagnostic| diagnostic
        .message
        .contains("remove `seeds::program` or `init`")));
    assert!(!conflict
        .iter()
        .any(|diagnostic| diagnostic.message == "seeds::program cannot be used with init"));
}

#[test]
fn reports_context_type_missing_accounts_derive() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

pub struct Initialize {}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("missing `#[derive(Accounts)]`")));
}

#[test]
fn accepts_local_binding_account_data_field_mutation() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
mod basic_2 {
    use super::*;

    pub fn create(ctx: Context<Create>, authority: Pubkey) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.authority = authority;
        counter.count = 0;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + 40)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Counter {
    pub authority: Pubkey,
    pub count: u64,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().all(|diagnostic| {
        !matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE
        ) || !diagnostic.message.contains("`count`")
    }));
}

#[test]
fn exposes_init_diagnostic_for_quickfix_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let diagnostic = collect(&document)
        .into_iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_INIT_CONSTRAINTS_CODE
            )
        })
        .unwrap();

    let actions = crate::actions::code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        diagnostic.range,
        std::slice::from_ref(&diagnostic),
    );

    assert_eq!(actions.len(), 1);
    assert!(actions[0]
        .diagnostics
        .as_ref()
        .is_some_and(|diagnostics| diagnostics == &[diagnostic]));
}

#[test]
fn ignores_non_init_diagnostics_for_quickfix_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let unrelated = Diagnostic {
        range: Range {
            start: Position {
                line: 3,
                character: 14,
            },
            end: Position {
                line: 3,
                character: 18,
            },
        },
        severity: None,
        code: Some(NumberOrString::String(
            registry::ANCHOR_SYN_CODE.to_string(),
        )),
        code_description: None,
        source: Some(SOURCE.to_string()),
        message: "unrelated".to_string(),
        related_information: None,
        tags: None,
        data: None,
    };

    let actions = crate::actions::code_actions(
        &document,
        Url::parse("file:///tmp/create.rs").unwrap(),
        unrelated.range,
        &[unrelated],
    );

    assert!(actions.is_empty());
}

#[test]
fn real_tutorial_programs_do_not_emit_high_signal_false_positives() {
    for path in [
        "../examples/tutorial/basic-1/programs/basic-1/src/lib.rs",
        "../examples/tutorial/basic-5/programs/basic-5/src/lib.rs",
    ] {
        let diagnostics = collect_real_program_diagnostics(path);
        let false_positives = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .code
                    .as_ref()
                    .and_then(code_text)
                    .is_some_and(|code| {
                        matches!(
                            code,
                            "anchor-context-accounts"
                                | "anchor-missing-account-reference"
                                | "anchor-missing-init-constraint"
                                | "anchor-account-usage"
                                | "anchor-constraint-shape"
                        )
                    })
            })
            .collect::<Vec<_>>();

        assert!(
            false_positives.is_empty(),
            "{path} produced high-signal false positives: {false_positives:#?}"
        );
    }
}

#[test]
fn real_puppet_master_cpi_usage_does_not_emit_cpi_program_false_positive() {
    let diagnostics = collect_real_program_diagnostics(
        "../examples/tutorial/basic-3/programs/puppet-master/src/lib.rs",
    );

    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic
                .code
                .as_ref()
                .and_then(code_text)
                .is_none_or(|code| code != "anchor-security-cpi-program")
        }),
        "puppet-master CPI usage produced a CPI program false positive: {diagnostics:#?}"
    );
}

fn collect_real_program_diagnostics(relative_path: &str) -> Vec<Diagnostic> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let document = ParsedDocument::parse_or_empty(source);
    collect(&document)
}

fn code_text(code: &NumberOrString) -> Option<&str> {
    match code {
        NumberOrString::String(code) => Some(code.as_str()),
        NumberOrString::Number(_) => None,
    }
}

fn is_unchecked_arithmetic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("topic"))
        .and_then(|value| value.as_str())
        == Some("seagrass/solana.code-quality.unchecked-arithmetic")
}
