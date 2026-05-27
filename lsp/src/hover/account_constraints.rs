use {
    crate::{
        constraint_catalog,
        document::ParsedDocument,
        evidence::{EvidenceGraph, SeedExpressionKind},
        range::{line_at, word_at_position, word_range_at_position},
    },
    tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range},
};

pub fn hover(document: &ParsedDocument, position: Position) -> Option<Hover> {
    if !document.is_in_account_attribute(position) {
        return None;
    }

    let (key, range) = constraint_key_at_position(document.source(), position).or_else(|| {
        let word = word_at_position(document.source(), position)?;
        let range = word_range_at_position(document.source(), position)?;
        Some((word, range))
    })?;
    let spec = constraint_catalog::by_key_with_assignment(
        &key,
        has_assignment_after_key(document.source(), position, &key),
    )?;

    let mut value = constraint_catalog::markdown_doc(spec);
    if key == "seeds" {
        if let Some(seed_details) = seed_hover_details(document, position) {
            value.push_str("\n\n");
            value.push_str(&seed_details);
        }
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(range),
    })
}

fn seed_hover_details(document: &ParsedDocument, position: Position) -> Option<String> {
    let graph = EvidenceGraph::from_document(document);
    graph.account_sets().iter().find_map(|accounts| {
        accounts.fields().iter().find_map(|field| {
            field.constraints().iter().find_map(|constraint| {
                if !contains_position(constraint.range(), position) {
                    return None;
                }
                let seeds = field.seed_expressions(accounts, constraint);
                if seeds.is_empty() {
                    return None;
                }

                let mut lines = vec!["Seeds in this constraint:".to_string()];
                lines.extend(seeds.iter().map(|seed| {
                    let idl_fate = idl_fate_label(seed.kind);
                    format!("- {}: `{}` {}", seed_kind_label(seed.kind), seed.expression, idl_fate)
                }));
                if let Some(pda) = field.pda() {
                    lines.push(format!("- bump: `{}`", pda_bump_label(&pda.bump)));
                    if let Some(program_seed) = &pda.program_seed {
                        lines.push(format!("- seeds::program: `{program_seed}`"));
                    }
                }

                let has_idl_dropped = seeds.iter().any(|s| s.kind == SeedExpressionKind::Expression);
                if has_idl_dropped {
                    lines.push(String::new());
                    lines.push("⚠️ **IDL Warning**".to_string());
                    lines.push("This PDA contains seed expressions that cannot be serialized to the Anchor IDL.".to_string());
                    lines.push("Clients will need to manually replicate the seed derivation logic.".to_string());
                }

                Some(lines.join("\n"))
            })
        })
    })
}

fn pda_bump_label(bump: &crate::document::PdaBump) -> String {
    match bump {
        crate::document::PdaBump::Canonical => "canonical".to_string(),
        crate::document::PdaBump::Explicit(expr) => expr.clone(),
        crate::document::PdaBump::Missing => "missing".to_string(),
    }
}

fn seed_kind_label(kind: SeedExpressionKind) -> &'static str {
    match kind {
        SeedExpressionKind::StaticBytes => "static bytes",
        SeedExpressionKind::AccountKey => "account key",
        SeedExpressionKind::InstructionArgument => "instruction argument",
        SeedExpressionKind::Expression => "expression",
    }
}

fn idl_fate_label(kind: SeedExpressionKind) -> &'static str {
    match kind {
        SeedExpressionKind::StaticBytes
        | SeedExpressionKind::AccountKey
        | SeedExpressionKind::InstructionArgument => "✅ IDL",
        SeedExpressionKind::Expression => "❌ IDL dropped",
    }
}

fn constraint_key_at_position(source: &str, position: Position) -> Option<(String, Range)> {
    let line = line_at(source, position.line)?;
    let character = usize::try_from(position.character).ok()?;
    constraint_catalog::CONSTRAINTS
        .iter()
        .map(|spec| constraint_catalog::key(spec.label))
        .filter(|key| key.contains("::"))
        .filter_map(|key| key_range_on_line(line, position.line, character, key))
        .max_by_key(|(key, _)| key.len())
}

fn key_range_on_line(
    line: &str,
    line_number: u32,
    character: usize,
    key: &'static str,
) -> Option<(String, Range)> {
    let mut search_start = 0usize;
    while let Some(relative) = line[search_start..].find(key) {
        let start = search_start + relative;
        let end = start + key.len();
        if start <= character && character <= end && has_key_boundaries(line, start, end) {
            return Some((
                key.to_string(),
                Range {
                    start: Position {
                        line: line_number,
                        character: u32::try_from(start).ok()?,
                    },
                    end: Position {
                        line: line_number,
                        character: u32::try_from(end).ok()?,
                    },
                },
            ));
        }
        search_start = end;
    }
    None
}

fn has_key_boundaries(line: &str, start: usize, end: usize) -> bool {
    line[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        && line[end..]
            .chars()
            .next()
            .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

fn has_assignment_after_key(source: &str, position: Position, key: &str) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let cursor = usize::try_from(position.character)
        .ok()
        .unwrap_or_default()
        .min(line.len());
    let tail = &line[cursor..];
    tail.trim_start().starts_with('=')
        || line[..cursor]
            .rfind(key)
            .is_some_and(|idx| line[idx + key.len()..cursor].contains('='))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn hovers_account_constraints() {
        let source = "#[account(init)]\npub state: Account<'info, State>,";
        let document = ParsedDocument::parse_or_empty(source);
        let hover = hover(
            &document,
            Position {
                line: 0,
                character: 11,
            },
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("Create and initialize"));
        assert!(markup.value.contains("Requires"));
    }

    #[test]
    fn hovers_every_generated_account_constraint() {
        for spec in constraint_catalog::CONSTRAINTS {
            let key = constraint_catalog::key(spec.label);
            let fragment = sample_constraint_fragment(spec);
            let source = format!("#[account({fragment})]\npub state: Account<'info, State>,");
            let document = ParsedDocument::parse_or_empty(&source);
            let hover = hover(&document, position_after(&source, key))
                .unwrap_or_else(|| panic!("expected hover for generated constraint `{key}`"));

            let HoverContents::Markup(markup) = hover.contents else {
                panic!("expected markup hover for `{key}`");
            };
            assert!(
                markup.value.contains(&format!("`{key}`")),
                "hover for `{key}` did not include generated catalog key: {}",
                markup.value
            );
            assert!(
                markup.value.contains("Family: `")
                    && markup
                        .value
                        .contains(&format!("Value: `{:?}`", spec.value_kind)),
                "hover for `{key}` did not include generated catalog metadata: {}",
                markup.value
            );
            let range = hover.range.unwrap();
            assert!(
                range.end.character > range.start.character,
                "hover range should stay on the generated constraint key `{key}`"
            );
        }
    }

    #[test]
    fn hovers_namespaced_account_constraints() {
        let source =
            "#[account(init, token::mint = mint)]\npub token: Account<'info, TokenAccount>,";
        let document = ParsedDocument::parse_or_empty(source);
        let hover = hover(
            &document,
            Position {
                line: 0,
                character: 24,
            },
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`token::mint`"));
        assert!(markup.value.contains("Family: `TokenAccount`"));
        assert_eq!(hover.range.unwrap().start.character, 16);
    }

    #[test]
    fn hovers_namespaced_parser_rule_context() {
        let source =
            "#[account(seeds = [b\"seed\"], bump, seeds::program = other.key())]\npub pda: AccountInfo<'info>,";
        let document = ParsedDocument::parse_or_empty(source);
        let hover = hover(
            &document,
            Position {
                line: 0,
                character: 42,
            },
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("`seeds::program`"));
        assert!(markup.value.contains("Requires: `seeds`"));
        assert!(markup.value.contains("Conflicts with: `init`"));
    }

    #[test]
    fn hovers_structured_pda_seed_evidence() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", user.key().as_ref(), name.as_bytes()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hover = hover(
            &document,
            Position {
                line: 8,
                character: 15,
            },
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("Seeds in this constraint"));
        assert!(markup.value.contains("static bytes: `b\"state\"`"));
        assert!(markup.value.contains("account key: `user.key().as_ref()`"));
        assert!(markup
            .value
            .contains("instruction argument: `name.as_bytes()`"));
        assert!(markup.value.contains("bump: `canonical`"));
    }

    #[test]
    fn hovers_parser_backed_pda_program_seed_and_explicit_bump() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state"], bump = state_bump, seeds::program = other_program.key())]
    pub state: Account<'info, State>,
    pub other_program: Program<'info, Other>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hover = hover(
            &document,
            Position {
                line: 3,
                character: 15,
            },
        )
        .unwrap();

        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup hover");
        };
        assert!(markup.value.contains("bump: `state_bump`"));
        assert!(markup
            .value
            .contains("seeds::program: `other_program.key()`"));
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

    fn position_after(source: &str, needle: &str) -> Position {
        let offset = source.find(needle).expect("needle in source") + needle.len();
        let prefix = &source[..offset];
        let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count()).unwrap();
        let character = prefix
            .lines()
            .next_back()
            .map(|line| u32::try_from(line.chars().count()).unwrap())
            .unwrap_or(0);
        Position { line, character }
    }
}
