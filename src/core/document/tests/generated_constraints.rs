use {
    crate::{
        constraint_catalog,
        document::{document_symbols, ParsedDocument},
    },
    tower_lsp::lsp_types::SymbolKind,
};

#[test]
fn document_symbols_include_every_generated_account_constraint_child() {
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
    let symbols = document_symbols(&document);
    let create = symbols
        .iter()
        .find(|symbol| symbol.name == "Create")
        .unwrap();
    let state = create
        .children
        .as_ref()
        .unwrap()
        .iter()
        .find(|symbol| symbol.name == "state")
        .unwrap();
    let constraints = state.children.as_ref().unwrap();

    for spec in constraint_catalog::CONSTRAINTS {
        let key = constraint_catalog::key(spec.label);
        let expected_occurrences = constraint_catalog::CONSTRAINTS
            .iter()
            .filter(|candidate| constraint_catalog::key(candidate.label) == key)
            .count();
        let actual_occurrences = constraints
            .iter()
            .filter(|symbol| symbol.name == key && symbol.kind == SymbolKind::PROPERTY)
            .count();

        assert!(
            actual_occurrences >= expected_occurrences,
            "missing document symbol child for generated constraint `{key}`: {constraints:?}"
        );
        assert!(
            constraints.iter().any(|symbol| {
                symbol.name == key
                    && symbol.detail.as_deref() == Some(&format!("{:?} constraint", spec.family))
            }),
            "missing document symbol metadata for generated constraint `{key}`: {constraints:?}"
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
