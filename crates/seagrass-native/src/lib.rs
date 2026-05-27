use seagrass_framework::{
    Framework, FrameworkKind, FrameworkMetadata, GeneratedCatalogStats, SourceDescriptor,
    SupportLevel,
};

pub struct NativeSolanaFramework;

const FRAMEWORK_ID: &str = "native";
const DISPLAY_NAME: &str = "Native Solana";

pub const SOURCES: &[SourceDescriptor] = &[SourceDescriptor {
    id: "cargo-meta",
    description: "Native Solana dependency and source detection",
    path_patterns: &["Cargo.toml", "src/**/*.rs"],
}];

impl Framework for NativeSolanaFramework {
    fn metadata(&self) -> FrameworkMetadata {
        FrameworkMetadata {
            id: FRAMEWORK_ID,
            display_name: DISPLAY_NAME,
            kind: FrameworkKind::Native,
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

pub fn framework() -> NativeSolanaFramework {
    NativeSolanaFramework
}
