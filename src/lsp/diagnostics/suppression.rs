use {
    crate::{constraint_ranges, document::ParsedDocument, range},
    quote::ToTokens,
    serde::Deserialize,
    std::collections::HashMap,
    syn::spanned::Spanned,
    syn::visit::{self, Visit},
    tower_lsp::lsp_types::{Diagnostic, NumberOrString, Range},
};

const LINE_ALLOW_MARKER: &str = "seagrass-allow:";
const FILE_ALLOW_MARKER: &str = "seagrass-allow-file:";
const FILE_IGNORE_MARKER: &str = "seagrass-ignore-file";
const LINE_IGNORE_MARKER: &str = "seagrass-ignore";
const LINE_COMMENT_DELIMITER: &str = "//";
const ANY_SUPPRESSION_PATTERN: &str = "*";
const NEXT_LINE_OFFSET: u32 = 1;
const CODE_RULE_SEPARATOR: &str = ".";
const PACKAGE_SUPPRESS_PATH: &[&str] = &["package", "metadata", "seagrass", "suppress"];
const WORKSPACE_SUPPRESS_PATH: &[&str] = &["workspace", "metadata", "seagrass", "suppress"];
#[allow(dead_code)] // Consumed by scripts/check-seagrass-toml-contract.ts as the TOML key canon.
pub const SEAGRASS_TOML_KEY_PATHS: &[&str] = &["lints.allow"];

#[derive(Clone, Copy, Default)]
pub(crate) struct SuppressionConfig<'a> {
    pub(crate) seagrass_toml: Option<&'a str>,
    pub(crate) manifest: Option<&'a str>,
    pub(crate) workspace_manifest: Option<&'a str>,
}

pub(crate) fn filter(
    document: &ParsedDocument,
    config: SuppressionConfig<'_>,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let index = SuppressionIndex::from_document(document, config);
    diagnostics
        .into_iter()
        .filter(|diagnostic| !index.suppresses(diagnostic))
        .collect()
}

#[derive(Default, Deserialize)]
struct SeagrassConfig {
    #[serde(default)]
    lints: LintConfig,
}

#[derive(Default, Deserialize)]
struct LintConfig {
    #[serde(default)]
    allow: Vec<String>,
}

#[derive(Default)]
struct SuppressionIndex {
    file_patterns: Vec<String>,
    line_patterns: HashMap<u32, Vec<String>>,
    range_patterns: Vec<RangeSuppression>,
}

struct RangeSuppression {
    range: Range,
    patterns: Vec<String>,
}

impl SuppressionIndex {
    fn from_document(document: &ParsedDocument, config: SuppressionConfig<'_>) -> Self {
        let mut index = Self::default();
        index.collect_workspace_suppressions(config);
        index.collect_comment_suppressions(document.source());
        index.collect_attribute_suppressions(document.syntax());
        index
    }

    fn suppresses(&self, diagnostic: &Diagnostic) -> bool {
        let topic = diagnostic_data(diagnostic, "topic");
        let rule = diagnostic_data(diagnostic, "rule");
        let code = diagnostic_code(diagnostic);
        self.file_patterns
            .iter()
            .any(|pattern| suppression_matches(pattern, topic, rule, code))
            || self
                .line_patterns_for(diagnostic.range.start.line)
                .any(|pattern| suppression_matches(pattern.as_str(), topic, rule, code))
            || self.range_patterns.iter().any(|suppression| {
                constraint_ranges::contains_position(suppression.range, diagnostic.range.start)
                    && suppression
                        .patterns
                        .iter()
                        .any(|pattern| suppression_matches(pattern, topic, rule, code))
            })
    }

    fn line_patterns_for(&self, line: u32) -> impl Iterator<Item = &String> {
        self.line_patterns.get(&line).into_iter().flatten()
    }

    fn collect_workspace_suppressions(&mut self, config: SuppressionConfig<'_>) {
        self.file_patterns
            .extend(workspace_allow_patterns(config.seagrass_toml));
        if cargo_manifest_suppresses_all(config.manifest)
            || cargo_manifest_suppresses_all(config.workspace_manifest)
        {
            self.file_patterns.push(ANY_SUPPRESSION_PATTERN.to_string());
        }
    }

    fn collect_comment_suppressions(&mut self, source: &str) {
        for (index, line) in source.split('\n').enumerate() {
            let Ok(line_number) = u32::try_from(index) else {
                continue;
            };
            let Some(comment) = line_comment(source, line, line_number) else {
                continue;
            };
            if let Some(patterns) = comment_patterns(comment, FILE_ALLOW_MARKER) {
                self.file_patterns.extend(patterns);
            }
            if comment_has_marker(comment, FILE_IGNORE_MARKER)
                && is_leading_file_comment(source, line_number)
            {
                self.file_patterns.push(ANY_SUPPRESSION_PATTERN.to_string());
            }
            if let Some(patterns) = comment_patterns(comment, LINE_ALLOW_MARKER) {
                self.line_patterns
                    .entry(line_number)
                    .or_default()
                    .extend(patterns.clone());
                self.line_patterns
                    .entry(line_number + NEXT_LINE_OFFSET)
                    .or_default()
                    .extend(patterns);
            }
            if comment_has_marker(comment, LINE_IGNORE_MARKER)
                && !comment_has_marker(comment, FILE_IGNORE_MARKER)
            {
                self.line_patterns
                    .entry(line_number)
                    .or_default()
                    .push(ANY_SUPPRESSION_PATTERN.to_string());
                self.line_patterns
                    .entry(line_number + NEXT_LINE_OFFSET)
                    .or_default()
                    .push(ANY_SUPPRESSION_PATTERN.to_string());
            }
        }
    }

    fn collect_attribute_suppressions(&mut self, syntax: &syn::File) {
        let mut visitor = AttributeSuppressionVisitor::default();
        visitor.visit_file(syntax);
        self.range_patterns.extend(visitor.range_patterns);
    }
}

#[derive(Default)]
struct AttributeSuppressionVisitor {
    range_patterns: Vec<RangeSuppression>,
}

impl AttributeSuppressionVisitor {
    fn collect_attrs(&mut self, attrs: &[syn::Attribute], span: proc_macro2::Span) {
        let patterns = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("seagrass"))
            .flat_map(|attr| suppression_patterns(&attr.meta.to_token_stream().to_string()))
            .collect::<Vec<_>>();
        if !patterns.is_empty() {
            self.range_patterns.push(RangeSuppression {
                range: crate::range::range_from_span(span),
                patterns,
            });
        }
    }
}

impl<'ast> Visit<'ast> for AttributeSuppressionVisitor {
    fn visit_item(&mut self, node: &'ast syn::Item) {
        self.collect_attrs(item_attrs(node), node.span());
        visit::visit_item(self, node);
    }

    fn visit_impl_item(&mut self, node: &'ast syn::ImplItem) {
        self.collect_attrs(impl_item_attrs(node), node.span());
        visit::visit_impl_item(self, node);
    }

    fn visit_trait_item(&mut self, node: &'ast syn::TraitItem) {
        self.collect_attrs(trait_item_attrs(node), node.span());
        visit::visit_trait_item(self, node);
    }

    fn visit_foreign_item(&mut self, node: &'ast syn::ForeignItem) {
        self.collect_attrs(foreign_item_attrs(node), node.span());
        visit::visit_foreign_item(self, node);
    }

    fn visit_expr_block(&mut self, node: &'ast syn::ExprBlock) {
        self.collect_attrs(&node.attrs, node.span());
        visit::visit_expr_block(self, node);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        self.collect_attrs(&node.attrs, node.span());
        visit::visit_local(self, node);
    }
}

fn workspace_allow_patterns(seagrass_toml: Option<&str>) -> Vec<String> {
    let Some(seagrass_toml) = seagrass_toml else {
        return Vec::new();
    };
    let Ok(config) = toml::from_str::<SeagrassConfig>(seagrass_toml) else {
        return Vec::new();
    };

    config
        .lints
        .allow
        .into_iter()
        .filter_map(normalized_config_pattern)
        .collect()
}

fn cargo_manifest_suppresses_all(manifest: Option<&str>) -> bool {
    let Some(manifest) = manifest else {
        return false;
    };
    let Ok(value) = toml::from_str::<toml::Value>(manifest) else {
        return false;
    };
    toml_bool_at_path(&value, PACKAGE_SUPPRESS_PATH)
        || toml_bool_at_path(&value, WORKSPACE_SUPPRESS_PATH)
}

fn toml_bool_at_path(value: &toml::Value, path: &[&str]) -> bool {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

fn normalized_config_pattern(pattern: String) -> Option<String> {
    let pattern = pattern.trim();
    (!pattern.is_empty()).then(|| pattern.to_string())
}

fn line_comment<'a>(source: &str, line: &'a str, line_number: u32) -> Option<&'a str> {
    line_comment_delimiter_offsets(line)
        .into_iter()
        .find_map(|offset| {
            let character = u32::try_from(line[..offset].chars().count()).ok()?;
            (!range::is_in_comment_or_string(
                source,
                tower_lsp::lsp_types::Position {
                    line: line_number,
                    character,
                },
            ))
            .then_some(&line[offset + LINE_COMMENT_DELIMITER.len()..])
        })
}

fn line_comment_delimiter_offsets(line: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut previous_slash_offset = None;

    for (offset, ch) in line.char_indices() {
        if ch == '/' {
            if let Some(start) = previous_slash_offset {
                offsets.push(start);
            }
            previous_slash_offset = Some(offset);
        } else {
            previous_slash_offset = None;
        }
    }

    offsets
}

fn comment_patterns(comment: &str, marker: &str) -> Option<Vec<String>> {
    let patterns = comment.split_once(marker)?.1.trim();
    Some(suppression_patterns(patterns))
}

fn comment_has_marker(comment: &str, marker: &str) -> bool {
    comment.contains(marker)
}

fn is_leading_file_comment(source: &str, line_number: u32) -> bool {
    let Ok(line_count) = usize::try_from(line_number) else {
        return false;
    };
    source.split('\n').take(line_count).all(is_header_line)
}

fn is_header_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
        || trimmed.starts_with("#![")
}

fn suppression_patterns(text: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if is_pattern_char(ch) {
            current.push(ch);
        } else if !current.is_empty() {
            patterns.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        patterns.push(current);
    }
    patterns
}

fn is_pattern_char(ch: char) -> bool {
    ch == '/' || ch == '.' || ch == '-' || ch == '_' || ch.is_ascii_alphanumeric()
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(item) => &item.attrs,
        syn::ImplItem::Fn(item) => &item.attrs,
        syn::ImplItem::Type(item) => &item.attrs,
        syn::ImplItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &syn::TraitItem) -> &[syn::Attribute] {
    match item {
        syn::TraitItem::Const(item) => &item.attrs,
        syn::TraitItem::Fn(item) => &item.attrs,
        syn::TraitItem::Type(item) => &item.attrs,
        syn::TraitItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn foreign_item_attrs(item: &syn::ForeignItem) -> &[syn::Attribute] {
    match item {
        syn::ForeignItem::Fn(item) => &item.attrs,
        syn::ForeignItem::Static(item) => &item.attrs,
        syn::ForeignItem::Type(item) => &item.attrs,
        syn::ForeignItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn suppression_matches(
    pattern: &str,
    topic: Option<&str>,
    rule: Option<&str>,
    code: Option<&str>,
) -> bool {
    if pattern == ANY_SUPPRESSION_PATTERN {
        return true;
    }
    topic.is_some_and(|topic| topic == pattern || topic.ends_with(&format!(".{pattern}")))
        || rule.is_some_and(|rule| rule == pattern || rule.ends_with(&format!("/{pattern}")))
        || code.is_some_and(|code| code == pattern)
        || code_rule_matches(pattern, code, rule)
}

fn code_rule_matches(pattern: &str, code: Option<&str>, rule: Option<&str>) -> bool {
    let (Some(code), Some(rule)) = (code, rule) else {
        return false;
    };
    pattern
        .strip_prefix(code)
        .and_then(|rest| rest.strip_prefix(CODE_RULE_SEPARATOR))
        == Some(rule)
}

fn diagnostic_data<'a>(diagnostic: &'a Diagnostic, key: &str) -> Option<&'a str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<&str> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(code) => Some(code),
        NumberOrString::Number(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_config_extracts_lint_allow_patterns() {
        let patterns = workspace_allow_patterns(Some(
            r#"
[lints]
allow = [
  "seagrass/solana.code-quality.unsafe-unwrap",
  "unsafe-unwrap",
]
"#,
        ));

        assert_eq!(
            patterns,
            vec![
                "seagrass/solana.code-quality.unsafe-unwrap".to_string(),
                "unsafe-unwrap".to_string()
            ]
        );
    }

    #[test]
    fn workspace_config_malformed_toml_is_ignored() {
        assert!(workspace_allow_patterns(Some("[lints]\nallow = [")).is_empty());
    }

    #[test]
    fn cargo_package_metadata_suppresses_all_diagnostics() {
        let manifest = r#"
[package]
name = "demo"
version = "0.1.0"

[package.metadata.seagrass]
suppress = true
"#;

        assert!(cargo_manifest_suppresses_all(Some(manifest)));
    }

    #[test]
    fn cargo_workspace_metadata_suppresses_all_diagnostics() {
        let manifest = r#"
[workspace]
members = ["programs/demo"]

[workspace.metadata.seagrass]
suppress = true
"#;

        assert!(cargo_manifest_suppresses_all(Some(manifest)));
    }

    #[test]
    fn cargo_metadata_requires_boolean_suppress() {
        let manifest = r#"
[package.metadata.seagrass]
suppress = "true"
"#;

        assert!(!cargo_manifest_suppresses_all(Some(manifest)));
    }

    #[test]
    fn seagrass_toml_key_paths_are_sorted_and_unique() {
        for pair in SEAGRASS_TOML_KEY_PATHS.windows(2) {
            assert!(
                pair[0] < pair[1],
                "Seagrass.toml key paths must be sorted and unique: {SEAGRASS_TOML_KEY_PATHS:?}"
            );
        }
    }

    #[test]
    fn suppression_matches_code_rule_pattern() {
        assert!(suppression_matches(
            "solana-code-quality.unsafe-unwrap",
            Some("seagrass/solana.code-quality.unsafe-unwrap"),
            Some("unsafe-unwrap"),
            Some("solana-code-quality"),
        ));
    }

    #[test]
    fn seagrass_ignore_suppresses_current_and_next_line() {
        let source = r#"
fn handler() {
    // seagrass-ignore
    let value = maybe_value().unwrap();
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let index = SuppressionIndex::from_document(&document, SuppressionConfig::default());
        let diagnostic = unsafe_unwrap_diagnostic(3);

        assert!(index.suppresses(&diagnostic));
    }

    #[test]
    fn seagrass_ignore_inside_string_does_not_suppress() {
        let source = r#"
fn handler() {
    let marker = "// seagrass-ignore";
    let value = maybe_value().unwrap();
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let index = SuppressionIndex::from_document(&document, SuppressionConfig::default());
        let diagnostic = unsafe_unwrap_diagnostic(3);

        assert!(!index.suppresses(&diagnostic));
    }

    #[test]
    fn seagrass_ignore_file_suppresses_only_from_header_comment() {
        let header_source = r#"
// seagrass-ignore-file
fn handler() {
    let value = maybe_value().unwrap();
}
"#;
        let header_document = ParsedDocument::parse(header_source).unwrap();
        let header_index =
            SuppressionIndex::from_document(&header_document, SuppressionConfig::default());

        assert!(header_index.suppresses(&unsafe_unwrap_diagnostic(3)));

        let body_source = r#"
fn handler() {
    // seagrass-ignore-file
    let value = maybe_value().unwrap();
}
"#;
        let body_document = ParsedDocument::parse(body_source).unwrap();
        let body_index =
            SuppressionIndex::from_document(&body_document, SuppressionConfig::default());

        assert!(!body_index.suppresses(&unsafe_unwrap_diagnostic(3)));
    }

    fn unsafe_unwrap_diagnostic(line: u32) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: tower_lsp::lsp_types::Position { line, character: 8 },
                end: tower_lsp::lsp_types::Position {
                    line,
                    character: 14,
                },
            },
            severity: None,
            code: Some(NumberOrString::String("solana-code-quality".to_string())),
            code_description: None,
            source: None,
            message: "unsafe unwrap".to_string(),
            related_information: None,
            tags: None,
            data: Some(serde_json::json!({
                "topic": "seagrass/solana.code-quality.unsafe-unwrap",
                "rule": "unsafe-unwrap",
            })),
        }
    }
}
