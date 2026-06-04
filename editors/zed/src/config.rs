use {
    crate::constants::{DEFAULT_DIAGNOSTICS_TRANSPORT, SECURITY_LEVEL_SETTINGS},
    zed_extension_api as zed,
};

pub(crate) fn initialization_options(
    agent_mode_enabled: bool,
    diagnostics_transport: &str,
) -> zed::serde_json::Value {
    zed::serde_json::json!({
        "seagrass": {
            "agent": {
                "mode": agent_mode_enabled
            },
            "diagnostics": {
                "transport": diagnostics_transport
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
    })
}

pub(crate) fn workspace_configuration(
    settings: Option<zed::serde_json::Value>,
) -> zed::serde_json::Value {
    let mut settings = settings.unwrap_or_else(|| zed::serde_json::json!({}));
    if !settings.is_object() {
        settings = zed::serde_json::json!({});
    }
    ensure_default(&mut settings, "agent.mode", false);
    if bool_setting(&settings, "agent.mode").unwrap_or(false) {
        apply_agent_mode_defaults(&mut settings);
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

fn apply_agent_mode_defaults(settings: &mut zed::serde_json::Value) {
    ensure_default(settings, "diagnostics.security.enabled", true);
    ensure_default(settings, "diagnostics.experimental.enabled", true);
    ensure_default(settings, "security.strictNative.enabled", true);
    ensure_default(settings, "trace.server", true);
    ensure_string_default(settings, "diagnostics.coldPath", "idle");
    for setting in SECURITY_LEVEL_SETTINGS {
        ensure_string_default(settings, setting, "warn");
    }
}

pub(crate) fn diagnostics_transport(settings: Option<&zed::serde_json::Value>) -> String {
    let Some(settings) = settings else {
        return DEFAULT_DIAGNOSTICS_TRANSPORT.to_string();
    };

    string_setting(settings, "diagnostics.transport")
        .filter(|value| matches!(value.as_str(), "push" | "pull"))
        .unwrap_or_else(|| DEFAULT_DIAGNOSTICS_TRANSPORT.to_string())
}

pub(crate) fn agent_mode(settings: Option<&zed::serde_json::Value>) -> bool {
    let Some(settings) = settings else {
        return false;
    };
    bool_setting(settings, "agent.mode").unwrap_or(false)
}

fn bool_setting(settings: &zed::serde_json::Value, key: &str) -> Option<bool> {
    setting_value(settings, key).and_then(|value| value.as_bool())
}

fn string_setting(settings: &zed::serde_json::Value, key: &str) -> Option<String> {
    setting_value(settings, key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn setting_value<'a>(
    settings: &'a zed::serde_json::Value,
    key: &str,
) -> Option<&'a zed::serde_json::Value> {
    settings.get(key).or_else(|| {
        key.split('.')
            .try_fold(settings, |value, part| value.get(part))
    })
}

fn ensure_default(settings: &mut zed::serde_json::Value, key: &str, default: bool) {
    if setting_value(settings, key).is_none() {
        settings[key] = zed::serde_json::Value::Bool(default);
    }
}

fn ensure_string_default(settings: &mut zed::serde_json::Value, key: &str, default: &str) {
    if setting_value(settings, key).is_none() {
        settings[key] = zed::serde_json::Value::String(default.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialization_options_enable_zed_completion_contract() {
        let options = initialization_options(true, "push");

        assert_eq!(options["seagrass"]["agent"]["mode"], true);
        assert_eq!(options["seagrass"]["diagnostics"]["transport"], "push");
        assert_eq!(options["seagrass"]["editor"]["client"], "zed");
        assert_eq!(
            options["seagrass"]["editor"]["inlineValues"]["enabled"],
            true
        );
        assert_eq!(
            options["seagrass"]["telemetry"]["completion"]["enabled"],
            true
        );
        assert_eq!(
            options["seagrass"]["telemetry"]["diagnostics"]["enabled"],
            true
        );
    }

    #[test]
    fn defaults_to_push_diagnostics_to_avoid_transport_duplicates() {
        assert_eq!(diagnostics_transport(None), "push");

        let config = workspace_configuration(None);
        assert_eq!(config["diagnostics.transport"], "push");
    }

    #[test]
    fn agent_mode_fills_unset_workspace_settings() {
        let config = workspace_configuration(Some(zed::serde_json::json!({
            "agent.mode": true
        })));

        assert!(agent_mode(Some(&config)));
        assert_eq!(config["diagnostics.security.enabled"], true);
        assert_eq!(config["diagnostics.experimental.enabled"], true);
        assert_eq!(config["security.strictNative.enabled"], true);
        assert_eq!(config["diagnostics.coldPath"], "idle");
        assert_eq!(config["trace.server"], true);
        assert_eq!(config["diagnostics.security.ownerChecks"], "warn");
        assert_eq!(config["diagnostics.security.pdaSeedCollision"], "warn");
    }

    #[test]
    fn agent_mode_preserves_explicit_workspace_settings() {
        let config = workspace_configuration(Some(zed::serde_json::json!({
            "agent": {
                "mode": true
            },
            "diagnostics": {
                "coldPath": "manual"
            },
            "trace": {
                "server": false
            },
            "diagnostics.security.ownerChecks": "error",
            "diagnostics.security.typeCosplay": "off"
        })));

        assert_eq!(
            string_setting(&config, "diagnostics.coldPath"),
            Some("manual".to_string())
        );
        assert_eq!(bool_setting(&config, "trace.server"), Some(false));
        assert_eq!(config["diagnostics.security.ownerChecks"], "error");
        assert_eq!(config["diagnostics.security.typeCosplay"], "off");
        assert_eq!(config["diagnostics.security.arbitraryCpi"], "warn");
    }

    #[test]
    fn initialization_agent_mode_reads_flat_and_nested_settings() {
        assert!(!agent_mode(None));
        assert!(agent_mode(Some(&zed::serde_json::json!({
            "agent.mode": true
        }))));
        assert!(agent_mode(Some(&zed::serde_json::json!({
            "agent": {
                "mode": true
            }
        }))));
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

        assert_eq!(diagnostics_transport(Some(&settings)), "push");
    }

    #[test]
    fn diagnostics_transport_rejects_mixed_transport() {
        let settings = zed::serde_json::json!({
            "diagnostics.transport": "both"
        });

        assert_eq!(diagnostics_transport(Some(&settings)), "push");
    }
}
