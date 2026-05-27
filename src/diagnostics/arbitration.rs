use {
    std::collections::{BTreeMap, HashSet},
    tower_lsp::lsp_types::{
        Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString,
        Position, Range, Url,
    },
};

const CURRENT_DOCUMENT_PLACEHOLDER_URI: &str = "file:///seagrass/current-document.rs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSettings {
    pub security_diagnostics: bool,
    pub experimental_diagnostics: bool,
    pub security_levels: BTreeMap<String, DiagnosticLevel>,
    pub strict_native_security: bool,
    pub typing_suppression: Option<TypingSuppressionRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypingSuppressionRegion {
    pub range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Off,
    Warn,
    Error,
    Hint,
}

impl DiagnosticLevel {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "off" | "disabled" | "false" => Some(Self::Off),
            "warn" | "warning" | "true" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "hint" => Some(Self::Hint),
            _ => None,
        }
    }

    fn severity(self) -> Option<DiagnosticSeverity> {
        match self {
            Self::Off => None,
            Self::Warn => Some(DiagnosticSeverity::WARNING),
            Self::Error => Some(DiagnosticSeverity::ERROR),
            Self::Hint => Some(DiagnosticSeverity::HINT),
        }
    }
}

impl Default for DiagnosticSettings {
    fn default() -> Self {
        Self {
            security_diagnostics: true,
            experimental_diagnostics: true,
            security_levels: BTreeMap::new(),
            strict_native_security: true,
            typing_suppression: None,
        }
    }
}

pub fn arbitrate(diagnostics: Vec<Diagnostic>, settings: DiagnosticSettings) -> Vec<Diagnostic> {
    let typing_suppression = settings.typing_suppression.clone();
    dedupe(prefer_highest_confidence_by_topic(suppress_typing_region(
        apply_settings(diagnostics, settings),
        typing_suppression,
    )))
}

fn apply_settings(diagnostics: Vec<Diagnostic>, settings: DiagnosticSettings) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter_map(|mut diagnostic| {
            let Some(code) = diagnostic_code(&diagnostic) else {
                return Some(diagnostic);
            };
            if !settings.security_diagnostics && is_security_code(code) {
                return None;
            }
            if !settings.experimental_diagnostics && code == "anchor-missing-init-constraint" {
                return None;
            }
            if !settings.strict_native_security
                && code == "solana-code-quality"
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("strictNative"))
                    .and_then(|value| value.as_bool())
                    == Some(true)
            {
                return None;
            }
            if let Some(level) = config_key(&diagnostic)
                .and_then(|key| settings.security_levels.get(key))
                .copied()
            {
                diagnostic.severity = level.severity();
                return (level != DiagnosticLevel::Off).then_some(diagnostic);
            }
            Some(diagnostic)
        })
        .collect()
}

fn suppress_typing_region(
    diagnostics: Vec<Diagnostic>,
    typing_suppression: Option<TypingSuppressionRegion>,
) -> Vec<Diagnostic> {
    let Some(region) = typing_suppression else {
        return diagnostics;
    };

    diagnostics
        .into_iter()
        .filter(|diagnostic| !ranges_touch(diagnostic.range, region.range))
        .collect()
}

pub fn dedupe(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = HashSet::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            seen.insert((
                diagnostic.range.start.line,
                diagnostic.range.start.character,
                diagnostic.range.end.line,
                diagnostic.range.end.character,
                diagnostic_code(diagnostic).unwrap_or_default().to_string(),
                diagnostic.message.clone(),
            ))
        })
        .collect()
}

fn prefer_highest_confidence_by_topic(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut selected = Vec::new();

    for diagnostic in diagnostics {
        let Some(topic_key) = diagnostic_topic_key(&diagnostic) else {
            selected.push(diagnostic);
            continue;
        };
        let Some(existing_index) = selected
            .iter()
            .position(|existing| diagnostic_topic_key(existing).as_ref() == Some(&topic_key))
        else {
            selected.push(diagnostic);
            continue;
        };

        if confidence_rank(&diagnostic) > confidence_rank(&selected[existing_index]) {
            let demoted = std::mem::replace(&mut selected[existing_index], diagnostic);
            append_related_diagnostic(&mut selected[existing_index], demoted);
        } else {
            append_related_diagnostic(&mut selected[existing_index], diagnostic);
        }
    }

    selected
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticTopicKey {
    start_line: u32,
    start_character: u32,
    end_line: u32,
    end_character: u32,
    topic: String,
}

fn diagnostic_topic_key(diagnostic: &Diagnostic) -> Option<DiagnosticTopicKey> {
    Some(DiagnosticTopicKey {
        start_line: diagnostic.range.start.line,
        start_character: diagnostic.range.start.character,
        end_line: diagnostic.range.end.line,
        end_character: diagnostic.range.end.character,
        topic: diagnostic_topic(diagnostic)?.to_string(),
    })
}

fn diagnostic_topic(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("topic"))
        .and_then(|value| value.as_str())
}

fn confidence_rank(diagnostic: &Diagnostic) -> u8 {
    match diagnostic_confidence(diagnostic) {
        Some("authoritative" | "high") => 3,
        Some("derived" | "medium") => 2,
        Some("heuristic" | "low") => 1,
        _ => 0,
    }
}

fn diagnostic_confidence(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("confidence"))
        .and_then(|value| value.as_str())
}

fn append_related_diagnostic(survivor: &mut Diagnostic, mut demoted: Diagnostic) {
    let Ok(uri) = Url::parse(CURRENT_DOCUMENT_PLACEHOLDER_URI) else {
        return;
    };
    let related = survivor.related_information.get_or_insert_with(Vec::new);
    related.push(DiagnosticRelatedInformation {
        location: Location {
            uri,
            range: demoted.range,
        },
        message: demoted_related_message(&demoted),
    });
    if let Some(mut demoted_related) = demoted.related_information.take() {
        related.append(&mut demoted_related);
    }
}

fn demoted_related_message(diagnostic: &Diagnostic) -> String {
    diagnostic_code(diagnostic)
        .map(|code| format!("{code}: {}", diagnostic.message))
        .unwrap_or_else(|| diagnostic.message.clone())
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<&str> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(code) => Some(code.as_str()),
        NumberOrString::Number(_) => None,
    }
}

fn is_security_code(code: &str) -> bool {
    code.starts_with("anchor-security-") || code == "solana-code-quality"
}

fn config_key(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("configKey"))
        .and_then(|value| value.as_str())
}

fn ranges_touch(left: Range, right: Range) -> bool {
    position_le(left.start, right.end) && position_le(right.start, left.end)
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        tower_lsp::lsp_types::{DiagnosticSeverity, Position, Range},
    };

    fn diagnostic(code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position {
                    line: 1,
                    character: 2,
                },
                end: Position {
                    line: 1,
                    character: 5,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(code.to_string())),
            code_description: None,
            source: Some("seagrass".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    fn diagnostic_with_topic(
        code: &str,
        message: &str,
        topic: &str,
        confidence: &str,
    ) -> Diagnostic {
        let mut diagnostic = diagnostic(code, message);
        diagnostic.data = Some(serde_json::json!({
            "topic": topic,
            "confidence": confidence,
        }));
        diagnostic
    }

    #[test]
    fn dedupes_same_range_code_and_message() {
        let diagnostics = dedupe(vec![
            diagnostic("anchor-context-accounts", "missing context"),
            diagnostic("anchor-context-accounts", "missing context"),
        ]);

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn keeps_highest_confidence_for_same_span_and_topic() {
        let diagnostics = arbitrate(
            vec![
                diagnostic_with_topic(
                    "solana-code-quality",
                    "heuristic signer near init",
                    "init-payer",
                    "medium",
                ),
                diagnostic_with_topic(
                    "anchor-init-constraints",
                    "missing payer for init",
                    "init-payer",
                    "high",
                ),
            ],
            DiagnosticSettings::default(),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "missing payer for init");
        let related = diagnostics[0]
            .related_information
            .as_ref()
            .expect("demoted diagnostic should be related");
        assert!(related
            .iter()
            .any(|info| info.message.contains("heuristic signer near init")));
    }

    #[test]
    fn keeps_different_topics_on_same_span() {
        let diagnostics = arbitrate(
            vec![
                diagnostic_with_topic(
                    "anchor-init-constraints",
                    "missing payer",
                    "init-payer",
                    "high",
                ),
                diagnostic_with_topic(
                    "anchor-init-constraints",
                    "missing space",
                    "init-space",
                    "high",
                ),
            ],
            DiagnosticSettings::default(),
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn suppresses_diagnostics_overlapping_active_typing_region() {
        let diagnostics = arbitrate(
            vec![
                diagnostic("anchor-init-constraints", "missing payer"),
                Diagnostic {
                    range: Range {
                        start: Position {
                            line: 3,
                            character: 0,
                        },
                        end: Position {
                            line: 3,
                            character: 5,
                        },
                    },
                    ..diagnostic("anchor-context-accounts", "missing accounts")
                },
            ],
            DiagnosticSettings {
                typing_suppression: Some(TypingSuppressionRegion {
                    range: Range {
                        start: Position {
                            line: 1,
                            character: 3,
                        },
                        end: Position {
                            line: 1,
                            character: 3,
                        },
                    },
                }),
                ..DiagnosticSettings::default()
            },
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostic_code(&diagnostics[0]),
            Some("anchor-context-accounts")
        );
    }

    #[test]
    fn settings_filter_security_and_experimental_diagnostics() {
        let diagnostics = arbitrate(
            vec![
                diagnostic("anchor-security-signer", "unchecked signer"),
                diagnostic("anchor-missing-init-constraint", "missing init"),
                diagnostic("anchor-context-accounts", "missing accounts"),
            ],
            DiagnosticSettings {
                security_diagnostics: false,
                experimental_diagnostics: false,
                security_levels: BTreeMap::new(),
                strict_native_security: true,
                typing_suppression: None,
            },
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostic_code(&diagnostics[0]),
            Some("anchor-context-accounts")
        );
    }

    #[test]
    fn settings_can_raise_or_disable_security_families_by_config_key() {
        let mut owner = diagnostic("anchor-security-owner-check", "owner");
        owner.data = Some(serde_json::json!({ "configKey": "security.ownerChecks" }));
        let mut signer = diagnostic("solana-code-quality", "signer");
        signer.data = Some(serde_json::json!({
            "configKey": "security.signerAuthorization",
            "strictNative": true,
        }));
        let mut levels = BTreeMap::new();
        levels.insert("security.ownerChecks".to_string(), DiagnosticLevel::Error);
        levels.insert(
            "security.signerAuthorization".to_string(),
            DiagnosticLevel::Off,
        );

        let diagnostics = arbitrate(
            vec![owner, signer],
            DiagnosticSettings {
                security_diagnostics: true,
                experimental_diagnostics: true,
                security_levels: levels,
                strict_native_security: true,
                typing_suppression: None,
            },
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }
}
