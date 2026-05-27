use seagrass_core::anchor_support::{
    AnchorSupportLevel, AnchorSupportManifest, AnchorSupportProfile, AnchorSupportSource,
};
use seagrass_framework::{
    Framework, FrameworkKind, FrameworkMetadata, GeneratedCatalogStats, SourceDescriptor,
    SupportLevel,
};

pub struct AnchorV1Framework;

const FRAMEWORK_ID: &str = "anchor-v1";
const DISPLAY_NAME: &str = "Anchor v1";
pub const MANIFEST: AnchorSupportManifest = include!("generated/anchor_support_generated.rs");

pub const SOURCES: &[SourceDescriptor] = &[
    SourceDescriptor {
        id: "constraint-parser",
        description: "Anchor account constraint parser",
        path_patterns: &["lang/syn/src/parser/accounts/constraints.rs"],
    },
    SourceDescriptor {
        id: "error-codes",
        description: "Anchor framework error catalog",
        path_patterns: &["lang/error/src/lib.rs"],
    },
    SourceDescriptor {
        id: "field-completions",
        description: "Anchor account container and SPL field completions",
        path_patterns: &[
            "lang/src/lib.rs",
            "spl/src/{associated_token,token,token_2022,token_interface}.rs",
        ],
    },
    SourceDescriptor {
        id: "program-corpus",
        description: "Anchor examples and test programs used as generation corpus",
        path_patterns: &[
            "examples/**/programs/**/src/**/*.rs",
            "tests/**/programs/**/src/**/*.rs",
        ],
    },
];

impl Framework for AnchorV1Framework {
    fn metadata(&self) -> FrameworkMetadata {
        FrameworkMetadata {
            id: FRAMEWORK_ID,
            display_name: DISPLAY_NAME,
            kind: FrameworkKind::AnchorV1,
            support_level: SupportLevel::Stable,
            generated: GeneratedCatalogStats {
                constraints: MANIFEST.generated_constraint_count,
                field_completions: MANIFEST.generated_field_completion_count,
                errors: MANIFEST.generated_error_count,
            },
        }
    }

    fn sources(&self) -> &'static [SourceDescriptor] {
        SOURCES
    }
}

pub fn framework() -> AnchorV1Framework {
    AnchorV1Framework
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_generated_anchor_v1_manifest() {
        let metadata = framework().metadata();
        assert_eq!(metadata.id, "anchor-v1");
        assert_eq!(
            metadata.generated.constraints,
            MANIFEST.generated_constraint_count
        );
        assert!(metadata.generated.constraints > 0);
        assert!(metadata.generated.errors > 0);
    }
}
