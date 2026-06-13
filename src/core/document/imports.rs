use {
    super::NamedRange,
    crate::range::range_from_span,
    std::collections::{HashMap, HashSet},
    syn::UseTree,
};

const LOCAL_GLOB_ROOTS: &[&str] = &["crate", "super", "self"];

/// Walks a `use` tree, recording every path segment as an imported name and
/// every `Original as Alias` rename as an alias→original mapping.
pub(super) fn collect_imported_names(
    tree: &UseTree,
    names: &mut Vec<NamedRange>,
    aliases: &mut HashMap<String, String>,
    origins: &mut HashMap<String, String>,
) {
    collect_imported_names_with_root(tree, names, aliases, origins, None);
}

fn collect_imported_names_with_root(
    tree: &UseTree,
    names: &mut Vec<NamedRange>,
    aliases: &mut HashMap<String, String>,
    origins: &mut HashMap<String, String>,
    root: Option<&syn::Ident>,
) {
    match tree {
        UseTree::Name(name) => {
            let imported_name = name.ident.to_string();
            names.push(NamedRange {
                name: imported_name.clone(),
                range: range_from_span(name.ident.span()),
            });
            record_import_origin(origins, &imported_name, root);
        }
        UseTree::Rename(rename) => {
            let alias = rename.rename.to_string();
            names.push(NamedRange {
                name: alias.clone(),
                range: range_from_span(rename.rename.span()),
            });
            aliases.insert(alias.clone(), rename.ident.to_string());
            record_import_origin(origins, &alias, root);
        }
        UseTree::Path(path) => {
            names.push(NamedRange {
                name: path.ident.to_string(),
                range: range_from_span(path.ident.span()),
            });
            collect_imported_names_with_root(
                &path.tree,
                names,
                aliases,
                origins,
                root.or(Some(&path.ident)),
            );
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_imported_names_with_root(tree, names, aliases, origins, root);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn record_import_origin(
    origins: &mut HashMap<String, String>,
    imported_name: &str,
    root: Option<&syn::Ident>,
) {
    if let Some(root) = root {
        origins.insert(imported_name.to_string(), root.to_string());
    }
}

pub(super) fn has_glob_in_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Glob(_) => true,
        UseTree::Path(path) => has_glob_in_use_tree(&path.tree),
        UseTree::Group(group) => group.items.iter().any(has_glob_in_use_tree),
        UseTree::Name(_) | UseTree::Rename(_) => false,
    }
}

pub(super) fn has_local_glob_in_use_tree(
    tree: &UseTree,
    local_module_names: &HashSet<String>,
) -> bool {
    glob_root_segments(tree)
        .iter()
        .any(|root| LOCAL_GLOB_ROOTS.contains(&root.as_str()) || local_module_names.contains(root))
}

fn glob_root_segments(tree: &UseTree) -> Vec<String> {
    let mut roots = Vec::new();
    collect_glob_root_segments(tree, None, &mut roots);
    roots
}

fn collect_glob_root_segments(
    tree: &UseTree,
    current_root: Option<&syn::Ident>,
    roots: &mut Vec<String>,
) {
    match tree {
        UseTree::Glob(_) => {
            if let Some(root) = current_root {
                roots.push(root.to_string());
            }
        }
        UseTree::Path(path) => {
            collect_glob_root_segments(&path.tree, current_root.or(Some(&path.ident)), roots);
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_glob_root_segments(tree, current_root, roots);
            }
        }
        UseTree::Name(_) | UseTree::Rename(_) => {}
    }
}
