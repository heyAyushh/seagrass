use {
    super::{
        account_set_touches_context, text::leading_seed_identifier, Assist, AssistApplicability,
        AssistContext, AssistId, AssistKind, AssistProvider, ADD_CANONICAL_SEEDS_STRUCT_ID,
        CANONICAL_SEEDS_REASON,
    },
    crate::{
        actions::common::{eof_range, single_document_edit},
        evidence::{
            AccountSetEvidence, EvidenceGraph, FieldEvidence, SeedExpressionEvidence,
            SeedExpressionKind,
        },
    },
    serde_json::json,
    std::collections::HashSet,
    tower_lsp::lsp_types::TextEdit,
};

pub(super) struct CanonicalSeedsProvider;

impl AssistProvider for CanonicalSeedsProvider {
    fn assists(&self, context: &AssistContext<'_>) -> Vec<Assist> {
        EvidenceGraph::from_document(context.document)
            .account_sets()
            .iter()
            .filter(|accounts| account_set_touches_context(context, accounts))
            .flat_map(|accounts| canonical_seed_assists(context, accounts))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalSeedField {
    name: String,
    type_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CanonicalSeedComponent {
    Literal(String),
    Field {
        field: CanonicalSeedField,
        expression: String,
    },
}

fn canonical_seed_assists(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
) -> Vec<Assist> {
    accounts
        .fields()
        .iter()
        .filter_map(|field| canonical_seed_assist(context, accounts, field))
        .collect()
}

fn canonical_seed_assist(
    context: &AssistContext<'_>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Option<Assist> {
    let helper_name = canonical_seed_helper_name(&field.field.name);
    if context.document.symbols().knows_type(&helper_name) {
        return None;
    }

    let components = canonical_seed_components(accounts, field)?;
    let helper_text = canonical_seed_helper_text(&helper_name, &components)?;
    let edit = TextEdit {
        range: eof_range(context.document.source()),
        new_text: top_level_append_text(context.document.source(), &helper_text),
    };

    Some(Assist {
        id: AssistId(ADD_CANONICAL_SEEDS_STRUCT_ID),
        title: "Add canonical PDA seeds helper".to_string(),
        kind: AssistKind::Refactor,
        applicability: AssistApplicability::MachineApplicable,
        range: field.field.range,
        edit: Some(single_document_edit(context.uri.clone(), edit)),
        data: json!({
            "accountsStruct": accounts.accounts.name,
            "field": field.field.name,
            "helper": helper_name,
            "seeds": components.iter().map(canonical_seed_component_summary).collect::<Vec<_>>(),
            "reason": CANONICAL_SEEDS_REASON,
        }),
    })
}

fn canonical_seed_components(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Option<Vec<CanonicalSeedComponent>> {
    field.constraints().iter().find_map(|constraint| {
        let seeds = field.seed_expressions(accounts, constraint);
        canonical_seed_components_from_seeds(&seeds)
    })
}

fn canonical_seed_components_from_seeds(
    seeds: &[SeedExpressionEvidence],
) -> Option<Vec<CanonicalSeedComponent>> {
    if seeds.is_empty() {
        return None;
    }

    let components = seeds
        .iter()
        .map(canonical_seed_component)
        .collect::<Option<Vec<_>>>()?;

    components
        .iter()
        .any(|component| matches!(component, CanonicalSeedComponent::Field { .. }))
        .then_some(components)
}

fn canonical_seed_component(seed: &SeedExpressionEvidence) -> Option<CanonicalSeedComponent> {
    match seed.kind {
        SeedExpressionKind::StaticBytes => {
            Some(CanonicalSeedComponent::Literal(seed.expression.clone()))
        }
        SeedExpressionKind::AccountKey => {
            direct_account_key_seed_name(&seed.expression).map(|name| {
                CanonicalSeedComponent::Field {
                    field: CanonicalSeedField {
                        name: name.to_string(),
                        type_name: "&'a Pubkey",
                    },
                    expression: format!("self.{name}.as_ref()"),
                }
            })
        }
        SeedExpressionKind::InstructionArgument => {
            instruction_argument_bytes_seed_name(&seed.expression).map(|name| {
                CanonicalSeedComponent::Field {
                    field: CanonicalSeedField {
                        name: name.to_string(),
                        type_name: "&'a str",
                    },
                    expression: format!("self.{name}.as_bytes()"),
                }
            })
        }
        SeedExpressionKind::Expression => None,
    }
}

fn canonical_seed_helper_text(
    helper_name: &str,
    components: &[CanonicalSeedComponent],
) -> Option<String> {
    let fields = canonical_seed_fields(components);
    if fields.is_empty() {
        return None;
    }

    let field_lines = fields
        .iter()
        .map(|field| format!("    pub {}: {},", field.name, field.type_name))
        .collect::<Vec<_>>()
        .join("\n");
    let seed_expressions = components
        .iter()
        .map(canonical_seed_expression)
        .collect::<Vec<_>>()
        .join(", ");
    let seed_count = components.len();

    Some(format!(
        "pub struct {helper_name}<'a> {{\n{field_lines}\n}}\n\nimpl<'a> {helper_name}<'a> {{\n    pub fn as_seeds(&self) -> [&[u8]; {seed_count}] {{\n        [{seed_expressions}]\n    }}\n}}\n"
    ))
}

fn canonical_seed_fields(components: &[CanonicalSeedComponent]) -> Vec<CanonicalSeedField> {
    let mut seen = HashSet::with_capacity(components.len());
    let mut fields = Vec::with_capacity(components.len());

    for component in components {
        let CanonicalSeedComponent::Field { field, .. } = component else {
            continue;
        };
        if seen.insert(field.name.clone()) {
            fields.push(field.clone());
        }
    }

    fields
}

fn canonical_seed_expression(component: &CanonicalSeedComponent) -> String {
    match component {
        CanonicalSeedComponent::Literal(expression) => expression.clone(),
        CanonicalSeedComponent::Field { expression, .. } => expression.clone(),
    }
}

fn canonical_seed_component_summary(component: &CanonicalSeedComponent) -> serde_json::Value {
    match component {
        CanonicalSeedComponent::Literal(expression) => json!({
            "kind": "literal",
            "expression": expression,
        }),
        CanonicalSeedComponent::Field { field, expression } => json!({
            "kind": "field",
            "name": field.name,
            "type": field.type_name,
            "expression": expression,
        }),
    }
}

fn direct_account_key_seed_name(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let name = leading_seed_identifier(expression)?;
    let rest = expression[name.len()..].trim_start();
    matches!(rest, ".key().as_ref()" | ".key().as_ref" | ".as_ref()").then_some(name)
}

fn instruction_argument_bytes_seed_name(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let name = leading_seed_identifier(expression)?;
    (expression[name.len()..].trim_start() == ".as_bytes()").then_some(name)
}

fn canonical_seed_helper_name(field_name: &str) -> String {
    format!("{}Seeds", pascal_case_identifier(field_name))
}

fn pascal_case_identifier(value: &str) -> String {
    let mut output = String::new();
    let mut uppercase_next = true;

    for ch in value.chars() {
        if !ch.is_ascii_alphanumeric() {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            output.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            output.push(ch);
        }
    }

    if output.is_empty() {
        "Pda".to_string()
    } else {
        output
    }
}

fn top_level_append_text(source: &str, text: &str) -> String {
    if source.ends_with('\n') {
        format!("\n{text}")
    } else {
        format!("\n\n{text}")
    }
}
