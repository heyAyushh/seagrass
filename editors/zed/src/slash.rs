use {
    crate::{
        constants::{
            SLASH_ARTIFACTS, SLASH_COVERAGE, SLASH_FEEDBACK, SLASH_STATUS, START_SERVER_FIRST,
        },
        lsp_execute::execute_lsp_command_for_worktree,
    },
    zed_extension_api as zed,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SlashCommandSpec {
    pub(crate) name: &'static str,
    pub(crate) lsp_command: &'static str,
}

pub(crate) fn slash_command_spec(name: &str) -> Option<SlashCommandSpec> {
    match name {
        SLASH_STATUS => Some(SlashCommandSpec {
            name: SLASH_STATUS,
            lsp_command: "seagrass/status",
        }),
        SLASH_COVERAGE => Some(SlashCommandSpec {
            name: SLASH_COVERAGE,
            lsp_command: "seagrass/projectCoverage",
        }),
        SLASH_ARTIFACTS => Some(SlashCommandSpec {
            name: SLASH_ARTIFACTS,
            lsp_command: "seagrass/artifacts",
        }),
        SLASH_FEEDBACK => Some(SlashCommandSpec {
            name: SLASH_FEEDBACK,
            lsp_command: "seagrass/feedback",
        }),
        _ => None,
    }
}

pub(crate) fn run_seagrass_slash_command(
    spec: SlashCommandSpec,
    worktree: Option<&zed::Worktree>,
) -> zed::SlashCommandOutput {
    let Some(worktree) = worktree else {
        return slash_command_error_output(
            spec,
            "no worktree is attached to this Assistant request",
        );
    };

    match execute_lsp_command_for_worktree(spec, worktree) {
        Ok(result) => zed::SlashCommandOutput {
            text: slash_command_result_text(spec, &result),
            sections: Vec::new(),
        },
        Err(error) => slash_command_error_output(spec, &error),
    }
}

fn slash_command_result_text(spec: SlashCommandSpec, result: &zed::serde_json::Value) -> String {
    if spec.name == SLASH_FEEDBACK {
        if let Some(url) = result.get("url").and_then(|value| value.as_str()) {
            let label = result
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("Seagrass feedback");
            return format!("{label}\n{url}");
        }
    }

    zed::serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
}

fn slash_command_error_output(spec: SlashCommandSpec, error: &str) -> zed::SlashCommandOutput {
    zed::SlashCommandOutput {
        text: format!(
            "{START_SERVER_FIRST}\n\n/{} could not dispatch `{}`: {error}",
            spec.name, spec.lsp_command
        ),
        sections: Vec::new(),
    }
}

pub(crate) fn unsupported_slash_command(name: &str) -> String {
    format!("unsupported Seagrass slash command `{name}`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_commands_map_to_server_execute_commands() {
        assert_eq!(
            slash_command_spec(SLASH_STATUS).map(|spec| spec.lsp_command),
            Some("seagrass/status")
        );
        assert_eq!(
            slash_command_spec(SLASH_COVERAGE).map(|spec| spec.lsp_command),
            Some("seagrass/projectCoverage")
        );
        assert_eq!(
            slash_command_spec(SLASH_ARTIFACTS).map(|spec| spec.lsp_command),
            Some("seagrass/artifacts")
        );
        assert_eq!(
            slash_command_spec(SLASH_FEEDBACK).map(|spec| spec.lsp_command),
            Some("seagrass/feedback")
        );
    }

    #[test]
    fn slash_command_error_output_keeps_graceful_start_hint() {
        let output = slash_command_error_output(
            slash_command_spec(SLASH_STATUS).unwrap(),
            "seagrass was unavailable",
        );

        assert!(output.text.contains(START_SERVER_FIRST));
        assert!(output.text.contains("seagrass was unavailable"));
        assert!(output.text.contains("seagrass/status"));
        assert!(output.sections.is_empty());
    }

    #[test]
    fn slash_command_feedback_text_prefers_url() {
        let text = slash_command_result_text(
            slash_command_spec(SLASH_FEEDBACK).unwrap(),
            &zed::serde_json::json!({
                "label": "Join the Seagrass Telegram",
                "url": "https://t.me/example"
            }),
        );

        assert_eq!(text, "Join the Seagrass Telegram\nhttps://t.me/example");
    }
}
