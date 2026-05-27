use {
    super::{push_unique_symbol, BridgeSymbol},
    std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::{Location, Range, SymbolKind, Url},
};

const MAX_ARTIFACT_FILES_PER_ROOT: usize = 96;

pub(super) fn collect_artifact_symbols(
    root: &Path,
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    for artifact in artifact_paths(root)
        .into_iter()
        .take(MAX_ARTIFACT_FILES_PER_ROOT)
    {
        let Ok(uri) = Url::from_file_path(&artifact.path) else {
            continue;
        };
        push_unique_symbol(
            symbols,
            seen,
            BridgeSymbol {
                name: artifact.name,
                kind: artifact.kind,
                location: Location {
                    uri,
                    range: Range::default(),
                },
                container_name: Some(artifact.container_name),
                type_display: Some(artifact.type_display),
            },
        );
    }
}

struct ArtifactBridgePath {
    path: PathBuf,
    name: String,
    kind: SymbolKind,
    container_name: String,
    type_display: String,
}

fn artifact_paths(root: &Path) -> Vec<ArtifactBridgePath> {
    let mut artifacts = Vec::new();
    collect_artifacts_from_dir(
        &root.join("target").join("types"),
        "ts",
        SymbolKind::FILE,
        "Anchor TypeScript artifacts",
        "generated client types",
        &mut artifacts,
    );
    collect_artifacts_from_dir(
        &root.join("target").join("deploy"),
        "so",
        SymbolKind::FILE,
        "Anchor SBPF artifacts",
        "SBPF ELF artifact",
        &mut artifacts,
    );
    collect_keypair_artifacts(&root.join("target").join("deploy"), &mut artifacts);
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    artifacts
}

fn collect_artifacts_from_dir(
    dir: &Path,
    extension: &str,
    kind: SymbolKind,
    container_name: &'static str,
    type_display: &'static str,
    artifacts: &mut Vec<ArtifactBridgePath>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some(extension) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let name = name.to_string();
        artifacts.push(ArtifactBridgePath {
            path,
            name,
            kind,
            container_name: container_name.to_string(),
            type_display: type_display.to_string(),
        });
    }
}

fn collect_keypair_artifacts(dir: &Path, artifacts: &mut Vec<ArtifactBridgePath>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with("-keypair.json") {
            continue;
        }
        let name = name.to_string();
        artifacts.push(ArtifactBridgePath {
            path,
            name,
            kind: SymbolKind::CONSTANT,
            container_name: "Anchor program keypairs".to_string(),
            type_display: "program keypair".to_string(),
        });
    }
}
