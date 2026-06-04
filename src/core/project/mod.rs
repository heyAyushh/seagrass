use {
    crate::{document::ParsedDocument, file_text, program_artifacts},
    std::path::{Path, PathBuf},
    tower_lsp::lsp_types::{Position, Range, Url},
};

const ANCHOR_TOML_FILE: &str = "Anchor.toml";
const SEAGRASS_TOML_FILE: &str = "Seagrass.toml";

/// Upper bound on the number of ancestor directories scanned while locating a
/// workspace configuration file. Bounds the walk so a deeply nested path or a
/// symlink loop cannot drive an unbounded ascent toward the filesystem root.
const MAX_WORKSPACE_ASCENT: usize = 64;

#[derive(Debug, Default, Clone)]
pub struct AnchorToml {
    pub provider_cluster: Option<String>,
    pub programs: Vec<AnchorProgramId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorProgramId {
    pub cluster: String,
    pub name: String,
    pub value: String,
    pub range: Range,
}

pub fn parse_anchor_toml(source: &str) -> AnchorToml {
    let mut current_cluster = None;
    let mut in_provider = false;
    let mut provider_cluster = None;
    let mut programs = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = strip_inline_comment(line).trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_cluster = trimmed
                .strip_prefix("[programs.")
                .and_then(|rest| rest.strip_suffix(']'))
                .map(str::to_string);
            in_provider = trimmed == "[provider]";
            continue;
        }

        let Some((raw_name, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        let name = raw_name.trim();
        if in_provider && name == "cluster" {
            provider_cluster = quoted_value(raw_value);
            continue;
        }

        let Some(cluster) = current_cluster.as_ref() else {
            continue;
        };
        let Some(value) = quoted_value(raw_value) else {
            continue;
        };

        let value_start = line.find(&value).unwrap_or_default();
        let line = u32::try_from(line_idx).unwrap_or_default();
        let start = u32::try_from(value_start).unwrap_or_default();
        let end = start + u32::try_from(value.chars().count()).unwrap_or_default();

        programs.push(AnchorProgramId {
            cluster: cluster.clone(),
            name: normalize_program_name(name),
            value,
            range: Range {
                start: Position {
                    line,
                    character: start,
                },
                end: Position {
                    line,
                    character: end,
                },
            },
        });
    }

    AnchorToml {
        provider_cluster,
        programs,
    }
}

pub fn nearest_anchor_toml(uri: &Url) -> Option<(Url, String)> {
    nearest_workspace_file(uri, ANCHOR_TOML_FILE, &[])
}

pub fn nearest_seagrass_toml(uri: &Url) -> Option<(Url, String)> {
    nearest_workspace_file(uri, SEAGRASS_TOML_FILE, &[])
}

pub fn nearest_anchor_toml_with_roots(uri: &Url, workspace_roots: &[Url]) -> Option<(Url, String)> {
    nearest_workspace_file(uri, ANCHOR_TOML_FILE, workspace_roots)
}

pub fn nearest_seagrass_toml_with_roots(
    uri: &Url,
    workspace_roots: &[Url],
) -> Option<(Url, String)> {
    nearest_workspace_file(uri, SEAGRASS_TOML_FILE, workspace_roots)
}

fn nearest_workspace_file(
    uri: &Url,
    file_name: &str,
    workspace_roots: &[Url],
) -> Option<(Url, String)> {
    let mut path = uri.to_file_path().ok()?;
    if path.is_file() {
        path.pop();
    }
    let root_paths = file_workspace_roots(workspace_roots);
    if !root_paths.is_empty() && !is_within_workspace_roots(&path, &root_paths) {
        return None;
    }

    for _ in 0..MAX_WORKSPACE_ASCENT {
        // A config found directly in a shared directory (filesystem root, the
        // user's home, or world-writable parents like /tmp) is never a real
        // project root. Treat it as absent so a hostile project cannot plant
        // `Anchor.toml`/`Seagrass.toml` in a shared location and have it
        // silently applied to unrelated files opened beneath it.
        if is_shared_directory(&path) {
            return None;
        }
        let workspace_file_path: PathBuf = path.join(file_name);
        if workspace_file_path.is_file() {
            let text = file_text::read_limited_text(&workspace_file_path)
                .ok()
                .flatten()?;
            let workspace_file_uri = Url::from_file_path(&workspace_file_path).ok()?;
            return Some((workspace_file_uri, text));
        }
        if is_workspace_root(&path, &root_paths) {
            return None;
        }
        if !path.pop() {
            return None;
        }
    }
    None
}

fn file_workspace_roots(workspace_roots: &[Url]) -> Vec<PathBuf> {
    workspace_roots
        .iter()
        .filter_map(|root| root.to_file_path().ok())
        .collect()
}

fn is_within_workspace_roots(path: &Path, workspace_roots: &[PathBuf]) -> bool {
    workspace_roots.iter().any(|root| path.starts_with(root))
}

fn is_workspace_root(path: &Path, workspace_roots: &[PathBuf]) -> bool {
    workspace_roots.iter().any(|root| path == root.as_path())
}

/// Directories that must never be treated as a workspace root. Reading a
/// configuration file located directly in one of these would let an attacker
/// influence analysis of any file opened beneath a shared path.
fn is_shared_directory(path: &Path) -> bool {
    // The filesystem root has no parent component.
    if path.parent().is_none() {
        return true;
    }
    if let Some(home) = std::env::var_os("HOME") {
        if path == Path::new(&home) {
            return true;
        }
    }
    let shared_parents = [
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
        PathBuf::from("/private/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/private/var/tmp"),
        PathBuf::from("/home"),  // parent of Linux home directories
        PathBuf::from("/Users"), // parent of macOS home directories
    ];
    shared_parents.iter().any(|shared| path == shared.as_path())
}

pub fn program_name_from_uri(uri: &Url) -> Option<String> {
    let path = uri.to_file_path().ok()?;
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str().map(str::to_string))
        .collect::<Vec<_>>();

    components.windows(3).find_map(|window| match window {
        [programs, name, src] if programs == "programs" && src == "src" => {
            Some(normalize_program_name(name))
        }
        _ => None,
    })
}

pub fn matching_program_ids(uri: &Url, anchor_toml_text: &str) -> Vec<AnchorProgramId> {
    let Some(program_name) = program_name_from_uri(uri) else {
        return Vec::new();
    };
    parse_anchor_toml(anchor_toml_text)
        .programs
        .into_iter()
        .filter(|program| program.name == program_name)
        .collect()
}

pub fn preferred_program_id(uri: &Url, anchor_toml_text: &str) -> Option<AnchorProgramId> {
    let parsed = parse_anchor_toml(anchor_toml_text);
    let program_name = program_name_from_uri(uri)?;
    let matches = parsed
        .programs
        .into_iter()
        .filter(|program| program.name == program_name)
        .collect::<Vec<_>>();
    if let Some(provider_cluster) = parsed.provider_cluster.as_deref() {
        if let Some(program) = matches
            .iter()
            .find(|program| program.cluster == provider_cluster)
            .cloned()
        {
            return Some(program);
        }
    }
    if let Some(program) = matches
        .iter()
        .find(|program| program.cluster == "localnet")
        .cloned()
    {
        return Some(program);
    }
    matches.into_iter().next()
}

pub fn summary(
    uri: &Url,
    document: &ParsedDocument,
    anchor_toml_uri: &Url,
    anchor_toml_text: &str,
) -> serde_json::Value {
    let matching = matching_program_ids(uri, anchor_toml_text);
    let artifacts =
        program_artifacts::report_for_document(uri, document).map(|report| report.to_json());
    serde_json::json!({
        "anchorToml": anchor_toml_uri,
        "programName": program_name_from_uri(uri),
        "providerCluster": parse_anchor_toml(anchor_toml_text).provider_cluster,
        "artifacts": artifacts,
        "declareId": document
            .symbols()
            .declared_program_id
            .as_ref()
            .map(|declared| declared.value.as_str()),
        "anchorTomlProgramIds": matching.iter().map(|program| {
            serde_json::json!({
                "cluster": program.cluster,
                "name": program.name,
                "value": program.value,
            })
        }).collect::<Vec<_>>(),
    })
}

fn quoted_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let start = trimmed.find('"')? + 1;
    let rest = &trimmed[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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

pub(crate) fn normalize_program_name(name: &str) -> String {
    name.trim().trim_matches('"').replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_program_ids_from_anchor_toml() {
        let parsed = parse_anchor_toml(
            r#"
[provider]
cluster = "localnet"

[programs.localnet]
basic_1 = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"

[programs.devnet]
basic_1 = "Devnet11111111111111111111111111111111111" # comment
"#,
        );

        assert_eq!(parsed.programs.len(), 2);
        assert_eq!(parsed.provider_cluster.as_deref(), Some("localnet"));
        assert_eq!(parsed.programs[0].cluster, "localnet");
        assert_eq!(parsed.programs[0].name, "basic_1");
        assert_eq!(
            parsed.programs[1].value,
            "Devnet11111111111111111111111111111111111"
        );
    }

    #[test]
    fn matches_program_name_from_anchor_layout_uri() {
        let uri = Url::parse("file:///tmp/project/programs/basic-1/src/lib.rs").unwrap();
        let anchor_toml = r#"
[programs.localnet]
basic_1 = "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
"#;

        let program = preferred_program_id(&uri, anchor_toml).unwrap();
        assert_eq!(program.name, "basic_1");
        assert_eq!(
            program.value,
            "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
        );
    }

    #[test]
    fn prefers_provider_cluster_program_id() {
        let uri = Url::parse("file:///tmp/project/programs/basic-1/src/lib.rs").unwrap();
        let anchor_toml = r#"
[provider]
cluster = "devnet"

[programs.localnet]
basic_1 = "Localnet111111111111111111111111111111111"

[programs.devnet]
basic_1 = "Devnet11111111111111111111111111111111111"
"#;

        let program = preferred_program_id(&uri, anchor_toml).unwrap();
        assert_eq!(program.cluster, "devnet");
        assert_eq!(program.value, "Devnet11111111111111111111111111111111111");
    }

    #[test]
    fn finds_nearest_seagrass_toml() {
        let root = unique_temp_dir("seagrass-seagrass-config");
        let program_src = root.join("programs/demo/src");
        std::fs::create_dir_all(&program_src).unwrap();
        let config = root.join(SEAGRASS_TOML_FILE);
        std::fs::write(&config, "[lints]\nallow = [\"unchecked-arithmetic\"]\n").unwrap();

        let uri = Url::from_file_path(program_src.join("lib.rs")).unwrap();
        let (found_uri, text) = nearest_seagrass_toml(&uri).expect("expected Seagrass.toml");

        assert_eq!(found_uri, Url::from_file_path(config).unwrap());
        assert!(text.contains("unchecked-arithmetic"));
    }

    #[test]
    fn stops_config_lookup_at_registered_workspace_root() {
        let parent = unique_temp_dir("seagrass-parent-config");
        let workspace = parent.join("workspace");
        let program_src = workspace.join("programs/demo/src");
        std::fs::create_dir_all(&program_src).unwrap();
        let parent_config = parent.join(ANCHOR_TOML_FILE);
        std::fs::write(&parent_config, "[provider]\ncluster = \"mainnet\"\n").unwrap();

        let uri = Url::from_file_path(program_src.join("lib.rs")).unwrap();
        let workspace_root = Url::from_file_path(&workspace).unwrap();

        assert!(nearest_anchor_toml_with_roots(&uri, &[workspace_root]).is_none());
        let _ = std::fs::remove_dir_all(parent);
    }

    #[test]
    fn does_not_read_config_planted_in_shared_directory() {
        // A config sitting directly in the system temp dir (a shared,
        // world-writable location) must never be treated as a workspace root,
        // even for files opened beneath it.
        let shared = std::env::temp_dir();
        assert!(is_shared_directory(&shared));
        assert!(is_shared_directory(Path::new("/")));
        assert!(is_shared_directory(Path::new("/private/tmp")));
        assert!(is_shared_directory(Path::new("/private/var/tmp")));
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{nonce}"))
    }
}
