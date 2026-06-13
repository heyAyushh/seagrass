use {
    crate::{
        anchor::idioms, document::ParsedDocument, file_text, project,
        solana::frameworks::FrameworkId,
    },
    cargo_toml::{DepsSet, Manifest},
    std::{
        collections::{HashMap, HashSet},
        fs,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::{Range, Url},
};

const MAX_MANIFEST_DEPTH: usize = 5;
const ANCHOR_LANG_DEPENDENCY: &str = "anchor-lang";
const ANCHOR_PROGRAM_ATTRIBUTE: &str = "program";
const PINOCCHIO_DEPENDENCIES: &[&str] = &[
    "pinocchio",
    "pinocchio-associated-token-account",
    "pinocchio-pubkey",
    "pinocchio-system",
    "pinocchio-token",
    "pinocchio-token-interface",
    "solana-account-view",
    "solana-instruction-view",
];
const PINOCCHIO_SOURCE_HINTS: &[&str] = &[
    "pinocchio::",
    "pinocchio_",
    "solana_account_view::",
    "solana_instruction_view::",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SolanaProjectKind {
    Anchor,
    Pinocchio,
    NativeSolana,
}

impl SolanaProjectKind {
    pub fn framework_id(self) -> FrameworkId {
        FrameworkId::from_project_kind(self)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::Pinocchio => "pinocchio",
            Self::NativeSolana => "native",
        }
    }

    pub fn label(self) -> &'static str {
        self.framework_id().label()
    }

    pub fn build_command(self) -> &'static str {
        self.framework_id().build_command()
    }

    pub fn idl_label(self) -> &'static str {
        self.framework_id().idl_label()
    }

    pub fn types_label(self) -> &'static str {
        self.framework_id().types_label()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolanaProgram {
    pub kind: SolanaProjectKind,
    pub root: PathBuf,
    pub source_root: Option<PathBuf>,
    pub name: String,
    pub id: Option<String>,
    pub cluster: Option<String>,
    pub metadata_uri: Url,
    pub metadata_range: Range,
}

impl SolanaProgram {
    pub fn id_display(&self) -> &str {
        self.id.as_deref().unwrap_or("unknown")
    }

    pub fn cluster_display(&self) -> &str {
        self.cluster.as_deref().unwrap_or("unknown")
    }
}

#[derive(Debug, Clone)]
struct CargoManifest {
    name: Option<String>,
    lib_name: Option<String>,
    lib_crate_types: Vec<String>,
    dependencies: CargoManifestDeps,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CargoManifestDeps {
    names: HashSet<String>,
    import_roots: HashMap<String, String>,
}

impl CargoManifestDeps {
    pub fn contains_crate(&self, crate_name: &str) -> bool {
        let expected = normalize_dependency_name(crate_name);
        self.names
            .iter()
            .any(|name| normalize_dependency_name(name) == expected)
    }

    pub fn import_root_matches_crate(&self, import_root: &str, crate_name: &str) -> bool {
        let normalized_root = normalize_dependency_name(import_root);
        let expected_crate = normalize_dependency_name(crate_name);
        self.import_roots
            .get(&normalized_root)
            .is_some_and(|dependency_package| dependency_package == &expected_crate)
    }

    fn insert_dependency(&mut self, import_root: String, package_name: Option<String>) {
        let dependency_package = package_name.unwrap_or_else(|| import_root.clone());
        self.names.insert(import_root.clone());
        self.names.insert(dependency_package.clone());
        self.import_roots.insert(
            normalize_dependency_name(&import_root),
            normalize_dependency_name(&dependency_package),
        );
    }

    fn iter(&self) -> impl Iterator<Item = &String> {
        self.names.iter()
    }
}

pub fn detect_for_document(uri: &Url, document: &ParsedDocument) -> Option<SolanaProgram> {
    detect_anchor_for_document(uri, document).or_else(|| detect_cargo_for_document(uri, document))
}

pub fn detect_for_roots(roots: &[Url]) -> Vec<SolanaProgram> {
    let mut programs = Vec::new();
    for root in roots {
        let Ok(root_path) = root.to_file_path() else {
            continue;
        };
        programs.extend(detect_anchor_programs(&root_path));
        for manifest_path in cargo_manifest_paths(&root_path) {
            if let Some(program) = detect_manifest_program(&manifest_path, None) {
                programs.push(program);
            }
        }
    }
    programs.sort_by(|left, right| {
        left.root
            .cmp(&right.root)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    programs.dedup_by(|left, right| {
        left.root == right.root && left.name == right.name && left.kind == right.kind
    });
    programs
}

fn detect_anchor_for_document(uri: &Url, document: &ParsedDocument) -> Option<SolanaProgram> {
    let (anchor_toml_uri, anchor_toml_text) = project::nearest_anchor_toml(uri)?;
    let root = anchor_toml_uri.to_file_path().ok()?.parent()?.to_path_buf();
    let Some(program) = project::preferred_program_id(uri, &anchor_toml_text) else {
        return detect_anchor_from_manifest(uri, document, anchor_toml_uri, root);
    };
    Some(SolanaProgram {
        kind: SolanaProjectKind::Anchor,
        source_root: program_source_root(&root, &program.name),
        root,
        name: program.name,
        id: document
            .symbols()
            .declared_program_id
            .as_ref()
            .map(|declared| declared.value.clone())
            .or(Some(program.value)),
        cluster: Some(program.cluster),
        metadata_uri: anchor_toml_uri,
        metadata_range: program.range,
    })
}

fn detect_anchor_from_manifest(
    uri: &Url,
    document: &ParsedDocument,
    anchor_toml_uri: Url,
    root: PathBuf,
) -> Option<SolanaProgram> {
    let manifest_path = nearest_manifest_path(uri)?;
    let text = file_text::read_limited_text(&manifest_path)
        .ok()
        .flatten()?;
    let manifest = parse_cargo_manifest(&text)?;
    if !manifest_is_program_library(&manifest)
        || !manifest_or_source_declares_anchor(&manifest, document)
    {
        return None;
    }
    let package_root = manifest_path.parent()?.to_path_buf();
    let raw_name = manifest.lib_name.as_ref().or(manifest.name.as_ref())?;
    Some(SolanaProgram {
        kind: SolanaProjectKind::Anchor,
        root,
        source_root: Some(package_root.join("src")).filter(|path| path.is_dir()),
        name: project::normalize_program_name(raw_name),
        id: document
            .symbols()
            .declared_program_id
            .as_ref()
            .map(|declared| declared.value.clone()),
        cluster: None,
        metadata_uri: anchor_toml_uri,
        metadata_range: Range::default(),
    })
}

fn detect_anchor_programs(root: &Path) -> Vec<SolanaProgram> {
    anchor_toml_paths(root)
        .into_iter()
        .flat_map(|path| {
            let Some(uri) = Url::from_file_path(&path).ok() else {
                return Vec::new();
            };
            let Ok(Some(text)) = file_text::read_limited_text(&path) else {
                return Vec::new();
            };
            let Some(root) = path.parent().map(Path::to_path_buf) else {
                return Vec::new();
            };
            project::parse_anchor_toml(&text)
                .programs
                .into_iter()
                .map(|program| SolanaProgram {
                    kind: SolanaProjectKind::Anchor,
                    source_root: program_source_root(&root, &program.name),
                    root: root.clone(),
                    name: program.name,
                    id: Some(program.value),
                    cluster: Some(program.cluster),
                    metadata_uri: uri.clone(),
                    metadata_range: program.range,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn detect_cargo_for_document(uri: &Url, document: &ParsedDocument) -> Option<SolanaProgram> {
    let manifest_path = nearest_manifest_path(uri)?;
    detect_manifest_program(&manifest_path, Some(document))
}

fn detect_manifest_program(
    path: &Path,
    document: Option<&ParsedDocument>,
) -> Option<SolanaProgram> {
    let text = file_text::read_limited_text(path).ok().flatten()?;
    let manifest = parse_cargo_manifest(&text)?;
    let package_root = path.parent()?.to_path_buf();
    let root = cargo_build_root(&package_root).unwrap_or_else(|| package_root.clone());
    let source_root = Some(package_root.join("src")).filter(|path| path.is_dir());
    let source_text = document
        .map(|document| document.source().to_string())
        .or_else(|| source_root.as_deref().and_then(read_package_source))
        .unwrap_or_default();
    let id = document
        .and_then(|document| document.symbols().declared_program_id.as_ref())
        .map(|declared| declared.value.clone())
        .or_else(|| {
            ParsedDocument::parse(&source_text)
                .ok()
                .and_then(|document| {
                    document
                        .symbols()
                        .declared_program_id
                        .as_ref()
                        .map(|declared| declared.value.clone())
                })
        });
    let kind = classify_program(&manifest, &source_text)?;
    let raw_name = manifest.lib_name.as_ref().or(manifest.name.as_ref())?;
    let name = project::normalize_program_name(raw_name);
    let metadata_uri = Url::from_file_path(path).ok()?;
    Some(SolanaProgram {
        kind,
        root,
        source_root,
        name,
        id,
        cluster: None,
        metadata_uri,
        metadata_range: Range::default(),
    })
}

fn classify_program(manifest: &CargoManifest, source_text: &str) -> Option<SolanaProjectKind> {
    if !manifest_is_program_library(manifest) {
        return None;
    }

    if manifest
        .dependencies
        .iter()
        .any(|dependency| PINOCCHIO_DEPENDENCIES.contains(&dependency.as_str()))
        || PINOCCHIO_SOURCE_HINTS
            .iter()
            .any(|hint| source_text.contains(hint))
    {
        return Some(SolanaProjectKind::Pinocchio);
    }
    let has_native_dependency = manifest.dependencies.iter().any(|dependency| {
        matches!(
            dependency.as_str(),
            "solana-program"
                | "solana-account-info"
                | "solana-program-entrypoint"
                | "solana-cpi"
                | "solana-pubkey"
        )
    });
    let has_native_entrypoint = source_text.contains("entrypoint!")
        || source_text.contains("process_instruction")
        || source_text.contains(idioms::DECLARE_ID_MACRO_INVOCATION);
    if (has_native_dependency && has_native_entrypoint)
        || source_text.contains("solana_program::entrypoint")
    {
        return Some(SolanaProjectKind::NativeSolana);
    }
    None
}

pub fn classify_manifest_text(manifest_text: &str, source_text: &str) -> Option<SolanaProjectKind> {
    let manifest = parse_cargo_manifest(manifest_text)?;
    classify_program(&manifest, source_text)
}

fn manifest_is_program_library(manifest: &CargoManifest) -> bool {
    manifest
        .lib_crate_types
        .iter()
        .any(|crate_type| crate_type == "cdylib")
}

fn manifest_or_source_declares_anchor(manifest: &CargoManifest, document: &ParsedDocument) -> bool {
    manifest.dependencies.contains_crate(ANCHOR_LANG_DEPENDENCY)
        || document.syntax().items.iter().any(|item| {
            matches!(
                item,
                syn::Item::Mod(item_mod)
                    if item_mod
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident(ANCHOR_PROGRAM_ATTRIBUTE))
            )
        })
}

pub fn parse_manifest_deps(manifest_text: &str) -> CargoManifestDeps {
    Manifest::from_str(manifest_text)
        .map(|manifest| {
            let mut dependencies = CargoManifestDeps::default();
            extend_all_dependency_names(&manifest, &mut dependencies);
            dependencies
        })
        .unwrap_or_default()
}

fn parse_cargo_manifest(text: &str) -> Option<CargoManifest> {
    let manifest = Manifest::from_str(text).ok()?;
    let mut dependencies = CargoManifestDeps::default();
    extend_all_dependency_names(&manifest, &mut dependencies);

    Some(CargoManifest {
        name: manifest
            .package
            .as_ref()
            .map(|package| package.name.clone()),
        lib_name: manifest
            .lib
            .as_ref()
            .and_then(|lib| lib.name.as_ref())
            .cloned(),
        lib_crate_types: manifest
            .lib
            .as_ref()
            .map(|lib| lib.crate_type.clone())
            .unwrap_or_default(),
        dependencies,
    })
}

fn extend_all_dependency_names(manifest: &Manifest, dependencies: &mut CargoManifestDeps) {
    extend_dependency_names(&manifest.dependencies, dependencies);
    extend_dependency_names(&manifest.dev_dependencies, dependencies);
    extend_dependency_names(&manifest.build_dependencies, dependencies);
    for target in manifest.target.values() {
        extend_dependency_names(&target.dependencies, dependencies);
        extend_dependency_names(&target.dev_dependencies, dependencies);
        extend_dependency_names(&target.build_dependencies, dependencies);
    }
}

fn extend_dependency_names(dependencies: &DepsSet, names: &mut CargoManifestDeps) {
    for (name, dependency) in dependencies {
        let package = dependency
            .detail()
            .and_then(|detail| detail.package.as_ref())
            .cloned();
        names.insert_dependency(name.clone(), package);
    }
}

fn normalize_dependency_name(name: &str) -> String {
    name.replace('_', "-")
}

pub fn nearest_manifest(uri: &Url) -> Option<(Url, String)> {
    let path = nearest_manifest_path(uri)?;
    let text = file_text::read_limited_text(&path).ok().flatten()?;
    let uri = Url::from_file_path(path).ok()?;
    Some((uri, text))
}

/// Locates the nearest workspace-root `Cargo.toml` for the package that owns
/// `uri`. This mirrors package manifest lookup, but only returns manifests with
/// a `[workspace]` section.
pub fn nearest_workspace_manifest(uri: &Url) -> Option<(Url, String)> {
    let mut path = uri.to_file_path().ok()?;
    if path.is_file() {
        path.pop();
    }

    loop {
        let manifest_path = path.join("Cargo.toml");
        if manifest_path.is_file() {
            if let Some(text) = file_text::read_limited_text(&manifest_path).ok().flatten() {
                if Manifest::from_str(&text).is_ok_and(|manifest| manifest.workspace.is_some()) {
                    let uri = Url::from_file_path(manifest_path).ok()?;
                    return Some((uri, text));
                }
            }
        }
        if !path.pop() {
            return None;
        }
    }
}

fn nearest_manifest_path(uri: &Url) -> Option<PathBuf> {
    let mut path = uri.to_file_path().ok()?;
    if path.is_file() {
        path.pop();
    }

    loop {
        let manifest_path = path.join("Cargo.toml");
        if manifest_path.is_file() {
            return Some(manifest_path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn cargo_build_root(package_root: &Path) -> Option<PathBuf> {
    let mut current = Some(package_root);
    while let Some(path) = current {
        let manifest_path = path.join("Cargo.toml");
        if manifest_path.is_file()
            && file_text::read_limited_text(&manifest_path)
                .ok()
                .flatten()
                .and_then(|text| Manifest::from_str(&text).ok())
                .is_some_and(|manifest| manifest.workspace.is_some())
        {
            return Some(path.to_path_buf());
        }
        current = path.parent();
    }
    None
}

fn read_package_source(source_root: &Path) -> Option<String> {
    file_text::read_limited_text(&source_root.join("lib.rs"))
        .ok()
        .flatten()
}

fn cargo_manifest_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_files_named(root, "Cargo.toml", 0, MAX_MANIFEST_DEPTH, &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn anchor_toml_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_files_named(root, "Anchor.toml", 0, MAX_MANIFEST_DEPTH, &mut paths);
    paths.sort();
    paths.dedup();
    paths
}

fn collect_files_named(
    root: &Path,
    name: &str,
    depth: usize,
    max_depth: usize,
    paths: &mut Vec<PathBuf>,
) {
    if depth > max_depth {
        return;
    }
    let direct = root.join(name);
    if direct.is_file() {
        paths.push(direct);
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut dirs = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && !should_skip_dir(path))
        .collect::<Vec<_>>();
    dirs.sort();
    for dir in dirs {
        collect_files_named(&dir, name, depth + 1, max_depth, paths);
    }
}

fn program_source_root(root: &Path, program_name: &str) -> Option<PathBuf> {
    let programs = root.join("programs");
    let direct = programs.join(program_name).join("src");
    if direct.is_dir() {
        return Some(direct);
    }

    let entries = fs::read_dir(programs).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if project::normalize_program_name(name) == program_name {
            let source_root = path.join("src");
            if source_root.is_dir() {
                return Some(source_root);
            }
        }
    }
    None
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "node_modules" | "target" | ".anchor")
    )
}

#[cfg(test)]
mod tests;
