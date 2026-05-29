use {
    crate::{
        constraint_catalog::{self, ConstraintSpec, ConstraintValueKind},
        document::ParsedDocument,
        range::{byte_offset_at, line_at, word_at_position},
    },
    tower_lsp::lsp_types::{
        Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, Position,
        SignatureHelp, SignatureInformation,
    },
};

pub fn signature_help(document: &ParsedDocument, position: Position) -> Option<SignatureHelp> {
    if !document.is_in_account_attribute(position) {
        return None;
    }

    let key = constraint_key_at_position(document.source(), position)
        .or_else(|| word_at_position(document.source(), position))?;
    let spec = signature_spec_for_key(document.source(), position, &key)?;
    let signature = signature_for(spec, document.source(), position);
    let active_parameter = active_parameter(document.source(), position, &signature.parameters);

    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter,
    })
}

fn signature_for(spec: &ConstraintSpec, source: &str, position: Position) -> SignatureInformation {
    let parameters = signature_parameter_labels(spec, source, position);
    SignatureInformation {
        label: signature_label_for_spec(spec, &parameters),
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: constraint_catalog::markdown_doc(spec),
        })),
        parameters: Some(
            parameters
                .iter()
                .map(|param| ParameterInformation {
                    label: ParameterLabel::Simple(param.clone()),
                    documentation: None,
                })
                .collect(),
        ),
        active_parameter: Some(0),
    }
}

fn signature_spec_for_key(
    source: &str,
    position: Position,
    key: &str,
) -> Option<&'static ConstraintSpec> {
    let prefix = account_attribute_prefix(source, position)?;
    let current_idx = prefix.rfind(key).unwrap_or(prefix.len());
    constraint_catalog::CONSTRAINTS
        .iter()
        .filter(|spec| {
            constraint_catalog::constraint_key_companions(spec).any(|companion| companion == key)
        })
        .filter_map(|spec| {
            let parent_key = constraint_catalog::key(spec.label);
            let parent_idx = prefix.rfind(parent_key)?;
            (parent_idx <= current_idx).then_some((parent_idx, spec))
        })
        .max_by_key(|(idx, spec)| (*idx, constraint_catalog::key(spec.label).len()))
        .map(|(_, spec)| spec)
        .or_else(|| {
            constraint_catalog::by_key_with_assignment(
                key,
                has_assignment_after_key(source, position, key),
            )
        })
}

fn account_attribute_prefix(source: &str, position: Position) -> Option<&str> {
    let cursor = byte_offset_at(source, position)?;
    let prefix = &source[..cursor];
    let start = prefix.rfind("#[account(")? + "#[account(".len();
    let inside = &prefix[start..];
    (!inside.contains(']')).then_some(inside)
}

fn constraint_key_at_position(source: &str, position: Position) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let prefix = &line[..cursor];
    constraint_catalog::CONSTRAINTS
        .iter()
        .filter_map(|spec| {
            let key = constraint_catalog::key(spec.label);
            let idx = prefix.rfind(key)?;
            if !has_key_boundary(prefix, idx) {
                return None;
            }
            Some((idx, key))
        })
        .max_by_key(|(idx, key)| (*idx, key.len()))
        .map(|(_, key)| key.to_string())
}

fn has_key_boundary(prefix: &str, idx: usize) -> bool {
    prefix[..idx]
        .chars()
        .next_back()
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .unwrap_or(true)
}

fn signature_label_for_spec(spec: &ConstraintSpec, parameters: &[String]) -> String {
    let key = constraint_catalog::key(spec.label);
    let own = if spec.value_kind == ConstraintValueKind::None {
        key.to_string()
    } else {
        constraint_fragment(key, spec.value_kind)
    };

    if parameters.is_empty() {
        return own;
    }

    std::iter::once(own)
        .chain(parameters.iter().map(|param| {
            constraint_catalog::by_key(param)
                .map(|spec| constraint_fragment(param, spec.value_kind))
                .unwrap_or_else(|| param.clone())
        }))
        .collect::<Vec<_>>()
        .join(", ")
}

fn signature_parameter_labels(
    spec: &ConstraintSpec,
    source: &str,
    position: Position,
) -> Vec<String> {
    let key = constraint_catalog::key(spec.label);
    let mut parameters = Vec::new();
    if spec.value_kind != ConstraintValueKind::None {
        parameters.push(key.to_string());
    }
    parameters.extend(
        constraint_catalog::constraint_key_companions(spec)
            .filter(|companion| *companion != key)
            .map(ToString::to_string),
    );
    if parameters.is_empty() && has_assignment_after_key(source, position, key) {
        parameters.push(key.to_string());
    }
    parameters
}

fn constraint_fragment(key: &str, value_kind: ConstraintValueKind) -> String {
    match value_kind {
        ConstraintValueKind::None => key.to_string(),
        ConstraintValueKind::AnyExpression => format!("{key} = <expr>"),
        ConstraintValueKind::AccountReference => format!("{key} = <account>"),
        ConstraintValueKind::SignerReference => format!("{key} = <signer>"),
        ConstraintValueKind::ProgramReference => format!("{key} = <program>"),
        ConstraintValueKind::InstructionArgument => format!("{key} = <arg-or-expr>"),
        ConstraintValueKind::Keyword => format!("{key} = <keyword>"),
        ConstraintValueKind::Boolean => format!("{key} = <bool>"),
        ConstraintValueKind::Space => format!("{key} = <bytes>"),
        ConstraintValueKind::Seeds => format!("{key} = [<seed>, ...]"),
    }
}

fn has_assignment_after_key(source: &str, position: Position, key: &str) -> bool {
    let Some(line) = line_at(source, position.line) else {
        return false;
    };
    let cursor = usize::try_from(position.character)
        .ok()
        .unwrap_or_default()
        .min(line.len());
    let prefix = &line[..cursor];
    let tail = &line[cursor..];
    if tail.trim_start().starts_with('=') {
        return true;
    }
    prefix
        .rfind(key)
        .is_some_and(|idx| prefix[idx + key.len()..].contains('='))
}

fn active_parameter(
    source: &str,
    position: Position,
    parameters: &Option<Vec<ParameterInformation>>,
) -> Option<u32> {
    let params = parameters.as_ref()?;
    let line = line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let prefix = &line[..cursor];
    params
        .iter()
        .enumerate()
        .filter_map(|(idx, param)| {
            let ParameterLabel::Simple(label) = &param.label else {
                return None;
            };
            let key = label
                .split_once(" =")
                .map(|(key, _)| key)
                .unwrap_or(label.as_str());
            prefix.rfind(key).map(|found| (found, idx as u32))
        })
        .max_by_key(|(found, _)| *found)
        .map(|(_, idx)| idx)
        .or(Some(0))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn provides_account_constraint_signature() {
        let document =
            ParsedDocument::parse_or_empty("#[account(init)]\npub state: Account<'info, State>,");
        let help = signature_help(
            &document,
            Position {
                line: 0,
                character: 11,
            },
        )
        .unwrap();

        assert!(help.signatures[0].label.contains("payer"));
        assert!(help.signatures[0].label.contains("space"));
    }

    #[test]
    fn provides_namespaced_constraint_signature() {
        let document = ParsedDocument::parse_or_empty(
            "#[account(mint::token_program = token_program)]\npub mint: Account<'info, Mint>,",
        );
        let help = signature_help(
            &document,
            Position {
                line: 0,
                character: 30,
            },
        )
        .unwrap();

        assert!(help.signatures[0].label.contains("mint::token_program"));
        assert!(help.signatures[0].label.contains("<program>"));
    }

    #[test]
    fn uses_generated_catalog_for_token_extension_signature() {
        let document = ParsedDocument::parse_or_empty(
            "#[account(extensions::transfer_hook::program_id = hook)]\npub mint: Account<'info, Mint>,",
        );
        let help = signature_help(
            &document,
            position_after(document.source(), "extensions::transfer_hook::program_id"),
        )
        .unwrap();

        assert!(help.signatures[0]
            .label
            .contains("extensions::transfer_hook::program_id = <program>"));
        assert!(help.signatures[0]
            .documentation
            .as_ref()
            .is_some_and(|documentation| format!("{documentation:?}").contains("Token-2022")));
    }

    #[test]
    fn provides_signature_help_for_every_generated_account_constraint() {
        for spec in constraint_catalog::CONSTRAINTS {
            let key = constraint_catalog::key(spec.label);
            let fragment = sample_constraint_fragment(spec);
            let source = format!("#[account({fragment})]\npub state: Account<'info, State>,");
            let document = ParsedDocument::parse_or_empty(&source);
            let help =
                signature_help(&document, position_after(&source, key)).unwrap_or_else(|| {
                    panic!("expected signature help for generated constraint `{key}`")
                });
            let signature = &help.signatures[0];

            assert!(
                signature
                    .label
                    .contains(&constraint_fragment(key, spec.value_kind)),
                "signature for `{key}` did not include generated value shape: {}",
                signature.label
            );
            assert!(
                signature
                    .documentation
                    .as_ref()
                    .is_some_and(|documentation| {
                        let rendered = format!("{documentation:?}");
                        rendered.contains(&format!("`{key}`"))
                            && rendered.contains("Family:")
                            && rendered.contains("Value:")
                    }),
                "signature for `{key}` did not include generated docs"
            );
        }
    }

    #[test]
    fn tracks_active_signature_parameter_from_constraint_key() {
        let document = ParsedDocument::parse_or_empty(
            "#[account(init, payer = user, space = 8 + State::INIT_SPACE)]\npub state: Account<'info, State>,",
        );
        let help = signature_help(&document, position_after(document.source(), "space")).unwrap();

        assert_eq!(help.active_parameter, Some(1));
    }

    #[test]
    fn supports_multiline_account_attribute_signature_help() {
        let document = ParsedDocument::parse_or_empty(
            r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + State::INIT_SPACE
    )]
    pub state: Account<'info, State>,
}
"#,
        );
        let help = signature_help(&document, position_after(document.source(), "space")).unwrap();

        assert!(help.signatures[0].label.contains("space = <bytes>"));
    }

    fn sample_constraint_fragment(spec: &ConstraintSpec) -> String {
        let key = constraint_catalog::key(spec.label);
        match spec.value_kind {
            ConstraintValueKind::None => key.to_string(),
            ConstraintValueKind::AnyExpression => format!("{key} = expr"),
            ConstraintValueKind::AccountReference => format!("{key} = account"),
            ConstraintValueKind::SignerReference => format!("{key} = signer"),
            ConstraintValueKind::ProgramReference => format!("{key} = program"),
            ConstraintValueKind::InstructionArgument => format!("{key} = arg"),
            ConstraintValueKind::Keyword => format!("{key} = skip"),
            ConstraintValueKind::Boolean => format!("{key} = true"),
            ConstraintValueKind::Space => format!("{key} = 8"),
            ConstraintValueKind::Seeds => format!("{key} = [b\"state\"]"),
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
