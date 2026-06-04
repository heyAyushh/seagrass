use {super::NamedRange, crate::range::range_from_span, std::collections::HashMap, syn::UseTree};

/// Walks a `use` tree, recording every path segment as an imported name and
/// every `Original as Alias` rename as an alias→original mapping.
pub(super) fn collect_imported_names(
    tree: &UseTree,
    names: &mut Vec<NamedRange>,
    aliases: &mut HashMap<String, String>,
) {
    match tree {
        UseTree::Name(name) => names.push(NamedRange {
            name: name.ident.to_string(),
            range: range_from_span(name.ident.span()),
        }),
        UseTree::Rename(rename) => {
            names.push(NamedRange {
                name: rename.rename.to_string(),
                range: range_from_span(rename.rename.span()),
            });
            aliases.insert(rename.rename.to_string(), rename.ident.to_string());
        }
        UseTree::Path(path) => {
            names.push(NamedRange {
                name: path.ident.to_string(),
                range: range_from_span(path.ident.span()),
            });
            collect_imported_names(&path.tree, names, aliases);
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_imported_names(tree, names, aliases);
            }
        }
        UseTree::Glob(_) => {}
    }
}
