use seagrass_framework::{
    Framework, FrameworkKind, FrameworkMetadata, GeneratedCatalogStats, SourceDescriptor,
    SupportLevel,
};

pub struct PinocchioFramework;

const FRAMEWORK_ID: &str = "pinocchio";
const DISPLAY_NAME: &str = "Pinocchio";

pub const SOURCES: &[SourceDescriptor] = &[SourceDescriptor {
    id: "cargo-meta",
    description: "Pinocchio dependency and source detection",
    path_patterns: &["Cargo.toml", "src/**/*.rs"],
}];

impl Framework for PinocchioFramework {
    fn metadata(&self) -> FrameworkMetadata {
        FrameworkMetadata {
            id: FRAMEWORK_ID,
            display_name: DISPLAY_NAME,
            kind: FrameworkKind::Pinocchio,
            support_level: SupportLevel::Experimental,
            generated: GeneratedCatalogStats {
                constraints: 0,
                field_completions: 0,
                errors: 0,
            },
        }
    }

    fn sources(&self) -> &'static [SourceDescriptor] {
        SOURCES
    }
}

pub fn framework() -> PinocchioFramework {
    PinocchioFramework
}
