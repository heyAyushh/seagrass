use {
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::{InstructionArgument, InstructionSymbol, ParsedDocument, SymbolRange},
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Url},
};

pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    document
        .symbols()
        .accounts_structs
        .values()
        .filter(|accounts| !accounts.instruction_arguments.is_empty())
        .flat_map(|accounts| instruction_attribute_diagnostics(document, accounts))
        .collect()
}

fn instruction_attribute_diagnostics(
    document: &ParsedDocument,
    accounts: &SymbolRange,
) -> Vec<Diagnostic> {
    let mapped_instructions = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        })
        .collect::<Vec<_>>();

    let [instruction] = mapped_instructions.as_slice() else {
        return Vec::new();
    };

    validate_instruction_attribute(accounts, instruction)
}

fn validate_instruction_attribute(
    accounts: &SymbolRange,
    instruction: &InstructionSymbol,
) -> Vec<Diagnostic> {
    accounts
        .instruction_arguments
        .iter()
        .enumerate()
        .filter_map(|(index, attribute_arg)| {
            let Some(expected_arg) = instruction.arguments.get(index) else {
                return Some(diagnostic_from_range_with_related(
                    attribute_arg.range,
                    AnchorDiagnosticKind::AnchorMissingInstructionArgument,
                    format!(
                        "`#[instruction]` argument `{}` has no matching handler argument on `{}`.",
                        attribute_arg.name, instruction.name
                    ),
                    Some(serde_json::json!({
                        "accountsStruct": accounts.name,
                        "instruction": instruction.name,
                        "argument": attribute_arg.name,
                        "quickfix": "remove-instruction-argument",
                        "reason": "extra-instruction-attribute-argument",
                    })),
                    Some(vec![handler_related_information(
                        instruction,
                        format!(
                            "`{}` only declares {} instruction argument(s) after `Context<{}>`.",
                            instruction.name,
                            instruction.arguments.len(),
                            accounts.name
                        ),
                    )]),
                ));
            };

            if !instruction_arg_names_match(&attribute_arg.name, &expected_arg.name) {
                return Some(diagnostic_from_range_with_related(
                    attribute_arg.range,
                    AnchorDiagnosticKind::AnchorMissingInstructionArgument,
                    format!(
                        "`#[instruction]` argument `{}` is out of order for `{}`; expected `{}` at position {}.",
                        attribute_arg.name,
                        instruction.name,
                        expected_arg.name,
                        index + 1
                    ),
                    Some(serde_json::json!({
                        "accountsStruct": accounts.name,
                        "instruction": instruction.name,
                        "argument": attribute_arg.name,
                        "expected": expected_arg.name,
                        "expectedType": expected_arg.type_name,
                        "position": index,
                        "quickfix": "replace-instruction-argument",
                        "reason": "instruction-attribute-order",
                    })),
                    Some(vec![handler_argument_related_information(
                        expected_arg,
                        format!(
                            "`{}` expects `{}` at this position.",
                            instruction.name, expected_arg.name
                        ),
                    )]),
                ));
            }

            if attribute_arg.type_name.is_some()
                && expected_arg.type_name.is_some()
                && attribute_arg.type_name != expected_arg.type_name
            {
                return Some(diagnostic_from_range_with_related(
                    attribute_arg.range,
                    AnchorDiagnosticKind::AnchorMissingInstructionArgument,
                    format!(
                        "`#[instruction]` argument `{}` has type `{}` but `{}` expects `{}`.",
                        attribute_arg.name,
                        attribute_arg.type_name.as_deref().unwrap_or("_"),
                        instruction.name,
                        expected_arg.type_name.as_deref().unwrap_or("_")
                    ),
                    Some(serde_json::json!({
                        "accountsStruct": accounts.name,
                        "instruction": instruction.name,
                        "argument": attribute_arg.name,
                        "expected": expected_arg.name,
                        "expectedType": expected_arg.type_name,
                        "actualType": attribute_arg.type_name,
                        "quickfix": "replace-instruction-argument",
                        "reason": "instruction-attribute-type",
                    })),
                    Some(vec![handler_argument_related_information(
                        expected_arg,
                        format!(
                            "`{}` declares `{}` with type `{}` here.",
                            instruction.name,
                            expected_arg.name,
                            expected_arg.type_name.as_deref().unwrap_or("_")
                        ),
                    )]),
                ));
            }

            None
        })
        .collect()
}

fn instruction_arg_names_match(attribute_name: &str, handler_name: &str) -> bool {
    attribute_name == handler_name
        || attribute_name.trim_start_matches('_') == handler_name.trim_start_matches('_')
}

fn handler_argument_related_information(
    argument: &InstructionArgument,
    message: String,
) -> DiagnosticRelatedInformation {
    DiagnosticRelatedInformation {
        location: Location {
            uri: current_document_uri(),
            range: argument.range,
        },
        message,
    }
}

fn handler_related_information(
    instruction: &InstructionSymbol,
    message: String,
) -> DiagnosticRelatedInformation {
    DiagnosticRelatedInformation {
        location: Location {
            uri: current_document_uri(),
            range: instruction.selection_range,
        },
        message,
    }
}

fn current_document_uri() -> Url {
    Url::parse("file:///seagrass/current-document.rs").unwrap()
}

#[cfg(test)]
mod tests {
    use {
        super::*, crate::diagnostics::registry::ANCHOR_MISSING_INSTRUCTION_ARGUMENT_CODE,
        tower_lsp::lsp_types::NumberOrString,
    };

    #[test]
    fn accepts_instruction_attribute_prefix_arguments() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(decimals: u8)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_skipped_instruction_attribute_argument() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#,
        );

        let diagnostic = instruction_attribute_diagnostic(&diagnostics);
        assert!(diagnostic.message.contains("out of order"));
        assert!(diagnostic.message.contains("expected `decimals`"));
        assert_eq!(diagnostic.range.start.line, 7);
        assert!(diagnostic
            .related_information
            .as_ref()
            .expect("expected related handler argument")
            .iter()
            .any(|info| info.message.contains("expects `decimals`")));
    }

    #[test]
    fn reports_instruction_attribute_type_mismatch() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(decimals: u64)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#,
        );

        let diagnostic = instruction_attribute_diagnostic(&diagnostics);
        assert!(diagnostic.message.contains("has type `u64`"));
        assert!(diagnostic.message.contains("expects `u8`"));
        assert!(diagnostic
            .related_information
            .as_ref()
            .expect("expected related handler argument")
            .iter()
            .any(|info| info.message.contains("type `u8`")));
    }

    #[test]
    fn accepts_handler_argument_with_leading_unused_underscore() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
pub mod demo {
    pub fn create_token(ctx: Context<Create>, _token_name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(token_name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_extra_instruction_attribute_argument() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, decimals: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(decimals: u8, name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#,
        );

        let diagnostic = instruction_attribute_diagnostic(&diagnostics);
        assert!(diagnostic
            .message
            .contains("has no matching handler argument"));
        assert!(diagnostic.message.contains("name"));
        assert!(diagnostic
            .related_information
            .as_ref()
            .expect("expected related handler signature")
            .iter()
            .any(|info| info
                .message
                .contains("only declares 1 instruction argument")));
    }

    fn diagnostics_for(source: &str) -> Vec<Diagnostic> {
        let document = ParsedDocument::parse(source).unwrap();
        collect(&document)
    }

    fn instruction_attribute_diagnostic(diagnostics: &[Diagnostic]) -> &Diagnostic {
        diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code
                    == Some(NumberOrString::String(
                        ANCHOR_MISSING_INSTRUCTION_ARGUMENT_CODE.to_string(),
                    ))
            })
            .expect("expected instruction attribute diagnostic")
    }
}
