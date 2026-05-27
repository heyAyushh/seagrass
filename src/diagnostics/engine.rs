use {
    super::{
        arbitration::{self, DiagnosticSettings},
        rules::{self, DiagnosticPhase},
    },
    crate::{document::ParsedDocument, solana_project::SolanaProgram, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::{Diagnostic, Url},
};

pub struct DiagnosticInput<'a> {
    pub document: &'a ParsedDocument,
    pub uri: Option<&'a Url>,
    pub workspace_index: Option<&'a WorkspaceIndex>,
    pub manifest: Option<(&'a Url, &'a str)>,
    pub anchor_toml: Option<(&'a Url, &'a str)>,
    pub seagrass_toml: Option<(&'a Url, &'a str)>,
    pub solana_program: Option<&'a SolanaProgram>,
    pub settings: DiagnosticSettings,
}

impl<'a> DiagnosticInput<'a> {
    #[cfg(test)]
    pub fn document_only(document: &'a ParsedDocument) -> Self {
        Self {
            document,
            uri: None,
            workspace_index: None,
            manifest: None,
            anchor_toml: None,
            seagrass_toml: None,
            solana_program: None,
            settings: DiagnosticSettings::default(),
        }
    }
}

pub fn collect(input: DiagnosticInput<'_>) -> Vec<Diagnostic> {
    collect_through_phase(input, None)
}

pub fn collect_hot(input: DiagnosticInput<'_>) -> Vec<Diagnostic> {
    collect_through_phase(input, Some(DiagnosticPhase::AnchorStructure))
}

fn collect_through_phase(
    input: DiagnosticInput<'_>,
    max_phase: Option<DiagnosticPhase>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules::registry() {
        if max_phase.is_some_and(|max_phase| rule.phase > max_phase) {
            continue;
        }
        let _rule_metadata = rule.id;
        diagnostics.extend((rule.collector)(&input));
    }

    let mut diagnostics = arbitration::arbitrate(diagnostics, input.settings);
    diagnostics = super::suppression::filter(
        input.document,
        input.seagrass_toml.map(|(_, text)| text),
        diagnostics,
    );
    super::enrich_current_document_related_information(input.document, &mut diagnostics);
    if let Some(uri) = input.uri {
        super::bind_current_document_related_uri(&mut diagnostics, uri);
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::document::ParsedDocument,
        tower_lsp::lsp_types::{NumberOrString, Url},
    };

    #[test]
    fn document_only_engine_preserves_existing_anchor_diagnostics() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(DiagnosticInput::document_only(&document));

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("payer") || diagnostic.message.contains("space")
        }));
    }

    #[test]
    fn uri_rules_run_from_same_engine_path() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

declare_id!("Declared111111111111111111111111111111111");
"#,
        )
        .unwrap();
        let uri = Url::parse("file:///workspace/programs/my-program/src/lib.rs").unwrap();
        let anchor_toml_uri = Url::parse("file:///workspace/Anchor.toml").unwrap();
        let anchor_toml = r#"
[programs.localnet]
my_program = "Expected111111111111111111111111111111111"
"#;

        let diagnostics = collect(DiagnosticInput {
            document: &document,
            uri: Some(&uri),
            workspace_index: None,
            manifest: None,
            anchor_toml: Some((&anchor_toml_uri, anchor_toml)),
            seagrass_toml: None,
            solana_program: None,
            settings: DiagnosticSettings::default(),
        });

        assert!(diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "anchor-project-id"
            )
        }));
    }

    #[test]
    fn pda_related_information_binds_current_document_uri() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", max_key(&ctx.accounts.user)], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
        )
        .unwrap();
        let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();

        let diagnostics = collect(DiagnosticInput {
            document: &document,
            uri: Some(&uri),
            workspace_index: None,
            manifest: None,
            anchor_toml: None,
            seagrass_toml: None,
            solana_program: None,
            settings: DiagnosticSettings::default(),
        });

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.message.contains("dropped from the IDL")
                    && matches!(
                        diagnostic.code.as_ref(),
                        Some(NumberOrString::String(code)) if code == "anchor-pda-seed-resolution"
                    )
            })
            .expect("expected pda seed diagnostic");
        let related_information = diagnostic
            .related_information
            .as_ref()
            .expect("expected pda diagnostic related information");

        assert!(related_information
            .iter()
            .all(|info| info.location.uri == uri));
    }

    #[test]
    fn init_constraint_related_information_binds_current_document_uri() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#,
        )
        .unwrap();
        let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();

        let diagnostics = collect(DiagnosticInput {
            document: &document,
            uri: Some(&uri),
            workspace_index: None,
            manifest: None,
            anchor_toml: None,
            seagrass_toml: None,
            solana_program: None,
            settings: DiagnosticSettings::default(),
        });

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.message.contains("missing `payer = ...`")
                    && matches!(
                        diagnostic.code.as_ref(),
                        Some(NumberOrString::String(code)) if code == "anchor-init-constraints"
                    )
            })
            .expect("expected init companion diagnostic");
        let related_information = diagnostic
            .related_information
            .as_ref()
            .expect("expected init diagnostic related information");

        assert!(related_information
            .iter()
            .all(|info| info.location.uri == uri));
    }

    #[test]
    fn hot_engine_keeps_structure_and_defers_project_rules() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

declare_id!("Declared111111111111111111111111111111111");

#[program]
pub mod demo {
    use super::*;

    pub fn create(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();
        let uri = Url::parse("file:///workspace/programs/demo/src/lib.rs").unwrap();
        let anchor_toml_uri = Url::parse("file:///workspace/Anchor.toml").unwrap();
        let anchor_toml = r#"
[programs.localnet]
demo = "Expected111111111111111111111111111111111"
"#;

        let input = || DiagnosticInput {
            document: &document,
            uri: Some(&uri),
            workspace_index: None,
            manifest: None,
            anchor_toml: Some((&anchor_toml_uri, anchor_toml)),
            seagrass_toml: None,
            solana_program: None,
            settings: DiagnosticSettings::default(),
        };

        let hot = collect_hot(input());
        assert!(hot.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "anchor-context-accounts"
            )
        }));
        assert!(hot.iter().all(|diagnostic| {
            !matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "anchor-project-id"
            )
        }));

        let full = collect(input());
        assert!(full.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "anchor-project-id"
            )
        }));
    }
}
