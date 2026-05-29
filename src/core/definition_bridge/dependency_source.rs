use {
    super::{paths, push_unique_symbol, BridgeSymbol},
    crate::range::range_from_span,
    quote::ToTokens,
    std::{
        collections::HashSet,
        env, fs,
        path::{Path, PathBuf},
    },
    syn::spanned::Spanned,
    tower_lsp::lsp_types::{Location, SymbolKind, Url},
};

const MAX_DEPENDENCY_CRATES: usize = 24;
const MAX_SOURCE_FILES_PER_CRATE: usize = 96;
const MAX_FILE_BYTES: u64 = 512 * 1024;

pub(super) fn collect_dependency_source_symbols(
    source_roots: &[PathBuf],
    symbols: &mut Vec<BridgeSymbol>,
    seen: &mut HashSet<String>,
) {
    if source_roots.is_empty() {
        return;
    }
    for source_root in source_roots.iter().take(MAX_DEPENDENCY_CRATES) {
        let crate_name = source_root
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("dependency")
            .to_string();
        for path in paths::rust_source_files(source_root)
            .into_iter()
            .take(MAX_SOURCE_FILES_PER_CRATE)
        {
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if metadata.len() > MAX_FILE_BYTES {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(file) = syn::parse_file(&text) else {
                continue;
            };
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            for symbol in public_source_symbols(&file, &uri, &crate_name) {
                push_unique_symbol(symbols, seen, symbol);
            }
        }
    }
}

pub(super) fn dependency_manifests(roots: &[Url]) -> Vec<(PathBuf, String)> {
    let mut manifests = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let Ok(root_path) = root.to_file_path() else {
            continue;
        };
        for manifest in paths::cargo_manifest_paths(&root_path) {
            if !seen.insert(manifest.clone()) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&manifest) else {
                continue;
            };
            manifests.push((manifest, text));
        }
    }
    manifests
}

pub(super) fn dependency_names(manifests: &[(PathBuf, String)]) -> Vec<String> {
    let mut names = HashSet::new();
    for (_, text) in manifests {
        names.extend(parse_dependency_names(text));
    }
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();
    names
}

pub(super) fn dependency_source_roots(
    manifests: &[(PathBuf, String)],
    dependency_names: &[String],
) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    roots.extend(path_dependency_source_roots(manifests));
    for source_root in local_anchor_dependency_roots() {
        if source_root.is_dir() {
            roots.push(source_root);
        }
    }

    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return roots;
    };
    let registry_src = home.join(".cargo").join("registry").join("src");
    let Ok(registries) = fs::read_dir(registry_src) else {
        return roots;
    };
    for registry in registries.flatten() {
        let Ok(entries) = fs::read_dir(registry.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if dependency_names.iter().any(|dependency| {
                dir_name == dependency || dir_name.starts_with(&format!("{dependency}-"))
            }) {
                let src = path.join("src");
                if src.is_dir() {
                    roots.push(src);
                }
            }
        }
    }
    dedupe_paths(roots)
}

pub(super) fn public_source_symbols(
    file: &syn::File,
    uri: &Url,
    crate_name: &str,
) -> Vec<BridgeSymbol> {
    let mut symbols = Vec::new();
    for item in &file.items {
        match item {
            syn::Item::Struct(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::STRUCT,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    None,
                ));
                symbols.extend(public_struct_field_symbols(item, uri));
            }
            syn::Item::Enum(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::ENUM,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    None,
                ));
                symbols.extend(enum_variant_symbols(item, uri));
            }
            syn::Item::Trait(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::INTERFACE,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    None,
                ));
            }
            syn::Item::Type(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::TYPE_PARAMETER,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    Some(type_display(&item.ty)),
                ));
            }
            syn::Item::Fn(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.sig.ident,
                    SymbolKind::FUNCTION,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    None,
                ));
            }
            syn::Item::Mod(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::MODULE,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    None,
                ));
            }
            syn::Item::Const(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::CONSTANT,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    Some(type_display(&item.ty)),
                ));
            }
            syn::Item::Static(item) if is_public(&item.vis) => {
                symbols.push(source_symbol(
                    &item.ident,
                    SymbolKind::CONSTANT,
                    item.span(),
                    uri,
                    crate_name,
                    None,
                    Some(type_display(&item.ty)),
                ));
            }
            syn::Item::Impl(item) => {
                symbols.extend(public_impl_item_symbols(item, uri));
            }
            _ => {}
        }
    }
    symbols
}

fn public_struct_field_symbols(item: &syn::ItemStruct, uri: &Url) -> Vec<BridgeSymbol> {
    let container_name = item.ident.to_string();
    match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .filter(|field| is_public(&field.vis))
            .filter_map(|field| {
                let ident = field.ident.as_ref()?;
                Some(BridgeSymbol {
                    name: ident.to_string(),
                    kind: SymbolKind::FIELD,
                    location: Location {
                        uri: uri.clone(),
                        range: range_from_span(ident.span()),
                    },
                    container_name: Some(container_name.clone()),
                    type_display: Some(type_display(&field.ty)),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn enum_variant_symbols(item: &syn::ItemEnum, uri: &Url) -> Vec<BridgeSymbol> {
    let container_name = item.ident.to_string();
    item.variants
        .iter()
        .map(|variant| BridgeSymbol {
            name: variant.ident.to_string(),
            kind: SymbolKind::ENUM_MEMBER,
            location: Location {
                uri: uri.clone(),
                range: range_from_span(variant.ident.span()),
            },
            container_name: Some(container_name.clone()),
            type_display: None,
        })
        .collect()
}

fn public_impl_item_symbols(item: &syn::ItemImpl, uri: &Url) -> Vec<BridgeSymbol> {
    let Some(container_name) = impl_container_name(&item.self_ty) else {
        return Vec::new();
    };
    item.items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Const(item) if is_public(&item.vis) => Some(BridgeSymbol {
                name: item.ident.to_string(),
                kind: SymbolKind::CONSTANT,
                location: Location {
                    uri: uri.clone(),
                    range: range_from_span(item.ident.span()),
                },
                container_name: Some(container_name.clone()),
                type_display: Some(type_display(&item.ty)),
            }),
            syn::ImplItem::Fn(item) if is_public(&item.vis) => Some(BridgeSymbol {
                name: item.sig.ident.to_string(),
                kind: SymbolKind::METHOD,
                location: Location {
                    uri: uri.clone(),
                    range: range_from_span(item.sig.ident.span()),
                },
                container_name: Some(container_name.clone()),
                type_display: None,
            }),
            syn::ImplItem::Type(item) if is_public(&item.vis) => Some(BridgeSymbol {
                name: item.ident.to_string(),
                kind: SymbolKind::TYPE_PARAMETER,
                location: Location {
                    uri: uri.clone(),
                    range: range_from_span(item.ident.span()),
                },
                container_name: Some(container_name.clone()),
                type_display: Some(type_display(&item.ty)),
            }),
            _ => None,
        })
        .collect()
}

fn source_symbol(
    ident: &syn::Ident,
    kind: SymbolKind,
    span: proc_macro2::Span,
    uri: &Url,
    crate_name: &str,
    container_name: Option<String>,
    type_display: Option<String>,
) -> BridgeSymbol {
    BridgeSymbol {
        name: ident.to_string(),
        kind,
        location: Location {
            uri: uri.clone(),
            range: range_from_span(span),
        },
        container_name: container_name.or_else(|| Some(format!("dependency {crate_name}"))),
        type_display,
    }
}

fn impl_container_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn type_display(ty: &syn::Type) -> String {
    ty.to_token_stream().to_string().replace(" :: ", "::")
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

pub(super) fn parse_dependency_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_dependency_section = false;
    for line in text.lines() {
        let line = strip_inline_comment(line).trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dependency_section = matches!(
                line,
                "[dependencies]" | "[dev-dependencies]" | "[build-dependencies]"
            ) || line.starts_with("[target.")
                && line.ends_with(".dependencies]");
            continue;
        }
        if !in_dependency_section || line.is_empty() {
            continue;
        }
        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().trim_matches('"').to_string();
        if is_relevant_dependency(&name) {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    names
}

fn is_relevant_dependency(name: &str) -> bool {
    name == "anchor-lang"
        || name == "anchor-spl"
        || name == "solana-program"
        || name.starts_with("spl-")
        || name.starts_with("mpl-")
}

fn path_dependency_source_roots(manifests: &[(PathBuf, String)]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for (manifest, text) in manifests {
        let Some(manifest_dir) = manifest.parent() else {
            continue;
        };
        for path in parse_relevant_path_dependencies(text) {
            let source_root = manifest_dir.join(path).join("src");
            if source_root.is_dir() {
                roots.push(source_root);
            }
        }
    }
    roots
}

pub(super) fn parse_relevant_path_dependencies(text: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut in_dependency_section = false;
    for line in text.lines() {
        let line = strip_inline_comment(line).trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dependency_section = matches!(
                line,
                "[dependencies]" | "[dev-dependencies]" | "[build-dependencies]"
            ) || line.starts_with("[target.")
                && line.ends_with(".dependencies]");
            continue;
        }
        if !in_dependency_section || line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().trim_matches('"');
        if !is_relevant_dependency(name) {
            continue;
        }
        if let Some(path) = inline_table_path_value(value) {
            paths.push(PathBuf::from(path));
        }
    }
    paths
}

fn inline_table_path_value(value: &str) -> Option<String> {
    let path_start = value.find("path")?;
    let after_path = &value[path_start + "path".len()..];
    let after_equals = after_path.split_once('=')?.1.trim_start();
    let after_quote = after_equals.strip_prefix('"')?;
    let end = after_quote.find('"')?;
    Some(after_quote[..end].to_string())
}

fn local_anchor_dependency_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        roots.push(cwd.join("lang").join("src"));
        roots.push(cwd.join("spl").join("src"));
    }
    roots
}

fn strip_inline_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
        } else if ch == '#' {
            return &line[..idx];
        }
    }
    line
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}
