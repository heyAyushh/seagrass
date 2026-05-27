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
pub const ADD_SOLANA_TARGET_OS_CHECK_CFG_QUICKFIX: &str = "add-solana-target-os-check-cfg";

const SOLANA_TARGET_OS_CHECK_CFG: &str = "cfg(target_os, values(\"solana\"))";
const UNEXPECTED_CFGS_SECTION: &str = "lints.rust.unexpected_cfgs";

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

    if manifest_needs_solana_target_os_check_cfg(document, manifest_text) {
        if let Some(site) = solana_target_os_check_cfg_site(document) {
            diagnostics.push(diagnostic_from_range(
                site.range,
                AnchorDiagnosticKind::AnchorCheckCfg,
                site.message(),
                Some(serde_json::json!({
                    "quickfix": ADD_SOLANA_TARGET_OS_CHECK_CFG_QUICKFIX,
                    "manifest": manifest_uri.as_str(),
                    "checkCfg": SOLANA_TARGET_OS_CHECK_CFG,
                })),
            ));
        }
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

struct SolanaTargetOsSite {
    name: String,
    range: Range,
    kind: SolanaTargetOsSiteKind,
}

impl SolanaTargetOsSite {
    fn message(&self) -> String {
        match self.kind {
            SolanaTargetOsSiteKind::ProgramId => format!(
                "`{}` declares a Solana program; add `{}` to Cargo.toml check-cfg so Rust accepts Solana cfgs.",
                self.name,
                SOLANA_TARGET_OS_CHECK_CFG
            ),
            SolanaTargetOsSiteKind::ProgramAttribute => format!(
                "`{}` uses Anchor's `#[program]` macro; add `{}` to Cargo.toml check-cfg so Rust accepts Solana cfgs.",
                self.name,
                SOLANA_TARGET_OS_CHECK_CFG
            ),
            SolanaTargetOsSiteKind::AccountsDerive => format!(
                "`{}` derives `Accounts`; add `{}` to Cargo.toml check-cfg so Rust accepts Solana cfgs.",
                self.name,
                SOLANA_TARGET_OS_CHECK_CFG
            ),
        }
    }
}

enum SolanaTargetOsSiteKind {
    ProgramId,
    ProgramAttribute,
    AccountsDerive,
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

fn solana_target_os_check_cfg_site(document: &ParsedDocument) -> Option<SolanaTargetOsSite> {
    if let Some(declared_program_id) = document.symbols().declared_program_id.as_ref() {
        return Some(SolanaTargetOsSite {
            name: "this program".to_string(),
            range: declared_program_id.range,
            kind: SolanaTargetOsSiteKind::ProgramId,
        });
    }

    if let Some(program_site) = program_macro_sites(document).into_iter().next() {
        return Some(SolanaTargetOsSite {
            name: program_site.name,
            range: program_site.range,
            kind: SolanaTargetOsSiteKind::ProgramAttribute,
        });
    }

    document
        .symbols()
        .accounts_structs
        .values()
        .min_by_key(|symbol| {
            let range = symbol
                .derive_accounts_range
                .unwrap_or(symbol.selection_range);
            (range.start.line, range.start.character)
        })
        .map(|symbol| SolanaTargetOsSite {
            name: symbol.name.clone(),
            range: symbol
                .derive_accounts_range
                .unwrap_or(symbol.selection_range),
            kind: SolanaTargetOsSiteKind::AccountsDerive,
        })
}

pub fn manifest_needs_anchor_debug(manifest_text: &str) -> bool {
    !features_section_contains(manifest_text, "anchor-debug")
}

pub fn manifest_needs_init_if_needed(document: &ParsedDocument, manifest_text: &str) -> bool {
    !init_if_needed_sites(document).is_empty()
        && !anchor_lang_dependency_has_feature(manifest_text, "init-if-needed")
}

pub fn manifest_needs_solana_target_os_check_cfg(
    document: &ParsedDocument,
    manifest_text: &str,
) -> bool {
    solana_target_os_check_cfg_site(document).is_some()
        && !manifest_has_solana_target_os_check_cfg(manifest_text)
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

pub fn solana_target_os_check_cfg_edit(manifest_text: &str) -> Option<TextEdit> {
    if manifest_has_solana_target_os_check_cfg(manifest_text) {
        return None;
    }

    if let Some(edit) = existing_solana_target_os_check_cfg_section_edit(manifest_text) {
        return Some(edit);
    }

    let owner = solana_target_os_check_cfg_owner(manifest_text);
    let section = format!("[{owner}.{UNEXPECTED_CFGS_SECTION}]");
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
        new_text: format!(
            "{prefix}\n{section}\nlevel = \"warn\"\ncheck-cfg = [\n    '{SOLANA_TARGET_OS_CHECK_CFG}',\n]\n"
        ),
    })
}

fn existing_solana_target_os_check_cfg_section_edit(manifest_text: &str) -> Option<TextEdit> {
    let section = solana_target_os_check_cfg_section(manifest_text)?;
    if let Some(array_end_line) =
        check_cfg_array_end_line(manifest_text, section.header_line, section.end_line)
    {
        return Some(insert_line_edit(
            array_end_line,
            &format!("    '{SOLANA_TARGET_OS_CHECK_CFG}',\n"),
        ));
    }

    Some(insert_line_edit(
        section.header_line.saturating_add(1),
        &format!("check-cfg = [\n    '{SOLANA_TARGET_OS_CHECK_CFG}',\n]\n"),
    ))
}

struct ManifestSection {
    header_line: usize,
    end_line: usize,
}

fn solana_target_os_check_cfg_section(manifest_text: &str) -> Option<ManifestSection> {
    let workspace_section = format!("[workspace.{UNEXPECTED_CFGS_SECTION}]");
    let package_section = format!("[package.{UNEXPECTED_CFGS_SECTION}]");
    table_section(manifest_text, &workspace_section)
        .or_else(|| table_section(manifest_text, &package_section))
}

fn table_section(manifest_text: &str, header: &str) -> Option<ManifestSection> {
    let lines = manifest_text.lines().collect::<Vec<_>>();
    let header_line = lines
        .iter()
        .enumerate()
        .find_map(|(line_number, line)| (line.trim() == header).then_some(line_number))?;
    let end_line = lines
        .iter()
        .enumerate()
        .skip(header_line.saturating_add(1))
        .find_map(|(line_number, line)| is_table_header(line).then_some(line_number))
        .unwrap_or(lines.len());
    Some(ManifestSection {
        header_line,
        end_line,
    })
}

fn check_cfg_array_end_line(
    manifest_text: &str,
    header_line: usize,
    section_end_line: usize,
) -> Option<usize> {
    let lines = manifest_text.lines().collect::<Vec<_>>();
    let start_line = lines
        .iter()
        .enumerate()
        .skip(header_line.saturating_add(1))
        .take(section_end_line.saturating_sub(header_line.saturating_add(1)))
        .find_map(|(line_number, line)| {
            let trimmed = line.split('#').next().unwrap_or_default().trim_start();
            (trimmed.starts_with("check-cfg") && trimmed.contains('[')).then_some(line_number)
        })?;
    lines
        .iter()
        .enumerate()
        .skip(start_line)
        .take(section_end_line.saturating_sub(start_line))
        .find_map(|(line_number, line)| (line.trim() == "]").then_some(line_number))
}

fn solana_target_os_check_cfg_owner(manifest_text: &str) -> &'static str {
    if has_table_header(manifest_text, "[workspace]") {
        "workspace"
    } else {
        "package"
    }
}

fn insert_line_edit(line_number: usize, new_text: &str) -> TextEdit {
    let line = u32::try_from(line_number).unwrap_or_default();
    TextEdit {
        range: Range {
            start: Position { line, character: 0 },
            end: Position { line, character: 0 },
        },
        new_text: new_text.to_string(),
    }
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

fn manifest_has_solana_target_os_check_cfg(manifest_text: &str) -> bool {
    let Ok(manifest) = toml::from_str::<toml::Value>(manifest_text) else {
        return false;
    };

    ["workspace", "package"].into_iter().any(|owner| {
        manifest
            .get(owner)
            .and_then(|owner| owner.get("lints"))
            .and_then(|lints| lints.get("rust"))
            .and_then(|rust| rust.get("unexpected_cfgs"))
            .and_then(|unexpected_cfgs| unexpected_cfgs.get("check-cfg"))
            .and_then(|check_cfg| check_cfg.as_array())
            .is_some_and(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .any(check_cfg_allows_solana_target_os)
            })
    })
}

fn check_cfg_allows_solana_target_os(value: &str) -> bool {
    let compact = value.split_whitespace().collect::<String>();
    compact.contains("cfg(target_os,values(") && compact.contains("\"solana\"")
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

fn has_table_header(manifest_text: &str, header: &str) -> bool {
    manifest_text.lines().any(|line| line.trim() == header)
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

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
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

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
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

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
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

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_solana_target_os_check_cfg_when_manifest_lacks_lint() {
        let document = ParsedDocument::parse(
            r#"
declare_id!("Demo111111111111111111111111111111111111");
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

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("target_os"));
        assert!(diagnostics[0]
            .message
            .contains("add `cfg(target_os, values(\"solana\"))`"));
        assert_eq!(
            diagnostics[0]
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str()),
            Some(ADD_SOLANA_TARGET_OS_CHECK_CFG_QUICKFIX)
        );
    }

    #[test]
    fn accepts_workspace_solana_target_os_check_cfg_lint() {
        let document = ParsedDocument::parse(
            r#"
declare_id!("Demo111111111111111111111111111111111111");
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[workspace]

[workspace.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn accepts_package_solana_target_os_check_cfg_lint() {
        let document = ParsedDocument::parse(
            r#"
declare_id!("Demo111111111111111111111111111111111111");
"#,
        )
        .unwrap();
        let diagnostics = collect(
            &document,
            &Url::parse("file:///tmp/program/Cargo.toml").unwrap(),
            r#"
[package]
name = "demo"

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
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

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
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

[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
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
    fn appends_workspace_solana_target_os_check_cfg_lint() {
        let edit = solana_target_os_check_cfg_edit("[workspace]\nmembers = []\n").unwrap();

        assert!(edit
            .new_text
            .contains("[workspace.lints.rust.unexpected_cfgs]"));
        assert!(edit
            .new_text
            .contains("'cfg(target_os, values(\"solana\"))'"));
    }

    #[test]
    fn appends_package_solana_target_os_check_cfg_lint() {
        let edit = solana_target_os_check_cfg_edit("[package]\nname = \"demo\"\n").unwrap();

        assert!(edit
            .new_text
            .contains("[package.lints.rust.unexpected_cfgs]"));
        assert!(edit
            .new_text
            .contains("'cfg(target_os, values(\"solana\"))'"));
    }

    #[test]
    fn extends_existing_solana_check_cfg_lint_array() {
        let edit = solana_target_os_check_cfg_edit(
            r#"
[workspace.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(feature, values("anchor-debug"))',
]
"#,
        )
        .unwrap();

        assert_eq!(edit.range.start.line, 5);
        assert_eq!(
            edit.new_text,
            "    'cfg(target_os, values(\"solana\"))',\n"
        );
    }

    #[test]
    fn skips_solana_target_os_check_cfg_edit_when_lint_exists() {
        let edit = solana_target_os_check_cfg_edit(
            r#"
[package.lints.rust.unexpected_cfgs]
level = "warn"
check-cfg = [
    'cfg(target_os, values("solana"))',
]
"#,
        );

        assert!(edit.is_none());
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
