use {
    super::common::snippet_text_edit,
    crate::{
        constraint_text,
        diagnostics::{ANCHOR_INIT_CONSTRAINTS_CODE, INIT_PLACEHOLDERS_QUICKFIX, SOURCE},
        document::{ParsedDocument, SymbolRange},
        range::{account_type_after_line, line_at},
        workspace::WorkspaceIndex,
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, TextEdit, Url,
        WorkspaceEdit,
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

#[cfg(test)]
pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    code_actions_with_workspace(document, uri, range, diagnostics, None)
}

pub fn code_actions_with_workspace(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
    workspace: Option<&WorkspaceIndex>,
) -> Vec<CodeAction> {
    let Some(line) = line_at(document.source(), range.start.line) else {
        return Vec::new();
    };

    let mut actions = derive_init_space_actions(document, uri.clone(), range, workspace);

    let matching_diagnostics = init_quickfix_diagnostics(diagnostics);
    if !diagnostics.is_empty() && matching_diagnostics.is_empty() {
        return actions;
    }

    let Some(quickfix) = InitQuickFix::from_document_line(document, range.start.line, line) else {
        return actions;
    };
    let Some(replacement) = quickfix.apply_to(line) else {
        return actions;
    };
    if replacement == line {
        return actions;
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

    actions.push(CodeAction {
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
    });
    actions
}

fn derive_init_space_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    workspace: Option<&WorkspaceIndex>,
) -> Vec<CodeAction> {
    let Some((_field, constraint_text)) =
        account_field_constraint_on_line(document, range.start.line)
    else {
        return Vec::new();
    };
    let Some(account_type) = init_space_type_from_constraint(constraint_text) else {
        return Vec::new();
    };
    let Some(target) = init_space_target(document, &uri, workspace, &account_type) else {
        return Vec::new();
    };
    if target.symbol.derive_init_space_range.is_some() {
        return Vec::new();
    }

    let mut edits = vec![derive_init_space_edit(document, &uri, &target)];
    let max_len_fields = fields_needing_max_len(&target.symbol);
    edits.extend(max_len_fields.iter().map(|field| max_len_stub_edit(field)));

    let mut changes = HashMap::new();
    changes.insert(target.uri.clone(), edits);
    let title = if max_len_fields.is_empty() {
        format!("Add `#[derive(InitSpace)]` to `{}`", target.symbol.name)
    } else {
        format!(
            "Add `#[derive(InitSpace)]` to `{}` and add max_len stubs",
            target.symbol.name
        )
    };

    vec![CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: Some(serde_json::json!({
            "anchorAction": "derive-init-space",
            "accountType": target.symbol.name,
        })),
    }]
}

#[derive(Debug, Clone)]
struct InitSpaceTarget {
    uri: Url,
    symbol: SymbolRange,
}

fn init_space_target(
    document: &ParsedDocument,
    uri: &Url,
    workspace: Option<&WorkspaceIndex>,
    account_type: &str,
) -> Option<InitSpaceTarget> {
    document
        .symbols()
        .all_structs
        .get(account_type)
        .cloned()
        .map(|symbol| InitSpaceTarget {
            uri: uri.clone(),
            symbol,
        })
        .or_else(|| {
            workspace
                .and_then(|workspace| workspace.account_data_struct(account_type))
                .map(|entry| InitSpaceTarget {
                    uri: entry.uri.clone(),
                    symbol: entry.symbol.clone(),
                })
        })
}

fn init_space_type_from_constraint(constraint_text: &str) -> Option<String> {
    let value = constraint_text::values_after_key(constraint_text, "space")
        .into_iter()
        .next()?;
    let prefix = value.split("::INIT_SPACE").next()?;
    let candidate = prefix
        .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .find(|part| !part.is_empty())?;
    Some(candidate.to_string())
}

fn derive_init_space_edit(
    document: &ParsedDocument,
    current_uri: &Url,
    target: &InitSpaceTarget,
) -> TextEdit {
    if let Some(range) = target.symbol.derive_attribute_range {
        let insert_position = Position {
            line: range.end.line,
            character: range.end.character.saturating_sub(2),
        };
        return snippet_text_edit(
            Range {
                start: insert_position,
                end: insert_position,
            },
            ", InitSpace",
        );
    }

    let insert_position = Position {
        line: target.symbol.range.start.line,
        character: 0,
    };
    let indent = if &target.uri == current_uri {
        line_at(document.source(), target.symbol.range.start.line)
            .map(line_indent)
            .unwrap_or_else(|| " ".repeat(target.symbol.range.start.character as usize))
    } else {
        " ".repeat(target.symbol.range.start.character as usize)
    };
    snippet_text_edit(
        Range {
            start: insert_position,
            end: insert_position,
        },
        &format!("{indent}#[derive(InitSpace)]\n"),
    )
}

fn fields_needing_max_len(symbol: &SymbolRange) -> Vec<&SymbolRange> {
    symbol
        .fields
        .iter()
        .filter(|field| field.max_len_args.is_empty() && field_type_needs_max_len(field))
        .collect()
}

fn field_type_needs_max_len(field: &SymbolRange) -> bool {
    field
        .type_signature
        .as_deref()
        .is_some_and(type_text_needs_max_len)
}

fn type_text_needs_max_len(type_text: &str) -> bool {
    syn::parse_str::<syn::Type>(type_text)
        .ok()
        .is_some_and(|ty| type_needs_max_len(&ty))
}

fn type_needs_max_len(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| {
                let name = segment.ident.to_string();
                name == "String"
                    || name == "Vec"
                    || matches!(&segment.arguments, syn::PathArguments::AngleBracketed(args) if args.args.iter().any(|arg| {
                        matches!(arg, syn::GenericArgument::Type(inner) if type_needs_max_len(inner))
                    }))
            }),
        syn::Type::Array(array) => type_needs_max_len(&array.elem),
        syn::Type::Tuple(tuple) => tuple.elems.iter().any(type_needs_max_len),
        _ => false,
    }
}

fn max_len_stub_edit(field: &SymbolRange) -> TextEdit {
    let insert_position = Position {
        line: field.range.start.line,
        character: 0,
    };
    let indent = " ".repeat(field.range.start.character as usize);
    snippet_text_edit(
        Range {
            start: insert_position,
            end: insert_position,
        },
        &format!("{indent}#[max_len(/* TODO */)]\n"),
    )
}

fn line_indent(line: &str) -> String {
    line.chars().take_while(|ch| ch.is_whitespace()).collect()
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
    use {super::*, crate::document::ParsedDocument, crate::workspace::WorkspaceIndex};

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

    #[test]
    fn offers_derive_init_space_same_file() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}

#[account]
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct Vault {
    pub authority: Pubkey,
}
"#;
        let uri = Url::parse("file:///tmp/create.rs").unwrap();
        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            uri,
            cursor_range(source, "Vault::INIT_SPACE"),
            &[],
        );
        let action = actions
            .iter()
            .find(|action| action.title.contains("derive(InitSpace)"))
            .unwrap();
        let edit = first_edit(action);

        assert_eq!(edit.new_text, ", InitSpace");
    }

    #[test]
    fn offers_derive_init_space_cross_file_on_defining_uri() {
        let accounts_source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}
"#;
        let vault_source = r#"
#[account]
pub struct Vault {
    pub authority: Pubkey,
}
"#;
        let accounts_uri = Url::parse("file:///tmp/accounts.rs").unwrap();
        let vault_uri = Url::parse("file:///tmp/vault.rs").unwrap();
        let document = ParsedDocument::parse(accounts_source).unwrap();
        let workspace = WorkspaceIndex::build(
            &[],
            [
                (accounts_uri.clone(), accounts_source.to_string()),
                (vault_uri.clone(), vault_source.to_string()),
            ],
        );
        let actions = code_actions_with_workspace(
            &document,
            accounts_uri,
            cursor_range(accounts_source, "Vault::INIT_SPACE"),
            &[],
            Some(&workspace),
        );
        let action = actions
            .iter()
            .find(|action| action.title.contains("derive(InitSpace)"))
            .unwrap();
        let changes = action.edit.as_ref().unwrap().changes.as_ref().unwrap();

        assert!(changes.contains_key(&vault_uri));
        assert_eq!(changes[&vault_uri][0].new_text, "#[derive(InitSpace)]\n");
    }

    #[test]
    fn derive_init_space_adds_max_len_stubs_for_variable_fields() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}

#[account]
pub struct Vault {
    pub name: String,
    pub values: Vec<u64>,
}
"#;
        let uri = Url::parse("file:///tmp/create.rs").unwrap();
        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            uri,
            cursor_range(source, "Vault::INIT_SPACE"),
            &[],
        );
        let action = actions
            .iter()
            .find(|action| action.title.contains("max_len stubs"))
            .unwrap();
        let edits = action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap();

        assert_eq!(
            edits
                .iter()
                .filter(|edit| edit.new_text.contains("#[max_len(/* TODO */)]"))
                .count(),
            2
        );
    }

    #[test]
    fn derive_init_space_ignores_already_derived_type() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub authority: Pubkey,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/create.rs").unwrap(),
            cursor_range(source, "Vault::INIT_SPACE"),
            &[],
        );

        assert!(!actions
            .iter()
            .any(|action| action.title.contains("derive(InitSpace)")));
    }

    #[test]
    fn derive_init_space_ignores_external_type() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + External::INIT_SPACE)]
    pub vault: Account<'info, External>,
    pub payer: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let actions = code_actions(
            &document,
            Url::parse("file:///tmp/create.rs").unwrap(),
            cursor_range(source, "External::INIT_SPACE"),
            &[],
        );

        assert!(actions.is_empty());
    }

    fn cursor_range(source: &str, needle: &str) -> Range {
        let position = position_of(source, needle);
        Range {
            start: position,
            end: position,
        }
    }

    fn position_of(source: &str, needle: &str) -> Position {
        let offset = source.find(needle).unwrap();
        let prefix = &source[..offset];
        Position {
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
            character: prefix
                .rsplit('\n')
                .next()
                .map(|line| line.chars().count())
                .unwrap_or_default() as u32,
        }
    }

    fn first_edit(action: &CodeAction) -> &TextEdit {
        action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .first()
            .unwrap()
    }
}
