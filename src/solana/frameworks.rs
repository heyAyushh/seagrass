use {
    crate::{
        document::ParsedDocument,
        solana_project::{SolanaProgram, SolanaProjectKind},
    },
    cargo_toml::{Dependency, DepsSet, Manifest},
    std::collections::HashSet,
    syn::visit::{self, Visit},
};

const ANCHOR_LANG_V2_DEPENDENCY: &str = "anchor-lang-v2";
const ANCHOR_SPL_V2_DEPENDENCY: &str = "anchor-spl-v2";
const ANCHOR_LANG_DEPENDENCY: &str = "anchor-lang";
const ANCHOR_SPL_DEPENDENCY: &str = "anchor-spl";
const ANCHOR_V2_MAJOR_PREFIX: char = '2';
const ANCHOR_SOURCE_HINTS: &[&str] = &[
    "anchor_lang::",
    "anchor_spl::",
    "anchor_lang_v2::",
    "anchor_spl_v2::",
    "#[program]",
];
const PINOCCHIO_DEPENDENCY: &str = "pinocchio";
const PINOCCHIO_DEPENDENCIES: &[&str] = &[
    PINOCCHIO_DEPENDENCY,
    "pinocchio-associated-token-account",
    "pinocchio-pubkey",
    "pinocchio-system",
    "pinocchio-token",
    "pinocchio-token-interface",
    "solana-account-view",
    "solana-instruction-view",
];
const PINOCCHIO_SOURCE_CRATES: &[&str] = &[
    "pinocchio",
    "pinocchio_associated_token_account",
    "pinocchio_pubkey",
    "pinocchio_system",
    "pinocchio_token",
    "pinocchio_token_interface",
    "solana_account_view",
    "solana_instruction_view",
];
const NATIVE_SOLANA_DEPENDENCIES: &[&str] = &[
    "solana-address",
    "solana-account-info",
    "solana-cpi",
    "solana-instruction",
    "solana-msg",
    "solana-program",
    "solana-program-entrypoint",
    "solana-program-error",
    "solana-pubkey",
    "solana-sdk-ids",
    "solana-sysvar",
    "solana-system-interface",
];
const NATIVE_SOLANA_SOURCE_CRATES: &[&str] = &[
    "solana_address",
    "solana_account_info",
    "solana_cpi",
    "solana_instruction",
    "solana_msg",
    "solana_program",
    "solana_program_entrypoint",
    "solana_program_error",
    "solana_pubkey",
    "solana_sdk_ids",
    "solana_sysvar",
    "solana_system_interface",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FrameworkId {
    Unknown,
    AnchorV1,
    AnchorV2Preview,
    Pinocchio,
    NativeSolana,
}

impl FrameworkId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::AnchorV1 => "anchor-v1",
            Self::AnchorV2Preview => "anchor-v2-preview",
            Self::Pinocchio => "pinocchio",
            Self::NativeSolana => "native-solana",
        }
    }

    pub const fn diagnostic_program_kind(self) -> &'static str {
        match self {
            Self::AnchorV1 | Self::AnchorV2Preview => "anchor",
            Self::Pinocchio => "pinocchio",
            Self::NativeSolana => "native-solana",
            Self::Unknown => "unknown",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown Solana framework",
            Self::AnchorV1 => "Anchor",
            Self::AnchorV2Preview => "Anchor v2 preview",
            Self::Pinocchio => "Pinocchio",
            Self::NativeSolana => "native Solana",
        }
    }

    pub const fn is_anchor(self) -> bool {
        matches!(self, Self::AnchorV1 | Self::AnchorV2Preview)
    }

    pub const fn build_command(self) -> &'static str {
        match self {
            Self::AnchorV1 | Self::AnchorV2Preview => "anchor build",
            Self::Pinocchio | Self::NativeSolana | Self::Unknown => "cargo build-sbf",
        }
    }

    pub const fn idl_label(self) -> &'static str {
        match self {
            Self::AnchorV1 | Self::AnchorV2Preview => "Anchor IDL",
            Self::Pinocchio | Self::NativeSolana | Self::Unknown => "Solana IDL",
        }
    }

    pub const fn types_label(self) -> &'static str {
        match self {
            Self::AnchorV1 | Self::AnchorV2Preview => "Anchor TypeScript types",
            Self::Pinocchio | Self::NativeSolana | Self::Unknown => "generated TypeScript client",
        }
    }

    pub const fn from_project_kind(kind: SolanaProjectKind) -> Self {
        match kind {
            SolanaProjectKind::Anchor => Self::AnchorV1,
            SolanaProjectKind::Pinocchio => Self::Pinocchio,
            SolanaProjectKind::NativeSolana => Self::NativeSolana,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameworkContext {
    id: FrameworkId,
}

impl FrameworkContext {
    pub const fn new(id: FrameworkId) -> Self {
        Self { id }
    }

    pub const fn unknown() -> Self {
        Self::new(FrameworkId::Unknown)
    }

    pub const fn id(self) -> FrameworkId {
        self.id
    }

    pub fn from_document(document: &ParsedDocument) -> Self {
        Self::new(framework_from_document(document))
    }

    pub fn from_project_and_document(
        program: Option<&SolanaProgram>,
        manifest_text: Option<&str>,
        document: &ParsedDocument,
    ) -> Self {
        let manifest_id = manifest_text.and_then(framework_from_manifest);
        let document_id = framework_from_document(document);
        let id = program
            .map(|program| FrameworkId::from_project_kind(program.kind))
            .map(|project_id| refine_project_framework(project_id, manifest_id, document_id))
            .or(manifest_id)
            .unwrap_or(document_id);

        Self::new(id)
    }
}

impl Default for FrameworkContext {
    fn default() -> Self {
        Self::unknown()
    }
}

pub fn source_has_anchor_framework_hint(source: &str) -> bool {
    ANCHOR_SOURCE_HINTS.iter().any(|hint| source.contains(hint))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameworkSet(u8);

const ANCHOR_V1_BIT: u8 = 1 << 0;
const ANCHOR_V2_PREVIEW_BIT: u8 = 1 << 1;
const PINOCCHIO_BIT: u8 = 1 << 2;
const NATIVE_SOLANA_BIT: u8 = 1 << 3;
const SOLANA_PROGRAM_BITS: u8 =
    ANCHOR_V1_BIT | ANCHOR_V2_PREVIEW_BIT | PINOCCHIO_BIT | NATIVE_SOLANA_BIT;

impl FrameworkSet {
    pub const ALL: Self = Self(SOLANA_PROGRAM_BITS);
    pub const ANCHOR: Self = Self(ANCHOR_V1_BIT | ANCHOR_V2_PREVIEW_BIT);
    pub const SOLANA_PROGRAMS: Self = Self(SOLANA_PROGRAM_BITS);

    pub const fn contains(self, framework: FrameworkId) -> bool {
        match framework {
            FrameworkId::Unknown => true,
            FrameworkId::AnchorV1 => self.0 & ANCHOR_V1_BIT != 0,
            FrameworkId::AnchorV2Preview => self.0 & ANCHOR_V2_PREVIEW_BIT != 0,
            FrameworkId::Pinocchio => self.0 & PINOCCHIO_BIT != 0,
            FrameworkId::NativeSolana => self.0 & NATIVE_SOLANA_BIT != 0,
        }
    }
}

fn refine_project_framework(
    project_id: FrameworkId,
    manifest_id: Option<FrameworkId>,
    document_id: FrameworkId,
) -> FrameworkId {
    if project_id.is_anchor()
        && (manifest_id == Some(FrameworkId::AnchorV2Preview)
            || document_id == FrameworkId::AnchorV2Preview)
    {
        return FrameworkId::AnchorV2Preview;
    }
    project_id
}

fn framework_from_manifest(manifest_text: &str) -> Option<FrameworkId> {
    let manifest = Manifest::from_str(manifest_text).ok()?;
    let mut dependencies = HashSet::new();
    extend_dependency_names(&manifest.dependencies, &mut dependencies);
    extend_dependency_names(&manifest.dev_dependencies, &mut dependencies);
    extend_dependency_names(&manifest.build_dependencies, &mut dependencies);
    for target in manifest.target.values() {
        extend_dependency_names(&target.dependencies, &mut dependencies);
        extend_dependency_names(&target.dev_dependencies, &mut dependencies);
        extend_dependency_names(&target.build_dependencies, &mut dependencies);
    }

    if manifest_has_anchor_v2_dependency(&manifest) {
        return Some(FrameworkId::AnchorV2Preview);
    }
    if dependencies.contains(ANCHOR_LANG_V2_DEPENDENCY)
        || dependencies.contains(ANCHOR_SPL_V2_DEPENDENCY)
    {
        return Some(FrameworkId::AnchorV2Preview);
    }
    if dependencies.contains(ANCHOR_LANG_DEPENDENCY) || dependencies.contains(ANCHOR_SPL_DEPENDENCY)
    {
        return Some(FrameworkId::AnchorV1);
    }
    if dependencies
        .iter()
        .any(|dependency| PINOCCHIO_DEPENDENCIES.contains(&dependency.as_str()))
    {
        return Some(FrameworkId::Pinocchio);
    }
    if dependencies
        .iter()
        .any(|dependency| NATIVE_SOLANA_DEPENDENCIES.contains(&dependency.as_str()))
    {
        return Some(FrameworkId::NativeSolana);
    }
    None
}

fn extend_dependency_names(dependencies: &DepsSet, names: &mut HashSet<String>) {
    for (name, dependency) in dependencies {
        names.insert(name.clone());
        if let Some(package) = dependency
            .detail()
            .and_then(|detail| detail.package.as_ref())
        {
            names.insert(package.clone());
        }
    }
}

fn manifest_has_anchor_v2_dependency(manifest: &Manifest) -> bool {
    dependency_sets(manifest).any(dependencies_have_anchor_v2_dependency)
}

fn dependency_sets(manifest: &Manifest) -> impl Iterator<Item = &DepsSet> {
    std::iter::once(&manifest.dependencies)
        .chain(std::iter::once(&manifest.dev_dependencies))
        .chain(std::iter::once(&manifest.build_dependencies))
        .chain(manifest.target.values().flat_map(|target| {
            [
                &target.dependencies,
                &target.dev_dependencies,
                &target.build_dependencies,
            ]
        }))
}

fn dependencies_have_anchor_v2_dependency(dependencies: &DepsSet) -> bool {
    dependencies.iter().any(|(name, dependency)| {
        let package_name = dependency.package().unwrap_or(name);
        matches!(package_name, ANCHOR_LANG_DEPENDENCY | ANCHOR_SPL_DEPENDENCY)
            && dependency_declares_v2(dependency)
    })
}

fn dependency_declares_v2(dependency: &Dependency) -> bool {
    dependency
        .try_req()
        .ok()
        .is_some_and(version_requirement_starts_at_v2)
}

fn version_requirement_starts_at_v2(requirement: &str) -> bool {
    let trimmed = requirement.trim_start_matches([' ', '^', '~', '=', '>', '<']);
    trimmed.starts_with(ANCHOR_V2_MAJOR_PREFIX)
        && trimmed
            .chars()
            .nth(1)
            .is_none_or(|character| !character.is_ascii_digit())
}

fn framework_from_document(document: &ParsedDocument) -> FrameworkId {
    let mut visitor = FrameworkVisitor {
        id: FrameworkId::Unknown,
    };
    visitor.visit_file(document.syntax());
    if visitor.id == FrameworkId::Unknown && source_has_anchor_framework_hint(document.source()) {
        return FrameworkId::AnchorV1;
    }
    visitor.id
}

struct FrameworkVisitor {
    id: FrameworkId,
}

impl FrameworkVisitor {
    fn record_crate_ident(&mut self, ident: &syn::Ident) {
        match ident.to_string().as_str() {
            "anchor_lang_v2" | "anchor_spl_v2" => self.id = FrameworkId::AnchorV2Preview,
            "anchor_lang" | "anchor_spl" if !self.id.is_anchor() => {
                self.id = FrameworkId::AnchorV1;
            }
            ident if PINOCCHIO_SOURCE_CRATES.contains(&ident) && !self.id.is_anchor() => {
                self.id = FrameworkId::Pinocchio;
            }
            ident
                if NATIVE_SOLANA_SOURCE_CRATES.contains(&ident)
                    && self.id == FrameworkId::Unknown =>
            {
                self.id = FrameworkId::NativeSolana;
            }
            _ => {}
        }
    }

    fn record_path(&mut self, path: &syn::Path) {
        let Some(first) = path.segments.first().map(|segment| &segment.ident) else {
            return;
        };
        self.record_crate_ident(first);
    }

    fn record_type_path(&mut self, path: &syn::Path) {
        self.record_path(path);
        if path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "ProgramResult")
            && self.id == FrameworkId::Unknown
        {
            self.id = FrameworkId::NativeSolana;
        }
    }
}

impl<'ast> Visit<'ast> for FrameworkVisitor {
    fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
        if node.path().is_ident("program") && !self.id.is_anchor() {
            self.id = FrameworkId::AnchorV1;
        }
        visit::visit_attribute(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.record_path(node);
        visit::visit_path(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        self.record_type_path(&node.path);
        visit::visit_type_path(self, node);
    }

    fn visit_use_tree(&mut self, node: &'ast syn::UseTree) {
        match node {
            syn::UseTree::Path(path) => self.record_crate_ident(&path.ident),
            syn::UseTree::Name(name) => self.record_crate_ident(&name.ident),
            syn::UseTree::Rename(rename) => self.record_crate_ident(&rename.ident),
            syn::UseTree::Glob(_) | syn::UseTree::Group(_) => {}
        }
        visit::visit_use_tree(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_anchor_v2_preview_dependency_from_manifest() {
        let manifest = r#"
[package]
name = "demo"

[dependencies]
anchor-lang-v2 = { git = "https://github.com/solana-foundation/anchor.git", branch = "anchor-next" }
pinocchio = "0.11"
"#;

        assert_eq!(
            framework_from_manifest(manifest),
            Some(FrameworkId::AnchorV2Preview)
        );
    }

    #[test]
    fn detects_anchor_v2_preview_from_real_anchor_lang_major_version() {
        let manifest = r#"
[package]
name = "demo"

[dependencies]
anchor-lang = "2"
anchor-spl-legacy = { package = "anchor-spl", version = "^2.0" }
"#;

        assert_eq!(
            framework_from_manifest(manifest),
            Some(FrameworkId::AnchorV2Preview)
        );
    }

    #[test]
    fn detects_pinocchio_from_document() {
        let document = ParsedDocument::parse_or_empty(
            r#"
use pinocchio::{entrypoint, AccountView, Address, ProgramResult};
entrypoint!(process_instruction);
"#,
        );

        assert_eq!(
            FrameworkContext::from_document(&document).id(),
            FrameworkId::Pinocchio
        );
    }

    #[test]
    fn detects_pinocchio_from_split_account_view_crates() {
        let document = ParsedDocument::parse_or_empty(
            r#"
use {
    solana_account_view::AccountView,
    solana_address::Address,
    solana_instruction_view::{InstructionAccount, InstructionView},
    solana_program_error::ProgramResult,
};

pub fn initialize(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let _ = (program_id, accounts, instruction_data);
    let _ = InstructionView { program_id, accounts: &[], data: &[] };
    let _ = InstructionAccount::readonly(program_id);
    Ok(())
}
"#,
        );

        assert_eq!(
            FrameworkContext::from_document(&document).id(),
            FrameworkId::Pinocchio
        );
    }

    #[test]
    fn detects_native_solana_from_modular_document_crates() {
        let document = ParsedDocument::parse_or_empty(
            r#"
use {
    solana_account_info::AccountInfo,
    solana_address::Address,
    solana_msg::msg,
    solana_program_error::ProgramResult,
};

solana_program_entrypoint::entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Address,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("processing native program");
    Ok(())
}
"#,
        );

        assert_eq!(
            FrameworkContext::from_document(&document).id(),
            FrameworkId::NativeSolana
        );
    }

    #[test]
    fn detects_native_solana_from_modular_manifest_crates() {
        let manifest = r#"
[package]
name = "native-program"

[dependencies]
solana-account-info = "3"
solana-program-entrypoint = "3"
solana-program-error = "3"
"#;

        assert_eq!(
            framework_from_manifest(manifest),
            Some(FrameworkId::NativeSolana)
        );
    }

    #[test]
    fn detects_pinocchio_from_split_account_view_manifest_crates() {
        let manifest = r#"
[package]
name = "pinocchio-view-demo"

[dependencies]
solana-account-view = "3"
solana-instruction-view = "3"
solana-program-error = "3"
"#;

        assert_eq!(
            framework_from_manifest(manifest),
            Some(FrameworkId::Pinocchio)
        );
    }

    #[test]
    fn unknown_framework_keeps_rules_eligible_for_partial_sources() {
        assert!(FrameworkSet::ANCHOR.contains(FrameworkId::Unknown));
    }

    #[test]
    fn detects_anchor_from_source_hint_when_syn_ast_is_empty() {
        let document = ParsedDocument::parse_or_empty(
            r#"
use anchor_lang::prelude::*;

pub fn run(ctx: Context<Run>) -> Result<()> {
    broken =
}
"#,
        );

        assert!(document.syntax().items.is_empty());
        assert_eq!(
            FrameworkContext::from_document(&document).id(),
            FrameworkId::AnchorV1
        );
    }

    #[test]
    fn anchor_source_hint_covers_v1_and_v2_import_paths() {
        for source in [
            "use anchor_lang::prelude::*;",
            "use anchor_spl::token::Token;",
            "use anchor_lang_v2::prelude::*;",
            "use anchor_spl_v2::token::Token;",
            "#[program]\npub mod demo {}",
        ] {
            assert!(source_has_anchor_framework_hint(source), "{source}");
        }
        assert!(!source_has_anchor_framework_hint(
            "use pinocchio::{entrypoint, ProgramResult};"
        ));
    }
}
