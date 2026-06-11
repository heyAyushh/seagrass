mod account_constraints;

use {
    crate::{
        account_semantics,
        anchor::space::{self, SpaceEstimate},
        anchor_types,
        completions::{CursorContext, CursorContextKind, ResolvedCursorContext},
        constraint_text,
        document::{ParsedDocument, SymbolRange},
        navigation,
        range::{word_at_position, word_range_at_position},
        workspace::{WorkspaceContextField, WorkspaceIndex},
    },
    tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position},
};

#[cfg(test)]
pub fn hover(document: &ParsedDocument, position: Position) -> Option<Hover> {
    hover_with_workspace(document, position, None)
}

pub fn hover_with_workspace(
    document: &ParsedDocument,
    position: Position,
    workspace: Option<&WorkspaceIndex>,
) -> Option<Hover> {
    let cursor_context = CursorContext::classify_document(document, position);
    hover_from_cursor_context(document, position, &cursor_context, workspace)
        .or_else(|| anchor_account_type_hover(document, position))
        .or_else(|| anchor_symbol_hover(document, position, cursor_context.context(), workspace))
}

fn hover_from_cursor_context(
    document: &ParsedDocument,
    position: Position,
    cursor_context: &CursorContext,
    workspace: Option<&WorkspaceIndex>,
) -> Option<Hover> {
    match cursor_context.kind() {
        CursorContextKind::AccountConstraintValue { .. } => {
            space_constraint_value_hover(document, position, cursor_context.context(), workspace)
                .or_else(|| {
                    account_constraints::hover(document, position).or_else(|| {
                        instruction_argument_hover(document, position, cursor_context.context())
                    })
                })
        }
        CursorContextKind::AccountConstraintKey { .. } => {
            account_constraints::hover(document, position).or_else(|| {
                instruction_argument_hover(document, position, cursor_context.context())
            })
        }
        _ => None,
    }
}

pub fn workspace_accounts_context_hover(
    document: &ParsedDocument,
    position: Position,
    accounts_name: &str,
    fields: &[WorkspaceContextField],
) -> Option<Hover> {
    let word = word_at_position(document.source(), position)?;
    if word != accounts_name
        || !is_context_generic_reference(document.source(), position, accounts_name)
    {
        return None;
    }

    let mut value = format!(
        "`Context<{accounts_name}>`\n\nAnchor accounts context resolved from the workspace index."
    );
    if fields.is_empty() {
        value.push_str("\n\nFields: none");
    } else {
        value.push_str("\n\nFields:");
        for field in fields.iter().take(12) {
            match field.type_display.as_deref() {
                Some(type_display) => {
                    value.push_str(&format!("\n- `{}`: `{type_display}`", field.name));
                }
                None => value.push_str(&format!("\n- `{}`", field.name)),
            }
        }
        if fields.len() > 12 {
            value.push_str(&format!("\n- ...{} more", fields.len() - 12));
        }
    }

    markdown_hover(document, position, value)
}

pub fn workspace_account_data_field_hover(
    document: &ParsedDocument,
    position: Position,
    account_data_type: &str,
    field_name: &str,
    type_display: Option<&str>,
) -> Option<Hover> {
    let word = word_at_position(document.source(), position)?;
    if word != field_name {
        return None;
    }

    let mut value = format!(
        "`{}.{}`\n\nAnchor account data field resolved from the workspace index.",
        account_data_type, field_name
    );
    if let Some(type_display) = type_display {
        value.push_str(&format!("\n\nType: `{type_display}`"));
    }
    markdown_hover(document, position, value)
}

pub fn workspace_account_field_hover(
    document: &ParsedDocument,
    position: Position,
    container: &str,
    field_name: &str,
    type_display: Option<&str>,
) -> Option<Hover> {
    let word = word_at_position(document.source(), position)?;
    if word != field_name {
        return None;
    }

    let mut value = format!(
        "`{}.{}`\n\nAnchor account field resolved from the workspace index.",
        container, field_name
    );
    if let Some(type_display) = type_display {
        value.push_str(&format!("\n\nType: `{type_display}`"));
    }
    markdown_hover(document, position, value)
}

fn anchor_account_type_hover(document: &ParsedDocument, position: Position) -> Option<Hover> {
    let (accounts, field) = account_field_type_at_position(document, position)?;
    let type_name = field.type_name.as_ref()?;
    let generic_type_names =
        account_semantics::declared_or_expected_account_inner_type(accounts, field)
            .map(|generic| vec![generic.to_string()])
            .unwrap_or_else(|| field.generic_type_names.clone());
    let completion = anchor_types::field_type_completion(type_name, &generic_type_names)?;
    let mut value = format!("`{}`\n\n{}", completion.label, completion.detail);
    value.push_str(&format!("\n\nGenerated from `{}`.", completion.source_path));
    value.push_str(&format!("\n\nField: `{}`.", field.name));
    if let Some(expected) =
        account_semantics::expected_account_inner_type_for_document_field_in_accounts(
            accounts, field,
        )
    {
        value.push_str(&format!(
            "\n\nExpected inner type: `{}` ({}).",
            expected.generic,
            match expected.evidence {
                account_semantics::AccountTypeEvidence::Constraint => "constraint",
                account_semantics::AccountTypeEvidence::AccountReference => "account reference",
                account_semantics::AccountTypeEvidence::TokenAccountProperty =>
                    "token account property",
            }
        ));
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: word_range_at_position(document.source(), position),
    })
}

fn account_field_type_at_position(
    document: &ParsedDocument,
    position: Position,
) -> Option<(&SymbolRange, &SymbolRange)> {
    document
        .symbols()
        .accounts_structs
        .values()
        .find_map(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| {
                    field
                        .type_range
                        .is_some_and(|range| contains_position(range, position))
                        || field
                            .generic_type_ranges
                            .iter()
                            .any(|generic| contains_position(generic.range, position))
                })
                .map(|field| (accounts, field))
        })
}

fn anchor_symbol_hover(
    document: &ParsedDocument,
    position: Position,
    cursor_context: &ResolvedCursorContext,
    workspace: Option<&WorkspaceIndex>,
) -> Option<Hover> {
    let word = word_at_position(document.source(), position)?;

    if let Some(target) = navigation::account_field_path_definition_target(document, position) {
        if target.field == word {
            let accounts = document.symbols().accounts_structs.get(&target.container)?;
            let field = accounts
                .fields
                .iter()
                .find(|field| field.name == target.field)?;
            let mut value = format!("`{}`\n\nAnchor field in `{}`.", field.name, accounts.name);
            if let Some(type_display) = field_type_display(field) {
                value.push_str(&format!("\n\nType: `{type_display}`"));
            }
            if !field.account_constraints.is_empty() {
                let constraints = field
                    .account_constraints
                    .iter()
                    .map(|constraint| format!("`{}`", constraint.text))
                    .collect::<Vec<_>>()
                    .join(", ");
                value.push_str(&format!("\n\nConstraints: {constraints}"));
            }
            if let Some(usages) =
                account_path_usage_summary(document, &target.container, &field.name)
            {
                value.push_str(&format!("\n\nUsed by: {usages}"));
            }
            return markdown_hover(document, position, value);
        }
    }

    if let Some((account_data_type, field_name)) =
        navigation::account_data_field_definition_target(document, position)
    {
        if field_name == word {
            let account = document
                .symbols()
                .account_data_structs
                .get(&account_data_type)?;
            let field = account
                .fields
                .iter()
                .find(|field| field.name == field_name)?;
            let mut value = format!(
                "`{}.{}`\n\nAnchor account data field accessed through `ctx.accounts`.",
                account.name, field.name
            );
            if let Some(type_display) = field_type_display(field) {
                value.push_str(&format!("\n\nType: `{type_display}`"));
            }
            if let Some(usages) =
                account_data_field_usage_summary(document, &account.name, &field.name)
            {
                value.push_str(&format!("\n\nUsed by: {usages}"));
            }
            return markdown_hover(document, position, value);
        }
    }

    if let Some(hover) = instruction_argument_hover(document, position, cursor_context) {
        return Some(hover);
    }

    if let Some(instruction) = document.symbols().callable_functions().find(|instruction| {
        instruction.name == word && contains_position(instruction.selection_range, position)
    }) {
        let context = instruction
            .context
            .as_ref()
            .map(|context| format!("Context<{}>", context.name))
            .unwrap_or_else(|| "no Anchor context".to_string());
        let args = instruction
            .arguments
            .iter()
            .map(|argument| {
                format!(
                    "`{}`{}",
                    argument.name,
                    argument
                        .type_name
                        .as_ref()
                        .map(|type_name| format!(": `{type_name}`"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        let kind = if document
            .symbols()
            .instructions
            .iter()
            .any(|candidate| candidate.selection_range == instruction.selection_range)
        {
            "Anchor program instruction"
        } else {
            "Anchor helper function"
        };
        let mut value = format!("`{}`\n\n{kind}.\n\nContext: `{context}`", instruction.name);
        if !args.is_empty() {
            value.push_str(&format!("\n\nArguments: {}", args.join(", ")));
        }
        return markdown_hover(document, position, value);
    }

    if let Some((container, field)) = field_at_position(document, &word, position) {
        let mut value = format!("`{}`\n\nAnchor field in `{container}`.", field.name);
        if let Some(type_display) = field_type_display(field) {
            value.push_str(&format!("\n\nType: `{type_display}`"));
        }
        if !field.account_constraints.is_empty() {
            let constraints = field
                .account_constraints
                .iter()
                .map(|constraint| format!("`{}`", constraint.text))
                .collect::<Vec<_>>()
                .join(", ");
            value.push_str(&format!("\n\nConstraints: {constraints}"));
        }
        if let Some(usages) = account_usage_summary(document, &container, &field.name) {
            value.push_str(&format!("\n\nUsed by: {usages}"));
        }
        return markdown_hover(document, position, value);
    }

    if let Some(symbol) = document.symbols().accounts_structs.get(&word) {
        if !contains_position(symbol.selection_range, position)
            && !is_context_generic_reference(document.source(), position, &word)
        {
            return None;
        }
        return markdown_hover(
            document,
            position,
            format!(
                "`{}`\n\nAnchor accounts context.\n\nFields: {}",
                symbol.name,
                field_names(symbol)
            ),
        );
    }

    if let Some(symbol) = document.symbols().account_data_structs.get(&word) {
        let mut value = format!(
            "`{}`\n\nAnchor account data struct.\n\nFields: {}",
            symbol.name,
            field_names(symbol)
        );
        if let Some(section) = account_space_section(symbol, document, workspace) {
            value.push_str(&section);
        }
        return markdown_hover(document, position, value);
    }

    None
}

fn space_constraint_value_hover(
    document: &ParsedDocument,
    position: Position,
    cursor_context: &ResolvedCursorContext,
    workspace: Option<&WorkspaceIndex>,
) -> Option<Hover> {
    let cursor = document.account_attribute_cursor(position)?;
    if cursor.constraint_key.as_deref() != Some("space") {
        return None;
    }
    let account_field_name = cursor.field_name.as_ref()?;
    let accounts_name = cursor_context.accounts_struct.as_ref()?.name.as_str();
    let accounts = document.symbols().accounts_structs.get(accounts_name)?;
    let field = accounts
        .fields
        .iter()
        .find(|field| field.name == *account_field_name)?;
    let account_data_type = account_data_type_for_space(accounts, field)?;
    let account_data = account_data_symbol(document, workspace, account_data_type)?;
    let report = space::account_space_report(&account_data, Some(document), workspace);
    if !report.estimate.is_known() {
        return None;
    }

    let mut value = format!(
        "`space` for `{account_data_type}`\n\nComputed: `{}`.",
        account_space_total_text(&report.estimate)
    );
    if let Some(declared) = declared_space_literal(field, cursor.range.start.line) {
        if let SpaceEstimate::Exact(data_bytes) = report.estimate {
            let computed = 8u64.saturating_add(data_bytes);
            value.push_str(&format!(
                "\n\nDeclared literal: `{declared} bytes`; computed requirement: `{computed} bytes`."
            ));
        }
    }
    markdown_hover(document, position, value)
}

fn account_data_type_for_space<'a>(
    accounts: &'a SymbolRange,
    field: &'a SymbolRange,
) -> Option<&'a str> {
    account_semantics::declared_or_expected_account_inner_type(accounts, field)
        .or_else(|| field.generic_type_names.last().map(String::as_str))
}

fn account_data_symbol(
    document: &ParsedDocument,
    workspace: Option<&WorkspaceIndex>,
    account_data_type: &str,
) -> Option<SymbolRange> {
    document
        .symbols()
        .account_data_structs
        .get(account_data_type)
        .cloned()
        .or_else(|| {
            workspace
                .and_then(|workspace| workspace.account_data_struct(account_data_type))
                .map(|entry| entry.symbol.clone())
        })
}

fn declared_space_literal(field: &SymbolRange, line: u32) -> Option<u64> {
    let constraint = field.account_constraints.iter().find(|constraint| {
        constraint.range.start.line <= line && line <= constraint.range.end.line
    })?;
    let value = constraint_text::values_after_key(&constraint.text, "space")
        .into_iter()
        .next()?;
    value.parse().ok()
}

fn account_space_section(
    symbol: &SymbolRange,
    document: &ParsedDocument,
    workspace: Option<&WorkspaceIndex>,
) -> Option<String> {
    let report = space::account_space_report(symbol, Some(document), workspace);
    if !report.estimate.is_known() {
        return None;
    }

    let mut section = "\n\n### Space\n\n| field | type | bytes |\n|---|---|---|".to_string();
    for field in &report.fields {
        let bytes = space::estimate_expr(&field.estimate)?;
        section.push_str(&format!(
            "\n| `{}` | `{}` | `{bytes}` |",
            field.field, field.type_display
        ));
    }
    section.push_str(&format!(
        "\n\n**{}** - `space = 8 + {}::INIT_SPACE`",
        account_space_total_text(&report.estimate),
        symbol.name
    ));
    Some(section)
}

fn account_space_total_text(estimate: &SpaceEstimate) -> String {
    match estimate {
        SpaceEstimate::Exact(data_bytes) => {
            let total = 8u64.saturating_add(*data_bytes);
            format!("8 (discriminator) + {data_bytes} = {total} bytes")
        }
        SpaceEstimate::Formula { fixed, symbolic } => {
            let data_expr = space::formula_expr(*fixed, symbolic);
            format!("8 (discriminator) + {data_expr} bytes")
        }
        SpaceEstimate::Unknown(reason) => format!("unknown: {reason}"),
    }
}

fn is_context_generic_reference(source: &str, position: Position, name: &str) -> bool {
    let Some(range) = word_range_at_position(source, position) else {
        return false;
    };
    let Some(line) = crate::range::line_at(source, position.line) else {
        return false;
    };
    let Ok(start) = usize::try_from(range.start.character) else {
        return false;
    };
    let Ok(end) = usize::try_from(range.end.character) else {
        return false;
    };
    let Some(word) = line.get(start..end) else {
        return false;
    };
    if word != name {
        return false;
    }
    let before = line[..start].trim_end();
    let after = line[end..].trim_start();
    before.ends_with("Context<") && (after.starts_with('>') || after.starts_with(','))
}

fn instruction_argument_hover(
    document: &ParsedDocument,
    position: Position,
    cursor_context: &ResolvedCursorContext,
) -> Option<Hover> {
    let target = cursor_context_instruction_argument_target(document, position, cursor_context)
        .or_else(|| navigation::instruction_argument_target(document, position))?;
    let type_name = cursor_context
        .instruction_arguments
        .iter()
        .find(|argument| instruction_argument_names_match(&argument.name, &target.name))
        .and_then(|argument| argument.type_name.clone())
        .or_else(|| instruction_argument_type(document, &target.context, &target.name));
    let mut value = format!(
        "`{}`\n\nAnchor instruction argument for `Context<{}>`.",
        target.name, target.context
    );
    if let Some(type_name) = type_name {
        value.push_str(&format!("\n\nType: `{type_name}`"));
    }
    if let Some(constraints) =
        instruction_argument_constraint_summary(document, &target.context, &target.name)
    {
        value.push_str(&format!("\n\nReferenced by: {constraints}"));
    }
    if let Some(count) = navigation::references(
        document,
        tower_lsp::lsp_types::Url::parse("file:///seagrass-hover.rs").ok()?,
        position,
    )
    .map(|locations| locations.len())
    .filter(|count| *count > 1)
    {
        value.push_str(&format!("\n\nKnown local references: {count}"));
    }

    markdown_hover(document, position, value)
}

fn cursor_context_instruction_argument_target(
    document: &ParsedDocument,
    position: Position,
    cursor_context: &ResolvedCursorContext,
) -> Option<navigation::InstructionArgumentTarget> {
    if !document.is_in_account_attribute(position) {
        return None;
    }
    let word = word_at_position(document.source(), position)?;
    let argument = cursor_context
        .instruction_arguments
        .iter()
        .find(|argument| instruction_argument_names_match(&argument.name, &word))?;
    Some(navigation::InstructionArgumentTarget {
        context: cursor_context.accounts_struct.as_ref()?.name.clone(),
        name: argument.name.clone(),
        range: argument.range,
    })
}

fn instruction_argument_type(
    document: &ParsedDocument,
    context_name: &str,
    argument_name: &str,
) -> Option<String> {
    document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == context_name)
        })
        .flat_map(|instruction| instruction.arguments.iter())
        .find(|argument| instruction_argument_names_match(&argument.name, argument_name))
        .and_then(|argument| argument.type_name.clone())
        .or_else(|| {
            document
                .symbols()
                .accounts_structs
                .get(context_name)?
                .instruction_arguments
                .iter()
                .find(|argument| instruction_argument_names_match(&argument.name, argument_name))
                .and_then(|argument| argument.type_name.clone())
        })
}

fn instruction_argument_constraint_summary(
    document: &ParsedDocument,
    context_name: &str,
    argument_name: &str,
) -> Option<String> {
    let accounts = document.symbols().accounts_structs.get(context_name)?;
    let mut references = Vec::new();
    for field in &accounts.fields {
        for constraint in &field.account_constraints {
            let text = constraint.text.as_str();
            if instruction_argument_aliases(argument_name)
                .iter()
                .any(|alias| text.contains(alias))
            {
                references.push(format!("`{}` on `{}`", constraint.text, field.name));
            }
        }
    }
    references.sort();
    references.dedup();
    (!references.is_empty()).then(|| references.join(", "))
}

fn instruction_argument_aliases(argument_name: &str) -> Vec<String> {
    let mut aliases = vec![argument_name.to_string()];
    let trimmed = argument_name.trim_start_matches('_');
    if trimmed != argument_name && !trimmed.is_empty() {
        aliases.push(trimmed.to_string());
    }
    aliases
}

fn instruction_argument_names_match(left: &str, right: &str) -> bool {
    left == right || left.trim_start_matches('_') == right.trim_start_matches('_')
}

fn markdown_hover(document: &ParsedDocument, position: Position, value: String) -> Option<Hover> {
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: word_range_at_position(document.source(), position),
    })
}

fn field_at_position<'a>(
    document: &'a ParsedDocument,
    word: &str,
    position: Position,
) -> Option<(String, &'a SymbolRange)> {
    if let Some(field) = document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .filter(|symbol| contains_position(symbol.range, position))
        .find_map(|symbol| {
            symbol
                .fields
                .iter()
                .find(|field| field.name == word)
                .map(|field| (symbol.name.clone(), field))
        })
    {
        return Some(field);
    }

    if let Some(field) = document
        .symbols()
        .callable_functions()
        .filter(|instruction| contains_position(instruction.range, position))
        .filter_map(|instruction| instruction.context.as_ref())
        .filter_map(|context| document.symbols().accounts_structs.get(&context.name))
        .find_map(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == word)
                .map(|field| (accounts.name.clone(), field))
        })
    {
        return Some(field);
    }

    let mut matches = document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .flat_map(|symbol| {
            symbol
                .fields
                .iter()
                .filter(move |field| field.name == word)
                .map(|field| (symbol.name.clone(), field))
        });
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn field_type_display(field: &SymbolRange) -> Option<String> {
    let type_name = field.type_name.as_ref()?;
    if field.generic_type_names.is_empty() {
        Some(type_name.clone())
    } else {
        Some(format!(
            "{}<{}>",
            type_name,
            field.generic_type_names.join(", ")
        ))
    }
}

fn field_names(symbol: &SymbolRange) -> String {
    if symbol.fields.is_empty() {
        "none".to_string()
    } else {
        symbol
            .fields
            .iter()
            .map(|field| format!("`{}`", field.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn account_usage_summary(
    document: &ParsedDocument,
    accounts_name: &str,
    field_name: &str,
) -> Option<String> {
    let usages = document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts_name)
        })
        .flat_map(|instruction| {
            instruction
                .account_usages
                .iter()
                .filter(move |usage| usage.name == field_name)
                .map(move |usage| {
                    if usage.mutable {
                        format!("`{}` mutates", instruction.name)
                    } else {
                        format!("`{}` reads", instruction.name)
                    }
                })
        })
        .collect::<Vec<_>>();
    (!usages.is_empty()).then(|| usages.join(", "))
}

fn account_data_field_usage_summary(
    document: &ParsedDocument,
    account_data_type: &str,
    field_name: &str,
) -> Option<String> {
    let usages = document
        .symbols()
        .callable_functions()
        .flat_map(|instruction| {
            instruction
                .account_data_field_usages
                .iter()
                .filter_map(move |usage| {
                    let accounts = instruction.context.as_ref().and_then(|context| {
                        document.symbols().accounts_structs.get(&context.name)
                    })?;
                    let usage_type = accounts
                        .fields
                        .iter()
                        .find(|field| field.name == usage.account)
                        .and_then(|field| field.generic_type_names.last())?;
                    (usage.field == field_name && usage_type == account_data_type).then(|| {
                        if usage.mutable {
                            format!("`{}` mutates", instruction.name)
                        } else {
                            format!("`{}` reads", instruction.name)
                        }
                    })
                })
        })
        .collect::<Vec<_>>();
    (!usages.is_empty()).then(|| usages.join(", "))
}

fn account_path_usage_summary(
    document: &ParsedDocument,
    accounts_name: &str,
    field_name: &str,
) -> Option<String> {
    let usages = document
        .symbols()
        .callable_functions()
        .filter_map(|instruction| {
            let context = instruction.context.as_ref()?;
            let used = instruction
                .account_path_usages
                .iter()
                .flat_map(|usage| {
                    usage
                        .segments
                        .iter()
                        .enumerate()
                        .filter_map(|(index, segment)| {
                            let position = navigation::AccountPathPosition {
                                context: context.name.clone(),
                                segments: usage
                                    .segments
                                    .iter()
                                    .map(|segment| segment.name.clone())
                                    .collect(),
                                segment_index: index,
                                field: segment.name.clone(),
                            };
                            navigation::account_field_path_definition_target_for_position(
                                document, &position,
                            )
                            .filter(|target| {
                                target.container == accounts_name && target.field == field_name
                            })
                            .map(|_| usage.mutable)
                        })
                })
                .next()?;
            Some(if used {
                format!("`{}` mutates", instruction.name)
            } else {
                format!("`{}` reads", instruction.name)
            })
        })
        .collect::<Vec<_>>();
    (!usages.is_empty()).then(|| usages.join(", "))
}

fn contains_position(range: tower_lsp::lsp_types::Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
}

#[cfg(test)]
mod tests;
