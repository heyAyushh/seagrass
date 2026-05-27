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
