pub mod diagnostics;
pub mod extractor;
pub mod lint;
pub mod native_rules;
pub mod range;
pub mod semantic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameworkKind {
    AnchorV1,
    AnchorV2Preview,
    Pinocchio,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    Stable,
    Preview,
    Experimental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedCatalogStats {
    pub constraints: usize,
    pub field_completions: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameworkMetadata {
    pub id: &'static str,
    pub display_name: &'static str,
    pub kind: FrameworkKind,
    pub support_level: SupportLevel,
    pub generated: GeneratedCatalogStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceDescriptor {
    pub id: &'static str,
    pub description: &'static str,
    pub path_patterns: &'static [&'static str],
}

pub trait Framework {
    fn metadata(&self) -> FrameworkMetadata;
    fn sources(&self) -> &'static [SourceDescriptor];
}

pub trait Source {
    fn descriptor(&self) -> SourceDescriptor;
}

impl FrameworkKind {
    pub const fn program_kind_label(self) -> &'static str {
        match self {
            Self::AnchorV1 | Self::AnchorV2Preview => "anchor",
            Self::Pinocchio => "pinocchio",
            Self::Native => "native-solana",
        }
    }
}
