use {
    crate::{
        constraint_catalog,
        document::ParsedDocument,
        range::matching_word_ranges,
        syntax::{AnchorQueryCapture, AnchorQueryKind},
    },
    tower_lsp::lsp_types::{
        Position, Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
        SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    },
};

const TOKEN_KEYWORD: u32 = 0;
const TOKEN_PROPERTY: u32 = 1;
const TOKEN_VARIABLE: u32 = 2;
const TOKEN_STRUCT: u32 = 3;
const MOD_DECLARATION: u32 = 1;

pub fn options() -> SemanticTokensOptions {
    SemanticTokensOptions {
        legend: SemanticTokensLegend {
            token_types: vec![
                SemanticTokenType::KEYWORD,
                SemanticTokenType::PROPERTY,
                SemanticTokenType::VARIABLE,
                SemanticTokenType::STRUCT,
            ],
            token_modifiers: vec![SemanticTokenModifier::DECLARATION],
        },
        full: Some(SemanticTokensFullOptions::Bool(true)),
        range: Some(true),
        ..SemanticTokensOptions::default()
    }
}

pub fn full(document: &ParsedDocument) -> SemanticTokens {
    semantic_tokens(semantic_raw_tokens(document))
}

pub fn range(document: &ParsedDocument, range: Range) -> SemanticTokens {
    semantic_tokens(
        semantic_raw_tokens(document)
            .into_iter()
            .filter(|token| token.overlaps(range))
            .collect(),
    )
}

fn semantic_raw_tokens(document: &ParsedDocument) -> Vec<RawToken> {
    let mut raw = Vec::new();

    for constraint in constraint_catalog::CONSTRAINTS
        .iter()
        .filter_map(|spec| spec.label.strip_suffix(" =").or(Some(spec.label)))
    {
        for range in matching_word_ranges(document.source(), constraint) {
            if document.is_in_account_attribute(range.start) {
                raw.push(RawToken {
                    line: range.start.line,
                    start: range.start.character,
                    length: range.end.character.saturating_sub(range.start.character),
                    token_type: TOKEN_PROPERTY,
                    modifiers: 0,
                });
            }
        }
    }

    for accounts in document.symbols().accounts_structs.values() {
        if let Some(range) = accounts.derive_accounts_range {
            raw.push(RawToken::from_range(range, TOKEN_STRUCT, 0));
        }
        raw.push(RawToken::from_range(
            accounts.selection_range,
            TOKEN_STRUCT,
            MOD_DECLARATION,
        ));
        for field in &accounts.fields {
            raw.push(RawToken::from_range(
                field.selection_range,
                TOKEN_VARIABLE,
                MOD_DECLARATION,
            ));
        }
    }
    for account in document.symbols().account_data_structs.values() {
        raw.push(RawToken::from_range(
            account.selection_range,
            TOKEN_STRUCT,
            MOD_DECLARATION,
        ));
    }
    for instruction in &document.symbols().instructions {
        raw.push(RawToken::from_range(
            instruction.selection_range,
            TOKEN_KEYWORD,
            MOD_DECLARATION,
        ));
    }
    for instruction in document
        .symbols()
        .functions
        .iter()
        .filter(|function| function.context.is_some())
    {
        raw.push(RawToken::from_range(
            instruction.selection_range,
            TOKEN_KEYWORD,
            MOD_DECLARATION,
        ));
    }

    if let Some(syntax) = document.tree_sitter() {
        raw.extend(
            syntax
                .anchor_query_captures(document.source())
                .into_iter()
                .filter_map(overlay_semantic_token),
        );
    }

    raw.sort_by_key(|token| (token.line, token.start, token.length));
    raw.dedup_by_key(|token| (token.line, token.start, token.length, token.token_type));
    raw
}

fn semantic_tokens(raw: Vec<RawToken>) -> SemanticTokens {
    SemanticTokens {
        result_id: None,
        data: encode(raw),
    }
}

fn encode(tokens: Vec<RawToken>) -> Vec<SemanticToken> {
    let mut previous_line = 0;
    let mut previous_start = 0;

    tokens
        .into_iter()
        .map(|token| {
            let delta_line = token.line.saturating_sub(previous_line);
            let delta_start = if delta_line == 0 {
                token.start.saturating_sub(previous_start)
            } else {
                token.start
            };
            previous_line = token.line;
            previous_start = token.start;

            SemanticToken {
                delta_line,
                delta_start,
                length: token.length,
                token_type: token.token_type,
                token_modifiers_bitset: token.modifiers,
            }
        })
        .collect()
}

fn overlay_semantic_token(capture: AnchorQueryCapture) -> Option<RawToken> {
    let (token_type, modifiers) = match capture.kind {
        AnchorQueryKind::ProgramModule
        | AnchorQueryKind::AccountsStruct
        | AnchorQueryKind::AccountDataStruct => (TOKEN_STRUCT, MOD_DECLARATION),
        AnchorQueryKind::ProgramInstruction => (TOKEN_KEYWORD, MOD_DECLARATION),
        AnchorQueryKind::AccountField => (TOKEN_VARIABLE, MOD_DECLARATION),
        AnchorQueryKind::AccountConstraintKey => (TOKEN_PROPERTY, 0),
        AnchorQueryKind::ProgramAttribute
        | AnchorQueryKind::DeriveAccountsAttribute
        | AnchorQueryKind::AccountAttribute => return None,
    };

    RawToken::from_single_line_range(capture.range, token_type, modifiers)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

impl RawToken {
    fn from_range(range: tower_lsp::lsp_types::Range, token_type: u32, modifiers: u32) -> Self {
        Self {
            line: range.start.line,
            start: range.start.character,
            length: range.end.character.saturating_sub(range.start.character),
            token_type,
            modifiers,
        }
    }

    fn from_single_line_range(
        range: tower_lsp::lsp_types::Range,
        token_type: u32,
        modifiers: u32,
    ) -> Option<Self> {
        (range.start.line == range.end.line && range.start.character < range.end.character)
            .then(|| Self::from_range(range, token_type, modifiers))
    }

    fn overlaps(self, range: Range) -> bool {
        let start = Position {
            line: self.line,
            character: self.start,
        };
        let end = Position {
            line: self.line,
            character: self.start.saturating_add(self.length),
        };

        position_le(start, range.end) && position_le(range.start, end)
    }
}

fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn emits_anchor_semantic_tokens() {
        let document = ParsedDocument::parse(
            r#"
#[program]
mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {}
"#,
        )
        .unwrap();

        assert!(!full(&document).data.is_empty());
    }

    #[test]
    fn semantic_tokens_use_tree_sitter_overlay_for_incomplete_anchor_rust() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = user,
        seeds::program = other_program.key(),
        space = 8 +
    )]
    pub state: Account<'info, State>,
    pub rent: Sysvar<'info, R
}

#[account]
pub struct State {}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let tokens = absolute_semantic_tokens(full(&document).data);
        let token_texts = tokens
            .iter()
            .filter_map(|token| token_text(source, token).map(|text| (text, *token)))
            .collect::<Vec<_>>();

        assert!(token_texts
            .iter()
            .any(|(text, token)| text == "demo" && token.token_type == TOKEN_STRUCT));
        assert!(token_texts
            .iter()
            .any(|(text, token)| text == "initialize" && token.token_type == TOKEN_KEYWORD));
        assert!(token_texts
            .iter()
            .any(|(text, token)| text == "Create" && token.token_type == TOKEN_STRUCT));
        assert!(token_texts
            .iter()
            .any(|(text, token)| text == "state" && token.token_type == TOKEN_VARIABLE));
        assert!(token_texts.iter().any(|(text, token)| {
            text == "seeds::program" && token.token_type == TOKEN_PROPERTY
        }));
    }

    #[test]
    fn semantic_token_range_returns_only_tokens_touching_requested_lines() {
        let source = r#"
#[program]
mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let tokens = absolute_semantic_tokens(
            range(
                &document,
                Range {
                    start: tower_lsp::lsp_types::Position {
                        line: 6,
                        character: 0,
                    },
                    end: tower_lsp::lsp_types::Position {
                        line: 11,
                        character: 1,
                    },
                },
            )
            .data,
        );
        let token_texts = tokens
            .iter()
            .filter_map(|token| token_text(source, token))
            .collect::<Vec<_>>();

        assert!(token_texts.iter().any(|text| text == "Initialize"));
        assert!(token_texts.iter().any(|text| text == "state"));
        assert!(!token_texts.iter().any(|text| text == "demo"));
        assert!(!token_texts.iter().any(|text| text == "State"));
    }

    #[test]
    fn ignores_plain_words_that_look_like_anchor_markers() {
        let document = ParsedDocument::parse(
            r#"
pub fn notes() {
    let account = "not an Anchor account attribute";
    let program = "not an Anchor program attribute";
}
"#,
        )
        .unwrap();

        assert!(full(&document).data.is_empty());
    }

    #[test]
    fn tokenizes_every_generated_account_constraint_key() {
        let constraints = constraint_catalog::CONSTRAINTS
            .iter()
            .map(sample_constraint_fragment)
            .collect::<Vec<_>>()
            .join(",\n        ");
        let source = format!(
            r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account(
        {constraints}
    )]
    pub state: Account<'info, State>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let tokens = absolute_semantic_tokens(full(&document).data);
        let property_texts = tokens
            .iter()
            .filter(|token| token.token_type == TOKEN_PROPERTY)
            .filter_map(|token| token_text(&source, token))
            .collect::<Vec<_>>();

        for key in constraint_catalog::CONSTRAINTS
            .iter()
            .map(|spec| constraint_catalog::key(spec.label))
        {
            let expected_occurrences = constraint_catalog::CONSTRAINTS
                .iter()
                .filter(|spec| constraint_catalog::key(spec.label) == key)
                .count();
            let actual_occurrences = property_texts
                .iter()
                .filter(|text| text.as_str() == key)
                .count();
            assert!(
                actual_occurrences >= expected_occurrences,
                "expected semantic token(s) for generated constraint `{key}`; got {:?}",
                property_texts
            );
        }
    }

    fn sample_constraint_fragment(spec: &constraint_catalog::ConstraintSpec) -> String {
        let key = constraint_catalog::key(spec.label);
        match spec.value_kind {
            constraint_catalog::ConstraintValueKind::None => key.to_string(),
            constraint_catalog::ConstraintValueKind::AnyExpression => format!("{key} = expr"),
            constraint_catalog::ConstraintValueKind::AccountReference => {
                format!("{key} = account")
            }
            constraint_catalog::ConstraintValueKind::SignerReference => format!("{key} = signer"),
            constraint_catalog::ConstraintValueKind::ProgramReference => {
                format!("{key} = program")
            }
            constraint_catalog::ConstraintValueKind::InstructionArgument => {
                format!("{key} = arg")
            }
            constraint_catalog::ConstraintValueKind::Keyword => format!("{key} = skip"),
            constraint_catalog::ConstraintValueKind::Boolean => format!("{key} = true"),
            constraint_catalog::ConstraintValueKind::Space => format!("{key} = 8"),
            constraint_catalog::ConstraintValueKind::Seeds => {
                format!("{key} = [b\"state\"]")
            }
        }
    }

    fn absolute_semantic_tokens(tokens: Vec<SemanticToken>) -> Vec<RawToken> {
        let mut line = 0;
        let mut start = 0;
        tokens
            .into_iter()
            .map(|token| {
                line += token.delta_line;
                start = if token.delta_line == 0 {
                    start + token.delta_start
                } else {
                    token.delta_start
                };
                RawToken {
                    line,
                    start,
                    length: token.length,
                    token_type: token.token_type,
                    modifiers: token.token_modifiers_bitset,
                }
            })
            .collect()
    }

    fn token_text(source: &str, token: &RawToken) -> Option<String> {
        let line = source.lines().nth(usize::try_from(token.line).ok()?)?;
        let start = usize::try_from(token.start).ok()?;
        let length = usize::try_from(token.length).ok()?;
        Some(line.chars().skip(start).take(length).collect())
    }
}
