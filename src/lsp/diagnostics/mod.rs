mod account_references;
mod account_usage;
mod anchor_syn;
mod arbitration;
mod artifacts;
pub(crate) mod check_cfg;
mod code_quality;
mod constraint_expressions;
mod constraint_shape;
mod context_accounts;
mod ecosystem;
mod engine;
mod handler_members;
mod handler_scope;
mod handler_struct_literals;
mod initialization;
mod instruction_attributes;
pub(crate) mod lint;
mod pda;
pub(crate) mod project_identity;
mod registry;
mod rules;
mod security;
mod spl_semantics;
mod suppression;

use {
    crate::{
        constraint_catalog, constraint_ranges,
        document::ParsedDocument,
        range::{line_at, range_from_span},
        workspace::WorkspaceIndex,
    },
    proc_macro2::Span,
    registry::AnchorDiagnosticKind,
    tower_lsp::lsp_types::{
        CodeDescription, Diagnostic, DiagnosticRelatedInformation, NumberOrString, Position, Range,
        Url,
    },
};

pub(crate) use arbitration::{DiagnosticLevel, DiagnosticSettings, TypingSuppressionRegion};
pub(crate) use engine::DiagnosticInput;
#[cfg(test)]
pub(crate) use registry::ANCHOR_SECURITY_SIGNER_CODE;
pub use registry::{
    ANCHOR_CONSTRAINT_EXPRESSION_CODE, ANCHOR_INIT_CONSTRAINTS_CODE,
    ANCHOR_MISSING_INIT_CONSTRAINT_CODE, ANCHOR_PDA_SEED_RESOLUTION_CODE,
    INIT_PLACEHOLDERS_QUICKFIX, REPLACE_CONSTRAINT_EXPRESSION_IDENTIFIER_QUICKFIX,
    REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX, SOURCE,
};

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    collect_with_input(DiagnosticInput {
        document,
        uri: None,
        workspace_index,
        framework: crate::solana::frameworks::FrameworkContext::from_document(document),
        manifest: None,
        anchor_toml: None,
        seagrass_toml: None,
        solana_program: None,
        settings: DiagnosticSettings::default(),
    })
}

pub(crate) fn collect_with_input(input: DiagnosticInput<'_>) -> Vec<Diagnostic> {
    engine::collect(input)
}

pub(crate) fn collect_hot_with_input(input: DiagnosticInput<'_>) -> Vec<Diagnostic> {
    engine::collect_hot(input)
}

pub fn diagnostic_from_parse_error_with_source(err: syn::Error, source: &str) -> Diagnostic {
    let parser_message = err.to_string();
    let corrected_range = parse_error_range_in_source(&err, source, &parser_message);
    let range = corrected_range.unwrap_or_else(|| range_from_span(err.span()));
    handler_scope::parse_error_unresolved_identifier_diagnostic(source, range)
        .unwrap_or_else(|| diagnostic_from_syn_error_with_range(err, Some(range)))
}

pub(crate) fn diagnostic_from_syn_error(err: syn::Error) -> Diagnostic {
    diagnostic_from_syn_error_with_range(err, None)
}

fn diagnostic_from_syn_error_with_range(err: syn::Error, range: Option<Range>) -> Diagnostic {
    let parser_message = err.to_string();
    let is_init_constraint = parser_message_has(&parser_message, INIT_PAYER_REQUIRED_MESSAGE)
        || parser_message_has(&parser_message, INIT_SPACE_REQUIRED_MESSAGE);
    let parser_rule = (!is_init_constraint)
        .then(|| constraint_catalog::parser_rule_for_message(&parser_message))
        .flatten();
    let kind = if is_init_constraint {
        AnchorDiagnosticKind::AnchorInitConstraints
    } else if parser_rule.is_some() {
        AnchorDiagnosticKind::AnchorConstraintShape
    } else {
        AnchorDiagnosticKind::AnchorSyn
    };
    let data = if is_init_constraint {
        Some(serde_json::json!({
            "quickfix": INIT_PLACEHOLDERS_QUICKFIX,
            "parserMessage": parser_message,
            "topic": init_constraint_topic(&parser_message),
        }))
    } else {
        parser_rule.map(|(spec, rule)| {
            serde_json::json!({
                "constraint": constraint_catalog::key(spec.label),
                "constraintLabel": spec.label,
                "parserRule": {
                    "kind": constraint_catalog::parser_rule_kind_name(rule.kind),
                    "message": rule.message,
                    "sourceMethod": rule.source_method,
                },
                "generatedFrom": "lang/syn/src/parser/accounts/constraints.rs",
            })
        })
    };
    let message = if is_init_constraint {
        init_constraint_message(&parser_message)
    } else if let Some((spec, rule)) = parser_rule {
        parser_rule_diagnostic_message(spec, rule, &parser_message)
    } else {
        parser_message
    };

    diagnostic_from_range(
        range.unwrap_or_else(|| range_from_span(err.span())),
        kind,
        message,
        data,
    )
}

fn init_constraint_topic(parser_message: &str) -> &'static str {
    if parser_message_has(parser_message, INIT_PAYER_REQUIRED_MESSAGE) {
        return ANCHOR_INIT_PAYER_TOPIC;
    }
    if parser_message_has(parser_message, INIT_SPACE_REQUIRED_MESSAGE) {
        return ANCHOR_INIT_SPACE_TOPIC;
    }
    "seagrass/anchor.init.constraints"
}

fn init_constraint_message(parser_message: &str) -> String {
    if parser_message_has(parser_message, INIT_PAYER_REQUIRED_MESSAGE) {
        return "Anchor `init` constraint is missing `payer = ...`; add the account that funds initialization.".to_string();
    }
    if parser_message_has(parser_message, INIT_SPACE_REQUIRED_MESSAGE) {
        return "Anchor `init` constraint is missing `space = ...`; add discriminator plus account data size.".to_string();
    }
    parser_message.to_string()
}

fn parser_rule_diagnostic_message(
    spec: &constraint_catalog::ConstraintSpec,
    rule: &constraint_catalog::ConstraintParserRule,
    parser_message: &str,
) -> String {
    let key = constraint_catalog::key(spec.label);
    match rule.kind {
        constraint_catalog::ConstraintParserRuleKind::Duplicate => {
            format!(
                "Anchor account constraint `{key}` is duplicated; remove the duplicate `{key}`."
            )
        }
        constraint_catalog::ConstraintParserRuleKind::Ordering => {
            if let Some((required, before)) = rule.message.split_once(" must be provided before ") {
                return format!(
                    "Anchor account constraints are out of order; place `{}` before `{}`.",
                    parser_rule_phrase_label(required),
                    parser_rule_phrase_label(before)
                );
            }
            format!("Anchor account constraint `{key}` is out of order.")
        }
        constraint_catalog::ConstraintParserRuleKind::Conflict => {
            if let Some((left, right)) = rule.message.split_once(" cannot be used with ") {
                return format!(
                    "Anchor account constraints conflict; remove `{}` or `{}`.",
                    parser_rule_phrase_label(left),
                    parser_rule_phrase_label(right)
                );
            }
            format!("Anchor account constraint `{key}` conflicts with another constraint.")
        }
        constraint_catalog::ConstraintParserRuleKind::TypeRequirement => {
            if rule.message.contains("close must be on") {
                return "`close` only works on `Account`, `LazyAccount`, or `AccountLoader` fields."
                    .to_string();
            }
            if rule.message.contains("Discriminator") {
                return "`zero` requires an account type that implements `Discriminator`; remove `zero` or use a compatible account type.".to_string();
            }
            format!("Anchor account constraint `{key}` is on an unsupported account type: {parser_message}.")
        }
    }
}

fn parser_rule_phrase_label(phrase: &str) -> String {
    let normalized = normalized_constraint_phrase(phrase);
    constraint_catalog::CONSTRAINTS
        .iter()
        .find_map(|spec| {
            let key = constraint_catalog::key(spec.label);
            (normalized_constraint_phrase(key) == normalized).then(|| key.to_string())
        })
        .unwrap_or_else(|| phrase.trim().trim_matches('`').to_string())
}

fn normalized_constraint_phrase(value: &str) -> String {
    value
        .trim()
        .trim_matches('`')
        .replace("::", " ")
        .replace('_', " ")
        .split_whitespace()
        .filter(|part| *part != "account")
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

const INIT_PAYER_REQUIRED_MESSAGE: &str = "payer must be provided";
const INIT_SPACE_REQUIRED_MESSAGE: &str = "space must be provided";
const EXPECTED_SEMICOLON_PARSE_MESSAGE: &str = "unexpected token, expected `;`";
const STATEMENT_TERMINATOR_CHARS: &[char] = &[';', '{', '}', ',', '(', '['];
const MAX_PREVIOUS_UNTERMINATED_STATEMENT_SCAN_LINES: u32 = 3;
const ATTRIBUTE_CLOSE_LINE_PREFIX: &str = ")]";

fn parser_message_has(parser_message: &str, expected: &str) -> bool {
    parser_message.contains(expected)
}

fn parse_error_range_in_source(
    err: &syn::Error,
    source: &str,
    parser_message: &str,
) -> Option<Range> {
    if !parser_message_has(parser_message, EXPECTED_SEMICOLON_PARSE_MESSAGE) {
        return None;
    }
    previous_unterminated_statement_range(source, range_from_span(err.span()).start.line)
}

fn previous_unterminated_statement_range(source: &str, error_line: u32) -> Option<Range> {
    let earliest_line = error_line.saturating_sub(MAX_PREVIOUS_UNTERMINATED_STATEMENT_SCAN_LINES);
    let candidate_line = (earliest_line..error_line).rev().find(|line_number| {
        line_at(source, *line_number).is_some_and(line_is_unterminated_statement_candidate)
    })?;
    let line = line_at(source, candidate_line)?;
    let trimmed_end = line.trim_end();
    let start_character = line.chars().position(|ch| !ch.is_whitespace())?;
    Some(Range {
        start: Position {
            line: candidate_line,
            character: u32::try_from(start_character).ok()?,
        },
        end: Position {
            line: candidate_line,
            character: u32::try_from(trimmed_end.chars().count()).ok()?,
        },
    })
}

fn line_is_unterminated_statement_candidate(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with("#[")
        || trimmed.starts_with(ATTRIBUTE_CLOSE_LINE_PREFIX)
    {
        return false;
    }
    trimmed
        .chars()
        .next_back()
        .is_some_and(|ch| !STATEMENT_TERMINATOR_CHARS.contains(&ch))
}

pub(crate) fn diagnostic_from_span(
    span: Span,
    kind: AnchorDiagnosticKind,
    message: String,
    data: Option<serde_json::Value>,
) -> Diagnostic {
    diagnostic_from_range(range_from_span(span), kind, message, data)
}

pub(crate) fn diagnostic_from_range(
    range: Range,
    kind: AnchorDiagnosticKind,
    message: String,
    data: Option<serde_json::Value>,
) -> Diagnostic {
    diagnostic_from_range_with_related(range, kind, message, data, None)
}

pub(crate) fn diagnostic_from_range_with_related(
    range: Range,
    kind: AnchorDiagnosticKind,
    message: String,
    data: Option<serde_json::Value>,
    related_information: Option<Vec<DiagnosticRelatedInformation>>,
) -> Diagnostic {
    let code_description = kind
        .docs_url_for_data(data.as_ref())
        .map(|href| CodeDescription { href });
    Diagnostic {
        range,
        severity: Some(kind.default_severity()),
        code: Some(NumberOrString::String(kind.code().to_string())),
        code_description,
        source: Some(SOURCE.to_string()),
        message,
        related_information,
        tags: None,
        data: diagnostic_data(kind, data),
    }
}

pub(crate) fn dedupe(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    arbitration::dedupe(diagnostics)
}

const CURRENT_DOCUMENT_PLACEHOLDER_URI: &str = "file:///seagrass/current-document.rs";
const ANCHOR_INIT_PAYER_TOPIC: &str = "seagrass/anchor.init.missing-payer";
const ANCHOR_INIT_SPACE_TOPIC: &str = "seagrass/anchor.init.missing-space";
const PAYER_COMPANION: &str = "payer";
const SPACE_COMPANION: &str = "space";
const INIT_FALLBACK_KEY: &str = "init";
const RELATED_ACCOUNT_DATA_KEYS: &[&str] = &[
    "account",
    "field",
    "missing",
    "peer",
    "requiredByAccount",
    "usedByAccount",
];
const RELATED_CONTEXT_DATA_KEYS: &[&str] = &["accountsStruct", "context"];
const RELATED_CONSTRAINT_DATA_KEYS: &[&str] =
    &["constraint", "missing", "requiredBy", "conflictsWith"];

pub(crate) fn bind_current_document_related_uri(
    diagnostics: &mut [Diagnostic],
    current_document_uri: &Url,
) {
    for diagnostic in diagnostics.iter_mut() {
        let Some(related_information) = diagnostic.related_information.as_mut() else {
            continue;
        };
        for info in related_information.iter_mut() {
            if info.location.uri.as_str() == CURRENT_DOCUMENT_PLACEHOLDER_URI {
                info.location.uri = current_document_uri.clone();
            }
        }
    }
}

pub(crate) fn enrich_confidence_related_information(diagnostics: &mut [Diagnostic]) {
    for diagnostic in diagnostics.iter_mut() {
        if diagnostic
            .related_information
            .as_ref()
            .is_some_and(|related| {
                related
                    .iter()
                    .any(|info| info.message.starts_with("Seagrass confidence:"))
            })
        {
            continue;
        }

        let related_information = diagnostic_metadata_related_information(diagnostic);
        if related_information.is_empty() {
            continue;
        }

        let related = diagnostic.related_information.get_or_insert_with(Vec::new);
        for info in related_information.into_iter().rev() {
            if !related
                .iter()
                .any(|existing| same_related_information(existing, &info))
            {
                related.insert(0, info);
            }
        }
    }
}

fn diagnostic_metadata_related_information(
    diagnostic: &Diagnostic,
) -> Vec<DiagnosticRelatedInformation> {
    let mut related_information = Vec::new();
    let confidence = diagnostic_data_str(diagnostic, "confidence");
    let topic = diagnostic_data_str(diagnostic, "topic");
    let applicability = diagnostic_data_str(diagnostic, "applicability");
    let quickfix = diagnostic_data_str(diagnostic, "quickfix");

    if let Some(summary) = diagnostic_metadata_summary(confidence, topic, applicability, quickfix) {
        if let Some(info) = current_document_related_information(diagnostic.range, summary) {
            related_information.push(info);
        }
    }

    if let Some(confidence) = confidence.and_then(confidence_reason) {
        if let Some(info) =
            current_document_related_information(diagnostic.range, confidence.to_string())
        {
            related_information.push(info);
        }
    }

    if let Some(preview) = quickfix.map(quickfix_preview) {
        if let Some(info) = current_document_related_information(diagnostic.range, preview) {
            related_information.push(info);
        }
    }

    related_information
}

fn diagnostic_metadata_summary(
    confidence: Option<&str>,
    topic: Option<&str>,
    applicability: Option<&str>,
    quickfix: Option<&str>,
) -> Option<String> {
    let mut message = match (confidence, topic) {
        (Some(confidence), Some(topic)) => {
            format!("Seagrass confidence: {confidence}; topic: {topic}")
        }
        (Some(confidence), None) => format!("Seagrass confidence: {confidence}"),
        (None, Some(topic)) => format!("Seagrass topic: {topic}"),
        _ => return None,
    };
    if let Some(applicability) = applicability {
        message.push_str(&format!("; applicability: {applicability}"));
    }
    if let Some(quickfix) = quickfix {
        message.push_str(&format!("; quickfix: {quickfix}"));
    }
    Some(message)
}

fn confidence_reason(confidence: &str) -> Option<&'static str> {
    match confidence {
        "authoritative" => Some(
            "Why authoritative: parsed Anchor or Solana semantic evidence directly identifies this finding.",
        ),
        "derived" => Some(
            "Why derived: Seagrass resolved the finding through local semantic inference, such as account or handler type flow.",
        ),
        "heuristic" => Some(
            "Why heuristic: Seagrass used conservative pattern evidence; review the local code path before broad fixes.",
        ),
        _ => None,
    }
}

fn quickfix_preview(quickfix: &str) -> String {
    let preview = match quickfix {
        "add-mut-constraint" => "add an Anchor `mut` constraint",
        "add-missing-constraint" => "add the missing Anchor account constraint",
        "add-instruction-argument" => "add the missing instruction argument",
        "replace-instruction-argument" => "replace the mistyped instruction argument",
        "remove-instruction-argument" => "remove the extra instruction argument",
        "replace-account-type" => "replace the account field type",
        "replace-invalid-sysvar" => "replace the invalid sysvar account type",
        "program-field-type" => "replace the program field with the typed program account",
        "system-program-type" => "replace the field with the System program account type",
        "replace-has-one-target" => "replace the `has_one` target with an in-scope field",
        "replace-keyword-value" => "replace the invalid constraint keyword value",
        "remove-conflicting-constraints" => "remove the conflicting Anchor constraint",
        "remove-handler-field-call" => "remove the method-call suffix from the account data field",
        "use-checked-data-access" => "replace native instruction byte access with checked access",
        "add-static-pda-domain-seed" => "add a static domain seed to the PDA",
        "prefer-anchor-close" => "use Anchor close semantics instead of manual lamport draining",
        "reject-reinit" => "guard against reinitializing account data",
        "insert-reload-after-cpi" => "reload account data after CPI before reading it",
        "add-signer-check" => "add a signer validation check",
        "add-writable-check" => "add a writable account validation check",
        "add-program-id-check" => "add a program id validation check",
        "add-owner-check" => "add an owner validation check",
        "add-discriminator-check" => "add an account discriminator validation check",
        _ => "open the lightbulb for the available Seagrass edit",
    };
    format!("Fix preview: {preview}.")
}

pub(crate) fn enrich_current_document_related_information(
    document: &ParsedDocument,
    diagnostics: &mut [Diagnostic],
) {
    let mut related_index = None;
    for diagnostic in diagnostics {
        let mut related_information = Vec::new();
        if is_init_companion_diagnostic(diagnostic) {
            if let Some(init_related) = init_companion_related_information(document, diagnostic) {
                related_information.extend(init_related);
            }
        }
        if has_semantic_related_data(diagnostic) {
            let index = related_index.get_or_insert_with(|| RelatedInformationIndex::new(document));
            related_information.extend(semantic_related_information(index, diagnostic));
        }

        if !related_information.is_empty() {
            let existing = diagnostic.related_information.get_or_insert_with(Vec::new);
            append_unique_related_information(existing, related_information);
        }
    }
}

fn has_semantic_related_data(diagnostic: &Diagnostic) -> bool {
    RELATED_CONSTRAINT_DATA_KEYS
        .iter()
        .chain(RELATED_ACCOUNT_DATA_KEYS)
        .chain(RELATED_CONTEXT_DATA_KEYS)
        .any(|key| diagnostic_data_str(diagnostic, key).is_some())
}

fn is_init_companion_diagnostic(diagnostic: &Diagnostic) -> bool {
    matches!(
        diagnostic_data_str(diagnostic, "topic"),
        Some(ANCHOR_INIT_PAYER_TOPIC | ANCHOR_INIT_SPACE_TOPIC)
    )
}

fn init_companion_related_information(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let missing = init_missing_companion(diagnostic)?;
    let cursor = document.account_attribute_cursor(diagnostic.range.end)?;
    let init_key = constraint_ranges::constraint_key_ranges(document.source(), cursor.range)
        .into_iter()
        .find(|range| constraint_catalog::INIT_LIKE_KEYS.contains(&range.key))
        .map(|range| (range.key, range.range));
    let (init_key, init_key_range) = init_key.unwrap_or((INIT_FALLBACK_KEY, diagnostic.range));
    let field = cursor
        .field_name
        .as_deref()
        .and_then(|name| account_field_by_name(document, name))?;

    Some(vec![
        current_document_related_information(
            init_key_range,
            format!("`{init_key}` requires this companion `{missing} = ...`."),
        )?,
        current_document_related_information(
            field.selection_range,
            format!("`{}` is the account being initialized.", field.name),
        )?,
        current_document_related_information(
            cursor.range,
            format!("Add the missing companion `{missing} = ...` inside this account attribute."),
        )?,
    ])
}

fn init_missing_companion(diagnostic: &Diagnostic) -> Option<&'static str> {
    match diagnostic_data_str(diagnostic, "topic")? {
        ANCHOR_INIT_PAYER_TOPIC => Some(PAYER_COMPANION),
        ANCHOR_INIT_SPACE_TOPIC => Some(SPACE_COMPANION),
        _ => None,
    }
}

fn semantic_related_information(
    index: &RelatedInformationIndex<'_>,
    diagnostic: &Diagnostic,
) -> Vec<DiagnosticRelatedInformation> {
    let mut related_information = Vec::new();

    for key in RELATED_CONSTRAINT_DATA_KEYS {
        if let Some(constraint) = diagnostic_data_str(diagnostic, key) {
            if let Some(info) = constraint_related_information(index, diagnostic, constraint) {
                related_information.push(info);
            }
        }
    }

    for key in RELATED_ACCOUNT_DATA_KEYS {
        if let Some(account) = diagnostic_data_str(diagnostic, key) {
            if let Some(field) = account_field_by_name(index.document, account) {
                if let Some(info) = current_document_related_information(
                    field.selection_range,
                    format!("Account field declaration for `{}`.", field.name),
                ) {
                    related_information.push(info);
                }
            }
        }
    }

    for key in RELATED_CONTEXT_DATA_KEYS {
        if let Some(context) = diagnostic_data_str(diagnostic, key) {
            if let Some(accounts) = accounts_struct_by_name(index.document, context) {
                if let Some(info) = current_document_related_information(
                    accounts.selection_range,
                    format!("Accounts context declaration for `{}`.", accounts.name),
                ) {
                    related_information.push(info);
                }
            }
        }
    }

    dedupe_related_information(related_information)
}

fn constraint_related_information(
    index: &RelatedInformationIndex<'_>,
    diagnostic: &Diagnostic,
    constraint: &str,
) -> Option<DiagnosticRelatedInformation> {
    let range = index.matching_constraint_key_range(diagnostic.range, constraint)?;
    current_document_related_information(range, format!("`{constraint}` constraint declaration."))
}

struct RelatedInformationIndex<'a> {
    document: &'a ParsedDocument,
    constraint_ranges: Vec<constraint_ranges::ConstraintKeyRange>,
}

impl<'a> RelatedInformationIndex<'a> {
    fn new(document: &'a ParsedDocument) -> Self {
        let constraint_ranges = document
            .tree_sitter()
            .map(|syntax| syntax.anchor_query_captures(document.source()))
            .unwrap_or_default()
            .into_iter()
            .filter(|capture| capture.kind == crate::syntax::AnchorQueryKind::AccountAttribute)
            .flat_map(|capture| {
                constraint_ranges::constraint_key_ranges(document.source(), capture.range)
            })
            .collect();
        Self {
            document,
            constraint_ranges,
        }
    }

    fn matching_constraint_key_range(
        &self,
        diagnostic_range: Range,
        constraint: &str,
    ) -> Option<Range> {
        self.constraint_ranges
            .iter()
            .filter(|range| range.key == constraint)
            .min_by_key(|range| {
                (
                    !ranges_overlap(range.range, diagnostic_range),
                    range.range.start.line,
                    range.range.start.character,
                )
            })
            .map(|range| range.range)
    }
}

fn ranges_overlap(left: Range, right: Range) -> bool {
    constraint_ranges::contains_position(left, right.start)
        || constraint_ranges::contains_position(left, right.end)
        || constraint_ranges::contains_position(right, left.start)
        || constraint_ranges::contains_position(right, left.end)
}

fn append_unique_related_information(
    existing: &mut Vec<DiagnosticRelatedInformation>,
    incoming: Vec<DiagnosticRelatedInformation>,
) {
    for info in incoming {
        if !existing
            .iter()
            .any(|existing| same_related_information(existing, &info))
        {
            existing.push(info);
        }
    }
}

fn dedupe_related_information(
    incoming: Vec<DiagnosticRelatedInformation>,
) -> Vec<DiagnosticRelatedInformation> {
    let mut deduped = Vec::new();
    append_unique_related_information(&mut deduped, incoming);
    deduped
}

fn same_related_information(
    left: &DiagnosticRelatedInformation,
    right: &DiagnosticRelatedInformation,
) -> bool {
    left.message == right.message
        && left.location.uri == right.location.uri
        && left.location.range == right.location.range
}

fn account_field_by_name<'a>(
    document: &'a ParsedDocument,
    field_name: &str,
) -> Option<&'a crate::document::SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| accounts.fields.iter())
        .find(|field| field.name == field_name)
}

fn accounts_struct_by_name<'a>(
    document: &'a ParsedDocument,
    name: &str,
) -> Option<&'a crate::document::SymbolRange> {
    document.symbols().accounts_structs.get(name)
}

fn current_document_related_information(
    range: Range,
    message: String,
) -> Option<DiagnosticRelatedInformation> {
    Some(DiagnosticRelatedInformation {
        location: tower_lsp::lsp_types::Location {
            uri: Url::parse(CURRENT_DOCUMENT_PLACEHOLDER_URI).ok()?,
            range,
        },
        message,
    })
}

fn diagnostic_data(
    kind: AnchorDiagnosticKind,
    data: Option<serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut object = match data {
        Some(serde_json::Value::Object(object)) => object,
        Some(value) => {
            return Some(value);
        }
        None => serde_json::Map::new(),
    };

    if let Some(rule) = kind.rule() {
        object
            .entry("rule".to_string())
            .or_insert_with(|| serde_json::Value::String(rule.to_string()));
    }
    object
        .entry("confidence".to_string())
        .or_insert_with(|| serde_json::Value::String(kind.confidence().to_string()));
    object
        .entry("applicability".to_string())
        .or_insert_with(|| serde_json::Value::String("Unspecified".to_string()));
    object
        .entry("topic".to_string())
        .or_insert_with(|| serde_json::Value::String(kind.topic().to_string()));
    object
        .entry("truthSource".to_string())
        .or_insert_with(|| serde_json::Value::String(kind.truth_source().as_str().to_string()));
    let anchor_errors = crate::anchor_errors::diagnostic_error_data(kind.anchor_error_names());
    if !anchor_errors.is_empty() {
        object.insert(
            "anchorErrors".to_string(),
            serde_json::Value::Array(anchor_errors),
        );
    }

    (!object.is_empty()).then_some(serde_json::Value::Object(object))
}

fn diagnostic_data_str<'a>(diagnostic: &'a Diagnostic, key: &str) -> Option<&'a str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
}

#[cfg(test)]
mod tests;
