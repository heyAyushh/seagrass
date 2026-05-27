use {
    crate::{constraint_ranges, document::ParsedDocument},
    quote::ToTokens,
    serde::Deserialize,
    std::collections::HashMap,
    syn::spanned::Spanned,
    syn::visit::{self, Visit},
    tower_lsp::lsp_types::{Diagnostic, NumberOrString, Range},
};

const LINE_ALLOW_MARKER: &str = "seagrass-allow:";
const FILE_ALLOW_MARKER: &str = "seagrass-allow-file:";
const NEXT_LINE_OFFSET: u32 = 1;
const CODE_RULE_SEPARATOR: &str = ".";

pub(super) fn filter(
    document: &ParsedDocument,
    seagrass_toml: Option<&str>,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let index = SuppressionIndex::from_document(document, seagrass_toml);
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
    fn from_document(document: &ParsedDocument, seagrass_toml: Option<&str>) -> Self {
        let mut index = Self::default();
        index.collect_workspace_suppressions(seagrass_toml);
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

    fn collect_workspace_suppressions(&mut self, seagrass_toml: Option<&str>) {
        self.file_patterns
            .extend(workspace_allow_patterns(seagrass_toml));
    }

    fn collect_comment_suppressions(&mut self, source: &str) {
        for (index, line) in source.split('\n').enumerate() {
            let Ok(line_number) = u32::try_from(index) else {
                continue;
            };
            if let Some(patterns) = comment_patterns(line, FILE_ALLOW_MARKER) {
                self.file_patterns.extend(patterns);
            }
            if let Some(patterns) = comment_patterns(line, LINE_ALLOW_MARKER) {
                self.line_patterns
                    .entry(line_number)
                    .or_default()
                    .extend(patterns.clone());
                self.line_patterns
                    .entry(line_number + NEXT_LINE_OFFSET)
                    .or_default()
                    .extend(patterns);
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

fn normalized_config_pattern(pattern: String) -> Option<String> {
    let pattern = pattern.trim();
    (!pattern.is_empty()).then(|| pattern.to_string())
}

fn comment_patterns(line: &str, marker: &str) -> Option<Vec<String>> {
    let comment = line.split_once("//")?.1;
    let patterns = comment.split_once(marker)?.1.trim();
    Some(suppression_patterns(patterns))
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
  "seagrass/solana.code-quality.unchecked-arithmetic",
  "unchecked-arithmetic",
]
"#,
        ));

        assert_eq!(
            patterns,
            vec![
                "seagrass/solana.code-quality.unchecked-arithmetic".to_string(),
                "unchecked-arithmetic".to_string()
            ]
        );
    }

    #[test]
    fn workspace_config_malformed_toml_is_ignored() {
        assert!(workspace_allow_patterns(Some("[lints]\nallow = [")).is_empty());
    }

    #[test]
    fn suppression_matches_code_rule_pattern() {
        assert!(suppression_matches(
            "solana-code-quality.unchecked-arithmetic",
            Some("seagrass/solana.code-quality.unchecked-arithmetic"),
            Some("unchecked-arithmetic"),
            Some("solana-code-quality"),
        ));
    }
}
