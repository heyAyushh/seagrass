use {
    crate::project,
    std::{collections::HashSet, fs, path::Path},
    tower_lsp::lsp_types::{Location, SymbolKind, Url},
};

mod artifacts;
mod dependency_source;
mod generated;
mod paths;

#[cfg(test)]
use {
    dependency_source::{
        parse_dependency_names, parse_relevant_path_dependencies, public_source_symbols,
    },
    std::{env, path::PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
    pub type_display: Option<String>,
}

pub fn collect(roots: &[Url]) -> Vec<BridgeSymbol> {
    let mut symbols = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        let Ok(root_path) = root.to_file_path() else {
            continue;
        };
        collect_project_metadata_symbols(&root_path, &mut symbols, &mut seen);
        generated::collect_idl_symbols(&root_path, &mut symbols, &mut seen);
        artifacts::collect_artifact_symbols(&root_path, &mut symbols, &mut seen);
    }

    let dependency_manifests = dependency_source::dependency_manifests(roots);
    let dependency_names = dependency_source::dependency_names(&dependency_manifests);
    let dependency_roots =
        dependency_source::dependency_source_roots(&dependency_manifests, &dependency_names);
    dependency_source::collect_dependency_source_symbols(
        &dependency_roots,
        &mut symbols,
        &mut seen,
    );
    symbols
}

fn collect_project_metadata_symbols(
    root: &Path,
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    for anchor_toml_path in paths::anchor_toml_paths(root) {
        let Ok(text) = fs::read_to_string(&anchor_toml_path) else {
            continue;
        };
        let Ok(uri) = Url::from_file_path(&anchor_toml_path) else {
            continue;
        };
        for program in project::parse_anchor_toml(&text).programs {
            push_unique_symbol(
                symbols,
                seen,
                BridgeSymbol {
                    name: program.name,
                    kind: SymbolKind::CONSTANT,
                    location: Location {
                        uri: uri.clone(),
                        range: program.range,
                    },
                    container_name: Some(format!("Anchor.toml [{}]", program.cluster)),
                    type_display: Some(program.value),
                },
            );
        }
    }
}

fn push_unique_symbol(
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
    symbol: BridgeSymbol,
) {
    let key = format!(
        "{}\0{:?}\0{}\0{}:{}:{}",
        symbol.name,
        symbol.kind,
        symbol.location.uri,
        symbol.location.range.start.line,
        symbol.location.range.start.character,
        symbol.location.range.end.character
    );
    if seen.insert(key) {
        symbols.push(symbol);
    }
}

#[cfg(test)]
mod tests;
