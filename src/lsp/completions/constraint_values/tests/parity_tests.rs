//! Regression guard for the "accept ≠ offer" class of bugs: every constraint
//! value idiom the completion engine *offers* must also be *accepted* by the
//! diagnostic. The two sides historically drifted (e.g. `token::ID` was
//! accepted but never suggested), so this test drives both from one idiom list.

use {super::*, crate::document::ParsedDocument, crate::lsp::diagnostics};

const UNRESOLVED_DIAGNOSTIC_MARKER: &str = "does not resolve";

struct PathIdiom {
    /// Import that makes the offered module path resolve. Empty for crate-relative paths.
    import_decl: &'static str,
    /// The `<owner>::` prefix the user has typed when completion fires.
    owner_prefix: &'static str,
    /// The full expression offered by whole-expression completion.
    full_expression: &'static str,
    /// The associated value completion is expected to suggest.
    offered_value: &'static str,
    /// Whether the path comes from the static non-sysvar completion table.
    non_sysvar_address: bool,
}

const PROGRAM_ID_IDIOMS: &[PathIdiom] = &[
    PathIdiom {
        import_decl: "use anchor_lang::system_program;",
        owner_prefix: "system_program::",
        full_expression: "system_program::ID",
        offered_value: "ID",
        non_sysvar_address: true,
    },
    PathIdiom {
        import_decl: "use anchor_spl::token;",
        owner_prefix: "token::",
        full_expression: "token::ID",
        offered_value: "ID",
        non_sysvar_address: true,
    },
    PathIdiom {
        import_decl: "use anchor_spl::token_2022;",
        owner_prefix: "token_2022::",
        full_expression: "token_2022::ID",
        offered_value: "ID",
        non_sysvar_address: true,
    },
    PathIdiom {
        import_decl: "use anchor_spl::associated_token;",
        owner_prefix: "associated_token::",
        full_expression: "associated_token::ID",
        offered_value: "ID",
        non_sysvar_address: true,
    },
    PathIdiom {
        import_decl: "use mpl_token_metadata;",
        owner_prefix: "mpl_token_metadata::",
        full_expression: "mpl_token_metadata::ID",
        offered_value: "ID",
        non_sysvar_address: true,
    },
    PathIdiom {
        import_decl: "",
        owner_prefix: "crate::",
        full_expression: "crate::ID",
        offered_value: "ID",
        non_sysvar_address: false,
    },
];

fn address_constraint_source(import_decl: &str, value_expression: &str) -> String {
    format!(
        r#"
{import_decl}
declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(address = {value_expression})]
    pub mint: AccountInfo<'info>,
}}
"#
    )
}

#[test]
fn completion_offers_are_accepted_by_diagnostic() {
    for idiom in PROGRAM_ID_IDIOMS {
        if idiom.non_sysvar_address {
            assert_whole_expression_completion_offers(idiom);
        }
        assert_module_completion_offers(idiom);
        assert_diagnostic_accepts(idiom);
    }
}

#[test]
fn non_sysvar_parity_cases_cover_static_address_completions() {
    let covered = PROGRAM_ID_IDIOMS
        .iter()
        .filter(|idiom| idiom.non_sysvar_address)
        .map(|idiom| idiom.full_expression)
        .collect::<Vec<_>>();
    let offered = program_ids::non_sysvar_address_paths().collect::<Vec<_>>();

    assert_eq!(covered, offered);
}

#[test]
fn unimported_program_id_paths_are_neither_offered_nor_accepted() {
    for idiom in PROGRAM_ID_IDIOMS
        .iter()
        .filter(|idiom| idiom.non_sysvar_address)
    {
        let source = address_constraint_source("", "");
        let cursor = position_after(&source, "address = ");
        let items = completions(&ParsedDocument::parse(&source).unwrap(), cursor)
            .unwrap_or_else(|| panic!("no address completions for `{}`", idiom.full_expression));

        assert!(
            items.iter().all(|item| item.label != idiom.full_expression),
            "completion offered unresolved idiom `{}` without import",
            idiom.full_expression
        );

        let source = address_constraint_source("", idiom.full_expression);
        let diagnostics = diagnostics::collect(&ParsedDocument::parse(&source).unwrap());
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(UNRESOLVED_DIAGNOSTIC_MARKER)),
            "diagnostic accepted unimported idiom `{}`",
            idiom.full_expression
        );
    }
}

fn assert_whole_expression_completion_offers(idiom: &PathIdiom) {
    let source = address_constraint_source(idiom.import_decl, "");
    let cursor = position_after(&source, "address = ");
    let items = completions(&ParsedDocument::parse(&source).unwrap(), cursor)
        .unwrap_or_else(|| panic!("no address completions for `{}`", idiom.full_expression));

    assert!(
        items.iter().any(|item| item.label == idiom.full_expression),
        "whole-expression completion did not offer `{}`",
        idiom.full_expression
    );
}

fn assert_module_completion_offers(idiom: &PathIdiom) {
    let source = address_constraint_source(idiom.import_decl, idiom.owner_prefix);
    let cursor = position_after(&source, &format!("address = {}", idiom.owner_prefix));
    let items = completions(&ParsedDocument::parse(&source).unwrap(), cursor)
        .unwrap_or_else(|| panic!("no completions offered for `{}`", idiom.owner_prefix));

    assert!(
        items.iter().any(|item| item.label == idiom.offered_value),
        "completion for `{}` did not offer `{}`",
        idiom.owner_prefix,
        idiom.offered_value
    );
}

fn assert_diagnostic_accepts(idiom: &PathIdiom) {
    let source = address_constraint_source(idiom.import_decl, idiom.full_expression);
    let diagnostics = diagnostics::collect(&ParsedDocument::parse(&source).unwrap());

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(UNRESOLVED_DIAGNOSTIC_MARKER)),
        "diagnostic rejected the offered idiom `{}`: {:?}",
        idiom.full_expression,
        diagnostics
            .iter()
            .map(|diagnostic| &diagnostic.message)
            .collect::<Vec<_>>()
    );
}
