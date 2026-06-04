use {
    crate::range::range_from_span,
    serde_json::Value,
    tower_lsp::lsp_types::{
        CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString, Range, Url,
    },
};

pub type LspDiagnostic = Diagnostic;
pub const SOURCE: &str = "seagrass";
pub const SOLANA_CODE_QUALITY_CODE: &str = "solana-code-quality";
const SOLANA_CODE_QUALITY_RULE: &str = "solana/program-code-quality";
const DEFAULT_CONFIDENCE: &str = "heuristic";
const DEFAULT_APPLICABILITY: &str = "Unspecified";
const DEFAULT_TOPIC: &str = "seagrass/solana.code-quality";
const SOLANA_CODE_QUALITY_DOCS_URL: &str = "https://solana.com/developers/courses/program-security";

#[derive(Debug, Clone, Copy)]
pub struct FrameworkDocument<'a> {
    source: &'a str,
    syntax: &'a syn::File,
}

impl<'a> FrameworkDocument<'a> {
    pub const fn new(source: &'a str, syntax: &'a syn::File) -> Self {
        Self { source, syntax }
    }

    pub const fn source(self) -> &'a str {
        self.source
    }

    pub const fn syntax(self) -> &'a syn::File {
        self.syntax
    }
}

pub fn solana_code_quality_from_span(
    span: proc_macro2::Span,
    message: String,
    data: Option<Value>,
) -> Diagnostic {
    solana_code_quality_from_range(range_from_span(span), message, data)
}

pub fn solana_code_quality_from_range(
    range: Range,
    message: String,
    data: Option<Value>,
) -> Diagnostic {
    let data = diagnostic_data(data);
    let code_description = docs_url_for_data(data.as_ref()).map(|href| CodeDescription { href });
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(SOLANA_CODE_QUALITY_CODE.to_string())),
        code_description,
        source: Some(SOURCE.to_string()),
        message,
        related_information: None,
        tags: None,
        data,
    }
}

fn diagnostic_data(data: Option<Value>) -> Option<Value> {
    let mut object = match data {
        Some(Value::Object(object)) => object,
        Some(value) => return Some(value),
        None => serde_json::Map::new(),
    };

    object
        .entry("rule".to_string())
        .or_insert_with(|| Value::String(SOLANA_CODE_QUALITY_RULE.to_string()));
    object
        .entry("confidence".to_string())
        .or_insert_with(|| Value::String(DEFAULT_CONFIDENCE.to_string()));
    object
        .entry("applicability".to_string())
        .or_insert_with(|| Value::String(DEFAULT_APPLICABILITY.to_string()));
    object
        .entry("topic".to_string())
        .or_insert_with(|| Value::String(DEFAULT_TOPIC.to_string()));

    (!object.is_empty()).then_some(Value::Object(object))
}

fn docs_url_for_data(data: Option<&Value>) -> Option<Url> {
    data.and_then(|value| value.get("topic"))
        .and_then(Value::as_str)
        .and_then(topic_lint_doc_url)
        .or_else(|| {
            data.and_then(|value| value.get("docsUrl"))
                .and_then(Value::as_str)
                .and_then(parse_url)
        })
        .or_else(|| parse_url(SOLANA_CODE_QUALITY_DOCS_URL))
}

fn topic_lint_doc_url(topic: &str) -> Option<Url> {
    let slug = topic.strip_prefix("seagrass/")?;
    parse_url(&format!(
        "https://github.com/heyAyushh/seagrass/blob/main/docs/lints/seagrass-{}.md",
        slug.replace(['.', '/'], "-")
    ))
}

fn parse_url(url: &str) -> Option<Url> {
    Url::parse(url).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solana_code_quality_payload_matches_root_lsp_contract() {
        let diagnostic = solana_code_quality_from_range(
            Range::default(),
            "message".to_string(),
            Some(serde_json::json!({
                "topic": "seagrass/security.owner-check",
                "quickfix": "add-owner-check",
            })),
        );

        assert_eq!(diagnostic.source.as_deref(), Some(SOURCE));
        assert_eq!(
            diagnostic.code,
            Some(NumberOrString::String(SOLANA_CODE_QUALITY_CODE.to_string()))
        );
        assert!(diagnostic.code_description.is_some());
        assert_eq!(
            diagnostic.data.as_ref().and_then(|data| data.get("rule")),
            Some(&serde_json::json!(SOLANA_CODE_QUALITY_RULE))
        );
        assert_eq!(
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("confidence")),
            Some(&serde_json::json!(DEFAULT_CONFIDENCE))
        );
    }
}
