use seagrass_framework::{Source, SourceDescriptor};

pub struct CargoMetadataSource;
pub struct IdlSource;
pub struct SbfArtifactSource;

const CARGO_METADATA_PATHS: &[&str] = &["Cargo.toml", "Cargo.lock"];
const IDL_PATHS: &[&str] = &["target/idl/*.json", "idls/**/*.json"];
const SBF_ARTIFACT_PATHS: &[&str] = &["target/deploy/*.so", "target/deploy/*-keypair.json"];

impl Source for CargoMetadataSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "cargo-meta",
            description: "Cargo metadata and dependency graph",
            path_patterns: CARGO_METADATA_PATHS,
        }
    }
}

impl Source for IdlSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "idl",
            description: "Anchor and Solana IDL artifacts",
            path_patterns: IDL_PATHS,
        }
    }
}

impl Source for SbfArtifactSource {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            id: "sbf-artifacts",
            description: "Compiled SBF program and keypair artifacts",
            path_patterns: SBF_ARTIFACT_PATHS,
        }
    }
}
