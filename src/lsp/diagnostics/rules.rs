use {
    super::{
        account_references, account_usage, anchor_syn, artifacts, check_cfg, code_quality,
        constraint_expressions, constraint_shape, context_accounts, ecosystem, handler_members,
        handler_scope, initialization, instruction_attributes, pda, project_identity, security,
        spl_semantics,
    },
    crate::{diagnostics::engine::DiagnosticInput, solana::frameworks::FrameworkSet},
    tower_lsp::lsp_types::Diagnostic,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticPhase {
    Syntax,
    AnchorStructure,
    AnchorUsage,
    Security,
    Project,
}

pub struct DiagnosticRule {
    pub id: &'static str,
    pub phase: DiagnosticPhase,
    pub frameworks: FrameworkSet,
    pub collector: fn(&DiagnosticInput<'_>) -> Vec<Diagnostic>,
}

pub fn registry() -> &'static [DiagnosticRule] {
    &RULES
}

static RULES: [DiagnosticRule; 18] = [
    DiagnosticRule {
        id: "anchor-syn",
        phase: DiagnosticPhase::Syntax,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_anchor_syn,
    },
    DiagnosticRule {
        id: "context-accounts",
        phase: DiagnosticPhase::AnchorStructure,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_context_accounts,
    },
    DiagnosticRule {
        id: "instruction-attributes",
        phase: DiagnosticPhase::AnchorStructure,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_instruction_attributes,
    },
    DiagnosticRule {
        id: "initialization",
        phase: DiagnosticPhase::AnchorStructure,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_initialization,
    },
    DiagnosticRule {
        id: "handler-scope",
        phase: DiagnosticPhase::AnchorStructure,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_handler_scope,
    },
    DiagnosticRule {
        id: "handler-members",
        phase: DiagnosticPhase::AnchorStructure,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_handler_members,
    },
    DiagnosticRule {
        id: "account-references",
        phase: DiagnosticPhase::AnchorUsage,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_account_references,
    },
    DiagnosticRule {
        id: "constraint-expressions",
        phase: DiagnosticPhase::AnchorUsage,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_constraint_expressions,
    },
    DiagnosticRule {
        id: "constraint-shape",
        phase: DiagnosticPhase::AnchorUsage,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_constraint_shape,
    },
    DiagnosticRule {
        id: "spl-semantics",
        phase: DiagnosticPhase::AnchorUsage,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_spl_semantics,
    },
    DiagnosticRule {
        id: "account-usage",
        phase: DiagnosticPhase::AnchorUsage,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_account_usage,
    },
    DiagnosticRule {
        id: "security",
        phase: DiagnosticPhase::Security,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_security,
    },
    DiagnosticRule {
        id: "pda",
        phase: DiagnosticPhase::Security,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_pda,
    },
    DiagnosticRule {
        id: "code-quality",
        phase: DiagnosticPhase::Security,
        frameworks: FrameworkSet::SOLANA_PROGRAMS,
        collector: collect_code_quality,
    },
    DiagnosticRule {
        id: "check-cfg",
        phase: DiagnosticPhase::Project,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_check_cfg,
    },
    DiagnosticRule {
        id: "project-identity",
        phase: DiagnosticPhase::Project,
        frameworks: FrameworkSet::ANCHOR,
        collector: collect_project_identity,
    },
    DiagnosticRule {
        id: "artifacts",
        phase: DiagnosticPhase::Project,
        frameworks: FrameworkSet::SOLANA_PROGRAMS,
        collector: collect_artifacts,
    },
    DiagnosticRule {
        id: "ecosystem",
        phase: DiagnosticPhase::Project,
        frameworks: FrameworkSet::SOLANA_PROGRAMS,
        collector: collect_ecosystem,
    },
];

fn collect_anchor_syn(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    anchor_syn::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_context_accounts(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    context_accounts::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_instruction_attributes(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    instruction_attributes::collect(input.document)
}

fn collect_initialization(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    initialization::collect(input.document)
}

fn collect_account_references(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    account_references::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_constraint_expressions(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    constraint_expressions::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_constraint_shape(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    constraint_shape::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_spl_semantics(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    spl_semantics::collect(input.document)
}

fn collect_account_usage(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    account_usage::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_handler_scope(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    handler_scope::collect(input.document)
}

fn collect_handler_members(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    handler_members::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_security(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    security::collect_with_workspace(input.document, input.workspace_index)
}

fn collect_pda(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    pda::collect(input.document)
}

fn collect_code_quality(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    code_quality::collect_with_framework(input.document, input.framework)
}

fn collect_check_cfg(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    let Some((manifest_uri, manifest_text)) = input.manifest else {
        return Vec::new();
    };
    check_cfg::collect(input.document, manifest_uri, manifest_text)
}

fn collect_project_identity(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    let (Some(uri), Some((anchor_toml_uri, anchor_toml_text))) = (input.uri, input.anchor_toml)
    else {
        return Vec::new();
    };
    project_identity::collect(input.document, uri, anchor_toml_uri, anchor_toml_text)
}

fn collect_artifacts(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    let Some(uri) = input.uri else {
        return Vec::new();
    };
    artifacts::collect(input.document, uri, input.solana_program)
}

fn collect_ecosystem(input: &DiagnosticInput<'_>) -> Vec<Diagnostic> {
    let Some(uri) = input.uri else {
        return Vec::new();
    };
    ecosystem::collect(input.document, uri, input.solana_program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_phase_ordered() {
        let mut previous = DiagnosticPhase::Syntax;
        for rule in registry() {
            assert!(
                rule.phase >= previous,
                "rule `{}` is out of phase order",
                rule.id
            );
            previous = rule.phase;
        }
    }

    #[test]
    fn registry_contains_uri_aware_project_rules() {
        let ids = registry().iter().map(|rule| rule.id).collect::<Vec<_>>();

        assert!(ids.contains(&"check-cfg"));
        assert!(ids.contains(&"project-identity"));
        assert!(ids.contains(&"artifacts"));
        assert!(ids.contains(&"ecosystem"));
    }

    #[test]
    fn anchor_rules_declare_anchor_framework_scope() {
        let anchor_rule = registry()
            .iter()
            .find(|rule| rule.id == "constraint-shape")
            .expect("constraint-shape rule");

        assert!(anchor_rule
            .frameworks
            .contains(crate::solana::frameworks::FrameworkId::AnchorV1));
        assert!(!anchor_rule
            .frameworks
            .contains(crate::solana::frameworks::FrameworkId::Pinocchio));
    }
}
