//! Regression guard for the "accept ≠ offer" class of bugs: every constraint
//! value idiom the completion engine *offers* must also be *accepted* by the
//! diagnostic. The two sides historically drifted (e.g. `token::ID` was
//! accepted but never suggested), so this test drives both from one idiom list.

use {super::*, crate::document::ParsedDocument, crate::lsp::diagnostics};

const UNRESOLVED_DIAGNOSTIC_MARKER: &str = "does not resolve";

struct PathIdiom {
    /// The `<owner>::` prefix the user has typed when completion fires.
    owner_prefix: &'static str,
    /// The associated value completion is expected to suggest.
    offered_value: &'static str,
}

const PROGRAM_ID_IDIOMS: &[PathIdiom] = &[
    PathIdiom {
        owner_prefix: "token::",
        offered_value: "ID",
    },
    PathIdiom {
        owner_prefix: "mpl_token_metadata::",
        offered_value: "ID",
    },
    PathIdiom {
        owner_prefix: "crate::",
        offered_value: "ID",
    },
];

fn address_constraint_source(value_expression: &str) -> String {
    format!(
        r#"
use anchor_spl::token::{{self, Token}};
use mpl_token_metadata;
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
        assert_completion_offers(idiom);
        assert_diagnostic_accepts(idiom);
    }
}

fn assert_completion_offers(idiom: &PathIdiom) {
    let source = address_constraint_source(idiom.owner_prefix);
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
    let full_expression = format!("{}{}", idiom.owner_prefix, idiom.offered_value);
    let source = address_constraint_source(&full_expression);
    let diagnostics = diagnostics::collect(&ParsedDocument::parse(&source).unwrap());

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(UNRESOLVED_DIAGNOSTIC_MARKER)),
        "diagnostic rejected the offered idiom `{full_expression}`: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| &diagnostic.message)
            .collect::<Vec<_>>()
    );
}
