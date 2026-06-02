use {
    crate::constants::{
        SERVER_BINARY, SERVER_ID, SERVER_MANIFEST_ENV, SERVER_MANIFEST_FROM_EXTENSION,
        SERVER_PACKAGE,
    },
    zed_extension_api::{self as zed, settings::LspSettings},
};

pub(crate) fn server_command_for_worktree(worktree: &zed::Worktree) -> zed::Result<zed::Command> {
    let settings = LspSettings::for_worktree(SERVER_ID, worktree)?;
    if let Some(binary) = settings.binary {
        let command = binary
            .path
            .ok_or_else(|| "`lsp.seagrass.binary.path` must be set".to_string())?;
        let args = binary.arguments.unwrap_or_default();
        return Ok(zed::Command {
            command,
            args,
            env: merge_env(worktree.shell_env(), binary.env),
        });
    }

    let env = worktree.shell_env();
    if let Some(command) = worktree.which(SERVER_BINARY) {
        return Ok(zed::Command {
            command,
            args: Vec::new(),
            env,
        });
    }

    let cargo = worktree
        .which("cargo")
        .ok_or_else(|| "seagrass was not found on PATH, and cargo is not available".to_string())?;

    let worktree_manifest = worktree.read_text_file("Cargo.toml").ok();
    let args = cargo_args(
        &env,
        worktree_manifest.as_deref(),
        &worktree_manifest_path(worktree),
    );

    Ok(zed::Command {
        command: cargo,
        args,
        env,
    })
}

fn cargo_args_for_manifest(manifest_path: impl Into<String>) -> Vec<String> {
    vec![
        "run".to_string(),
        "--manifest-path".to_string(),
        manifest_path.into(),
        "--quiet".to_string(),
    ]
}

fn cargo_args(
    env: &[(String, String)],
    worktree_manifest: Option<&str>,
    worktree_manifest_path: &str,
) -> Vec<String> {
    if let Some(manifest_path) = env_value(env, SERVER_MANIFEST_ENV) {
        return cargo_args_for_manifest(manifest_path);
    }

    if worktree_manifest.is_some_and(is_seagrass_server_manifest) {
        return cargo_args_for_manifest(worktree_manifest_path);
    }

    cargo_args_for_manifest(SERVER_MANIFEST_FROM_EXTENSION)
}

fn worktree_manifest_path(worktree: &zed::Worktree) -> String {
    format!("{}/Cargo.toml", worktree.root_path())
}

fn is_seagrass_server_manifest(manifest: &str) -> bool {
    section_has_toml_string(manifest, "[package]", "name", SERVER_PACKAGE)
        && section_has_toml_string(manifest, "[[bin]]", "name", SERVER_BINARY)
}

fn section_has_toml_string(manifest: &str, section: &str, key: &str, expected: &str) -> bool {
    let mut in_section = false;

    for raw_line in manifest.lines() {
        let line = strip_toml_comment(raw_line).trim();
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if in_section && toml_string_value(line, key).as_deref() == Some(expected) {
            return true;
        }
    }

    false
}

fn toml_string_value(line: &str, key: &str) -> Option<String> {
    let value = line
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim_start()
        .strip_prefix('"')?;
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn strip_toml_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("")
}

fn env_value(env: &[(String, String)], key: &str) -> Option<String> {
    env.iter()
        .find_map(|(name, value)| (name == key).then(|| value.clone()))
}

fn merge_env(
    mut base: zed::EnvVars,
    overrides: Option<std::collections::HashMap<String, String>>,
) -> zed::EnvVars {
    if let Some(overrides) = overrides {
        for (name, value) in overrides {
            if let Some((_, existing)) = base.iter_mut().find(|(existing, _)| existing == &name) {
                *existing = value;
            } else {
                base.push((name, value));
            }
        }
    }

    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_args_use_manifest_env_override() {
        let env = vec![(
            SERVER_MANIFEST_ENV.to_string(),
            "/tmp/seagrass/Cargo.toml".to_string(),
        )];

        assert_eq!(
            cargo_args(
                &env,
                Some(GENERIC_WORKTREE_MANIFEST),
                "/tmp/workspace/Cargo.toml",
            ),
            cargo_args_for_manifest("/tmp/seagrass/Cargo.toml")
        );
    }

    #[test]
    fn cargo_args_use_worktree_manifest_only_for_seagrass_server() {
        assert_eq!(
            cargo_args(
                &[],
                Some(SEAGRASS_SERVER_MANIFEST),
                "/tmp/workspace/Cargo.toml",
            ),
            cargo_args_for_manifest("/tmp/workspace/Cargo.toml")
        );
    }

    #[test]
    fn cargo_args_skip_generic_worktree_manifest_without_bin() {
        assert_eq!(
            cargo_args(
                &[],
                Some(GENERIC_WORKTREE_MANIFEST),
                "/tmp/workspace/Cargo.toml",
            ),
            cargo_args_for_manifest(SERVER_MANIFEST_FROM_EXTENSION)
        );
    }

    const GENERIC_WORKTREE_MANIFEST: &str = r#"
[package]
name = "some-program"
version = "0.1.0"
edition = "2021"
"#;

    const SEAGRASS_SERVER_MANIFEST: &str = r#"
[package]
name = "seagrass-cli"
version = "1.0.2"
edition = "2021"

[[bin]]
name = "seagrass"
path = "src/main.rs"
"#;
}
