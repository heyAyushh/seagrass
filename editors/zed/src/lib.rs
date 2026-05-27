use zed_extension_api::{self as zed, settings::LspSettings};

const SERVER_ID: &str = "seagrass";
const SERVER_BINARY: &str = "seagrass";
const SERVER_MANIFEST_ENV: &str = "SEAGRASS_MANIFEST_PATH";
const DEFAULT_DIAGNOSTICS_TRANSPORT: &str = "both";
const SERVER_MANIFEST_FROM_EXTENSION: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");

struct SeagrassExtension;

impl zed::Extension for SeagrassExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != SERVER_ID {
            return Err(format!(
                "unsupported language server `{language_server_id}`"
            ));
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
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

        let cargo = worktree.which("cargo").ok_or_else(|| {
            "seagrass was not found on PATH, and cargo is not available".to_string()
        })?;

        let worktree_manifest = worktree.read_text_file("Cargo.toml").ok();
        let args = cargo_args(&env, worktree_manifest.as_deref());

        Ok(zed::Command {
            command: cargo,
            args,
            env,
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        if language_server_id.as_ref() != SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        let transport = diagnostics_transport(settings.settings.as_ref());

        Ok(Some(zed::serde_json::json!({
            "seagrass": {
                "diagnostics": {
                    "transport": transport
                },
                "editor": {
                    "client": "zed",
                    "inlineValues": {
                        "enabled": true
                    }
                },
                "telemetry": {
                    "completion": {
                        "enabled": true
                    },
                    "diagnostics": {
                        "enabled": true
                    }
                }
            }
        })))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        if language_server_id.as_ref() != SERVER_ID {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        Ok(Some(workspace_configuration(settings.settings)))
    }
}

fn cargo_args_for_manifest(manifest_path: impl Into<String>) -> Vec<String> {
    vec![
        "run".to_string(),
        "--manifest-path".to_string(),
        manifest_path.into(),
        "--quiet".to_string(),
    ]
}

fn cargo_args(env: &[(String, String)], worktree_manifest: Option<&str>) -> Vec<String> {
    if let Some(manifest_path) = env_value(env, SERVER_MANIFEST_ENV) {
        return cargo_args_for_manifest(manifest_path);
    }

    if worktree_manifest.is_some_and(is_seagrass_server_manifest) {
        return cargo_args_for_manifest("Cargo.toml");
    }

    cargo_args_for_manifest(SERVER_MANIFEST_FROM_EXTENSION)
}

fn is_seagrass_server_manifest(manifest: &str) -> bool {
    section_has_toml_string(manifest, "[package]", "name", SERVER_BINARY)
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

fn workspace_configuration(settings: Option<zed::serde_json::Value>) -> zed::serde_json::Value {
    let mut settings = settings.unwrap_or_else(|| zed::serde_json::json!({}));
    if !settings.is_object() {
        settings = zed::serde_json::json!({});
    }
    ensure_default(&mut settings, "diagnostics.security.enabled", true);
    ensure_default(&mut settings, "diagnostics.experimental.enabled", true);
    ensure_default(&mut settings, "security.strictNative.enabled", true);
    ensure_default(&mut settings, "editor.inlineValues.enabled", true);
    ensure_default(&mut settings, "telemetry.completion.enabled", true);
    ensure_default(&mut settings, "telemetry.diagnostics.enabled", true);
    ensure_default(&mut settings, "inlayHints.enabled", true);
    ensure_default(&mut settings, "workspaceIndex.enabled", true);
    ensure_default(&mut settings, "trace.server", false);
    ensure_string_default(&mut settings, "editor.client", "zed");
    ensure_string_default(
        &mut settings,
        "diagnostics.transport",
        DEFAULT_DIAGNOSTICS_TRANSPORT,
    );
    ensure_string_default(&mut settings, "diagnostics.coldPath", "idle");
    settings
}

fn diagnostics_transport(settings: Option<&zed::serde_json::Value>) -> String {
    let Some(settings) = settings else {
        return DEFAULT_DIAGNOSTICS_TRANSPORT.to_string();
    };

    string_setting(settings, "diagnostics.transport")
        .or_else(|| {
            settings
                .get("diagnostics")
                .and_then(|diagnostics| diagnostics.get("transport"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .filter(|value| matches!(value.as_str(), "push" | "pull" | "both"))
        .unwrap_or_else(|| DEFAULT_DIAGNOSTICS_TRANSPORT.to_string())
}

fn string_setting(settings: &zed::serde_json::Value, key: &str) -> Option<String> {
    settings
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn ensure_default(settings: &mut zed::serde_json::Value, key: &str, default: bool) {
    if settings.get(key).is_none() {
        settings[key] = zed::serde_json::Value::Bool(default);
    }
}

fn ensure_string_default(settings: &mut zed::serde_json::Value, key: &str, default: &str) {
    if settings.get(key).is_none() {
        settings[key] = zed::serde_json::Value::String(default.to_string());
    }
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
    fn defaults_to_pull_and_push_diagnostics_for_on_open_feedback() {
        assert_eq!(diagnostics_transport(None), "both");

        let config = workspace_configuration(None);
        assert_eq!(config["diagnostics.transport"], "both");
    }

    #[test]
    fn diagnostics_transport_respects_flat_setting() {
        let settings = zed::serde_json::json!({
            "diagnostics.transport": "pull"
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "pull");
    }

    #[test]
    fn diagnostics_transport_respects_nested_setting() {
        let settings = zed::serde_json::json!({
            "diagnostics": {
                "transport": "push"
            }
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "push");
    }

    #[test]
    fn diagnostics_transport_rejects_unknown_values() {
        let settings = zed::serde_json::json!({
            "diagnostics.transport": "invalid"
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "both");
    }

    #[test]
    fn cargo_args_use_manifest_env_override() {
        let env = vec![(
            SERVER_MANIFEST_ENV.to_string(),
            "/tmp/seagrass/Cargo.toml".to_string(),
        )];

        assert_eq!(
            cargo_args(&env, Some(GENERIC_WORKTREE_MANIFEST)),
            cargo_args_for_manifest("/tmp/seagrass/Cargo.toml")
        );
    }

    #[test]
    fn cargo_args_use_worktree_manifest_only_for_seagrass_server() {
        assert_eq!(
            cargo_args(&[], Some(SEAGRASS_SERVER_MANIFEST)),
            cargo_args_for_manifest("Cargo.toml")
        );
    }

    #[test]
    fn cargo_args_skip_generic_worktree_manifest_without_bin() {
        assert_eq!(
            cargo_args(&[], Some(GENERIC_WORKTREE_MANIFEST)),
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
name = "seagrass"
version = "1.0.2"
edition = "2021"

[[bin]]
name = "seagrass"
path = "src/main.rs"
"#;
}

zed::register_extension!(SeagrassExtension);
