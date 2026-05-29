use tower_lsp::lsp_types::{DiagnosticSeverity, Url};

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
pub const ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE: &str = "anchor-security-token-account";
pub const ANCHOR_SECURITY_CPI_PROGRAM_CODE: &str = "anchor-security-cpi-program";
pub const ANCHOR_SECURITY_SYSVAR_CODE: &str = "anchor-security-sysvar";
pub const ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE: &str = "anchor-security-duplicate-account";
pub const ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE: &str = "anchor-security-unchecked-account";
pub const ANCHOR_SECURITY_STATIC_PDA_CODE: &str = "anchor-security-static-pda";
pub const ANCHOR_SECURITY_OWNER_CHECK_CODE: &str = "anchor-security-owner-check";
pub const ANCHOR_SECURITY_TYPE_COSPLAY_CODE: &str = "anchor-security-type-cosplay";
pub const SOLANA_CODE_QUALITY_CODE: &str = "solana-code-quality";
pub const ANCHOR_PDA_SEED_RESOLUTION_CODE: &str = "anchor-pda-seed-resolution";
pub const INIT_PLACEHOLDERS_QUICKFIX: &str = "init-placeholders";
pub const REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX: &str =
    "replace-constraint-expression-member";
pub const SOURCE: &str = "seagrass";

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
    SecurityTokenAccount,
    SecurityCpiProgram,
    SecuritySysvar,
    SecurityDuplicateAccount,
    SecurityUncheckedAccount,
    SecurityStaticPda,
    SecurityOwnerCheck,
    SecurityTypeCosplay,
    SolanaCodeQuality,
    PdaSeedResolution,
}

impl AnchorDiagnosticKind {
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
            Self::SecurityTokenAccount => ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE,
            Self::SecurityCpiProgram => ANCHOR_SECURITY_CPI_PROGRAM_CODE,
            Self::SecuritySysvar => ANCHOR_SECURITY_SYSVAR_CODE,
            Self::SecurityDuplicateAccount => ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE,
            Self::SecurityUncheckedAccount => ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE,
            Self::SecurityStaticPda => ANCHOR_SECURITY_STATIC_PDA_CODE,
            Self::SecurityOwnerCheck => ANCHOR_SECURITY_OWNER_CHECK_CODE,
            Self::SecurityTypeCosplay => ANCHOR_SECURITY_TYPE_COSPLAY_CODE,
            Self::SolanaCodeQuality => SOLANA_CODE_QUALITY_CODE,
            Self::PdaSeedResolution => ANCHOR_PDA_SEED_RESOLUTION_CODE,
        }
    }

    pub fn default_severity(self) -> DiagnosticSeverity {
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
            | Self::AnchorProjectId => DiagnosticSeverity::ERROR,
            Self::AnchorCheckCfg
            | Self::AnchorSbfArtifact
            | Self::AnchorProgramKeypair
            | Self::AnchorIdlArtifact
            | Self::AnchorTypesArtifact
            | Self::SolanaIdlArtifact
            | Self::SolanaProgramMetadata
            | Self::SolanaTestHarness
            | Self::SolanaSurfpoolWorkspace
            | Self::AnchorSplTokenInterface => DiagnosticSeverity::WARNING,
            Self::SecuritySigner
            | Self::SecurityTokenAccount
            | Self::SecurityCpiProgram
            | Self::SecuritySysvar
            | Self::SecurityDuplicateAccount
            | Self::SecurityUncheckedAccount
            | Self::SecurityStaticPda
            | Self::SecurityOwnerCheck
            | Self::SecurityTypeCosplay
            | Self::SolanaCodeQuality => DiagnosticSeverity::WARNING,
            Self::PdaSeedResolution => DiagnosticSeverity::WARNING,
        }
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
                "https://www.anchor-lang.com/docs/references/account-constraints#token"
            }
            Self::SecuritySigner => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/0-signer-authorization"
            }
            Self::SecurityTokenAccount => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/1-account-data-matching"
            }
            Self::SecurityCpiProgram => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/5-arbitrary-cpi"
            }
            Self::SecuritySysvar => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/10-sysvar-address-checking"
            }
            Self::SecurityDuplicateAccount => {
                "https://github.com/coral-xyz/sealevel-attacks/tree/master/programs/6-duplicate-mutable-accounts"
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
                "https://www.anchor-lang.com/docs/references/account-constraints#accountseeds"
            }
        };
        Url::parse(url).ok()
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
            Self::SecurityTokenAccount => Some("sealevel-attacks/account-data-matching"),
            Self::SecurityCpiProgram => Some("sealevel-attacks/arbitrary-cpi"),
            Self::SecuritySysvar => Some("sealevel-attacks/sysvar-address-checking"),
            Self::SecurityDuplicateAccount => Some("sealevel-attacks/duplicate-mutable-accounts"),
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
            Self::SecurityTokenAccount
            | Self::SecurityOwnerCheck
            | Self::SecurityTypeCosplay
            | Self::SecurityDuplicateAccount
            | Self::SecurityUncheckedAccount
            | Self::SecurityStaticPda
            | Self::SolanaCodeQuality => "heuristic",
            Self::PdaSeedResolution => "derived",
        }
    }

    pub fn topic(self) -> &'static str {
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
            Self::SecurityTokenAccount => "seagrass/security.token-account",
            Self::SecurityCpiProgram => "seagrass/security.cpi.program",
            Self::SecuritySysvar => "seagrass/security.sysvar.address",
            Self::SecurityDuplicateAccount => "seagrass/security.account.duplicate-mutable",
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
            Self::SecurityDuplicateAccount => &["ConstraintDuplicateMutableAccount"],
            Self::SecuritySigner => &["AccountNotSigner"],
            Self::SecurityTokenAccount => {
                &["AccountOwnedByWrongProgram", "AccountDidNotDeserialize"]
            }
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
