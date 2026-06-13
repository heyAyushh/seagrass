use {
    crate::{
        constants::{
            SLASH_ANALYZE, SLASH_ARTIFACTS, SLASH_COVERAGE, SLASH_ERROR_COVERAGE, SLASH_FEEDBACK,
            SLASH_GENERATOR_PROFILE, SLASH_LOGS, SLASH_PROGRAM_REPORT, SLASH_STATUS,
            SLASH_SUPPORT_MATRIX, START_SERVER_FIRST,
        },
        lsp_execute::{execute_lsp_command_for_worktree, LspDocumentSnapshot},
        uri::file_uri_from_path,
    },
    std::path::{Component, Path},
    zed_extension_api as zed,
};

const SUMMARY_PREVIEW_LIMIT: usize = 5;
const RAW_JSON_HEADING: &str = "\n\nRaw JSON\n```json\n";
const PATH_COMPLETION_LABEL: &str = "<path>";
const PATH_COMPLETION_TEXT: &str = "programs/demo/src/lib.rs";
const INSTRUCTION_COMPLETION_TEXT: &str = "instruction=initialize";
const FUNCTION_COMPLETION_TEXT: &str = "function=initialize";
const CONTEXT_COMPLETION_TEXT: &str = "context=Create";
const INSTRUCTION_KEY: &str = "instruction";
const FUNCTION_KEY: &str = "function";
const CONTEXT_KEY: &str = "context";
const URI_KEY: &str = "uri";
const PATH_KEY: &str = "path";

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

pub(crate) fn complete_slash_command_argument(
    spec: SlashCommandSpec,
    args: &[String],
) -> Vec<zed::SlashCommandArgumentCompletion> {
    if !supports_document_argument(spec) {
        return Vec::new();
    }

    let query = args.last().map(String::as_str).unwrap_or("");
    slash_argument_completions()
        .into_iter()
        .filter(|completion| {
            completion.label.starts_with(query) || completion.new_text.starts_with(query)
        })
        .collect()
}

pub(crate) fn run_seagrass_slash_command(
    spec: SlashCommandSpec,
    args: Vec<String>,
    worktree: Option<&zed::Worktree>,
) -> zed::SlashCommandOutput {
    let Some(worktree) = worktree else {
        return slash_command_error_output(
            spec,
            "no worktree is attached to this Assistant request",
        );
    };

    let worktree_root = worktree.root_path();
    let parsed_args = ParsedSlashArgs::from_args(&args);
    let arguments = lsp_arguments_for_parsed_slash_command(spec, &parsed_args, &worktree_root);
    let document = match lsp_document_snapshot_for_slash_command(spec, &parsed_args, worktree) {
        Ok(document) => document,
        Err(error) => return slash_command_error_output(spec, &error),
    };

    match execute_lsp_command_for_worktree(spec, worktree, arguments, document) {
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

fn supports_document_argument(spec: SlashCommandSpec) -> bool {
    matches!(spec.name, SLASH_ANALYZE | SLASH_ARTIFACTS)
}

fn slash_argument_completions() -> Vec<zed::SlashCommandArgumentCompletion> {
    [
        (PATH_COMPLETION_LABEL, PATH_COMPLETION_TEXT),
        (INSTRUCTION_COMPLETION_TEXT, INSTRUCTION_COMPLETION_TEXT),
        (FUNCTION_COMPLETION_TEXT, FUNCTION_COMPLETION_TEXT),
        (CONTEXT_COMPLETION_TEXT, CONTEXT_COMPLETION_TEXT),
    ]
    .into_iter()
    .map(|(label, new_text)| zed::SlashCommandArgumentCompletion {
        label: label.to_string(),
        new_text: new_text.to_string(),
        run_command: false,
    })
    .collect()
}

#[cfg(test)]
fn lsp_arguments_for_slash_command(
    spec: SlashCommandSpec,
    args: &[String],
    worktree_root: &str,
) -> Vec<zed::serde_json::Value> {
    let parsed = ParsedSlashArgs::from_args(args);
    lsp_arguments_for_parsed_slash_command(spec, &parsed, worktree_root)
}

fn lsp_arguments_for_parsed_slash_command(
    spec: SlashCommandSpec,
    parsed: &ParsedSlashArgs,
    worktree_root: &str,
) -> Vec<zed::serde_json::Value> {
    if !supports_document_argument(spec) {
        return Vec::new();
    }

    let mut object = zed::serde_json::Map::new();
    if let Some(uri) = parsed.uri(worktree_root) {
        object.insert(URI_KEY.to_string(), zed::serde_json::Value::String(uri));
    }
    if let Some(instruction) = &parsed.instruction {
        object.insert(
            INSTRUCTION_KEY.to_string(),
            zed::serde_json::Value::String(instruction.clone()),
        );
    }
    if let Some(function) = &parsed.function {
        object.insert(
            FUNCTION_KEY.to_string(),
            zed::serde_json::Value::String(function.clone()),
        );
    }
    if let Some(context) = &parsed.context {
        object.insert(
            CONTEXT_KEY.to_string(),
            zed::serde_json::Value::String(context.clone()),
        );
    }

    (!object.is_empty())
        .then_some(zed::serde_json::Value::Object(object))
        .into_iter()
        .collect()
}

fn lsp_document_snapshot_for_slash_command(
    spec: SlashCommandSpec,
    parsed: &ParsedSlashArgs,
    worktree: &zed::Worktree,
) -> Result<Option<LspDocumentSnapshot>, String> {
    if spec.name != SLASH_ANALYZE {
        return Ok(None);
    }

    let worktree_root = worktree.root_path();
    let Some(uri) = parsed.uri(&worktree_root) else {
        return Ok(None);
    };
    let Some(relative_path) = parsed.worktree_relative_path(&worktree_root) else {
        return Err("document path must point inside the current Zed worktree".to_string());
    };
    let text = worktree.read_text_file(&relative_path).map_err(|error| {
        format!("could not open `{relative_path}` before running analysis: {error}")
    })?;

    Ok(Some(LspDocumentSnapshot { uri, text }))
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedSlashArgs {
    uri: Option<String>,
    path: Option<String>,
    instruction: Option<String>,
    function: Option<String>,
    context: Option<String>,
}

impl ParsedSlashArgs {
    fn from_args(args: &[String]) -> Self {
        let mut parsed = Self::default();
        let mut pending_key: Option<&str> = None;

        for arg in normalize_slash_args(args) {
            let arg = arg.as_str();
            if let Some(key) = pending_key.take() {
                parsed.set_key_value(key, arg);
                continue;
            }

            if matches!(arg, "--instruction" | "-i") {
                pending_key = Some(INSTRUCTION_KEY);
                continue;
            }
            if matches!(arg, "--function" | "-f") {
                pending_key = Some(FUNCTION_KEY);
                continue;
            }
            if matches!(arg, "--context" | "-c") {
                pending_key = Some(CONTEXT_KEY);
                continue;
            }

            if let Some((key, value)) = arg.split_once('=') {
                parsed.set_key_value(key.trim_start_matches('-'), value);
                continue;
            }

            if parsed.path.is_none() && parsed.uri.is_none() {
                parsed.set_path_or_uri(arg);
            }
        }

        parsed
    }

    fn set_key_value(&mut self, key: &str, value: &str) {
        if value.is_empty() {
            return;
        }

        match key {
            INSTRUCTION_KEY => self.instruction = Some(value.to_string()),
            FUNCTION_KEY => self.function = Some(value.to_string()),
            CONTEXT_KEY => self.context = Some(value.to_string()),
            URI_KEY => self.uri = Some(value.to_string()),
            PATH_KEY => self.set_path_or_uri(value),
            _ => {}
        }
    }

    fn set_path_or_uri(&mut self, value: &str) {
        if value.starts_with("file://") {
            self.uri = Some(value.to_string());
        } else {
            self.path = Some(value.to_string());
        }
    }

    fn uri(&self, worktree_root: &str) -> Option<String> {
        self.uri.clone().or_else(|| {
            self.path
                .as_deref()
                .map(|path| path_uri(worktree_root, path))
        })
    }

    fn worktree_relative_path(&self, worktree_root: &str) -> Option<String> {
        self.path
            .as_deref()
            .and_then(|path| relative_worktree_path_from_input(worktree_root, path))
            .or_else(|| {
                self.uri
                    .as_deref()
                    .and_then(file_path_from_uri)
                    .and_then(|path| absolute_worktree_relative_path(worktree_root, &path))
            })
    }
}

fn normalize_slash_args(args: &[String]) -> Vec<String> {
    let non_empty_args = args
        .iter()
        .map(String::as_str)
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    if non_empty_args.len() != 1 {
        return non_empty_args.into_iter().map(str::to_string).collect();
    }

    let raw_arg = non_empty_args[0];
    let split_args = split_slash_argument_text(raw_arg);
    if should_use_split_slash_args(raw_arg, &split_args) {
        split_args
    } else {
        vec![raw_arg.to_string()]
    }
}

fn should_use_split_slash_args(raw_arg: &str, split_args: &[String]) -> bool {
    split_args.len() > 1
        && (raw_arg.starts_with('"')
            || raw_arg.starts_with('\'')
            || split_args
                .iter()
                .skip(1)
                .any(|arg| is_named_argument_token(arg)))
}

fn is_named_argument_token(arg: &str) -> bool {
    if matches!(
        arg,
        "--instruction" | "-i" | "--function" | "-f" | "--context" | "-c"
    ) {
        return true;
    }

    arg.split_once('=')
        .map(|(key, _)| {
            matches!(
                key.trim_start_matches('-'),
                INSTRUCTION_KEY | FUNCTION_KEY | CONTEXT_KEY | URI_KEY | PATH_KEY
            )
        })
        .unwrap_or(false)
}

fn split_slash_argument_text(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaping = false;

    for character in input.chars() {
        if escaping {
            current.push(character);
            escaping = false;
            continue;
        }

        match character {
            '\\' => escaping = true,
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            character if character.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }

    if escaping {
        current.push('\\');
    }
    if !current.is_empty() {
        args.push(current);
    }

    args
}

fn path_uri(worktree_root: &str, path: &str) -> String {
    let path = std::path::Path::new(path);
    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::Path::new(worktree_root).join(path)
    };
    file_uri_from_path(&full_path.to_string_lossy())
}

fn relative_worktree_path_from_input(worktree_root: &str, path: &str) -> Option<String> {
    let path = Path::new(path);
    if path.is_absolute() {
        return absolute_worktree_relative_path(worktree_root, path);
    }
    safe_relative_worktree_path(path)
}

fn absolute_worktree_relative_path(
    worktree_root: &str,
    absolute_path: impl AsRef<Path>,
) -> Option<String> {
    let relative_path = absolute_path
        .as_ref()
        .strip_prefix(Path::new(worktree_root))
        .ok()?;
    safe_relative_worktree_path(relative_path)
}

fn safe_relative_worktree_path(path: &Path) -> Option<String> {
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(path_to_slash_string(path))
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(segment) => Some(segment.to_string_lossy()),
            Component::CurDir => None,
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn file_path_from_uri(uri: &str) -> Option<String> {
    let encoded_path = uri.strip_prefix("file://")?;
    percent_decode_file_uri_path(encoded_path)
}

fn percent_decode_file_uri_path(encoded_path: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(encoded_path.len());
    let bytes = encoded_path.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }

        let high = bytes.get(index + 1).copied().and_then(hex_value)?;
        let low = bytes.get(index + 2).copied().and_then(hex_value)?;
        decoded.push((high << 4) | low);
        index += 3;
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
    fn slash_argument_completions_are_document_command_only() {
        let completions = complete_slash_command_argument(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &["inst".to_string()],
        );
        assert!(completions.iter().any(|completion| {
            completion.new_text == INSTRUCTION_COMPLETION_TEXT && !completion.run_command
        }));

        assert!(
            complete_slash_command_argument(slash_command_spec(SLASH_STATUS).unwrap(), &[])
                .is_empty()
        );
    }

    #[test]
    fn slash_arguments_convert_relative_path_to_execute_command_uri() {
        let args = lsp_arguments_for_slash_command(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &[
                "programs/demo/src/lib.rs".to_string(),
                "instruction=initialize".to_string(),
                "context=Create".to_string(),
            ],
            "/tmp/workspace",
        );

        assert_eq!(
            args,
            vec![zed::serde_json::json!({
                "uri": "file:///tmp/workspace/programs/demo/src/lib.rs",
                "instruction": "initialize",
                "context": "Create"
            })]
        );
    }

    #[test]
    fn slash_arguments_split_raw_argument_text() {
        let args = lsp_arguments_for_slash_command(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &["programs/demo/src/lib.rs instruction=initialize context=Create".to_string()],
            "/tmp/workspace",
        );

        assert_eq!(
            args,
            vec![zed::serde_json::json!({
                "uri": "file:///tmp/workspace/programs/demo/src/lib.rs",
                "instruction": "initialize",
                "context": "Create"
            })]
        );
    }

    #[test]
    fn slash_arguments_preserve_quoted_paths_in_raw_text() {
        let args = lsp_arguments_for_slash_command(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &["\"programs/demo/src/lib file.rs\" --function initialize".to_string()],
            "/tmp/workspace",
        );

        assert_eq!(
            args,
            vec![zed::serde_json::json!({
                "uri": "file:///tmp/workspace/programs/demo/src/lib%20file.rs",
                "function": "initialize"
            })]
        );
    }

    #[test]
    fn slash_arguments_preserve_tokenized_path_with_spaces() {
        let args = lsp_arguments_for_slash_command(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &["programs/demo/src/lib file.rs".to_string()],
            "/tmp/workspace",
        );

        assert_eq!(
            args,
            vec![zed::serde_json::json!({
                "uri": "file:///tmp/workspace/programs/demo/src/lib%20file.rs"
            })]
        );
    }

    #[test]
    fn slash_analyze_path_can_be_opened_relative_to_worktree() {
        let parsed = ParsedSlashArgs::from_args(&["programs/demo/src/lib.rs".to_string()]);

        assert_eq!(
            parsed.worktree_relative_path("/tmp/workspace"),
            Some("programs/demo/src/lib.rs".to_string())
        );
        assert_eq!(
            parsed.uri("/tmp/workspace"),
            Some("file:///tmp/workspace/programs/demo/src/lib.rs".to_string())
        );
    }

    #[test]
    fn slash_analyze_file_uri_can_be_mapped_to_worktree_path() {
        let parsed = ParsedSlashArgs::from_args(&[
            "uri=file:///tmp/workspace/programs/demo/src/lib%20file.rs".to_string(),
        ]);

        assert_eq!(
            parsed.worktree_relative_path("/tmp/workspace"),
            Some("programs/demo/src/lib file.rs".to_string())
        );
    }

    #[test]
    fn slash_analyze_rejects_parent_directory_path() {
        let parsed = ParsedSlashArgs::from_args(&["../outside.rs".to_string()]);

        assert_eq!(parsed.worktree_relative_path("/tmp/workspace"), None);
    }

    #[test]
    fn slash_arguments_accept_file_uri_and_flag_pairs() {
        let args = lsp_arguments_for_slash_command(
            slash_command_spec(SLASH_ANALYZE).unwrap(),
            &[
                "uri=file:///tmp/workspace/programs/demo/src/lib.rs".to_string(),
                "--function".to_string(),
                "initialize".to_string(),
            ],
            "/tmp/workspace",
        );

        assert_eq!(
            args,
            vec![zed::serde_json::json!({
                "uri": "file:///tmp/workspace/programs/demo/src/lib.rs",
                "function": "initialize"
            })]
        );
    }

    #[test]
    fn slash_arguments_do_not_touch_workspace_commands() {
        let args = lsp_arguments_for_slash_command(
            slash_command_spec(SLASH_COVERAGE).unwrap(),
            &["programs/demo/src/lib.rs".to_string()],
            "/tmp/workspace",
        );

        assert!(args.is_empty());
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
