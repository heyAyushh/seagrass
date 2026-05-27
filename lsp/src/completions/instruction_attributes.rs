use {
    crate::{
        document::{InstructionArgument, InstructionSymbol, ParsedDocument, SymbolRange},
        range::line_at,
    },
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Position},
};

pub fn completions(document: &ParsedDocument, position: Position) -> Option<Vec<CompletionItem>> {
    let context = instruction_attribute_context(document.source(), position)?;
    let accounts = accounts_struct_for_instruction_attribute(document, position)?;
    let instruction = mapped_instruction(document, accounts)?;
    let completed = completed_attribute_arguments(context.prefix);
    let current_prefix = current_attribute_argument_prefix(context.prefix);
    let next_index = completed.len();
    let expected_next = instruction.arguments.get(next_index)?;

    let items = instruction
        .arguments
        .iter()
        .enumerate()
        .skip(next_index)
        .filter(|(_, argument)| argument.name.starts_with(current_prefix))
        .map(|(index, argument)| {
            instruction_argument_item(argument, index, expected_next.name.as_str())
        })
        .collect::<Vec<_>>();

    (!items.is_empty()).then_some(items)
}

struct InstructionAttributeContext<'a> {
    prefix: &'a str,
}

fn instruction_attribute_context(
    source: &str,
    position: Position,
) -> Option<InstructionAttributeContext<'_>> {
    let line = line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let prefix = &line[..cursor];
    let open = prefix.rfind("#[instruction(")?;
    let after_open = &prefix[open + "#[instruction(".len()..];
    if after_open.contains(']') {
        return None;
    }

    Some(InstructionAttributeContext { prefix: after_open })
}

fn accounts_struct_for_instruction_attribute(
    document: &ParsedDocument,
    position: Position,
) -> Option<&SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .filter(|accounts| {
            accounts.range.start.line <= position.line
                && position.line <= accounts.selection_range.start.line
        })
        .min_by_key(|accounts| {
            accounts
                .selection_range
                .start
                .line
                .saturating_sub(position.line)
        })
        .or_else(|| {
            document
                .symbols()
                .accounts_structs
                .values()
                .filter(|accounts| position.line < accounts.selection_range.start.line)
                .min_by_key(|accounts| accounts.selection_range.start.line - position.line)
        })
}

fn mapped_instruction<'a>(
    document: &'a ParsedDocument,
    accounts: &SymbolRange,
) -> Option<&'a InstructionSymbol> {
    let mut instructions = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        });

    let instruction = instructions.next()?;
    instructions.next().is_none().then_some(instruction)
}

fn completed_attribute_arguments(prefix: &str) -> Vec<&str> {
    prefix
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.split_once(':').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty())
        .collect()
}

fn current_attribute_argument_prefix(prefix: &str) -> &str {
    let tail = prefix.rsplit(',').next().unwrap_or(prefix).trim_start();
    let end = tail
        .char_indices()
        .find_map(|(idx, ch)| (!(ch.is_ascii_alphanumeric() || ch == '_')).then_some(idx))
        .unwrap_or(tail.len());
    &tail[..end]
}

fn instruction_argument_item(
    argument: &InstructionArgument,
    index: usize,
    expected_next: &str,
) -> CompletionItem {
    let label = match &argument.type_name {
        Some(type_name) => format!("{}: {}", argument.name, type_name),
        None => argument.name.clone(),
    };
    let is_next = argument.name == expected_next;
    CompletionItem {
        label: label.clone(),
        kind: Some(CompletionItemKind::VARIABLE),
        detail: Some("Anchor instruction argument".to_string()),
        insert_text: Some(label),
        sort_text: Some(format!(
            "{:03}_anchor_instruction_arg_{:04}_{}",
            if is_next { 0 } else { 1 },
            index,
            argument.name
        )),
        preselect: Some(is_next),
        data: Some(serde_json::json!({
            "anchorCompletion": "instruction-argument",
            "argument": argument.name,
            "position": index,
        })),
        ..CompletionItem::default()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn completes_first_instruction_attribute_argument_from_handler_after_typing() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(d)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let completions =
            completions(&document, position_after(source, "#[instruction(d")).unwrap();

        assert_eq!(completions[0].label, "decimals: u8");
        assert_eq!(completions[0].preselect, Some(true));
        assert!(!completions.iter().any(|item| item.label == "name: String"));
    }

    #[test]
    fn completes_first_instruction_attribute_argument_after_space_trigger() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction( )]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let completions =
            completions(&document, position_after(source, "#[instruction( ")).unwrap();

        assert_eq!(completions[0].label, "decimals: u8");
        assert_eq!(completions[0].preselect, Some(true));
    }

    #[test]
    fn completes_next_instruction_attribute_argument_after_valid_prefix_and_typing() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(decimals: u8, n)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let completions = completions(
            &document,
            position_after(source, "#[instruction(decimals: u8, n"),
        )
        .unwrap();

        assert_eq!(completions[0].label, "name: String");
        assert_eq!(completions[0].preselect, Some(true));
        assert!(!completions.iter().any(|item| item.label == "decimals: u8"));
    }

    #[test]
    fn does_not_complete_when_multiple_handlers_share_accounts_struct() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8) -> Result<()> { Ok(()) }
    pub fn reset(ctx: Context<Create>, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction()]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse_or_empty(source);

        assert!(completions(&document, position_after(source, "#[instruction(")).is_none());
    }

    fn position_after(source: &str, needle: &str) -> Position {
        let offset = source.find(needle).expect("needle present") + needle.len();
        let before = &source[..offset];
        Position {
            line: before.bytes().filter(|byte| *byte == b'\n').count() as u32,
            character: before.rsplit('\n').next().unwrap_or("").chars().count() as u32,
        }
    }
}
