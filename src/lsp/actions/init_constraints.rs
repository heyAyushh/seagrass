use {
    super::common::snippet_text_edit,
    crate::{
        diagnostics::{ANCHOR_INIT_CONSTRAINTS_CODE, INIT_PLACEHOLDERS_QUICKFIX, SOURCE},
        document::{ParsedDocument, SymbolRange},
        range::{account_type_after_line, line_at},
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, Url, WorkspaceEdit,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitQuickFix {
    account_type: String,
    needs_payer: bool,
    needs_space: bool,
}

impl InitQuickFix {
    fn from_document_line(document: &ParsedDocument, line_number: u32, line: &str) -> Option<Self> {
        if let Some((field, constraint_text)) =
            account_field_constraint_on_line(document, line_number)
        {
            return Self::from_constraint_text(field, constraint_text)
                .or_else(|| Self::from_attribute_line(document.source(), line_number, line));
        }

        Self::from_attribute_line(document.source(), line_number, line)
    }

    fn from_constraint_text(field: &SymbolRange, constraint_text: &str) -> Option<Self> {
        if !constraint_has_flag_or_assignment(constraint_text, "init")
            && !constraint_has_flag_or_assignment(constraint_text, "init_if_needed")
        {
            return None;
        }

        let needs_payer = !constraint_has_assignment(constraint_text, "payer");
        let needs_space = !constraint_has_assignment(constraint_text, "space");
        if !needs_payer && !needs_space {
            return None;
        }

        Some(Self {
            account_type: field
                .generic_type_names
                .last()
                .cloned()
                .unwrap_or_else(|| "AccountType".to_string()),
            needs_payer,
            needs_space,
        })
    }

    fn from_attribute_line(text: &str, line_number: u32, line: &str) -> Option<Self> {
        if !line.contains("#[account(") {
            return None;
        }

        Self::from_constraint_text_fallback(text, line_number, line)
    }

    fn from_constraint_text_fallback(text: &str, line_number: u32, line: &str) -> Option<Self> {
        if !constraint_has_flag_or_assignment(line, "init")
            && !constraint_has_flag_or_assignment(line, "init_if_needed")
        {
            return None;
        }

        let needs_payer = !constraint_has_assignment(line, "payer");
        let needs_space = !constraint_has_assignment(line, "space");
        if !needs_payer && !needs_space {
            return None;
        }

        Some(Self {
            account_type: account_type_after_line(text, line_number)
                .unwrap_or_else(|| "AccountType".to_string()),
            needs_payer,
            needs_space,
        })
    }

    fn apply_to(&self, line: &str) -> Option<String> {
        let insert_at = line.rfind(")]")?;
        let mut additions = Vec::with_capacity(2);
        if self.needs_payer {
            additions.push("payer = payer".to_string());
        }
        if self.needs_space {
            additions.push(format!("space = 8 + {}::INIT_SPACE", self.account_type));
        }

        let needs_separator = !line[..insert_at].trim_end().ends_with('(')
            && !line[..insert_at].trim_end().ends_with(',');
        let separator = if needs_separator { ", " } else { "" };

        Some(format!(
            "{}{}{}{}",
            &line[..insert_at],
            separator,
            additions.join(", "),
            &line[insert_at..]
        ))
    }
}

pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let Some(line) = line_at(document.source(), range.start.line) else {
        return Vec::new();
    };

    let matching_diagnostics = init_quickfix_diagnostics(diagnostics);
    if !diagnostics.is_empty() && matching_diagnostics.is_empty() {
        return Vec::new();
    }

    let Some(quickfix) = InitQuickFix::from_document_line(document, range.start.line, line) else {
        return Vec::new();
    };
    let Some(replacement) = quickfix.apply_to(line) else {
        return Vec::new();
    };
    if replacement == line {
        return Vec::new();
    }

    let line_range = Range {
        start: Position {
            line: range.start.line,
            character: 0,
        },
        end: Position {
            line: range.start.line,
            character: u32::try_from(line.chars().count()).unwrap_or_default(),
        },
    };

    let mut changes = HashMap::new();
    changes.insert(uri, vec![snippet_text_edit(line_range, &replacement)]);

    vec![CodeAction {
        title: "Add Anchor init payer/space placeholders".to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: (!matching_diagnostics.is_empty()).then_some(matching_diagnostics),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: None,
    }]
}

fn account_field_constraint_on_line(
    document: &ParsedDocument,
    line_number: u32,
) -> Option<(&SymbolRange, &str)> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| accounts.fields.iter())
        .find_map(|field| {
            field
                .account_constraints
                .iter()
                .find(|constraint| {
                    constraint.range.start.line <= line_number
                        && line_number <= constraint.range.end.line
                })
                .map(|constraint| (field, constraint.text.as_str()))
        })
}

fn constraint_has_flag_or_assignment(text: &str, key: &str) -> bool {
    constraint_has_flag(text, key) || constraint_has_assignment(text, key)
}

fn constraint_has_assignment(text: &str, key: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        if constraint_key_boundary(text, idx, key.len()) {
            let after_key = &text[idx + key.len()..];
            if after_key.trim_start().starts_with('=') {
                return true;
            }
        }
        search_start = idx + 1;
    }
    false
}

fn constraint_has_flag(text: &str, key: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        if constraint_key_boundary(text, idx, key.len()) {
            let after_key = text[idx + key.len()..].trim_start();
            if after_key.is_empty() || after_key.starts_with(',') || after_key.starts_with(')') {
                return true;
            }
        }
        search_start = idx + 1;
    }
    false
}

fn constraint_key_boundary(text: &str, idx: usize, key_len: usize) -> bool {
    let previous = text[..idx].chars().next_back();
    let next = text[idx + key_len..].chars().next();
    previous
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .unwrap_or(true)
        && next
            .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
            .unwrap_or(true)
}

fn init_quickfix_diagnostics(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.source.as_deref() == Some(SOURCE)
                && matches!(
                    diagnostic.code.as_ref(),
                    Some(NumberOrString::String(code)) if code == ANCHOR_INIT_CONSTRAINTS_CODE
                )
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some(INIT_PLACEHOLDERS_QUICKFIX)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn offers_init_placeholder_quickfix() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;

        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/create.rs").unwrap(),
            Range {
                start: Position {
                    line: 3,
                    character: 14,
                },
                end: Position {
                    line: 3,
                    character: 18,
                },
            },
            &[],
        );

        assert_eq!(actions.len(), 1);
        let edit = actions[0].edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let text_edit = changes.values().next().unwrap().first().unwrap();
        assert!(text_edit.new_text.contains("payer = payer"));
        assert!(text_edit.new_text.contains("space = 8 + State::INIT_SPACE"));
    }

    #[test]
    fn init_placeholder_quickfix_ignores_init_substrings() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(initializer = true)]
    pub state: Account<'info, State>,
}
"#;

        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/create.rs").unwrap(),
            Range {
                start: Position {
                    line: 3,
                    character: 14,
                },
                end: Position {
                    line: 3,
                    character: 18,
                },
            },
            &[],
        );

        assert!(actions.is_empty());
    }

    #[test]
    fn init_placeholder_quickfix_ignores_payer_and_space_in_constraint_expression() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, constraint = payer.key() != Pubkey::default(), constraint = has_space())]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
"#;

        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/create.rs").unwrap(),
            Range {
                start: Position {
                    line: 3,
                    character: 14,
                },
                end: Position {
                    line: 3,
                    character: 18,
                },
            },
            &[],
        );

        assert_eq!(actions.len(), 1);
        let edit = actions[0].edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let text_edit = changes.values().next().unwrap().first().unwrap();
        assert!(text_edit.new_text.contains("payer = payer"));
        assert!(text_edit.new_text.contains("space = 8 + State::INIT_SPACE"));
    }

    #[test]
    fn init_placeholder_quickfix_accepts_spaced_existing_assignments() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = wallet)]
    pub state: Account<'info, State>,
    pub wallet: Signer<'info>,
}
"#;

        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/create.rs").unwrap(),
            Range {
                start: Position {
                    line: 3,
                    character: 14,
                },
                end: Position {
                    line: 3,
                    character: 18,
                },
            },
            &[],
        );

        assert_eq!(actions.len(), 1);
        let edit = actions[0].edit.as_ref().unwrap();
        let changes = edit.changes.as_ref().unwrap();
        let text_edit = changes.values().next().unwrap().first().unwrap();
        assert!(!text_edit.new_text.contains("payer = payer"));
        assert!(text_edit.new_text.contains("space = 8 + State::INIT_SPACE"));
    }
}
