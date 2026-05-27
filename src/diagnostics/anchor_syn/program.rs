use {
    crate::{
        diagnostics::{diagnostic_from_range, diagnostic_from_syn_error},
        range::range_from_span,
    },
    syn::{
        spanned::Spanned, FnArg, GenericArgument, Item, ItemFn, ItemMod, PathArguments, ReturnType,
        Type,
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

use super::AnchorDiagnosticKind;

pub(super) fn validate_program(item_mod: &ItemMod) -> Vec<Diagnostic> {
    match anchor_syn::parser::program::parse(item_mod.clone()) {
        Ok(_) => Vec::new(),
        Err(err) => match program_parser_diagnostic(item_mod, &err) {
            Some(diagnostic) => vec![diagnostic],
            None => vec![diagnostic_from_syn_error(err)],
        },
    }
}

fn program_parser_diagnostic(item_mod: &ItemMod, err: &syn::Error) -> Option<Diagnostic> {
    let parser_message = err.to_string();
    if parser_message_has(&parser_message, EXPECTED_RETURN_TYPE_MESSAGE)
        || parser_message_has(&parser_message, EXPECTED_GENERIC_RETURN_TYPE_MESSAGE)
    {
        return program_handler_return_diagnostic(item_mod, &parser_message);
    }

    if parser_message_has(&parser_message, MULTIPLE_FALLBACKS_MESSAGE) {
        return multiple_fallback_functions_diagnostic(item_mod, err, &parser_message);
    }

    if parser_message_has(&parser_message, PROGRAM_CONTENT_NOT_PROVIDED_MESSAGE) {
        return Some(diagnostic_from_range(
            range_from_span(item_mod.ident.span()),
            AnchorDiagnosticKind::AnchorSyn,
            format!(
                "`#[program]` module `{}` must contain inline handlers; use `pub mod {} {{ ... }}`.",
                item_mod.ident, item_mod.ident
            ),
            Some(serde_json::json!({
                "program": item_mod.ident.to_string(),
                "reason": "anchor-program-inline-module",
                "parserMessage": parser_message,
            })),
        ));
    }

    None
}

fn program_handler_return_diagnostic(
    item_mod: &ItemMod,
    parser_message: &str,
) -> Option<Diagnostic> {
    let handler = program_functions(item_mod).find(|function| {
        has_anchor_context_argument(function) && has_invalid_handler_return(function)
    })?;
    let handler_name = handler.sig.ident.to_string();
    let message = if parser_message_has(parser_message, EXPECTED_GENERIC_RETURN_TYPE_MESSAGE) {
        format!(
            "Anchor handler `{handler_name}` should use a type return argument, for example `Result<()>` or `Result<MyValue>`."
        )
    } else {
        format!("Anchor handler `{handler_name}` should return `Result<()>` or `Result<T>`.")
    };

    Some(diagnostic_from_range(
        program_handler_return_range(handler),
        AnchorDiagnosticKind::AnchorSyn,
        message,
        Some(serde_json::json!({
            "handler": handler_name,
            "expected": "Result<()> or Result<T>",
            "reason": "anchor-program-handler-return",
            "parserMessage": parser_message,
        })),
    ))
}

fn multiple_fallback_functions_diagnostic(
    item_mod: &ItemMod,
    err: &syn::Error,
    parser_message: &str,
) -> Option<Diagnostic> {
    let fallback_handlers = fallback_program_functions(item_mod)
        .map(|function| function.sig.ident.to_string())
        .collect::<Vec<_>>();
    if fallback_handlers.len() < 2 {
        return None;
    }

    let handlers = fallback_handlers.join("`, `");
    Some(diagnostic_from_range(
        range_from_span(err.span()),
        AnchorDiagnosticKind::AnchorSyn,
        format!(
            "`#[program]` module `{}` has more than one fallback handler (`{handlers}`); keep only one non-`Context<...>` fallback function.",
            item_mod.ident
        ),
        Some(serde_json::json!({
            "program": item_mod.ident.to_string(),
            "fallbackHandlers": fallback_handlers,
            "reason": "anchor-program-fallback",
            "parserMessage": parser_message,
        })),
    ))
}

fn program_functions(item_mod: &ItemMod) -> impl Iterator<Item = &ItemFn> {
    item_mod
        .content
        .iter()
        .flat_map(|(_, items)| items.iter())
        .filter_map(|item| match item {
            Item::Fn(function) => Some(function),
            _ => None,
        })
}

const EXPECTED_RETURN_TYPE_MESSAGE: &str = "expected a return type";
const EXPECTED_GENERIC_RETURN_TYPE_MESSAGE: &str = "expected generic return type to be a type";
const MULTIPLE_FALLBACKS_MESSAGE: &str = "More than one fallback function found";
const PROGRAM_CONTENT_NOT_PROVIDED_MESSAGE: &str = "program content not provided";

fn parser_message_has(parser_message: &str, expected: &str) -> bool {
    parser_message.find(expected).is_some()
}

fn fallback_program_functions(item_mod: &ItemMod) -> impl Iterator<Item = &ItemFn> {
    program_functions(item_mod).filter(|function| {
        function
            .sig
            .inputs
            .first()
            .is_some_and(|argument| matches!(argument, FnArg::Typed(_)))
            && !has_anchor_context_argument(function)
    })
}

fn has_anchor_context_argument(function: &ItemFn) -> bool {
    let Some(FnArg::Typed(argument)) = function.sig.inputs.first() else {
        return false;
    };
    let Type::Path(type_path) = argument.ty.as_ref() else {
        return false;
    };
    type_path
        .path
        .segments
        .first()
        .is_some_and(|segment| match &segment.arguments {
            PathArguments::AngleBracketed(arguments) => arguments
                .args
                .iter()
                .any(|argument| matches!(argument, GenericArgument::Type(Type::Path(_)))),
            _ => false,
        })
}

fn has_invalid_handler_return(function: &ItemFn) -> bool {
    match &function.sig.output {
        ReturnType::Default => true,
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Path(type_path) => {
                type_path
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| match &segment.arguments {
                        PathArguments::AngleBracketed(arguments) => {
                            arguments.args.iter().next_back().is_some_and(|argument| {
                                !matches!(argument, GenericArgument::Type(_))
                            })
                        }
                        _ => false,
                    })
            }
            _ => true,
        },
    }
}

fn program_handler_return_range(function: &ItemFn) -> Range {
    match &function.sig.output {
        ReturnType::Default => range_from_span(function.sig.ident.span()),
        ReturnType::Type(_, ty) => range_from_span(ty.span()),
    }
}
