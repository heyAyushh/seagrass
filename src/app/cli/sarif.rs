use {
    super::diagnostics::{CliDiagnostic, SeverityLabel},
    serde::Serialize,
    serde_json::{json, Value},
};

#[derive(Debug, Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Debug, Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Debug, Serialize)]
struct SarifDriver {
    name: &'static str,
    version: String,
    information_uri: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Debug, Serialize)]
struct SarifRule {
    id: String,
    name: String,
    short_description: SarifMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    help_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<Value>,
}

#[derive(Debug, Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Debug, Serialize)]
struct SarifResult {
    rule_id: String,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<Value>,
}

#[derive(Debug, Serialize)]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize)]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Debug, Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Debug, Serialize)]
struct SarifRegion {
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

pub(super) fn write_sarif(diagnostics: &[CliDiagnostic]) -> Result<(), Box<dyn std::error::Error>> {
    write_sarif_to_writer(diagnostics, &mut std::io::stdout())?;
    println!();
    Ok(())
}

pub(super) fn write_sarif_to_writer(
    diagnostics: &[CliDiagnostic],
    writer: &mut impl std::io::Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let log = sarif_log(diagnostics);
    serde_json::to_writer_pretty(writer, &log)?;
    Ok(())
}

fn sarif_log(diagnostics: &[CliDiagnostic]) -> SarifLog {
    let rules = collect_rules(diagnostics);
    let results = diagnostics.iter().map(diagnostic_to_result).collect();
    SarifLog {
        schema: "https://json.schemastore.org/sarif-2.1.0.json",
        version: "2.1.0",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "seagrass",
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/heyAyushh/seagrass",
                    rules,
                },
            },
            results,
        }],
    }
}

fn collect_rules(diagnostics: &[CliDiagnostic]) -> Vec<SarifRule> {
    let mut rules = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for diagnostic in diagnostics {
        let id = rule_id(diagnostic);
        if !seen.insert(id.clone()) {
            continue;
        }
        rules.push(SarifRule {
            name: diagnostic.topic.clone().unwrap_or_else(|| id.clone()),
            short_description: SarifMessage {
                text: diagnostic.message.clone(),
            },
            help_uri: diagnostic.docs_url.clone(),
            properties: diagnostic.topic.as_ref().map(|topic| {
                json!({
                    "topic": topic,
                    "confidence": diagnostic.confidence,
                    "applicability": diagnostic.applicability,
                })
            }),
            id,
        });
    }
    rules
}

fn diagnostic_to_result(diagnostic: &CliDiagnostic) -> SarifResult {
    SarifResult {
        rule_id: rule_id(diagnostic),
        level: sarif_level(diagnostic.severity),
        message: SarifMessage {
            text: diagnostic.message.clone(),
        },
        locations: vec![SarifLocation {
            physical_location: SarifPhysicalLocation {
                artifact_location: SarifArtifactLocation {
                    uri: diagnostic.file.clone(),
                },
                region: SarifRegion {
                    start_line: diagnostic.range.start.line + 1,
                    start_column: diagnostic.range.start.character + 1,
                    end_line: diagnostic.range.end.line + 1,
                    end_column: diagnostic.range.end.character + 1,
                },
            },
        }],
        properties: diagnostic.topic.as_ref().map(|topic| {
            json!({
                "topic": topic,
                "confidence": diagnostic.confidence,
                "applicability": diagnostic.applicability,
            })
        }),
    }
}

fn rule_id(diagnostic: &CliDiagnostic) -> String {
    diagnostic
        .code
        .clone()
        .or_else(|| diagnostic.topic.clone())
        .unwrap_or_else(|| "seagrass-diagnostic".to_string())
}

fn sarif_level(severity: SeverityLabel) -> &'static str {
    match severity {
        SeverityLabel::Error => "error",
        SeverityLabel::Warning => "warning",
        SeverityLabel::Information => "note",
        SeverityLabel::Hint => "note",
        SeverityLabel::Unknown => "warning",
    }
}
