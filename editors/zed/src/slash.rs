use {
    crate::{
        constants::{
            SLASH_ANALYZE, SLASH_ARTIFACTS, SLASH_COVERAGE, SLASH_ERROR_COVERAGE, SLASH_FEEDBACK,
            SLASH_GENERATOR_PROFILE, SLASH_LOGS, SLASH_PROGRAM_REPORT, SLASH_STATUS,
            SLASH_SUPPORT_MATRIX, START_SERVER_FIRST,
        },
        lsp_execute::execute_lsp_command_for_worktree,
    },
    zed_extension_api as zed,
};

const SUMMARY_PREVIEW_LIMIT: usize = 5;
const RAW_JSON_HEADING: &str = "\n\nRaw JSON\n```json\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SlashCommandSpec {
    pub(crate) name: &'static str,
    pub(crate) lsp_command: &'static str,
}

const SLASH_COMMANDS: &[SlashCommandSpec] = &[
    SlashCommandSpec {
        name: SLASH_STATUS,
        lsp_command: "seagrass/status",
    },
    SlashCommandSpec {
        name: SLASH_ANALYZE,
        lsp_command: "seagrass/analyze",
    },
    SlashCommandSpec {
        name: SLASH_COVERAGE,
        lsp_command: "seagrass/projectCoverage",
    },
    SlashCommandSpec {
        name: SLASH_ARTIFACTS,
        lsp_command: "seagrass/artifacts",
    },
    SlashCommandSpec {
        name: SLASH_PROGRAM_REPORT,
        lsp_command: "seagrass/programReport",
    },
    SlashCommandSpec {
        name: SLASH_ERROR_COVERAGE,
        lsp_command: "seagrass/errorCoverage",
    },
    SlashCommandSpec {
        name: SLASH_SUPPORT_MATRIX,
        lsp_command: "seagrass/supportMatrix",
    },
    SlashCommandSpec {
        name: SLASH_GENERATOR_PROFILE,
        lsp_command: "seagrass/generatorProfile",
    },
    SlashCommandSpec {
        name: SLASH_LOGS,
        lsp_command: "seagrass/logs",
    },
    SlashCommandSpec {
        name: SLASH_FEEDBACK,
        lsp_command: "seagrass/feedback",
    },
];

pub(crate) fn slash_command_spec(name: &str) -> Option<SlashCommandSpec> {
    SLASH_COMMANDS
        .iter()
        .copied()
        .find(|spec| spec.name == name)
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
    if result.is_null() {
        return format!(
            "{} needs the active document URI. Zed slash commands currently dispatch workspace-only Seagrass commands, so use `seagrass analyze <path> --json` for file-level analysis.",
            spec.lsp_command
        );
    }

    if spec.name == SLASH_FEEDBACK {
        if let Some(url) = result.get("url").and_then(|value| value.as_str()) {
            let label = result
                .get("label")
                .and_then(|value| value.as_str())
                .unwrap_or("Seagrass feedback");
            return format!("{label}\n{url}");
        }
    }

    if let Some(text) = result.as_str() {
        return text.to_string();
    }

    let raw_json = pretty_json(result);
    let summary = match spec.name {
        SLASH_COVERAGE => Some(project_coverage_text(result)),
        SLASH_ARTIFACTS => Some(artifacts_text(result)),
        SLASH_PROGRAM_REPORT => Some(program_report_text(result)),
        SLASH_ERROR_COVERAGE => Some(error_coverage_text(result)),
        SLASH_SUPPORT_MATRIX => Some(support_matrix_text(result)),
        SLASH_GENERATOR_PROFILE => Some(generator_profile_text(result)),
        SLASH_LOGS => Some(logs_text(result)),
        _ => None,
    };

    summary
        .map(|summary| format!("{summary}{RAW_JSON_HEADING}{raw_json}\n```"))
        .unwrap_or(raw_json)
}

fn project_coverage_text(result: &zed::serde_json::Value) -> String {
    let open_documents = result
        .get("openDocuments")
        .and_then(zed::serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let diagnostics = result
        .get("diagnosticsByCode")
        .and_then(zed::serde_json::Value::as_object)
        .map(|codes| {
            codes
                .values()
                .filter_map(zed::serde_json::Value::as_u64)
                .sum()
        })
        .unwrap_or(0);
    let attacks = result
        .get("diagnosticsByAttack")
        .and_then(zed::serde_json::Value::as_object)
        .map_or(0, |attacks| attacks.len());
    let gaps = result
        .get("capabilityGaps")
        .and_then(zed::serde_json::Value::as_array)
        .map_or(0, Vec::len);

    format!(
        "Seagrass project coverage\n- Open documents: {open_documents}\n- Active diagnostics: {diagnostics}\n- Attack classes touched: {attacks}\n- Capability gaps tracked: {gaps}"
    )
}

fn artifacts_text(result: &zed::serde_json::Value) -> String {
    let programs = result
        .get("programs")
        .and_then(zed::serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let roots = result
        .get("workspaceRoots")
        .and_then(zed::serde_json::Value::as_array)
        .map_or(0, Vec::len);

    if let Some(artifacts) = result.get("artifacts") {
        let stale = artifacts
            .get("stale")
            .and_then(zed::serde_json::Value::as_bool)
            .unwrap_or(false);
        let program = artifacts
            .get("program")
            .and_then(|program| program.get("name"))
            .and_then(zed::serde_json::Value::as_str)
            .unwrap_or("current program");
        return format!(
            "Seagrass artifact report\n- Program: {program}\n- Deploy artifact stale: {stale}"
        );
    }

    format!("Seagrass artifact report\n- Workspace roots: {roots}\n- Programs with artifact evidence: {programs}")
}

fn program_report_text(result: &zed::serde_json::Value) -> String {
    let programs = array_len(result, "programs");
    let instructions = array_len(result, "instructions");
    let pdas = array_len(result, "pdas");
    let errors = array_len(result, "errors");

    let mut lines = vec![
        "Seagrass program report".to_string(),
        format!("- Programs: {programs}"),
        format!("- Open-document instructions: {instructions}"),
        format!("- PDA declarations: {pdas}"),
        format!("- Diagnostics: {errors}"),
    ];
    append_preview_names(result, "instructions", "Instruction preview", &mut lines);
    append_preview_names(result, "pdas", "PDA preview", &mut lines);
    lines.join("\n")
}

fn error_coverage_text(result: &zed::serde_json::Value) -> String {
    let summary = result.get("summary").unwrap_or(result);
    let static_covered = summary_u64(summary, "staticCovered");
    let preflight_covered = summary_u64(summary, "preflightCovered");
    let runtime_only = summary_u64(summary, "runtimeOnly");
    let total = result
        .get("total")
        .and_then(zed::serde_json::Value::as_u64)
        .unwrap_or(static_covered + preflight_covered + runtime_only);

    format!(
        "Anchor error coverage\n- Total framework errors: {total}\n- Static diagnostics: {static_covered}\n- Preflight-covered: {preflight_covered}\n- Runtime-only: {runtime_only}"
    )
}

fn support_matrix_text(result: &zed::serde_json::Value) -> String {
    let summary = result.get("summary").unwrap_or(result);
    let anchor_version = summary
        .get("anchorVersion")
        .and_then(zed::serde_json::Value::as_str)
        .unwrap_or("unknown");
    let support_level = summary
        .get("supportLevel")
        .and_then(zed::serde_json::Value::as_str)
        .unwrap_or("unknown");
    let generated = summary.get("generated").unwrap_or(summary);
    let constraints = summary_u64(generated, "constraints");
    let field_completions = summary_u64(generated, "fieldCompletions");
    let errors = summary_u64(generated, "errors");
    let gaps = array_len(result, "gaps");
    let capability_gaps = array_len(result, "capabilityGaps");

    format!(
        "Anchor support matrix\n- Anchor version: {anchor_version}\n- Support level: {support_level}\n- Generated constraints: {constraints}\n- Field completions: {field_completions}\n- Error catalog entries: {errors}\n- Support gaps: {gaps}\n- Capability gaps: {capability_gaps}"
    )
}

fn generator_profile_text(result: &zed::serde_json::Value) -> String {
    let anchor_version = result
        .get("anchorVersion")
        .and_then(zed::serde_json::Value::as_str)
        .unwrap_or("unknown");
    let generation_mode = result
        .get("generationMode")
        .and_then(zed::serde_json::Value::as_str)
        .unwrap_or("unknown");
    let inputs = result.get("coverageInputs").unwrap_or(result);
    let parser_constraints = summary_u64(inputs, "parserConstraints");
    let parser_rules = summary_u64(inputs, "parserRules");
    let corpus_files = summary_u64(inputs, "programCorpusFiles");
    let corpus_constraints = summary_u64(inputs, "programCorpusConstraints");
    let generated_constraints = summary_u64(inputs, "generatedConstraints");

    format!(
        "Anchor generator profile\n- Anchor version: {anchor_version}\n- Generation mode: {generation_mode}\n- Parser constraints: {parser_constraints}\n- Parser rules: {parser_rules}\n- Corpus files: {corpus_files}\n- Corpus constraints: {corpus_constraints}\n- Generated constraints: {generated_constraints}"
    )
}

fn logs_text(result: &zed::serde_json::Value) -> String {
    let entries = result
        .get("entries")
        .and_then(zed::serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut lines = vec![
        "Seagrass recent logs".to_string(),
        format!("- Entries retained: {}", entries.len()),
    ];

    for entry in entries.iter().rev().take(SUMMARY_PREVIEW_LIMIT) {
        let event = entry
            .get("event")
            .and_then(zed::serde_json::Value::as_str)
            .unwrap_or("event");
        let message = entry
            .get("message")
            .and_then(zed::serde_json::Value::as_str)
            .unwrap_or("");
        lines.push(format!("- {event}: {message}"));
    }

    lines.join("\n")
}

fn append_preview_names(
    result: &zed::serde_json::Value,
    key: &str,
    label: &str,
    lines: &mut Vec<String>,
) {
    let names = result
        .get(key)
        .and_then(zed::serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .or_else(|| item.get("field"))
                        .and_then(zed::serde_json::Value::as_str)
                })
                .take(SUMMARY_PREVIEW_LIMIT)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !names.is_empty() {
        lines.push(format!("- {label}: {}", names.join(", ")));
    }
}

fn array_len(result: &zed::serde_json::Value, key: &str) -> usize {
    result
        .get(key)
        .and_then(zed::serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

fn summary_u64(result: &zed::serde_json::Value, key: &str) -> u64 {
    result
        .get(key)
        .and_then(zed::serde_json::Value::as_u64)
        .unwrap_or(0)
}

fn pretty_json(result: &zed::serde_json::Value) -> String {
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
        let expected = [
            (SLASH_STATUS, "seagrass/status"),
            (SLASH_ANALYZE, "seagrass/analyze"),
            (SLASH_COVERAGE, "seagrass/projectCoverage"),
            (SLASH_ARTIFACTS, "seagrass/artifacts"),
            (SLASH_PROGRAM_REPORT, "seagrass/programReport"),
            (SLASH_ERROR_COVERAGE, "seagrass/errorCoverage"),
            (SLASH_SUPPORT_MATRIX, "seagrass/supportMatrix"),
            (SLASH_GENERATOR_PROFILE, "seagrass/generatorProfile"),
            (SLASH_LOGS, "seagrass/logs"),
            (SLASH_FEEDBACK, "seagrass/feedback"),
        ];

        for (name, lsp_command) in expected {
            assert_eq!(
                slash_command_spec(name).map(|spec| spec.lsp_command),
                Some(lsp_command),
                "missing slash command mapping for {name}"
            );
        }
    }

    #[test]
    fn slash_commands_are_declared_in_extension_manifest() {
        let manifest = include_str!("../extension.toml");

        for spec in SLASH_COMMANDS {
            let manifest_section = format!("[slash_commands.{}]", spec.name);
            assert!(
                manifest.contains(&manifest_section),
                "missing manifest section {manifest_section}"
            );
        }

        assert!(
            manifest.contains(r#"code_action_kinds = ["quickfix", "refactor", "source"]"#),
            "Zed manifest must advertise every Seagrass code-action kind"
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

    #[test]
    fn slash_command_reports_null_results_as_missing_document_context() {
        let text = slash_command_result_text(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &zed::serde_json::Value::Null,
        );

        assert!(text.contains("needs the active document URI"));
        assert!(text.contains("seagrass analyze <path> --json"));
    }

    #[test]
    fn slash_command_error_coverage_prefers_engineering_summary() {
        let text = slash_command_result_text(
            slash_command_spec(SLASH_ERROR_COVERAGE).unwrap(),
            &zed::serde_json::json!({
                "total": 79,
                "summary": {
                    "staticCovered": 58,
                    "preflightCovered": 12,
                    "runtimeOnly": 9
                },
                "errors": []
            }),
        );

        assert!(text.starts_with("Anchor error coverage"));
        assert!(text.contains("- Static diagnostics: 58"));
        assert!(text.contains("- Preflight-covered: 12"));
        assert!(text.contains("- Runtime-only: 9"));
        assert!(text.contains(RAW_JSON_HEADING));
    }

    #[test]
    fn slash_command_program_report_summarizes_core_counts() {
        let text = slash_command_result_text(
            slash_command_spec(SLASH_PROGRAM_REPORT).unwrap(),
            &zed::serde_json::json!({
                "programs": [{"program": {"name": "whirlpool"}}],
                "instructions": [{"name": "close_position"}],
                "pdas": [{"field": "position"}],
                "errors": [{ "code": "seagrass/security.owner-check" }]
            }),
        );

        assert!(text.starts_with("Seagrass program report"));
        assert!(text.contains("- Programs: 1"));
        assert!(text.contains("- Open-document instructions: 1"));
        assert!(text.contains("- PDA declarations: 1"));
        assert!(text.contains("- Diagnostics: 1"));
        assert!(text.contains("- Instruction preview: close_position"));
        assert!(text.contains("- PDA preview: position"));
    }

    #[test]
    fn slash_command_logs_show_recent_events_before_raw_json() {
        let text = slash_command_result_text(
            slash_command_spec(SLASH_LOGS).unwrap(),
            &zed::serde_json::json!({
                "limit": 200,
                "entries": [
                    {"event": "serverStarted", "message": "ready"},
                    {"event": "completionServed", "message": "5 items"}
                ]
            }),
        );

        assert!(text.starts_with("Seagrass recent logs"));
        assert!(text.contains("- Entries retained: 2"));
        assert!(text.contains("- completionServed: 5 items"));
        assert!(text.contains("- serverStarted: ready"));
    }
}
