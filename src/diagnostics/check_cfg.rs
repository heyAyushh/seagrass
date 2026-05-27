use {
    crate::{
        constraint_text,
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        range::range_from_span,
    },
    cargo_toml::Manifest,
    syn::spanned::Spanned,
    tower_lsp::lsp_types::{Position, Range, TextEdit, Url},
};

pub const ADD_ANCHOR_DEBUG_FEATURE_QUICKFIX: &str = "add-anchor-debug-feature";
pub const ADD_INIT_IF_NEEDED_FEATURE_QUICKFIX: &str = "add-init-if-needed-feature";

pub fn collect(
    document: &ParsedDocument,
    manifest_uri: &Url,
    manifest_text: &str,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let mut diagnostics = Vec::new();

    if manifest_needs_anchor_debug(manifest_text) {
        let mut macro_sites = document
            .symbols()
            .accounts_structs
            .values()
            .map(|symbol| CheckCfgMacroSite {
                name: symbol.name.clone(),
                range: symbol
                    .derive_accounts_range
                    .unwrap_or(symbol.selection_range),
                macro_kind: AnchorMacroKind::AccountsDerive,
            })
            .collect::<Vec<_>>();
        macro_sites.extend(program_macro_sites(document));
        macro_sites.sort_by_key(|site| (site.range.start.line, site.range.start.character));

        diagnostics.extend(macro_sites.into_iter().map(|site| {
            diagnostic_from_range(
                site.range,
                AnchorDiagnosticKind::AnchorCheckCfg,
                site.message(),
                Some(serde_json::json!({
                    "quickfix": ADD_ANCHOR_DEBUG_FEATURE_QUICKFIX,
                    "manifest": manifest_uri.as_str(),
                })),
            )
        }));
    }

    if manifest_needs_init_if_needed(document, manifest_text) {
        diagnostics.extend(init_if_needed_sites(document).into_iter().map(|site| {
            diagnostic_from_range(
                site.range,
                AnchorDiagnosticKind::AnchorCheckCfg,
                format!(
                    "`{}` uses `init_if_needed`, but this crate's `anchor-lang` dependency is missing the `init-if-needed` Cargo feature.",
                    site.account
                ),
                Some(serde_json::json!({
                    "quickfix": ADD_INIT_IF_NEEDED_FEATURE_QUICKFIX,
                    "manifest": manifest_uri.as_str(),
                    "feature": "init-if-needed",
                })),
            )
        }));
    }

    diagnostics
}

struct CheckCfgMacroSite {
    name: String,
    range: Range,
    macro_kind: AnchorMacroKind,
}

impl CheckCfgMacroSite {
    fn message(&self) -> String {
        match self.macro_kind {
            AnchorMacroKind::AccountsDerive => format!(
                "`{}` derives `Accounts`, but this crate's Cargo.toml is missing the `anchor-debug` feature expected by Anchor's derive expansion under Rust check-cfg.",
                self.name
            ),
            AnchorMacroKind::ProgramAttribute => format!(
                "`{}` uses Anchor's `#[program]` macro, but this crate's Cargo.toml is missing the `anchor-debug` feature expected by Anchor's program macro expansion under Rust check-cfg.",
                self.name
            ),
        }
    }
}

enum AnchorMacroKind {
    AccountsDerive,
    ProgramAttribute,
}

struct InitIfNeededSite {
    account: String,
    range: Range,
}

fn program_macro_sites(document: &ParsedDocument) -> Vec<CheckCfgMacroSite> {
    document
        .syntax()
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Mod(item_mod) = item else {
                return None;
            };
            let attr = item_mod
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("program"))?;
            Some(CheckCfgMacroSite {
                name: item_mod.ident.to_string(),
                range: range_from_span(attr.path().span()),
                macro_kind: AnchorMacroKind::ProgramAttribute,
            })
        })
        .collect()
}

pub fn manifest_needs_anchor_debug(manifest_text: &str) -> bool {
    !features_section_contains(manifest_text, "anchor-debug")
}

pub fn manifest_needs_init_if_needed(document: &ParsedDocument, manifest_text: &str) -> bool {
    !init_if_needed_sites(document).is_empty()
        && !anchor_lang_dependency_has_feature(manifest_text, "init-if-needed")
}

pub fn anchor_debug_feature_edit(manifest_text: &str) -> Option<TextEdit> {
    if !manifest_needs_anchor_debug(manifest_text) {
        return None;
    }

    if let Some(line) = features_header_line(manifest_text) {
        let insert_line = line.saturating_add(1);
        return Some(TextEdit {
            range: Range {
                start: Position {
                    line: u32::try_from(insert_line).ok()?,
                    character: 0,
                },
                end: Position {
                    line: u32::try_from(insert_line).ok()?,
                    character: 0,
                },
            },
            new_text: "anchor-debug = []\n".to_string(),
        });
    }

    let position = end_position(manifest_text)?;
    let prefix = if manifest_text.is_empty() || manifest_text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    Some(TextEdit {
        range: Range {
            start: position,
            end: position,
        },
        new_text: format!("{prefix}\n[features]\nanchor-debug = []\n"),
    })
}

pub fn anchor_lang_feature_edit(manifest_text: &str, feature: &str) -> Option<TextEdit> {
    let dependency = anchor_lang_dependency_line(manifest_text)?;
    if dependency_has_feature(dependency.line, feature) {
        return None;
    }

    if let Some(edit) = inline_table_feature_edit(dependency, feature) {
        return Some(edit);
    }
    quoted_dependency_feature_edit(dependency, feature)
}

fn init_if_needed_sites(document: &ParsedDocument) -> Vec<InitIfNeededSite> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| {
            accounts.fields.iter().flat_map(|field| {
                field
                    .account_constraints
                    .iter()
                    .filter(|constraint| {
                        constraint_text::has_flag_or_key(&constraint.text, "init_if_needed")
                    })
                    .map(|constraint| InitIfNeededSite {
                        account: field.name.clone(),
                        range: constraint.range,
                    })
            })
        })
        .collect()
}

fn anchor_lang_dependency_has_feature(manifest_text: &str, feature: &str) -> bool {
    anchor_lang_dependency_line(manifest_text)
        .is_some_and(|dependency| dependency_has_feature(dependency.line, feature))
}

#[derive(Debug, Clone, Copy)]
struct ManifestLine<'a> {
    index: usize,
    line: &'a str,
}

fn anchor_lang_dependency_line(manifest_text: &str) -> Option<ManifestLine<'_>> {
    manifest_text
        .lines()
        .enumerate()
        .find(|(_, line)| {
            let trimmed = line.split('#').next().unwrap_or_default().trim_start();
            trimmed
                .strip_prefix("anchor-lang")
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
        .map(|(index, line)| ManifestLine { index, line })
}

fn dependency_has_feature(line: &str, feature: &str) -> bool {
    let manifest = format!("[dependencies]\n{line}");
    Manifest::from_str(&manifest)
        .ok()
        .and_then(|manifest| manifest.dependencies.get("anchor-lang").cloned())
        .is_some_and(|dependency| dependency.req_features().iter().any(|name| name == feature))
}

fn inline_table_feature_edit(dependency: ManifestLine<'_>, feature: &str) -> Option<TextEdit> {
    let uncommented = dependency.line.split('#').next().unwrap_or_default();
    uncommented.find('{')?;
    let table_end = last_char_index(uncommented, '}')?;

    let new_text = if let Some(features_idx) = uncommented.find("features") {
        let relative_close = uncommented[features_idx..].find(']')?;
        let insert_at = features_idx + relative_close;
        format!(
            "{}{}{}",
            &dependency.line[..insert_at],
            format_args!(", \"{feature}\""),
            &dependency.line[insert_at..]
        )
    } else {
        format!(
            "{}{}{}",
            &dependency.line[..table_end].trim_end(),
            format_args!(", features = [\"{feature}\"]"),
            &dependency.line[table_end..]
        )
    };

    Some(full_line_edit(dependency, &new_text))
}

fn last_char_index(text: &str, target: char) -> Option<usize> {
    text.char_indices()
        .rev()
        .find_map(|(index, ch)| (ch == target).then_some(index))
}

fn quoted_dependency_feature_edit(dependency: ManifestLine<'_>, feature: &str) -> Option<TextEdit> {
    let uncommented = dependency.line.split('#').next().unwrap_or_default();
    let equals = uncommented.find('=')?;
    let value = uncommented[equals + 1..].trim();
    if !(value.starts_with('"') && value.ends_with('"')) {
        return None;
    }

    let prefix = dependency.line[..=equals].trim_end();
    let new_text = format!("{prefix} {{ version = {value}, features = [\"{feature}\"] }}");
    Some(full_line_edit(dependency, &new_text))
}

fn full_line_edit(dependency: ManifestLine<'_>, new_text: &str) -> TextEdit {
    TextEdit {
        range: Range {
            start: Position {
                line: u32::try_from(dependency.index).unwrap_or_default(),
                character: 0,
            },
            end: Position {
                line: u32::try_from(dependency.index).unwrap_or_default(),
                character: u32::try_from(dependency.line.chars().count()).unwrap_or_default(),
            },
        },
        new_text: format!("{new_text}\n"),
    }
}

fn features_section_contains(manifest_text: &str, feature: &str) -> bool {
    let Some(header_line) = features_header_line(manifest_text) else {
        return false;
    };

    manifest_text
        .lines()
        .enumerate()
        .skip(header_line + 1)
        .take_while(|(_, line)| !is_table_header(line))
        .any(|(_, line)| {
            line.split('#')
                .next()
                .unwrap_or_default()
                .trim_start()
                .strip_prefix(feature)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
}

fn features_header_line(manifest_text: &str) -> Option<usize> {
    manifest_text
        .lines()
        .enumerate()
        .find_map(|(idx, line)| (line.trim() == "[features]").then_some(idx))
}

fn is_table_header(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[') && trimmed.ends_with(']')
}

fn end_position(text: &str) -> Option<Position> {
    let line = text.lines().count();
    let character = text.lines().last().map_or(0, |line| line.chars().count());
    Some(Position {
        line: u32::try_from(line.saturating_sub(usize::from(!text.ends_with('\n')))).ok()?,
        character: if text.ends_with('\n') {
            0
        } else {
            u32::try_from(character).ok()?
        },
    })
}

#[cfg(test)]
mod tests {
    use {super::*, tower_lsp::lsp_types::NumberOrString};

    #[test]
    fn reports_accounts_derive_when_manifest_lacks_anchor_debug() {
        let document = ParsedDocument::parse(
            r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    pub user: Signer<'info>,
}
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[features]
default = []
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(matches!(
            diagnostics[0].code.as_ref(),
            Some(NumberOrString::String(code)) if code == "anchor-check-cfg"
        ));
        assert_eq!(
            diagnostics[0]
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str()),
            Some(ADD_ANCHOR_DEBUG_FEATURE_QUICKFIX)
        );
    }

    #[test]
    fn reports_program_attribute_when_manifest_lacks_anchor_debug() {
        let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[features]
default = []
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("#[program]"));
        assert_eq!(diagnostics[0].range.start.line, 1);
    }

    #[test]
    fn reports_each_anchor_macro_site_when_manifest_lacks_anchor_debug() {
        let document = ParsedDocument::parse(
            r#"
#[program]
pub mod demo {}

#[derive(Accounts)]
pub struct Initialize<'info> {}
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[features]
default = []
"#,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("#[program]"));
        assert!(diagnostics[1].message.contains("derives `Accounts`"));
    }

    #[test]
    fn ignores_manifest_that_declares_anchor_debug() {
        let document = ParsedDocument::parse(
            r#"
#[derive(Accounts)]
pub struct Initialize<'info> {}
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[features]
anchor-debug = []
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_init_if_needed_when_anchor_lang_feature_is_missing() {
        let document = ParsedDocument::parse(
            r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[features]
anchor-debug = []

[dependencies]
anchor-lang = "1.0.0"
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("init-if-needed"));
        assert_eq!(
            diagnostics[0]
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str()),
            Some(ADD_INIT_IF_NEEDED_FEATURE_QUICKFIX)
        );
    }

    #[test]
    fn accepts_init_if_needed_when_anchor_lang_feature_is_present() {
        let document = ParsedDocument::parse(
            r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = payer)]
    pub state: Account<'info, State>,
}
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[features]
anchor-debug = []

[dependencies]
anchor-lang = { version = "1.0.0", features = ["init-if-needed"] }
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn inserts_anchor_debug_under_existing_features_header() {
        let edit = anchor_debug_feature_edit(
            r#"
[features]
default = []

[dependencies]
anchor-lang = "0.31"
"#,
        )
        .unwrap();

        assert_eq!(edit.range.start.line, 2);
        assert_eq!(edit.new_text, "anchor-debug = []\n");
    }

    #[test]
    fn appends_features_section_when_missing() {
        let edit = anchor_debug_feature_edit("[package]\nname = \"demo\"\n").unwrap();

        assert_eq!(edit.range.start.line, 2);
        assert_eq!(edit.range.start.character, 0);
        assert_eq!(edit.new_text, "\n[features]\nanchor-debug = []\n");
    }

    #[test]
    fn converts_anchor_lang_version_to_feature_table() {
        let edit = anchor_lang_feature_edit(
            r#"[dependencies]
anchor-lang = "1.0.0"
"#,
            "init-if-needed",
        )
        .unwrap();

        assert_eq!(edit.range.start.line, 1);
        assert_eq!(
            edit.new_text,
            "anchor-lang = { version = \"1.0.0\", features = [\"init-if-needed\"] }\n"
        );
    }

    #[test]
    fn inserts_anchor_lang_feature_into_inline_table() {
        let edit = anchor_lang_feature_edit(
            r#"[dependencies]
anchor-lang = { version = "1.0.0", features = ["idl-build"] }
"#,
            "init-if-needed",
        )
        .unwrap();

        assert_eq!(
            edit.new_text,
            "anchor-lang = { version = \"1.0.0\", features = [\"idl-build\", \"init-if-needed\"] }\n"
        );
    }
}
