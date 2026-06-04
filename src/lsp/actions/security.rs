//! Security-smell code actions: account type replacements, invalid sysvar fixes.
//! Single seam (`code_actions`) for the thin central router.

use {
    super::common::{diagnostic_code, single_document_edit, single_text_edit, snippet_text_edit},
    crate::{
        anchor_types,
        document::{ParsedDocument, SymbolRange},
        range::byte_offset_at,
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
    },
};

const ACCOUNT_DISCRIMINATOR_BYTES: usize = 8;
const PINOCCHIO_PROGRAM_KIND: &str = "pinocchio";
const DISCRIMINATOR_DESERIALIZER_MARKERS: &[&str] = &[
    "::try_from_slice",
    "::deserialize",
    "::try_deserialize_unchecked",
];
const PINOCCHIO_LOAD_MARKER: &str = "load::<";

pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    _range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    actions.extend(replace_account_type_actions(uri.clone(), diagnostics));
    actions.extend(replace_invalid_sysvar_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(owner_constraint_actions(document, uri.clone(), diagnostics));
    actions.extend(security_edit_actions(document, uri.clone(), diagnostics));
    actions.extend(security_guidance_actions(diagnostics));
    actions
}

fn owner_constraint_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-security-owner-check")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("add-owner-constraint")
        })
        .filter_map(|diagnostic| {
            let account = diagnostic
                .data
                .as_ref()?
                .get("account")
                .and_then(|value| value.as_str())?;
            let field = document
                .symbols()
                .accounts_structs
                .values()
                .flat_map(|accounts| accounts.fields.iter())
                .find(|field| field.name == account)?;
            let line_number = field
                .account_constraints
                .first()
                .map(|constraint| constraint.range.start.line)
                .unwrap_or(field.selection_range.start.line);
            let line = crate::range::line_at(document.source(), line_number)?;
            let indent = line
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            Some(CodeAction {
                title: format!("Constrain `{account}` owner to `crate::ID`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(
                    uri.clone(),
                    snippet_text_edit(
                        Range {
                            start: Position {
                                line: line_number,
                                character: 0,
                            },
                            end: Position {
                                line: line_number,
                                character: 0,
                            },
                        },
                        &format!("{indent}#[account(owner = crate::ID)]\n"),
                    ),
                )),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "add-owner-constraint",
                    "account": account,
                })),
            })
        })
        .collect()
}

fn security_guidance_actions(diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let quickfix = data.get("quickfix").and_then(|value| value.as_str())?;
            let attack = data.get("attack").and_then(|value| value.as_str())?;
            let title = match quickfix {
                "typed-account-or-discriminator" => {
                    "Use a typed account or add discriminator validation"
                }
                "add-owner-check" => "Add an owner check before raw account data access",
                "add-discriminator-check" => "Add discriminator/type validation before decoding",
                "prefer-anchor-close" => "Use Anchor close or write a closed discriminator",
                "reject-reinit" => "Reject already-initialized accounts before unchecked init",
                "insert-reload-after-cpi" => "Reload the account after CPI before reading fields",
                "add-signer-check" => "Check `is_signer` before signer use",
                "add-writable-check" => "Check account writability before writable CPI use",
                "add-program-id-check" => "Check the CPI program id before invoke",
                "use-checked-data-access" => "Use checked data access before indexing",
                "add-static-pda-domain-seed" => "Add a static PDA namespace seed",
                _ => return None,
            };
            Some(CodeAction {
                title: title.to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: None,
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "security-guidance",
                    "quickfix": quickfix,
                    "attack": attack,
                })),
            })
        })
        .collect()
}

fn security_edit_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let quickfix = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())?;
            let edit = match quickfix {
                "insert-reload-after-cpi" => reload_after_cpi_edit(document, diagnostic),
                "add-signer-check" => signer_guard_edit(document, diagnostic),
                "add-writable-check" => writable_guard_edit(document, diagnostic),
                "add-program-id-check" => program_id_guard_edit(document, diagnostic),
                "use-checked-data-access" => bounds_guard_edit(document, diagnostic),
                "add-static-pda-domain-seed" => static_pda_domain_seed_edit(diagnostic),
                "add-scoped-pda-seed" => scoped_pda_seed_edit(document, diagnostic),
                "add-owner-check" => native_owner_guard_edit(document, diagnostic),
                "add-discriminator-check" => native_discriminator_guard_edit(document, diagnostic),
                _ => None,
            }?;
            let title = match quickfix {
                "insert-reload-after-cpi" => "Insert reload after CPI",
                "add-signer-check" => "Insert signer guard",
                "add-writable-check" => "Insert writable guard",
                "add-program-id-check" => "Insert CPI program-id guard",
                "use-checked-data-access" => "Insert data bounds guard",
                "add-static-pda-domain-seed" => "Insert static PDA seed",
                "add-scoped-pda-seed" => "Insert scoped PDA seed",
                "add-owner-check" => "Insert owner guard",
                "add-discriminator-check" => "Insert discriminator guard",
                _ => return None,
            };
            Some(CodeAction {
                title: title.to_string(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "security-edit",
                    "quickfix": quickfix,
                })),
            })
        })
        .collect()
}

fn reload_after_cpi_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let account = diagnostic
        .data
        .as_ref()?
        .get("account")
        .and_then(|value| value.as_str())?;
    let line = diagnostic.range.end.line.saturating_add(1);
    let indent = indent_at(document, diagnostic.range.start.line);
    Some(insert_line_edit(
        line,
        format!("{indent}ctx.accounts.{account}.reload()?;\n"),
    ))
}

fn signer_guard_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let account =
        data_string(diagnostic, "accountExpression").unwrap_or_else(|| "accounts[0]".to_string());
    let line = diagnostic.range.start.line;
    let indent = indent_at(document, line);
    let signer_check = if is_pinocchio_diagnostic(diagnostic) {
        format!("!{account}.is_signer()")
    } else {
        format!("!{account}.is_signer")
    };
    Some(insert_line_edit(
        line,
        format!(
            "{indent}if {signer_check} {{ return Err(ProgramError::MissingRequiredSignature); }}\n"
        ),
    ))
}

fn writable_guard_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let account =
        data_string(diagnostic, "accountExpression").unwrap_or_else(|| "accounts[0]".to_string());
    let line = diagnostic.range.start.line;
    let indent = indent_at(document, line);
    let writable_check = if is_pinocchio_diagnostic(diagnostic) {
        format!("!{account}.is_writable()")
    } else {
        format!("!{account}.is_writable")
    };
    Some(insert_line_edit(
        line,
        format!(
            "{indent}if {writable_check} {{ return Err(ProgramError::InvalidAccountData); }}\n"
        ),
    ))
}

fn program_id_guard_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let program_id =
        data_string(diagnostic, "programIdExpression").unwrap_or_else(|| "*program_id".to_string());
    let line = diagnostic.range.start.line;
    let indent = indent_at(document, line);
    Some(insert_line_edit(
        line,
        format!(
            "{indent}if {program_id} != &crate::ID {{ return Err(ProgramError::IncorrectProgramId); }}\n"
        ),
    ))
}

fn bounds_guard_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let data =
        data_string(diagnostic, "dataExpression").unwrap_or_else(|| "instruction_data".to_string());
    let line = diagnostic.range.start.line;
    let indent = indent_at(document, line);
    Some(insert_line_edit(
        line,
        format!(
            "{indent}if {data}.is_empty() {{ return Err(ProgramError::InvalidInstructionData); }}\n"
        ),
    ))
}

fn static_pda_domain_seed_edit(diagnostic: &Diagnostic) -> Option<TextEdit> {
    Some(snippet_text_edit(
        Range {
            start: diagnostic.range.end,
            end: diagnostic.range.end,
        },
        "b\"state\", ",
    ))
}

fn scoped_pda_seed_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let seed = data_string(diagnostic, "scopedSeed")?;
    let position = seed_list_insert_position(document.source(), diagnostic.range)?;
    Some(snippet_text_edit(
        Range {
            start: position,
            end: position,
        },
        &format!("{seed}, "),
    ))
}

fn seed_list_insert_position(source: &str, range: Range) -> Option<Position> {
    const SEEDS_KEY: &str = "seeds";
    let start = byte_offset_at(source, range.start)?;
    let end = byte_offset_at(source, range.end)?;
    let attribute = source.get(start..end)?;
    let seeds_offset = attribute.find(SEEDS_KEY)?;
    let bracket_offset = attribute.get(seeds_offset..)?.find('[')?;
    position_at_byte_offset(
        source,
        start + seeds_offset + bracket_offset + '['.len_utf8(),
    )
}

fn native_owner_guard_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let account =
        data_string(diagnostic, "accountExpression").unwrap_or_else(|| "account".to_string());
    let line = diagnostic.range.start.line;
    let indent = indent_at(document, line);
    let guard = if is_pinocchio_diagnostic(diagnostic) {
        format!(
            "{indent}if !{account}.is_owned_by(&crate::ID) {{ return Err(ProgramError::IncorrectProgramId); }}\n"
        )
    } else {
        format!(
            "{indent}if {account}.owner != &crate::ID {{ return Err(ProgramError::IncorrectProgramId); }}\n"
        )
    };

    Some(insert_line_edit(line, guard))
}

fn is_pinocchio_diagnostic(diagnostic: &Diagnostic) -> bool {
    data_string(diagnostic, "programKind").as_deref() == Some(PINOCCHIO_PROGRAM_KIND)
}

fn native_discriminator_guard_edit(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let line_number = diagnostic.range.start.line;
    let source_line = crate::range::line_at(document.source(), line_number)?;
    let evidence = discriminator_guard_evidence(source_line)?;
    let indent = indent_at(document, line_number);
    Some(insert_line_edit(
        line_number,
        format!(
            "{indent}if {data}.len() < {ACCOUNT_DISCRIMINATOR_BYTES} || &{data}[..{ACCOUNT_DISCRIMINATOR_BYTES}] != {account_type}::DISCRIMINATOR.as_ref() {{ return Err(ProgramError::InvalidAccountData); }}\n",
            data = evidence.data_expression,
            account_type = evidence.account_type,
        ),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscriminatorGuardEvidence {
    account_type: String,
    data_expression: String,
}

fn discriminator_guard_evidence(line: &str) -> Option<DiscriminatorGuardEvidence> {
    let call = discriminator_deserializer_call(line).or_else(|| pinocchio_load_call(line))?;
    let arguments = call_arguments_after(line, call.arguments_start)?;
    let data_expression = safe_discriminator_data_expression(first_call_argument(arguments)?)?;
    Some(DiscriminatorGuardEvidence {
        account_type: call.account_type,
        data_expression,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeserializerCall {
    account_type: String,
    arguments_start: usize,
}

fn discriminator_deserializer_call(line: &str) -> Option<DeserializerCall> {
    DISCRIMINATOR_DESERIALIZER_MARKERS
        .iter()
        .find_map(|marker| {
            let marker_start = line.find(marker)?;
            Some(DeserializerCall {
                account_type: account_type_before_marker(line, marker_start)?,
                arguments_start: marker_start + marker.len(),
            })
        })
}

fn pinocchio_load_call(line: &str) -> Option<DeserializerCall> {
    let marker_start = line.find(PINOCCHIO_LOAD_MARKER)?;
    let type_start = marker_start + PINOCCHIO_LOAD_MARKER.len();
    let type_end = line.get(type_start..)?.find('>')? + type_start;
    let account_type = terminal_rust_path_segment(line.get(type_start..type_end)?.trim())?;
    Some(DeserializerCall {
        account_type: account_type.to_string(),
        arguments_start: type_end + '>'.len_utf8(),
    })
}

fn account_type_before_marker(line: &str, marker_start: usize) -> Option<String> {
    let raw_path = line
        .get(..marker_start)?
        .trim_end()
        .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .find(|segment| !segment.is_empty())?;
    Some(terminal_rust_path_segment(raw_path)?.to_string())
}

fn terminal_rust_path_segment(raw_path: &str) -> Option<&str> {
    let segment = raw_path.rsplit("::").next()?.trim();
    is_rust_identifier(segment).then_some(segment)
}

fn call_arguments_after(line: &str, search_start: usize) -> Option<&str> {
    let open = line.get(search_start..)?.find('(')? + search_start;
    let close = matching_closing_paren(line, open)?;
    line.get(open + '('.len_utf8()..close)
}

fn matching_closing_paren(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (relative_offset, ch) in line.get(open..)?.char_indices() {
        match ch {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + relative_offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn first_call_argument(arguments: &str) -> Option<&str> {
    let mut depth = 0usize;
    for (index, ch) in arguments.char_indices() {
        match ch {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => return non_empty_trimmed(arguments.get(..index)?),
            _ => {}
        }
    }
    non_empty_trimmed(arguments)
}

fn safe_discriminator_data_expression(argument: &str) -> Option<String> {
    let mut candidate = argument.trim();
    while let Some(stripped) = candidate.strip_prefix('&') {
        candidate = stripped.trim_start();
    }
    if let Some((base, _slice)) = candidate.split_once('[') {
        candidate = base.trim_end();
    }
    is_rust_identifier(candidate).then(|| candidate.to_string())
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn is_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(ch) if ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn data_string(diagnostic: &Diagnostic, key: &str) -> Option<String> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn indent_at(document: &ParsedDocument, line_number: u32) -> String {
    crate::range::line_at(document.source(), line_number)
        .map(|line| {
            line.chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn insert_line_edit(line: u32, new_text: String) -> TextEdit {
    snippet_text_edit(
        Range {
            start: Position { line, character: 0 },
            end: Position { line, character: 0 },
        },
        &new_text,
    )
}

fn position_at_byte_offset(source: &str, offset: usize) -> Option<Position> {
    let offset = offset.min(source.len());
    if !source.is_char_boundary(offset) {
        return None;
    }

    let mut line = 0u32;
    let mut line_start = 0usize;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            line_start = index + 1;
        }
    }

    Some(Position {
        line,
        character: u32::try_from(source[line_start..offset].chars().count()).ok()?,
    })
}

fn replace_account_type_actions(uri: Url, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic_code(diagnostic),
                Some("anchor-security-signer")
                    | Some("anchor-security-sysvar")
                    | Some("anchor-security-token-account")
                    | Some("anchor-security-cpi-program")
                    | Some("anchor-constraint-shape")
                    | Some("anchor-syn")
            )
        })
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some("replace-account-type")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let expected = data.get("expected").and_then(|value| value.as_str())?;
            let account = data
                .get("account")
                .and_then(|value| value.as_str())
                .unwrap_or("account");
            let edit = single_text_edit(diagnostic.range, expected.to_string());

            Some(CodeAction {
                title: format!("Replace `{account}` type with `{expected}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "replace-account-type",
                    "account": account,
                    "expected": expected,
                })),
            })
        })
        .collect()
}

fn replace_invalid_sysvar_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-syn")
                && (diagnostic.message.contains("invalid sysvar provided")
                    || diagnostic
                        .data
                        .as_ref()
                        .and_then(|data| data.get("quickfix"))
                        .and_then(|value| value.as_str())
                        == Some("replace-invalid-sysvar"))
        })
        .filter_map(|diagnostic| {
            let (field, replacement, replacement_range) =
                invalid_sysvar_replacement(document, diagnostic)?;
            let current = field
                .generic_type_names
                .last()
                .map(String::as_str)
                .unwrap_or("current type");
            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![snippet_text_edit(replacement_range, replacement)],
            );

            Some(CodeAction {
                title: format!("Replace `{current}` with sysvar `{replacement}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "replace-invalid-sysvar",
                    "account": field.name,
                    "replacement": replacement,
                    "generatedFrom": "lang/src/lib.rs",
                })),
            })
        })
        .collect()
}

fn invalid_sysvar_replacement<'a>(
    document: &'a ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<(&'a SymbolRange, &'static str, Range)> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| accounts.fields.iter())
        .filter(|field| field.type_name.as_deref() == Some("Sysvar"))
        .filter(|field| ranges_overlap(field.range, diagnostic.range))
        .filter_map(|field| {
            let generic = field.generic_type_ranges.last()?;
            let replacement = anchor_types::sysvar_generic_for_field(&field.name)?;
            if field
                .generic_type_names
                .last()
                .is_some_and(|current| current.as_str() == replacement)
            {
                return None;
            }
            Some((field, replacement, generic.range))
        })
        .next()
}

fn ranges_overlap(left: Range, right: Range) -> bool {
    position_le(left.start, right.end) && position_le(right.start, left.end)
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || left.line == right.line && left.character <= right.character
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{diagnostics, document::ParsedDocument},
        tower_lsp::lsp_types::Url,
    };

    #[test]
    fn security_quickfixes_include_real_edits_for_native_bounds() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(_accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let tag = instruction_data[0];
    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let diagnostics = diagnostics::collect(&document);
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let actions = code_actions(&document, uri, diagnostics[0].range, &diagnostics);

        let action = actions
            .iter()
            .find(|action| action.title == "Insert data bounds guard")
            .expect("expected native bounds edit action");
        let edit = action
            .edit
            .as_ref()
            .and_then(|edit| edit.changes.as_ref())
            .and_then(|changes| changes.values().next())
            .and_then(|edits| edits.first())
            .expect("expected text edit");
        assert!(edit.new_text.contains("instruction_data.is_empty()"));
    }

    #[test]
    fn security_quickfixes_include_real_edits_for_native_discriminators() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let data = account.try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let diagnostics = diagnostics::collect(&document);
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let actions = code_actions(&document, uri, diagnostics[0].range, &diagnostics);

        let edit = action_edit(&actions, "Insert discriminator guard");
        assert!(edit.new_text.contains("data.len() < 8"));
        assert!(edit.new_text.contains("State::DISCRIMINATOR.as_ref()"));
    }

    #[test]
    fn security_quickfixes_use_pinocchio_owner_api_for_pinocchio() {
        let source = r#"
use pinocchio::{account_info::AccountInfo, ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let _data = account.borrow_data_unchecked();
    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let diagnostics = diagnostics::collect(&document);
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let actions = code_actions(&document, uri, diagnostics[0].range, &diagnostics);

        let edit = action_edit(&actions, "Insert owner guard");
        assert!(edit.new_text.contains("!account.is_owned_by(&crate::ID)"));
    }

    #[test]
    fn security_quickfixes_use_pinocchio_signer_and_writable_api_for_pinocchio() {
        let source = r#"
use pinocchio::{
    account_info::AccountInfo,
    cpi::invoke,
    instruction::{InstructionAccount, InstructionView},
    pubkey::Pubkey,
    ProgramResult,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let instruction_accounts = [InstructionAccount::writable_signer(account.key())];
    let instruction = InstructionView {
        program_id,
        accounts: &instruction_accounts,
        data: &[],
    };
    invoke(&instruction, &[account])?;
    Ok(())
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let diagnostics = diagnostics::collect(&document);
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let actions = code_actions(&document, uri, diagnostics[0].range, &diagnostics);

        let signer_edit = action_edit(&actions, "Insert signer guard");
        assert!(signer_edit.new_text.contains("!account.is_signer()"));

        let writable_edit = action_edit(&actions, "Insert writable guard");
        assert!(writable_edit.new_text.contains("!account.is_writable()"));

        let cpi_edit = action_edit(&actions, "Insert CPI program-id guard");
        assert!(cpi_edit.new_text.contains("program_id != &crate::ID"));
    }

    fn action_edit<'a>(actions: &'a [CodeAction], title: &str) -> &'a TextEdit {
        actions
            .iter()
            .find(|action| action.title == title)
            .and_then(|action| action.edit.as_ref())
            .and_then(|edit| edit.changes.as_ref())
            .and_then(|changes| changes.values().next())
            .and_then(|edits| edits.first())
            .unwrap_or_else(|| panic!("expected {title} text edit"))
    }
}
