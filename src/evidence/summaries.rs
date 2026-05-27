use {
    super::{AccountSetEvidence, ConstraintEvidence, EvidenceGraph, FieldEvidence},
    crate::document::ParsedDocument,
};

pub fn summary(document: &ParsedDocument) -> serde_json::Value {
    let graph = EvidenceGraph::from_document(document);
    serde_json::json!({
        "declaredProgramId": document
            .symbols()
            .declared_program_id
            .as_ref()
            .map(|declared| declared.value.as_str()),
        "instructions": document.symbols().instructions.iter().map(|instruction| {
            serde_json::json!({
                "name": instruction.name,
                "context": instruction.context.as_ref().map(|context| context.name.as_str()),
                "arguments": instruction.arguments.iter().map(|argument| {
                    serde_json::json!({
                        "name": argument.name,
                        "type": argument.type_name,
                    })
                }).collect::<Vec<_>>(),
                "functionCalls": instruction.function_calls.iter().map(|call| call.name.as_str()).collect::<Vec<_>>(),
                "accountUsages": instruction.account_usages.iter().map(|usage| {
                    serde_json::json!({
                        "account": usage.name,
                        "mutable": usage.mutable,
                    })
                }).collect::<Vec<_>>(),
                "accountDataFieldUsages": instruction.account_data_field_usages.iter().map(|usage| {
                    serde_json::json!({
                        "account": usage.account,
                        "field": usage.field,
                        "mutable": usage.mutable,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "functions": document.symbols().functions.iter().map(|function| {
            serde_json::json!({
                "name": function.name,
                "context": function.context.as_ref().map(|context| context.name.as_str()),
                "arguments": function.arguments.iter().map(|argument| {
                    serde_json::json!({
                        "name": argument.name,
                        "type": argument.type_name,
                    })
                }).collect::<Vec<_>>(),
                "functionCalls": function.function_calls.iter().map(|call| call.name.as_str()).collect::<Vec<_>>(),
                "accountUsages": function.account_usages.iter().map(|usage| {
                    serde_json::json!({
                        "account": usage.name,
                        "mutable": usage.mutable,
                    })
                }).collect::<Vec<_>>(),
                "accountDataFieldUsages": function.account_data_field_usages.iter().map(|usage| {
                    serde_json::json!({
                        "account": usage.account,
                        "field": usage.field,
                        "mutable": usage.mutable,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "accountDataStructs": document
            .symbols()
            .account_data_structs
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        "syntaxOverlay": document
            .tree_sitter()
            .map(|syntax| {
                syntax
                    .anchor_query_captures(document.source())
                    .into_iter()
                    .map(|capture| {
                        serde_json::json!({
                            "kind": format!("{:?}", capture.kind),
                            "name": capture.name,
                            "range": capture.range,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        "accounts": graph.account_sets().iter().map(account_set_summary).collect::<Vec<_>>(),
    })
}

fn account_set_summary(accounts: &AccountSetEvidence<'_>) -> serde_json::Value {
    serde_json::json!({
        "name": accounts.accounts.name,
        "usedByInstructions": accounts
            .instructions
            .iter()
            .map(|instruction| instruction.name.as_str())
            .collect::<Vec<_>>(),
        "hasPayerCandidate": accounts.has_payer_candidate(),
        "hasSystemProgram": accounts.has_system_program(),
        "fields": accounts.fields().iter().map(|field| field_summary(accounts, field)).collect::<Vec<_>>(),
    })
}

fn field_summary(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> serde_json::Value {
    serde_json::json!({
        "name": field.field.name,
        "type": field.field.type_name,
        "genericTypes": field.field.generic_type_names,
        "optional": field.field.is_optional,
        "isUncheckedAccount": field.is_unchecked_account(),
        "usedByInstructions": field.used_by_instructions().iter().map(|usage| {
            serde_json::json!({
                "instruction": usage.instruction,
                "mutable": usage.mutable,
            })
        }).collect::<Vec<_>>(),
        "constraints": field.constraints().iter().map(|constraint| constraint_summary(accounts, field, constraint)).collect::<Vec<_>>(),
    })
}

fn constraint_summary(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> serde_json::Value {
    let pda = field.pda().or_else(|| constraint.pda());
    serde_json::json!({
        "text": constraint.text(),
        "accountReferences": constraint.account_references().iter().map(|reference| {
            serde_json::json!({
                "constraint": reference.key,
                "account": reference.name,
                "valueKind": format!("{:?}", reference.value_kind),
            })
        }).collect::<Vec<_>>(),
        "instructionArgumentReferences": constraint.instruction_argument_references().iter().map(|reference| {
            serde_json::json!({
                "constraint": reference.key,
                "argument": reference.name,
            })
        }).collect::<Vec<_>>(),
        "usesSeeds": constraint.has_key("seeds"),
        "usesBump": constraint.has_flag_or_key("bump"),
        "pdaParserBacked": field.pda().is_some(),
        "staticOnlySeeds": field.seeds_are_static_only(),
        "bump": pda.map(|pda| format!("{:?}", pda.bump)),
        "seedsProgram": pda.and_then(|pda| pda.program_seed.as_deref()),
        "seeds": field.seed_expressions(accounts, constraint).iter().map(|seed| {
            serde_json::json!({
                "expression": seed.expression,
                "kind": format!("{:?}", seed.kind),
            })
        }).collect::<Vec<_>>(),
    })
}
