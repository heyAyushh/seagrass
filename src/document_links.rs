use {
    crate::{
        constraint_catalog, constraint_ranges::constraint_key_ranges, document::ParsedDocument,
    },
    tower_lsp::lsp_types::{DocumentLink, Url},
};

const ACCOUNT_CONSTRAINT_DOCS: &str =
    "https://www.anchor-lang.com/docs/references/account-constraints";

pub fn document_links(document: &ParsedDocument) -> Vec<DocumentLink> {
    let mut links = Vec::new();

    for accounts in document.symbols().accounts_structs.values() {
        for field in &accounts.fields {
            for attribute in &field.account_constraints {
                for key_range in constraint_key_ranges(document.source(), attribute.range) {
                    let Some(spec) = constraint_catalog::by_key(key_range.key) else {
                        continue;
                    };
                    links.push(DocumentLink {
                        range: key_range.range,
                        target: Url::parse(ACCOUNT_CONSTRAINT_DOCS).ok(),
                        tooltip: Some(format!(
                            "Open Anchor docs for `{}`",
                            constraint_catalog::key(spec.label)
                        )),
                        data: Some(serde_json::json!({
                            "anchor": {
                                "kind": "accountConstraint",
                                "key": constraint_catalog::key(spec.label),
                                "family": format!("{:?}", spec.family),
                            }
                        })),
                    });
                }
            }
        }
    }

    links.sort_by_key(|link| (link.range.start.line, link.range.start.character));
    links.dedup_by_key(|link| {
        (
            link.range.start.line,
            link.range.start.character,
            link.range.end.line,
            link.range.end.character,
        )
    });
    links
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn links_generated_account_constraints_to_anchor_docs() {
        let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, seeds::program = token_program.key(), bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let links = document_links(&document);
        let linked_text = links.iter().filter_map(link_key).collect::<Vec<_>>();

        assert!(linked_text.contains(&"init".to_string()));
        assert!(linked_text.contains(&"payer".to_string()));
        assert!(linked_text.contains(&"seeds::program".to_string()));
        assert!(linked_text.contains(&"bump".to_string()));
        assert!(links.iter().all(|link| link
            .target
            .as_ref()
            .is_some_and(|target| target.as_str() == ACCOUNT_CONSTRAINT_DOCS)));
    }

    #[test]
    fn links_every_generated_account_constraint_to_anchor_docs() {
        let constraints = constraint_catalog::CONSTRAINTS
            .iter()
            .map(sample_constraint_fragment)
            .collect::<Vec<_>>()
            .join(",\n        ");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {{
    #[account(
        {constraints}
    )]
    pub state: Account<'info, State>,
}}
"#,
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let links = document_links(&document);
        let linked_keys = links.iter().filter_map(link_key).collect::<Vec<_>>();

        for spec in constraint_catalog::CONSTRAINTS {
            let key = constraint_catalog::key(spec.label);
            let expected_occurrences = constraint_catalog::CONSTRAINTS
                .iter()
                .filter(|candidate| constraint_catalog::key(candidate.label) == key)
                .count();
            let actual_occurrences = linked_keys.iter().filter(|linked| *linked == key).count();

            assert!(
                actual_occurrences >= expected_occurrences,
                "expected document link(s) for generated constraint `{key}`; got {:?}",
                linked_keys
            );
            assert!(
                links.iter().any(|link| {
                    link_key(link).as_deref() == Some(key)
                        && link
                            .target
                            .as_ref()
                            .is_some_and(|target| target.as_str() == ACCOUNT_CONSTRAINT_DOCS)
                        && link
                            .data
                            .as_ref()
                            .and_then(|data| data.get("anchor"))
                            .and_then(|anchor| anchor.get("family"))
                            .and_then(|family| family.as_str())
                            == Some(&format!("{:?}", spec.family))
                }),
                "expected document link metadata for generated constraint `{key}`"
            );
        }
    }

    #[test]
    fn links_prefer_namespaced_constraints_over_suffixes() {
        let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::mint = mint, mint::authority = user)]
    pub token: Account<'info, TokenAccount>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let linked_text = document_links(&document)
            .iter()
            .filter_map(link_key)
            .collect::<Vec<_>>();

        assert!(linked_text.contains(&"token::mint".to_string()));
        assert!(linked_text.contains(&"mint::authority".to_string()));
        assert!(!linked_text.contains(&"mint".to_string()));
        assert!(!linked_text.contains(&"authority".to_string()));
    }

    fn link_key(link: &DocumentLink) -> Option<String> {
        link.data
            .as_ref()?
            .get("anchor")?
            .get("key")?
            .as_str()
            .map(str::to_string)
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
}
