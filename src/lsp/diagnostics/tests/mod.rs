use {
    super::*,
    crate::{
        diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    std::{collections::BTreeMap, fs, path::PathBuf},
    tower_lsp::lsp_types::{NumberOrString, Position, Range, Url},
};

mod corpus;
mod parse_errors;
mod related_information;

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
fn fixture_anchor_workspace_reports_static_anchor_error_coverage() {
    let fixture_source = anchor_error_fixture_source();
    let source = fs::read_to_string(&fixture_source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_root = fixture_source.parent().unwrap();
    let workspace_url = Url::from_directory_path(workspace_root).unwrap();
    let workspace_index =
        WorkspaceIndex::build(&[workspace_url], std::iter::empty::<(Url, String)>());

    let diagnostics = collect_with_workspace(&document, Some(&workspace_index));
    let anchor_errors = anchor_errors_by_name(&diagnostics);

    for name in [
        "ConstraintSpace",
        "ConstraintMut",
        "AccountNotMutable",
        "AccountNotSigner",
    ] {
        let coverage = anchor_errors.get(name).unwrap_or_else(|| {
            panic!("fixture missing Anchor error metadata for {name}: {diagnostics:#?}")
        });
        assert_eq!(
            coverage, "static-covered",
            "{name} must be covered by pre-build static analysis"
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
        AnchorDiagnosticKind::AnchorConstraintExpression,
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
        AnchorDiagnosticKind::SecurityCpiProgram,
        AnchorDiagnosticKind::SecuritySysvar,
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
        assert!(
            diagnostic.code_description.is_some(),
            "{kind:?} must include a codeDescription docs link"
        );
    }
}

#[test]
fn suppression_comment_filters_next_line_diagnostic_by_topic_suffix() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Err(ProgramError::InvalidArgument).expect("invalid argument") // seagrass-allow: unsafe-unwrap
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unsafe_unwrap(diagnostic)),
        "line suppression should remove unsafe unwrap diagnostic: {diagnostics:#?}"
    );
}

fn anchor_error_fixture_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/anchor-error-coverage-workspace/programs/error-coverage-fixture/src/lib.rs")
}

fn anchor_errors_by_name(diagnostics: &[Diagnostic]) -> BTreeMap<String, String> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.data.as_ref())
        .filter_map(|data| data.get("anchorErrors"))
        .filter_map(|errors| errors.as_array())
        .flat_map(|errors| errors.iter())
        .filter_map(|error| {
            let name = error.get("name")?.as_str()?;
            let coverage = error.get("coverage")?.as_str()?;
            Some((name.to_string(), coverage.to_string()))
        })
        .collect()
}

#[test]
fn suppression_marker_inside_string_does_not_filter_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    let _marker = "// seagrass-ignore";
    Err(ProgramError::InvalidArgument).expect("invalid argument")
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics.iter().any(is_unsafe_unwrap),
        "string contents must not suppress unsafe unwrap diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn file_suppression_filters_diagnostic_by_code() {
    let source = r#"
// seagrass-allow-file: solana-code-quality
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Err(ProgramError::InvalidArgument).expect("invalid argument")
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unsafe_unwrap(diagnostic)),
        "file suppression should remove unsafe unwrap diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn seagrass_allow_attribute_filters_item_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

#[seagrass(allow("seagrass/solana.code-quality.unsafe-unwrap"))]
fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Err(ProgramError::InvalidArgument).expect("invalid argument")
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unsafe_unwrap(diagnostic)),
        "attribute suppression should remove unsafe unwrap diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn seagrass_allow_attribute_filters_nested_item_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

mod handlers {
    use super::*;

    #[seagrass(allow("seagrass/solana.code-quality.unsafe-unwrap"))]
    fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
        Err(ProgramError::InvalidArgument).expect("invalid argument")
    }
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unsafe_unwrap(diagnostic)),
        "nested item suppression should remove unsafe unwrap diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn seagrass_allow_attribute_filters_block_diagnostic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    #[seagrass(allow("seagrass/solana.code-quality.unsafe-unwrap"))]
    {
        Err(ProgramError::InvalidArgument).expect("invalid argument")
    }
}
"#;

    let diagnostics = collect(&ParsedDocument::parse(source).unwrap());

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unsafe_unwrap(diagnostic)),
        "block suppression should remove unsafe unwrap diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn workspace_config_filters_diagnostic_by_lints_allow() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Err(ProgramError::InvalidArgument).expect("invalid argument")
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
    let seagrass_toml_uri = Url::parse("file:///workspace/Seagrass.toml").unwrap();
    let seagrass_toml = r#"
[lints]
allow = ["solana-code-quality.unsafe-unwrap"]
"#;

    let diagnostics = collect_with_input(DiagnosticInput {
        document: &document,
        uri: Some(&uri),
        workspace_index: None,
        framework: crate::solana::frameworks::FrameworkContext::from_document(&document),
        manifest: None,
        anchor_toml: None,
        seagrass_toml: Some((&seagrass_toml_uri, seagrass_toml)),
        solana_program: None,
        settings: DiagnosticSettings::default(),
    });

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !is_unsafe_unwrap(diagnostic)),
        "workspace lint config should remove unsafe unwrap diagnostic: {diagnostics:#?}"
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
    for (name, source) in [
        ("basic-1", BASIC_TUTORIAL_SOURCE),
        ("basic-5", BASIC_STATE_TUTORIAL_SOURCE),
    ] {
        let diagnostics = collect_source_diagnostics(source);
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
            "{name} produced high-signal false positives: {false_positives:#?}"
        );
    }
}

#[test]
fn real_puppet_master_cpi_usage_does_not_emit_cpi_program_false_positive() {
    let diagnostics = collect_source_diagnostics(PUPPET_MASTER_CPI_SOURCE);

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

fn collect_source_diagnostics(source: &str) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse_or_empty(source);
    collect(&document)
}

const BASIC_TUTORIAL_SOURCE: &str = r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod basic_1 {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
"#;

const BASIC_STATE_TUTORIAL_SOURCE: &str = r#"
use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod basic_5 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: u64) -> Result<()> {
        ctx.accounts.data.data = data;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + 8)]
    pub data: Account<'info, Data>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Data {
    pub data: u64,
}
"#;

const PUPPET_MASTER_CPI_SOURCE: &str = r#"
use anchor_lang::prelude::*;
use puppet::cpi::accounts::SetData;
use puppet::program::Puppet;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod puppet_master {
    use super::*;

    pub fn pull_strings(ctx: Context<PullStrings>, data: u64) -> Result<()> {
        let cpi_program = ctx.accounts.puppet_program.to_account_info();
        let cpi_accounts = SetData {
            puppet: ctx.accounts.puppet.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        puppet::cpi::set_data(cpi_ctx, data)
    }
}

#[derive(Accounts)]
pub struct PullStrings<'info> {
    #[account(mut)]
    pub puppet: AccountInfo<'info>,
    pub puppet_program: Program<'info, Puppet>,
}
"#;

fn code_text(code: &NumberOrString) -> Option<&str> {
    match code {
        NumberOrString::String(code) => Some(code.as_str()),
        NumberOrString::Number(_) => None,
    }
}

fn is_unsafe_unwrap(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("topic"))
        .and_then(|value| value.as_str())
        == Some("seagrass/solana.code-quality.unsafe-unwrap")
}
