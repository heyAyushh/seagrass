use crate::{anchor_errors, constraint_catalog};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorSupportLevel {
    AnchorV1,
    AnchorV2Preview,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorSupportSource {
    pub kind: &'static str,
    pub path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorSupportManifest {
    pub anchor_version: &'static str,
    pub support_level: AnchorSupportLevel,
    pub generator_version: u32,
    pub profile: AnchorSupportProfile,
    pub generated_constraint_count: usize,
    pub generated_field_completion_count: usize,
    pub generated_error_count: usize,
    pub sources: &'static [AnchorSupportSource],
    pub notes: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorSupportProfile {
    pub version_major: u32,
    pub version_minor: u32,
    pub version_patch: u32,
    pub version_family: &'static str,
    pub source_fingerprint: &'static str,
    pub parser_fingerprint: &'static str,
    pub error_fingerprint: &'static str,
    pub field_completion_fingerprint: &'static str,
    pub corpus_fingerprint: &'static str,
    pub parser_constraint_count: usize,
    pub parser_rule_count: usize,
    pub program_corpus_file_count: usize,
    pub program_corpus_constraint_count: usize,
    pub generation_mode: &'static str,
    pub regeneration_strategy: &'static str,
    pub anchor_v1_policy: &'static str,
    pub anchor_v2_policy: &'static str,
}

pub const MANIFEST: AnchorSupportManifest = include!("../generated/anchor_support_generated.rs");

pub fn summary() -> serde_json::Value {
    serde_json::json!({
        "anchorVersion": MANIFEST.anchor_version,
        "supportLevel": support_level_name(MANIFEST.support_level),
        "generatorVersion": MANIFEST.generator_version,
        "generatorProfile": generator_profile(),
        "generated": {
            "constraints": MANIFEST.generated_constraint_count,
            "fieldCompletions": MANIFEST.generated_field_completion_count,
            "errors": MANIFEST.generated_error_count,
        },
        "runtimeLoaded": {
            "constraints": constraint_catalog::CONSTRAINTS.len(),
            "errors": anchor_errors::ERRORS.len(),
        },
        "sources": MANIFEST.sources.iter().map(|source| {
            serde_json::json!({
                "kind": source.kind,
                "path": source.path,
            })
        }).collect::<Vec<_>>(),
        "notes": MANIFEST.notes,
        "families": constraint_family_summary(),
        "constraintSupport": constraint_support_summary(),
        "errorCoverage": anchor_errors::coverage_summary()["summary"].clone(),
        "capabilityCoverage": capability_support_summary(),
    })
}

pub fn generator_profile() -> serde_json::Value {
    serde_json::json!({
        "anchorVersion": MANIFEST.anchor_version,
        "version": {
            "major": MANIFEST.profile.version_major,
            "minor": MANIFEST.profile.version_minor,
            "patch": MANIFEST.profile.version_patch,
            "family": MANIFEST.profile.version_family,
        },
        "generationMode": MANIFEST.profile.generation_mode,
        "fingerprints": {
            "source": MANIFEST.profile.source_fingerprint,
            "parser": MANIFEST.profile.parser_fingerprint,
            "errors": MANIFEST.profile.error_fingerprint,
            "fieldCompletions": MANIFEST.profile.field_completion_fingerprint,
            "corpus": MANIFEST.profile.corpus_fingerprint,
        },
        "coverageInputs": {
            "parserConstraints": MANIFEST.profile.parser_constraint_count,
            "parserRules": MANIFEST.profile.parser_rule_count,
            "programCorpusFiles": MANIFEST.profile.program_corpus_file_count,
            "programCorpusConstraints": MANIFEST.profile.program_corpus_constraint_count,
            "generatedConstraints": MANIFEST.generated_constraint_count,
            "generatedFieldCompletions": MANIFEST.generated_field_completion_count,
            "generatedErrors": MANIFEST.generated_error_count,
        },
        "regenerationStrategy": MANIFEST.profile.regeneration_strategy,
        "policies": {
            "anchorV1": MANIFEST.profile.anchor_v1_policy,
            "anchorV2": MANIFEST.profile.anchor_v2_policy,
        },
    })
}

pub fn matrix() -> serde_json::Value {
    let summary = summary();
    serde_json::json!({
        "summary": summary,
        "generator": generator_profile(),
        "constraints": constraint_catalog::support_matrix(),
        "capabilities": capability_matrix(),
        "securityPatternCoverage": security_pattern_coverage(),
        "securityRegressionCorpus": security_regression_corpus(),
        "errors": anchor_errors::coverage_summary()["errors"].clone(),
        "gaps": support_gaps(),
        "capabilityGaps": capability_gaps(),
        "upgradePolicy": {
            "anchorV1": "generated from the active checkout and expected to track newer Anchor v1 parser/error changes",
            "anchorV2": "preview until the generated parser/error/source coverage proves stable for Anchor v2",
        },
    })
}

fn support_level_name(level: AnchorSupportLevel) -> &'static str {
    match level {
        AnchorSupportLevel::AnchorV1 => "anchor-v1",
        AnchorSupportLevel::AnchorV2Preview => "anchor-v2-preview",
        AnchorSupportLevel::Unknown => "unknown",
    }
}

fn constraint_family_summary() -> serde_json::Value {
    let mut core = 0usize;
    let mut pda = 0usize;
    let mut token_account = 0usize;
    let mut associated_token_account = 0usize;
    let mut mint = 0usize;
    let mut mint_extension = 0usize;
    let mut realloc = 0usize;

    for spec in constraint_catalog::CONSTRAINTS {
        match spec.family {
            constraint_catalog::ConstraintFamily::Core => core += 1,
            constraint_catalog::ConstraintFamily::Pda => pda += 1,
            constraint_catalog::ConstraintFamily::TokenAccount => token_account += 1,
            constraint_catalog::ConstraintFamily::AssociatedTokenAccount => {
                associated_token_account += 1;
            }
            constraint_catalog::ConstraintFamily::Mint => mint += 1,
            constraint_catalog::ConstraintFamily::MintExtension => mint_extension += 1,
            constraint_catalog::ConstraintFamily::Realloc => realloc += 1,
        }
    }

    serde_json::json!({
        "core": core,
        "pda": pda,
        "tokenAccount": token_account,
        "associatedTokenAccount": associated_token_account,
        "mint": mint,
        "mintExtension": mint_extension,
        "realloc": realloc,
    })
}

fn constraint_support_summary() -> serde_json::Value {
    let matrix = constraint_catalog::support_matrix();
    let mut rich = 0usize;
    let mut actionable = 0usize;
    let mut semantic = 0usize;
    let mut surface = 0usize;

    for entry in &matrix {
        match entry["supportDepth"].as_str() {
            Some("rich") => rich += 1,
            Some("actionable") => actionable += 1,
            Some("semantic") => semantic += 1,
            _ => surface += 1,
        }
    }

    serde_json::json!({
        "rich": rich,
        "actionable": actionable,
        "semantic": semantic,
        "surface": surface,
    })
}

pub fn capability_matrix() -> Vec<serde_json::Value> {
    CAPABILITIES
        .iter()
        .map(|capability| {
            serde_json::json!({
                "key": capability.key,
                "label": capability.label,
                "supportDepth": capability.support_depth,
                "editorFeatures": capability.editor_features,
                "semanticChecks": capability.semantic_checks,
                "codeActions": capability.code_actions,
                "evidence": capability.evidence,
                "gaps": capability.gaps,
            })
        })
        .collect()
}

fn capability_support_summary() -> serde_json::Value {
    let mut rich = 0usize;
    let mut actionable = 0usize;
    let mut semantic = 0usize;
    let mut surface = 0usize;

    for capability in CAPABILITIES {
        match capability.support_depth {
            "rich" => rich += 1,
            "actionable" => actionable += 1,
            "semantic" => semantic += 1,
            _ => surface += 1,
        }
    }

    serde_json::json!({
        "rich": rich,
        "actionable": actionable,
        "semantic": semantic,
        "surface": surface,
    })
}

fn capability_gaps() -> Vec<serde_json::Value> {
    CAPABILITIES
        .iter()
        .filter(|capability| !capability.gaps.is_empty())
        .map(|capability| {
            serde_json::json!({
                "key": capability.key,
                "supportDepth": capability.support_depth,
                "gaps": capability.gaps,
            })
        })
        .collect()
}

fn security_pattern_coverage() -> Vec<serde_json::Value> {
    [
        (
            "signer-authorization",
            "covered",
            "anchor-security-signer plus native code-quality invariant",
        ),
        (
            "account-data-matching",
            "covered",
            "anchor-security-token-account",
        ),
        (
            "owner-checks",
            "covered",
            "anchor-security-owner-check plus native raw account invariant",
        ),
        (
            "type-cosplay",
            "covered",
            "anchor-security-type-cosplay plus native raw account invariant",
        ),
        (
            "initialization",
            "covered",
            "Anchor init constraints plus unchecked init invariant",
        ),
        (
            "arbitrary-cpi",
            "covered",
            "anchor-security-cpi-program plus native program-id invariant",
        ),
        (
            "duplicate-mutable-accounts",
            "covered",
            "anchor-security-duplicate-account",
        ),
        (
            "bump-seed-canonicalization",
            "covered",
            "manual create_program_address invariant",
        ),
        (
            "pda-sharing",
            "covered",
            "static PDA and seed-domain diagnostics",
        ),
        (
            "account-closing",
            "covered",
            "Anchor close shape plus manual close invariant",
        ),
        (
            "sysvar-address-checking",
            "covered",
            "anchor-security-sysvar",
        ),
        (
            "stale-account-after-cpi",
            "covered",
            "post-CPI reload invariant",
        ),
        (
            "instruction-data-bounds",
            "covered",
            "native/Pinocchio data indexing invariant",
        ),
        (
            "pda-seed-collision",
            "covered",
            "dynamic seed domain-separation invariant",
        ),
    ]
    .into_iter()
    .map(|(key, status, evidence)| {
        serde_json::json!({
            "key": key,
            "status": status,
            "evidence": evidence,
        })
    })
    .collect()
}

fn security_regression_corpus() -> Vec<serde_json::Value> {
    [
        ("alias-raw-account-owner", "anchor-security-owner-check"),
        ("manual-close-reinit", "solana-code-quality/account-closing"),
        (
            "stale-cpi-reload",
            "solana-code-quality/stale-account-after-cpi",
        ),
        (
            "native-signer-check",
            "solana-code-quality/signer-authorization",
        ),
        (
            "native-program-id-check",
            "solana-code-quality/arbitrary-cpi",
        ),
        (
            "instruction-data-bounds",
            "solana-code-quality/instruction-data-bounds",
        ),
        (
            "pda-seed-collision",
            "solana-code-quality/pda-seed-collision",
        ),
    ]
    .into_iter()
    .map(|(name, expected)| {
        serde_json::json!({
            "name": name,
            "expectedDiagnostic": expected,
        })
    })
    .collect()
}

struct CapabilitySupport {
    key: &'static str,
    label: &'static str,
    support_depth: &'static str,
    editor_features: &'static [&'static str],
    semantic_checks: &'static [&'static str],
    code_actions: &'static [&'static str],
    evidence: &'static [&'static str],
    gaps: &'static [&'static str],
}

const CAPABILITIES: &[CapabilitySupport] = &[
    CapabilitySupport {
        key: "anchorSyntax",
        label: "Anchor syntax and account wrapper diagnostics",
        support_depth: "rich",
        editor_features: &["diagnostics", "quickFixes", "relatedInformation"],
        semantic_checks: &[
            "programHandlerShape",
            "accountWrapperShape",
            "sysvarTypeShape",
        ],
        code_actions: &["replaceAccountType", "replaceInvalidSysvar"],
        evidence: &["src/lsp/diagnostics/anchor_syn/mod.rs"],
        gaps: &[],
    },
    CapabilitySupport {
        key: "anchorConstraints",
        label: "Generated Anchor account constraints",
        support_depth: "rich",
        editor_features: &[
            "completion",
            "hover",
            "signatureHelp",
            "diagnostics",
            "quickFixes",
        ],
        semantic_checks: &["parserRules", "constraintShape", "referenceResolution"],
        code_actions: &[
            "addMissingConstraint",
            "replaceMissingReference",
            "removeInvalidConstraint",
        ],
        evidence: &["src/anchor/constraint_catalog/mod.rs", "build.rs"],
        gaps: &[],
    },
    CapabilitySupport {
        key: "cpiProgramSecurity",
        label: "CPI program account validation",
        support_depth: "actionable",
        editor_features: &["diagnostics", "quickFixes"],
        semantic_checks: &[
            "nativeInstructionProgramId",
            "cpiContextProgram",
            "reachableSplitHelperUsage",
        ],
        code_actions: &["addExecutableConstraint", "replaceAccountTypeWhenGenerated"],
        evidence: &[
            "src/core/document/mod.rs",
            "src/core/workspace/mod.rs",
            "src/lsp/diagnostics/security/mod.rs",
        ],
        gaps: &["structuredExternalInstructionAccountValidation"],
    },
    CapabilitySupport {
        key: "securityPatternCorpus",
        label: "Legacy Solana security pattern corpora",
        support_depth: "actionable",
        editor_features: &["diagnostics", "diagnosticData", "supportMatrix"],
        semantic_checks: &[
            "legacyInvariantMapping",
            "sealevelAttackTaxonomy",
            "programAutofixerTaxonomy",
            "anchorVersionMetadata",
            "ownerChecks",
            "typeCosplay",
            "nativeRawAccountData",
            "aliasRawAccountData",
            "manualCloseAndReinit",
            "staleCpiReload",
            "nativeSignerAndProgramChecks",
            "instructionDataBounds",
            "pdaSeedCollision",
            "securityPatternCoverage",
        ],
        code_actions: &[
            "addOwnerConstraint",
            "securityGuidance",
            "reloadGuidance",
            "checkedDataGuidance",
        ],
        evidence: &[
            "src/lsp/diagnostics/code_quality/mod.rs",
            "src/anchor/support/mod.rs",
        ],
        gaps: &["versionSpecificTemplateRewriteSuggestions"],
    },
    CapabilitySupport {
        key: "workspaceModules",
        label: "Workspace split modules and helper functions",
        support_depth: "actionable",
        editor_features: &[
            "diagnostics",
            "workspaceSymbols",
            "goToDefinition",
            "implementation",
        ],
        semantic_checks: &[
            "contextInstructionMapping",
            "reachableHelperFunctions",
            "nestedAccountFields",
        ],
        code_actions: &[
            "createAccountsStruct",
            "addMissingAccountField",
            "addMutConstraint",
        ],
        evidence: &["src/core/workspace/mod.rs", "src/lsp/navigation/mod.rs"],
        gaps: &["fullRustModuleResolver"],
    },
    CapabilitySupport {
        key: "dependencyBridge",
        label: "Solana dependency source bridge",
        support_depth: "semantic",
        editor_features: &["workspaceSymbols", "goToDefinition", "typeDefinition"],
        semantic_checks: &[
            "publicSymbolBridge",
            "pathDependencySourceScan",
            "cargoRegistrySourceScan",
        ],
        code_actions: &[],
        evidence: &["src/core/definition_bridge/mod.rs"],
        gaps: &[
            "arbitraryNonSolanaCrateSemantics",
            "macroExpandedDependencyBodies",
        ],
    },
    CapabilitySupport {
        key: "multiProgramArtifacts",
        label: "Anchor, Pinocchio, and native Solana program artifacts",
        support_depth: "rich",
        editor_features: &["diagnostics", "analysisReport", "artifactReport"],
        semantic_checks: &[
            "sbfElfHeader",
            "programId",
            "idl",
            "typescriptTypes",
            "staleness",
        ],
        code_actions: &[],
        evidence: &[
            "src/solana/project/mod.rs",
            "src/solana/program_artifacts/mod.rs",
        ],
        gaps: &[],
    },
    CapabilitySupport {
        key: "ecosystemTooling",
        label: "Codama, Shank, Program Metadata, LiteSVM, Mollusk, and Surfpool",
        support_depth: "actionable",
        editor_features: &["diagnostics", "analysisReport", "artifactReport"],
        semantic_checks: &[
            "idlSources",
            "programMetadataPayload",
            "testHarnesses",
            "surfpoolDeployArtifact",
        ],
        code_actions: &[],
        evidence: &[
            "src/solana/ecosystem/mod.rs",
            "src/lsp/diagnostics/ecosystem.rs",
        ],
        gaps: &["toolSpecificRemediationActions"],
    },
    CapabilitySupport {
        key: "editorProtocol",
        label: "Editor-facing LSP protocol behavior",
        support_depth: "rich",
        editor_features: &[
            "pushDiagnostics",
            "pullDiagnostics",
            "completionTriggers",
            "codeLens",
            "commands",
            "logs",
        ],
        semantic_checks: &[
            "hotColdDiagnosticPhases",
            "staleDiagnosticGuard",
            "protocolSmoke",
        ],
        code_actions: &[],
        evidence: &["src/server/mod.rs", "scripts/protocol-smoke.ts"],
        gaps: &[],
    },
];

fn support_gaps() -> Vec<serde_json::Value> {
    constraint_catalog::support_matrix()
        .into_iter()
        .filter_map(|entry| {
            let gaps = entry["gaps"].as_array()?;
            (!gaps.is_empty()).then(|| {
                serde_json::json!({
                    "key": entry["key"].clone(),
                    "family": entry["family"].clone(),
                    "supportDepth": entry["supportDepth"].clone(),
                    "gaps": gaps,
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_manifest_matches_generated_catalogs() {
        assert_eq!(
            MANIFEST.generated_constraint_count,
            constraint_catalog::CONSTRAINTS.len()
        );
        assert_eq!(MANIFEST.generated_error_count, anchor_errors::ERRORS.len());
        assert_eq!(MANIFEST.profile.version_family, "anchor-v1");
        assert!(!MANIFEST.profile.source_fingerprint.is_empty());
        let parser_rule_count = MANIFEST.profile.parser_rule_count;
        let field_completion_count = MANIFEST.generated_field_completion_count;
        let program_corpus_file_count = MANIFEST.profile.program_corpus_file_count;
        assert!(parser_rule_count > 0);
        assert!(field_completion_count > 0);
        assert!(program_corpus_file_count > 0);
    }

    #[test]
    fn support_matrix_exposes_capability_coverage() {
        let matrix = matrix();
        let capabilities = matrix["capabilities"]
            .as_array()
            .expect("capabilities array");

        let cpi = capabilities
            .iter()
            .find(|entry| entry["key"] == "cpiProgramSecurity")
            .expect("CPI capability coverage");
        assert!(cpi["semanticChecks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check == "reachableSplitHelperUsage"));
        assert!(cpi["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gap| gap == "structuredExternalInstructionAccountValidation"));

        let security_corpus = capabilities
            .iter()
            .find(|entry| entry["key"] == "securityPatternCorpus")
            .expect("legacy security corpus capability coverage");
        assert!(security_corpus["semanticChecks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check == "anchorVersionMetadata"));
        assert!(security_corpus["semanticChecks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check == "securityPatternCoverage"));
        assert!(
            matrix["securityPatternCoverage"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["key"] == "instruction-data-bounds"
                    && entry["status"] == "covered")
        );
        assert!(matrix["securityRegressionCorpus"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "alias-raw-account-owner"));

        let artifacts = capabilities
            .iter()
            .find(|entry| entry["key"] == "multiProgramArtifacts")
            .expect("multi-program artifact coverage");
        assert_eq!(artifacts["supportDepth"], "rich");
        assert!(matrix["capabilityGaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gap| gap["key"] == "workspaceModules"));
        assert_eq!(
            matrix["summary"]["capabilityCoverage"]["rich"].as_u64(),
            Some(4)
        );
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn manifest_is_correct() {
        assert_eq!(MANIFEST.anchor_version, "1.0.2");
        assert_eq!(
            MANIFEST.generated_constraint_count,
            constraint_catalog::CONSTRAINTS.len()
        );
        assert_eq!(MANIFEST.generated_error_count, anchor_errors::ERRORS.len());
        assert_eq!(MANIFEST.profile.version_family, "anchor-v1");
        assert!(!MANIFEST.profile.source_fingerprint.is_empty());
        assert!(MANIFEST.profile.parser_rule_count > 0);
        assert!(MANIFEST.generated_field_completion_count > 0);
        assert!(MANIFEST.profile.program_corpus_file_count > 0);
    }
}
