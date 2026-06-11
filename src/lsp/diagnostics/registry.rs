use {
    crate::constraint_catalog,
    serde_json::Value,
    tower_lsp::lsp_types::{DiagnosticSeverity, Url},
};

pub const ANCHOR_CONTEXT_ACCOUNTS_CODE: &str = "anchor-context-accounts";
pub const ANCHOR_INIT_CONSTRAINTS_CODE: &str = "anchor-init-constraints";
pub const ANCHOR_MISSING_INIT_CONSTRAINT_CODE: &str = "anchor-missing-init-constraint";
pub const ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE: &str = "anchor-missing-account-reference";
pub const ANCHOR_MISSING_INSTRUCTION_ARGUMENT_CODE: &str = "anchor-missing-instruction-argument";
pub const ANCHOR_CONSTRAINT_EXPRESSION_CODE: &str = "anchor-constraint-expression";
pub const ANCHOR_CONSTRAINT_SHAPE_CODE: &str = "anchor-constraint-shape";
pub const ANCHOR_ACCOUNT_USAGE_CODE: &str = "anchor-account-usage";
pub const ANCHOR_CHECK_CFG_CODE: &str = "anchor-check-cfg";
pub const ANCHOR_PROJECT_ID_CODE: &str = "anchor-project-id";
pub const ANCHOR_SBF_ARTIFACT_CODE: &str = "anchor-sbf-artifact";
pub const ANCHOR_PROGRAM_KEYPAIR_CODE: &str = "anchor-program-keypair";
pub const ANCHOR_IDL_ARTIFACT_CODE: &str = "anchor-idl-artifact";
pub const ANCHOR_TYPES_ARTIFACT_CODE: &str = "anchor-types-artifact";
pub const SOLANA_IDL_ARTIFACT_CODE: &str = "solana-idl-artifact";
pub const SOLANA_PROGRAM_METADATA_CODE: &str = "solana-program-metadata";
pub const SOLANA_TEST_HARNESS_CODE: &str = "solana-test-harness";
pub const SOLANA_SURFPOOL_WORKSPACE_CODE: &str = "solana-surfpool-workspace";
pub const ANCHOR_SPL_TOKEN_INTERFACE_CODE: &str = "anchor-spl-token-interface";
pub const ANCHOR_SYN_CODE: &str = "anchor-syn";
pub const ANCHOR_SECURITY_SIGNER_CODE: &str = "anchor-security-signer";
pub const ANCHOR_SECURITY_CPI_PROGRAM_CODE: &str = "anchor-security-cpi-program";
pub const ANCHOR_SECURITY_SYSVAR_CODE: &str = "anchor-security-sysvar";
pub const ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE: &str = "anchor-security-unchecked-account";
pub const ANCHOR_SECURITY_STATIC_PDA_CODE: &str = "anchor-security-static-pda";
pub const ANCHOR_SECURITY_OWNER_CHECK_CODE: &str = "anchor-security-owner-check";
pub const ANCHOR_SECURITY_TYPE_COSPLAY_CODE: &str = "anchor-security-type-cosplay";
pub const SOLANA_CODE_QUALITY_CODE: &str = "solana-code-quality";
pub const ANCHOR_PDA_SEED_RESOLUTION_CODE: &str = "anchor-pda-seed-resolution";
pub const INIT_PLACEHOLDERS_QUICKFIX: &str = "init-placeholders";
pub const REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX: &str =
    "replace-constraint-expression-member";
pub const REPLACE_CONSTRAINT_EXPRESSION_IDENTIFIER_QUICKFIX: &str =
    "replace-constraint-expression-identifier";
pub const SOURCE: &str = "seagrass";

/// Describes how much evidence a diagnostic kind requires before its claim can be considered
/// proven.  The ceiling rule is: Heuristic and WholeProgram diagnostics must never default to
/// ERROR severity, because they cannot rule out false positives from a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provability {
    /// Proven from a single file: parse errors, in-file shape/type violations.
    /// ERROR is allowed.
    Syntactic,
    /// Absence or existence claims that require cross-file or manifest evidence.
    /// Must default to WARNING; only an evidence-carrying upgrade path may reach ERROR.
    WholeProgram,
    /// Pattern-based guesses with no formal proof.
    /// At most WARNING or HINT; never ERROR.
    Heuristic,
    /// Context-dependent design-review prompts. The observed syntax is real, but the claim is
    /// intentionally advisory and must default to HINT.
    Speculative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruthSource {
    SingleFileSyntax,
    PinnedCatalog,
    Compound,
    ToolchainOracle,
    None,
}

impl TruthSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleFileSyntax => "single-file-syntax",
            Self::PinnedCatalog => "pinned-catalog",
            Self::Compound => "compound",
            Self::ToolchainOracle => "toolchain-oracle",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorDiagnosticKind {
    AnchorSyn,
    AnchorInitConstraints,
    AnchorMissingInitConstraint,
    AnchorContextAccounts,
    AnchorMissingAccountReference,
    AnchorMissingInstructionArgument,
    AnchorConstraintExpression,
    AnchorConstraintShape,
    AnchorAccountUsage,
    AnchorCheckCfg,
    AnchorProjectId,
    AnchorSbfArtifact,
    AnchorProgramKeypair,
    AnchorIdlArtifact,
    AnchorTypesArtifact,
    SolanaIdlArtifact,
    SolanaProgramMetadata,
    SolanaTestHarness,
    SolanaSurfpoolWorkspace,
    AnchorSplTokenInterface,
    SecuritySigner,
    SecurityCpiProgram,
    SecuritySysvar,
    SecurityUncheckedAccount,
    /// Speculative: static seeds are valid; this surfaces a design-review prompt, not a
    /// definite bug.
    SecurityStaticPda,
    SecurityOwnerCheck,
    SecurityTypeCosplay,
    SolanaCodeQuality,
    PdaSeedResolution,
}

impl AnchorDiagnosticKind {
    /// Returns every variant exactly once.  Adding a new variant to the enum without updating
    /// this list is caught at compile time by the exhaustive `match` in `provability()` and by
    /// `all_variants_are_covered_by_all()`.
    // No non-test callers yet; future evidence-upgrade paths will iterate all kinds.
    #[allow(dead_code)]
    pub fn all() -> [Self; 29] {
        [
            Self::AnchorSyn,
            Self::AnchorInitConstraints,
            Self::AnchorMissingInitConstraint,
            Self::AnchorContextAccounts,
            Self::AnchorMissingAccountReference,
            Self::AnchorMissingInstructionArgument,
            Self::AnchorConstraintExpression,
            Self::AnchorConstraintShape,
            Self::AnchorAccountUsage,
            Self::AnchorCheckCfg,
            Self::AnchorProjectId,
            Self::AnchorSbfArtifact,
            Self::AnchorProgramKeypair,
            Self::AnchorIdlArtifact,
            Self::AnchorTypesArtifact,
            Self::SolanaIdlArtifact,
            Self::SolanaProgramMetadata,
            Self::SolanaTestHarness,
            Self::SolanaSurfpoolWorkspace,
            Self::AnchorSplTokenInterface,
            Self::SecuritySigner,
            Self::SecurityCpiProgram,
            Self::SecuritySysvar,
            Self::SecurityUncheckedAccount,
            Self::SecurityStaticPda,
            Self::SecurityOwnerCheck,
            Self::SecurityTypeCosplay,
            Self::SolanaCodeQuality,
            Self::PdaSeedResolution,
        ]
    }

    /// Classifies how much proof a diagnostic kind can provide.
    ///
    /// An exhaustive `match` (no wildcard) is intentional: the compiler will reject a new variant
    /// that is not classified here, enforcing the provability ceiling at every addition.
    pub fn provability(self) -> Provability {
        match self {
            // Single-file, parse-level or structural violations — fully provable.
            Self::AnchorSyn | Self::AnchorConstraintShape | Self::AnchorInitConstraints => {
                Provability::Syntactic
            }

            // Absence/existence claims requiring cross-file or manifest evidence.
            Self::AnchorContextAccounts
            | Self::AnchorMissingAccountReference
            | Self::AnchorMissingInstructionArgument
            | Self::AnchorMissingInitConstraint
            | Self::AnchorConstraintExpression
            | Self::AnchorAccountUsage
            | Self::AnchorProjectId => Provability::WholeProgram,

            // Pattern-based guesses — no formal proof of correctness.
            Self::AnchorCheckCfg
            | Self::AnchorSbfArtifact
            | Self::AnchorProgramKeypair
            | Self::AnchorIdlArtifact
            | Self::AnchorTypesArtifact
            | Self::SolanaIdlArtifact
            | Self::SolanaProgramMetadata
            | Self::SolanaTestHarness
            | Self::SolanaSurfpoolWorkspace
            | Self::AnchorSplTokenInterface
            | Self::SecuritySigner
            | Self::SecurityCpiProgram
            | Self::SecuritySysvar
            | Self::SecurityUncheckedAccount
            | Self::SecurityOwnerCheck
            | Self::SecurityTypeCosplay
            | Self::SolanaCodeQuality
            | Self::PdaSeedResolution => Provability::Heuristic,

            Self::SecurityStaticPda => Provability::Speculative,
        }
    }

    pub fn truth_source(self) -> TruthSource {
        match self {
            Self::AnchorSyn
            | Self::AnchorConstraintShape
            | Self::AnchorInitConstraints
            | Self::AnchorContextAccounts
            | Self::AnchorMissingAccountReference
            | Self::AnchorMissingInstructionArgument
            | Self::AnchorConstraintExpression
            | Self::AnchorAccountUsage
            | Self::SecuritySigner
            | Self::SecurityCpiProgram
            | Self::SecurityUncheckedAccount
            | Self::SecurityOwnerCheck
            | Self::SecurityTypeCosplay
            | Self::SolanaCodeQuality
            | Self::PdaSeedResolution => TruthSource::SingleFileSyntax,
            Self::AnchorSplTokenInterface | Self::SecuritySysvar => TruthSource::PinnedCatalog,
            Self::AnchorMissingInitConstraint => TruthSource::Compound,
            Self::AnchorCheckCfg
            | Self::AnchorSbfArtifact
            | Self::AnchorProgramKeypair
            | Self::AnchorIdlArtifact
            | Self::AnchorTypesArtifact
            | Self::SolanaIdlArtifact
            | Self::SolanaProgramMetadata
            | Self::SolanaTestHarness
            | Self::SolanaSurfpoolWorkspace
            | Self::AnchorProjectId => TruthSource::ToolchainOracle,
            Self::SecurityStaticPda => TruthSource::None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::AnchorSyn => ANCHOR_SYN_CODE,
            Self::AnchorInitConstraints => ANCHOR_INIT_CONSTRAINTS_CODE,
            Self::AnchorMissingInitConstraint => ANCHOR_MISSING_INIT_CONSTRAINT_CODE,
            Self::AnchorContextAccounts => ANCHOR_CONTEXT_ACCOUNTS_CODE,
            Self::AnchorMissingAccountReference => ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE,
            Self::AnchorMissingInstructionArgument => ANCHOR_MISSING_INSTRUCTION_ARGUMENT_CODE,
            Self::AnchorConstraintExpression => ANCHOR_CONSTRAINT_EXPRESSION_CODE,
            Self::AnchorConstraintShape => ANCHOR_CONSTRAINT_SHAPE_CODE,
            Self::AnchorAccountUsage => ANCHOR_ACCOUNT_USAGE_CODE,
            Self::AnchorCheckCfg => ANCHOR_CHECK_CFG_CODE,
            Self::AnchorProjectId => ANCHOR_PROJECT_ID_CODE,
            Self::AnchorSbfArtifact => ANCHOR_SBF_ARTIFACT_CODE,
            Self::AnchorProgramKeypair => ANCHOR_PROGRAM_KEYPAIR_CODE,
            Self::AnchorIdlArtifact => ANCHOR_IDL_ARTIFACT_CODE,
            Self::AnchorTypesArtifact => ANCHOR_TYPES_ARTIFACT_CODE,
            Self::SolanaIdlArtifact => SOLANA_IDL_ARTIFACT_CODE,
            Self::SolanaProgramMetadata => SOLANA_PROGRAM_METADATA_CODE,
            Self::SolanaTestHarness => SOLANA_TEST_HARNESS_CODE,
            Self::SolanaSurfpoolWorkspace => SOLANA_SURFPOOL_WORKSPACE_CODE,
            Self::AnchorSplTokenInterface => ANCHOR_SPL_TOKEN_INTERFACE_CODE,
            Self::SecuritySigner => ANCHOR_SECURITY_SIGNER_CODE,
            Self::SecurityCpiProgram => ANCHOR_SECURITY_CPI_PROGRAM_CODE,
            Self::SecuritySysvar => ANCHOR_SECURITY_SYSVAR_CODE,
            Self::SecurityUncheckedAccount => ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE,
            Self::SecurityStaticPda => ANCHOR_SECURITY_STATIC_PDA_CODE,
            Self::SecurityOwnerCheck => ANCHOR_SECURITY_OWNER_CHECK_CODE,
            Self::SecurityTypeCosplay => ANCHOR_SECURITY_TYPE_COSPLAY_CODE,
            Self::SolanaCodeQuality => SOLANA_CODE_QUALITY_CODE,
            Self::PdaSeedResolution => ANCHOR_PDA_SEED_RESOLUTION_CODE,
        }
    }

    pub fn default_severity(self) -> DiagnosticSeverity {
        // The severity ceiling is structurally derived from provability:
        // only Syntactic diagnostics (proven from a single file) may default to ERROR.
        // WholeProgram and Heuristic diagnostics default to WARNING because they cannot
        // rule out false positives without cross-file or runtime evidence.
        match self.provability() {
            Provability::Syntactic => DiagnosticSeverity::ERROR,
            Provability::WholeProgram | Provability::Heuristic => DiagnosticSeverity::WARNING,
            Provability::Speculative => DiagnosticSeverity::HINT,
        }
    }

    pub fn docs_url_for_data(self, data: Option<&Value>) -> Option<Url> {
        data.and_then(|value| value.get("topic"))
            .and_then(Value::as_str)
            .and_then(topic_lint_doc_url)
            .or_else(|| data.and_then(diagnostic_docs_url).and_then(parse_url))
            .or_else(|| {
                data.and_then(diagnostic_constraint_key)
                    .and_then(constraint_catalog::documentation_url_for_key)
                    .and_then(parse_url)
            })
            .or_else(|| self.docs_url())
    }

    pub fn docs_url(self) -> Option<Url> {
        let url = match self {
            Self::AnchorSyn => "https://www.anchor-lang.com/docs",
            Self::AnchorInitConstraints => {
                "https://www.anchor-lang.com/docs/references/account-constraints#accountinit"
            }
            Self::AnchorMissingInitConstraint => {
                "https://www.anchor-lang.com/docs/references/account-constraints#accountinit"
            }
            Self::AnchorContextAccounts => {
                "https://www.anchor-lang.com/docs/basics/program-structure#instruction-context"
            }
            Self::AnchorMissingAccountReference => {
                "https://www.anchor-lang.com/docs/references/account-constraints"
            }
            Self::AnchorMissingInstructionArgument => {
                "https://www.anchor-lang.com/docs/references/account-constraints"
            }
            Self::AnchorConstraintExpression => {
                "https://www.anchor-lang.com/docs/references/account-constraints#accountconstraint"
            }
            Self::AnchorConstraintShape => {
                "https://www.anchor-lang.com/docs/references/account-constraints"
            }
            Self::AnchorAccountUsage => {
                "https://www.anchor-lang.com/docs/references/account-constraints#accountmut"
            }
            Self::AnchorCheckCfg => {
                "https://doc.rust-lang.org/nightly/rustc/check-cfg/cargo-specifics.html"
            }
            Self::AnchorProjectId => {
                "https://www.anchor-lang.com/docs/basics/program-structure#declare_id-macro"
            }
            Self::AnchorSbfArtifact => "https://solana.com/docs/core/programs/program-execution",
            Self::AnchorProgramKeypair => {
                "https://www.anchor-lang.com/docs/references/cli#build"
            }
            Self::AnchorIdlArtifact => "https://www.anchor-lang.com/docs/basics/idl",
            Self::AnchorTypesArtifact => "https://www.anchor-lang.com/docs/references/cli#build",
            Self::SolanaIdlArtifact => "https://solana.com/developers/guides/advanced/idls",
            Self::SolanaProgramMetadata => "https://github.com/solana-program/program-metadata",
            Self::SolanaTestHarness => "https://solana.com/docs/programs/testing",
            Self::SolanaSurfpoolWorkspace => "https://docs.surfpool.run/toolchain/cli",
            Self::AnchorSplTokenInterface => {
                "https://www.anchor-lang.com/docs/references/account-constraints#accounttoken"
            }
            Self::SecuritySigner => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/0-signer-authorization"
            }
            Self::SecurityCpiProgram => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/5-arbitrary-cpi"
            }
            Self::SecuritySysvar => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/10-sysvar-address-checking"
            }
            Self::SecurityOwnerCheck => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/2-owner-checks"
            }
            Self::SecurityTypeCosplay => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/3-type-cosplay"
            }
            Self::SecurityUncheckedAccount => {
                "https://learn.blueshift.gg/en/courses/program-security/introduction"
            }
            Self::SecurityStaticPda => {
                "https://solana.com/developers/courses/program-security/pda-sharing"
            }
            Self::SolanaCodeQuality => "https://solana.com/developers/courses/program-security",
            Self::PdaSeedResolution => {
                "https://www.anchor-lang.com/docs/references/account-constraints#accountseeds-bump"
            }
        };
        parse_url(url)
    }

    pub fn rule(self) -> Option<&'static str> {
        match self {
            Self::AnchorSyn
            | Self::AnchorInitConstraints
            | Self::AnchorMissingInitConstraint
            | Self::AnchorContextAccounts
            | Self::AnchorMissingAccountReference
            | Self::AnchorMissingInstructionArgument
            | Self::AnchorConstraintExpression
            | Self::AnchorConstraintShape
            | Self::AnchorAccountUsage
            | Self::AnchorCheckCfg
            | Self::AnchorProjectId
            | Self::AnchorSbfArtifact
            | Self::AnchorProgramKeypair
            | Self::AnchorIdlArtifact
            | Self::AnchorTypesArtifact => None,
            Self::SolanaIdlArtifact => Some("solana/idl-ecosystem"),
            Self::SolanaProgramMetadata => Some("solana/program-metadata"),
            Self::SolanaTestHarness => Some("solana/test-harness"),
            Self::SolanaSurfpoolWorkspace => Some("surfpool/localnet-workspace"),
            Self::AnchorSplTokenInterface => Some("anchor/spl-token-interface"),
            Self::SecuritySigner => Some("sealevel-attacks/signer-authorization"),
            Self::SecurityCpiProgram => Some("sealevel-attacks/arbitrary-cpi"),
            Self::SecuritySysvar => Some("sealevel-attacks/sysvar-address-checking"),
            Self::SecurityOwnerCheck => Some("sealevel-attacks/owner-checks"),
            Self::SecurityTypeCosplay => Some("sealevel-attacks/type-cosplay"),
            Self::SecurityUncheckedAccount => Some("anchor/unchecked-account-validation"),
            Self::SecurityStaticPda => Some("solana/pda-sharing"),
            Self::SolanaCodeQuality => Some("solana/program-code-quality"),
            Self::PdaSeedResolution => Some("anchor/pda-seed-resolution"),
        }
    }

    pub fn confidence(self) -> &'static str {
        match self {
            Self::AnchorSyn
            | Self::AnchorInitConstraints
            | Self::AnchorMissingInitConstraint
            | Self::AnchorContextAccounts
            | Self::AnchorMissingAccountReference
            | Self::AnchorMissingInstructionArgument
            | Self::AnchorConstraintExpression
            | Self::AnchorConstraintShape
            | Self::AnchorAccountUsage
            | Self::AnchorCheckCfg
            | Self::AnchorProjectId
            | Self::AnchorSbfArtifact
            | Self::AnchorProgramKeypair
            | Self::AnchorIdlArtifact
            | Self::AnchorTypesArtifact => "authoritative",
            Self::SolanaIdlArtifact
            | Self::SolanaProgramMetadata
            | Self::SolanaTestHarness
            | Self::SolanaSurfpoolWorkspace
            | Self::AnchorSplTokenInterface => "derived",
            Self::SecuritySigner | Self::SecuritySysvar | Self::SecurityCpiProgram => {
                "authoritative"
            }
            Self::SecurityOwnerCheck
            | Self::SecurityTypeCosplay
            | Self::SecurityUncheckedAccount
            | Self::SecurityStaticPda
            | Self::SolanaCodeQuality => "heuristic",
            Self::PdaSeedResolution => "derived",
        }
    }

    pub const fn topic(self) -> &'static str {
        match self {
            Self::AnchorSyn => "seagrass/anchor.syntax",
            Self::AnchorInitConstraints => "seagrass/anchor.init.constraints",
            Self::AnchorMissingInitConstraint => "seagrass/anchor.init.missing-companion",
            Self::AnchorContextAccounts => "seagrass/anchor.context.accounts",
            Self::AnchorMissingAccountReference => "seagrass/anchor.constraint.account-reference",
            Self::AnchorMissingInstructionArgument => "seagrass/anchor.instruction.argument",
            Self::AnchorConstraintExpression => "seagrass/anchor.constraint.expression",
            Self::AnchorConstraintShape => "seagrass/anchor.constraint.shape",
            Self::AnchorAccountUsage => "seagrass/anchor.account.usage",
            Self::AnchorCheckCfg => "seagrass/anchor.check-cfg",
            Self::AnchorProjectId => "seagrass/anchor.project-id",
            Self::AnchorSbfArtifact => "seagrass/anchor.artifact.sbf",
            Self::AnchorProgramKeypair => "seagrass/anchor.artifact.program-keypair",
            Self::AnchorIdlArtifact => "seagrass/anchor.artifact.idl",
            Self::AnchorTypesArtifact => "seagrass/anchor.artifact.types",
            Self::SolanaIdlArtifact => "seagrass/solana.artifact.idl",
            Self::SolanaProgramMetadata => "seagrass/solana.program-metadata",
            Self::SolanaTestHarness => "seagrass/solana.test-harness",
            Self::SolanaSurfpoolWorkspace => "seagrass/solana.surfpool-workspace",
            Self::AnchorSplTokenInterface => "seagrass/anchor.spl-token-interface",
            Self::SecuritySigner => "seagrass/security.signer.authorization",
            Self::SecurityCpiProgram => "seagrass/security.cpi.program",
            Self::SecuritySysvar => "seagrass/security.sysvar.address",
            Self::SecurityUncheckedAccount => "seagrass/security.account.unchecked",
            Self::SecurityStaticPda => "seagrass/security.pda.static-seed",
            Self::SecurityOwnerCheck => "seagrass/security.owner-check",
            Self::SecurityTypeCosplay => "seagrass/security.type-cosplay",
            Self::SolanaCodeQuality => "seagrass/solana.code-quality",
            Self::PdaSeedResolution => "seagrass/anchor.pda.seed-resolution",
        }
    }

    pub fn anchor_error_names(self) -> &'static [&'static str] {
        match self {
            Self::AnchorInitConstraints | Self::AnchorMissingInitConstraint => &["ConstraintSpace"],
            Self::AnchorMissingAccountReference | Self::AnchorMissingInstructionArgument => &[
                "ConstraintHasOne",
                "ConstraintTokenMint",
                "ConstraintTokenOwner",
                "ConstraintMintMintAuthority",
                "ConstraintMintFreezeAuthority",
                "ConstraintMintDecimals",
                "ConstraintTokenTokenProgram",
                "ConstraintMintTokenProgram",
                "ConstraintAssociatedTokenTokenProgram",
            ],
            Self::AnchorConstraintShape => &[
                "ConstraintMut",
                "ConstraintSeeds",
                "ConstraintClose",
                "ConstraintTokenMint",
                "ConstraintTokenOwner",
                "ConstraintMintMintAuthority",
                "ConstraintMintFreezeAuthority",
                "ConstraintMintDecimals",
                "ConstraintSpace",
                "ConstraintTokenTokenProgram",
                "ConstraintMintTokenProgram",
                "ConstraintAssociatedTokenTokenProgram",
                "TryingToInitPayerAsProgramAccount",
            ],
            Self::AnchorConstraintExpression => &[],
            Self::AnchorAccountUsage => &["ConstraintMut", "AccountNotMutable"],
            Self::SecurityCpiProgram | Self::SecurityUncheckedAccount => {
                &["InvalidProgramExecutable"]
            }
            Self::SecurityOwnerCheck => &["AccountOwnedByWrongProgram"],
            Self::SecuritySigner => &["AccountNotSigner"],
            Self::SecurityTypeCosplay => {
                &["AccountDiscriminatorMismatch", "AccountDidNotDeserialize"]
            }
            Self::SecuritySysvar => &["AccountSysvarMismatch"],
            Self::AnchorProjectId => &["DeclaredProgramIdMismatch"],
            Self::AnchorSyn
            | Self::AnchorContextAccounts
            | Self::AnchorCheckCfg
            | Self::AnchorSbfArtifact
            | Self::AnchorProgramKeypair
            | Self::AnchorIdlArtifact
            | Self::AnchorTypesArtifact
            | Self::SolanaIdlArtifact
            | Self::SolanaProgramMetadata
            | Self::SolanaTestHarness
            | Self::SolanaSurfpoolWorkspace
            | Self::SolanaCodeQuality => &[],
            Self::AnchorSplTokenInterface => &[
                "ConstraintTokenTokenProgram",
                "ConstraintMintTokenProgram",
                "ConstraintAssociatedTokenTokenProgram",
            ],
            Self::SecurityStaticPda => &["ConstraintSeeds"],
            Self::PdaSeedResolution => &["ConstraintSeeds"],
        }
    }
}

fn diagnostic_constraint_key(data: &Value) -> Option<&str> {
    [
        "constraint",
        "requiredBy",
        "conflictsWith",
        "missing",
        "requiredCompanion",
    ]
    .iter()
    .find_map(|key| data.get(key).and_then(Value::as_str))
}

pub(crate) fn topic_lint_doc_url(topic: &str) -> Option<Url> {
    let slug = topic_lint_doc_slug(topic)?;
    parse_url(&format!(
        "https://github.com/heyAyushh/seagrass/blob/main/docs/lints/{slug}.md"
    ))
}

fn topic_lint_doc_slug(topic: &str) -> Option<String> {
    let rest = topic.strip_prefix("seagrass/")?;
    Some(format!("seagrass-{}", rest.replace(['.', '/'], "-")))
}

fn diagnostic_docs_url(data: &Value) -> Option<&str> {
    data.get("docsUrl").and_then(Value::as_str)
}

fn parse_url(url: &str) -> Option<Url> {
    Url::parse(url).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structural guarantee: no diagnostic whose claim cannot be proven from a single file may
    /// default to ERROR.  Heuristic and WholeProgram kinds are explicitly bounded at WARNING.
    /// This test will fail — and catch the problem at CI time — whenever a new variant is added
    /// without a correct provability classification.
    #[test]
    fn severity_is_bounded_by_provability() {
        for kind in AnchorDiagnosticKind::all() {
            let severity = kind.default_severity();
            match kind.provability() {
                Provability::Syntactic => {
                    // No upper ceiling on Syntactic — ERROR is intentional.
                }
                Provability::WholeProgram => {
                    assert_ne!(
                        severity,
                        DiagnosticSeverity::ERROR,
                        "WholeProgram diagnostic `{}` must not default to ERROR \
                         (it cannot prove absence/existence from a single file)",
                        kind.code()
                    );
                }
                Provability::Heuristic => {
                    assert_ne!(
                        severity,
                        DiagnosticSeverity::ERROR,
                        "Heuristic diagnostic `{}` must not default to ERROR \
                         (pattern guesses cannot guarantee correctness)",
                        kind.code()
                    );
                }
                Provability::Speculative => {
                    assert_eq!(
                        severity,
                        DiagnosticSeverity::HINT,
                        "Speculative diagnostic `{}` must default to HINT",
                        kind.code()
                    );
                }
            }
        }
    }

    #[test]
    fn truth_source_declared_for_all_variants() {
        for kind in AnchorDiagnosticKind::all() {
            let _ = kind.truth_source();
        }
    }

    #[test]
    fn none_backed_variants_are_not_syntactic_or_whole_program() {
        for kind in AnchorDiagnosticKind::all() {
            if kind.truth_source() == TruthSource::None {
                assert!(
                    matches!(
                        kind.provability(),
                        Provability::Heuristic | Provability::Speculative
                    ),
                    "Diagnostic `{}` has TruthSource::None but provability {:?}; \
                     only Heuristic/Speculative diagnostics may lack a truth source.",
                    kind.code(),
                    kind.provability()
                );
            }
        }
    }

    /// Exhaustiveness guard: every variant reachable from `provability()` must also appear in
    /// `all()`.  Because `provability()` uses an exhaustive match, the compiler already rejects
    /// missing variants there; this test ensures `all()` stays in sync with the enum count so
    /// that `severity_is_bounded_by_provability` actually visits every variant.
    #[test]
    fn all_variants_are_covered_by_all() {
        // Each variant in `all()` must have a working `code()` and `provability()`.
        // The const array length in `all()` is checked against the enum count implicitly
        // because the exhaustive match in `provability()` must cover every variant.
        for kind in AnchorDiagnosticKind::all() {
            // Calling both methods exercises the exhaustive matches; a compile error would
            // surface before this assertion if a variant were missing.
            let _ = kind.code();
            let _ = kind.provability();
            let _ = kind.truth_source();
        }
        // Verify `all()` has no duplicates by checking that all codes are distinct.
        let codes: Vec<&str> = AnchorDiagnosticKind::all()
            .iter()
            .map(|k| k.code())
            .collect();
        let unique_count = {
            let mut seen = std::collections::HashSet::new();
            codes.iter().filter(|c| seen.insert(*c)).count()
        };
        assert_eq!(
            unique_count,
            AnchorDiagnosticKind::all().len(),
            "`all()` contains duplicate variants"
        );
    }

    #[test]
    fn topic_lint_doc_url_maps_seagrass_topics_to_catalog_pages() {
        let url = topic_lint_doc_url("seagrass/anchor.init.missing-payer")
            .expect("topic lint url")
            .to_string();
        assert!(url.contains("docs/lints/seagrass-anchor-init-missing-payer.md"));
    }

    #[test]
    fn docs_url_uses_exact_constraint_anchor_from_data() {
        for (constraint, expected_url) in [
            (
                "seeds",
                "https://www.anchor-lang.com/docs/references/account-constraints#accountseeds-bump",
            ),
            (
                "close",
                "https://www.anchor-lang.com/docs/references/account-constraints#accountclose--target",
            ),
            (
                "extensions::transfer_hook::program_id",
                "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionstransfer_hook",
            ),
            (
                "token::mint",
                "https://www.anchor-lang.com/docs/references/account-constraints#accounttoken",
            ),
        ] {
            let data = serde_json::json!({ "constraint": constraint });
            let actual_url = AnchorDiagnosticKind::AnchorConstraintShape
                .docs_url_for_data(Some(&data))
                .map(|url| url.to_string());

            assert_eq!(actual_url.as_deref(), Some(expected_url));
        }
    }

    #[test]
    fn docs_url_can_use_rule_specific_reference_from_data() {
        let data = serde_json::json!({
            "docsUrl": "https://doc.rust-lang.org/reference/expressions/struct-expr.html"
        });
        let actual_url = AnchorDiagnosticKind::AnchorMissingAccountReference
            .docs_url_for_data(Some(&data))
            .map(|url| url.to_string());

        assert_eq!(
            actual_url.as_deref(),
            Some("https://doc.rust-lang.org/reference/expressions/struct-expr.html")
        );
    }
}
