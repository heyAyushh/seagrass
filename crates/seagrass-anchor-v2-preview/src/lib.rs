use seagrass_core::anchor_support::{
    AnchorSupportLevel, AnchorSupportManifest, AnchorSupportProfile, AnchorSupportSource,
};
use seagrass_framework::{
    Framework, FrameworkKind, FrameworkMetadata, GeneratedCatalogStats, SourceDescriptor,
    SupportLevel,
};

pub struct AnchorV2PreviewFramework;

const FRAMEWORK_ID: &str = "anchor-v2-preview";
const DISPLAY_NAME: &str = "Anchor v2 preview";
pub const MANIFEST: AnchorSupportManifest = include!("generated/anchor_support_generated.rs");

pub const SOURCES: &[SourceDescriptor] = &[SourceDescriptor {
    id: "anchor-next",
    description: "Anchor-next parser and framework catalog regeneration input",
    path_patterns: &["--anchor-path <anchor-next checkout>"],
}];

impl Framework for AnchorV2PreviewFramework {
    fn metadata(&self) -> FrameworkMetadata {
        FrameworkMetadata {
            id: FRAMEWORK_ID,
            display_name: DISPLAY_NAME,
            kind: FrameworkKind::AnchorV2Preview,
            support_level: SupportLevel::Preview,
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

pub fn framework() -> AnchorV2PreviewFramework {
    AnchorV2PreviewFramework
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_generated_anchor_v2_preview_manifest() {
        let metadata = framework().metadata();
        assert_eq!(metadata.id, "anchor-v2-preview");
        assert_eq!(metadata.display_name, "Anchor v2 preview");
        assert_eq!(metadata.kind, FrameworkKind::AnchorV2Preview);
        assert_eq!(metadata.support_level, SupportLevel::Preview);
        assert_eq!(MANIFEST.anchor_version, "2.0.0");
        assert_eq!(MANIFEST.support_level, AnchorSupportLevel::AnchorV2Preview);
        assert_eq!(MANIFEST.profile.version_major, 2);
        assert_eq!(MANIFEST.profile.version_family, "anchor-v2-preview");
        assert_eq!(
            metadata.generated.constraints,
            MANIFEST.generated_constraint_count
        );
        assert!(metadata.generated.constraints > 0);
        assert!(metadata.generated.errors > 0);
        assert!(MANIFEST
            .notes
            .iter()
            .any(|note| note.contains("anchor-next")));
        assert!(!MANIFEST
            .notes
            .iter()
            .any(|note| note.contains("newer Anchor v1 releases")));
    }
}
