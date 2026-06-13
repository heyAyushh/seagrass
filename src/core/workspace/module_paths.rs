//! Module-path segment derivation for the workspace trie.
//!
//! Given a source file URI this module computes the `crate::…` module-path
//! segments that Rust assigns to that file, by locating the `src/` ancestor
//! directory in the file path.
//!
//! Rules (examples for a file at `…/programs/demo/src/`):
//!  - `src/lib.rs`              → `["crate"]`
//!  - `src/state.rs`            → `["crate", "state"]`
//!  - `src/instructions/mod.rs` → `["crate", "instructions"]`
//!  - `src/instructions/make.rs`→ `["crate", "instructions", "make"]`

use tower_lsp::lsp_types::Url;

/// The sentinel stem that the Rust module system treats as a crate root or
/// sub-module root (rather than a leaf module named `"lib"`).
const MOD_RS_STEM: &str = "mod";
/// `lib.rs` is the crate root — it does not add a `lib` segment.
const LIB_RS_STEM: &str = "lib";
/// `src` directory name that marks the boundary between workspace layout and
/// module hierarchy.
const SRC_DIR: &str = "src";

/// Derive the `crate::…` module-path segments for `file_uri` by finding the
/// `src/` ancestor directory in the file path.
///
/// Returns `None` when `file_uri` cannot be mapped (e.g. no `src/` ancestor or
/// a non-identifier path segment).
pub(super) fn module_path_segments(file_uri: &Url) -> Option<Vec<String>> {
    let all_components: Vec<&str> = file_uri
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();

    // Find the index of the last `src` component — the one directly above
    // the module files.  We take the *last* occurrence to handle layouts like
    // `workspace/programs/demo/src/…` correctly.
    let src_index = all_components
        .iter()
        .rposition(|component| *component == SRC_DIR)?;

    // Components after `src/` are the module path components.
    let module_components = &all_components[src_index + 1..];

    let mut segments = vec![super::CRATE_ROOT_SEGMENT.to_string()];

    let last_index = module_components.len().checked_sub(1)?;

    for (index, &name) in module_components.iter().enumerate() {
        if index == last_index {
            // Last component is the filename — strip `.rs` and skip the
            // crate/module root files that don't add a new segment.
            let stem = std::path::Path::new(name).file_stem()?.to_str()?;
            if stem == LIB_RS_STEM || stem == MOD_RS_STEM || stem == "main" {
                break;
            }
            if !is_valid_module_name(stem) {
                return None;
            }
            segments.push(stem.to_string());
        } else {
            // Intermediate directory — becomes a module-path segment.
            if !is_valid_module_name(name) {
                return None;
            }
            segments.push(name.to_string());
        }
    }

    Some(segments)
}

/// A module name is a valid Rust identifier: starts with a letter or `_`,
/// followed by letters, digits, or `_`.
fn is_valid_module_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(path: &str) -> Url {
        Url::parse(&format!("file://{path}")).unwrap()
    }

    #[test]
    fn lib_rs_maps_to_crate_root() {
        let segments = module_path_segments(&url("/workspace/src/lib.rs"));
        assert_eq!(segments, Some(vec!["crate".to_string()]));
    }

    #[test]
    fn top_level_module_maps_to_crate_plus_stem() {
        let segments = module_path_segments(&url("/workspace/src/state.rs"));
        assert_eq!(
            segments,
            Some(vec!["crate".to_string(), "state".to_string()])
        );
    }

    #[test]
    fn nested_module_file_maps_to_full_path() {
        let segments = module_path_segments(&url("/workspace/src/instructions/make.rs"));
        assert_eq!(
            segments,
            Some(vec![
                "crate".to_string(),
                "instructions".to_string(),
                "make".to_string(),
            ])
        );
    }

    #[test]
    fn mod_rs_does_not_add_extra_segment() {
        let segments = module_path_segments(&url("/workspace/src/instructions/mod.rs"));
        assert_eq!(
            segments,
            Some(vec!["crate".to_string(), "instructions".to_string()])
        );
    }

    #[test]
    fn programs_anchor_layout_is_handled() {
        // The canonical Anchor layout has `programs/<name>/src/<file>.rs`.
        // There is a `src` component in the path, so the function finds it.
        let segments = module_path_segments(&url("/workspace/programs/demo/src/state.rs"));
        assert_eq!(
            segments,
            Some(vec!["crate".to_string(), "state".to_string()])
        );
    }

    #[test]
    fn file_with_no_src_ancestor_returns_none() {
        // A path with no `src` component cannot be mapped to a crate module path.
        let segments = module_path_segments(&url("/other/state.rs"));
        assert_eq!(segments, None);
    }
}
