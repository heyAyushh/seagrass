use super::*;

pub(super) fn analysis_report(
    uri: &Url,
    document: &ParsedDocument,
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
    project: Option<serde_json::Value>,
    focus_instruction: Option<&str>,
    focus_context: Option<&str>,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> serde_json::Value {
    serde_json::json!({
        "uri": uri,
        "anchorSupport": anchor_support::summary(),
        "project": project,
        "focus": focused_analysis(document, diagnostics, focus_instruction, focus_context, workspace_index),
        "evidence": evidence::summary(document),
        "documentSymbols": document::document_symbols(document),
        "diagnostics": diagnostics.iter().map(diagnostic_summary).collect::<Vec<_>>(),
    })
}

pub(super) fn instruction_summary(
    document: &ParsedDocument,
    _diagnostics: &[tower_lsp::lsp_types::Diagnostic],
    instruction_name: &str,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> serde_json::Value {
    let Some(instruction) = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == instruction_name)
    else {
        return serde_json::json!({
            "name": instruction_name,
            "found": false,
        });
    };

    let context_name = instruction
        .context
        .as_ref()
        .map(|context| context.name.as_str());
    serde_json::json!({
        "name": instruction.name,
        "found": true,
        "context": context_name,
        "accounts": instruction_accounts_summary(document, context_name, workspace_index),
        "args": instruction.arguments.iter().map(|argument| serde_json::json!({
            "name": argument.name,
            "ty": argument.type_name,
            "range": argument.range,
        })).collect::<Vec<_>>(),
        "mutates": instruction_account_names(instruction.account_usages.iter().filter(|usage| usage.mutable)),
        "cpisCalled": instruction_account_names(instruction.cpi_program_usages.iter()),
        "errorsReturned": Vec::<String>::new(),
    })
}

pub(super) fn instruction_accounts_summary(
    document: &ParsedDocument,
    context_name: Option<&str>,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> Vec<serde_json::Value> {
    let Some(context_name) = context_name else {
        return Vec::new();
    };
    if let Some(accounts) = document.symbols().accounts_structs.get(context_name) {
        return accounts
            .fields
            .iter()
            .map(agent_account_field_summary)
            .collect();
    }
    workspace_index
        .map(|workspace_index| {
            workspace_index
                .account_context_fields(context_name)
                .into_iter()
                .map(|field| {
                    serde_json::json!({
                        "name": field.name,
                        "ty": field.type_display,
                        "constraints": Vec::<String>::new(),
                        "mutability": false,
                        "signer": false,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn agent_account_field_summary(field: &document::SymbolRange) -> serde_json::Value {
    serde_json::json!({
        "name": field.name,
        "ty": field.type_name,
        "constraints": field.account_constraints.iter().map(|constraint| constraint.text.as_str()).collect::<Vec<_>>(),
        "mutability": account_field_has_constraint(field, "mut"),
        "signer": field.type_name.as_deref() == Some("Signer")
            || account_field_has_constraint(field, "signer"),
    })
}

pub(super) fn account_field_has_constraint(field: &document::SymbolRange, key: &str) -> bool {
    field
        .account_constraints
        .iter()
        .any(|constraint| account_constraint_starts_with_key(&constraint.text, key))
}

pub(super) fn account_constraint_starts_with_key(text: &str, key: &str) -> bool {
    let trimmed = text.trim_start();
    let inner = trimmed
        .strip_prefix("account(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(trimmed);
    inner.split(',').any(|part| {
        part.trim_start().strip_prefix(key).is_some_and(|tail| {
            tail.is_empty()
                || tail
                    .chars()
                    .next()
                    .is_some_and(|ch| matches!(ch, '=' | ' ' | '\t'))
        })
    })
}

pub(super) fn instruction_account_names<'a>(
    usages: impl Iterator<Item = &'a document::AccountUsage>,
) -> Vec<String> {
    let names = usages
        .map(|usage| usage.name.clone())
        .collect::<BTreeSet<_>>();
    names.iter().cloned().collect()
}

pub(super) fn open_document_instruction_summaries(
    uri: &Url,
    document: &ParsedDocument,
) -> Vec<serde_json::Value> {
    document
        .symbols()
        .instructions
        .iter()
        .map(|instruction| {
            serde_json::json!({
                "uri": uri,
                "name": instruction.name,
                "context": instruction.context.as_ref().map(|context| context.name.as_str()),
                "range": instruction.range,
            })
        })
        .collect()
}

pub(super) fn open_document_pda_summaries(
    uri: &Url,
    document: &ParsedDocument,
) -> Vec<serde_json::Value> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| {
            accounts.fields.iter().filter_map(|field| {
                field.pda_constraint.as_ref().map(|pda| {
                    serde_json::json!({
                        "uri": uri,
                        "accounts": accounts.name,
                        "field": field.name,
                        "pda": pda_constraint_summary(pda),
                    })
                })
            })
        })
        .collect()
}

pub(super) fn solana_project_summary(
    uri: &Url,
    document: &ParsedDocument,
) -> Option<serde_json::Value> {
    let program = solana_project::detect_for_document(uri, document)?;
    let artifacts =
        program_artifacts::report_for_program(program.clone()).map(|report| report.to_json());
    let ecosystem = ecosystem::report_for_program(&program).to_json();
    Some(serde_json::json!({
        "anchorToml": serde_json::Value::Null,
        "framework": program.kind.label(),
        "programKind": program.kind.as_str(),
        "programName": program.name.as_str(),
        "programId": program.id.as_deref(),
        "root": program.root.to_string_lossy().into_owned(),
        "manifest": program.metadata_uri,
        "artifacts": artifacts,
        "ecosystem": ecosystem,
        "declareId": document
            .symbols()
            .declared_program_id
            .as_ref()
            .map(|declared| declared.value.as_str()),
    }))
}

pub(super) fn focused_analysis(
    document: &ParsedDocument,
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
    focus_instruction: Option<&str>,
    focus_context: Option<&str>,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> serde_json::Value {
    if focus_instruction.is_some() {
        return focused_instruction_analysis(
            document,
            diagnostics,
            focus_instruction,
            workspace_index,
        );
    }
    focused_accounts_context_analysis(document, diagnostics, focus_context, workspace_index)
}

pub(super) fn focused_instruction_analysis(
    document: &ParsedDocument,
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
    focus_instruction: Option<&str>,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> serde_json::Value {
    let Some(instruction_name) = focus_instruction else {
        return serde_json::Value::Null;
    };
    let Some(instruction) = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == instruction_name)
    else {
        return serde_json::json!({
            "kind": "anchor.programInstruction",
            "instruction": instruction_name,
            "found": false,
        });
    };

    serde_json::json!({
        "kind": "anchor.programInstruction",
        "found": true,
        "instruction": instruction.name,
        "range": instruction.range,
        "selectionRange": instruction.selection_range,
        "context": instruction.context.as_ref().map(|context| serde_json::json!({
            "name": context.name,
            "range": context.range,
        })),
        "contextFields": focused_instruction_context_fields(document, instruction, workspace_index),
        "arguments": instruction.arguments.iter().map(|argument| serde_json::json!({
            "name": argument.name,
            "type": argument.type_name,
            "range": argument.range,
        })).collect::<Vec<_>>(),
        "accountUsages": instruction.account_usages.iter().map(account_usage_summary).collect::<Vec<_>>(),
        "resolvedAccountUsages": focused_instruction_resolved_account_usages(document, instruction, workspace_index),
        "accountDataFieldUsages": instruction.account_data_field_usages.iter().map(|usage| serde_json::json!({
            "account": usage.account,
            "field": usage.field,
            "mutable": usage.mutable,
            "range": usage.range,
        })).collect::<Vec<_>>(),
        "accountPathUsages": instruction.account_path_usages.iter().map(|usage| serde_json::json!({
            "path": usage.segments.iter().map(|segment| segment.name.as_str()).collect::<Vec<_>>().join("."),
            "mutable": usage.mutable,
            "segments": usage.segments.iter().map(|segment| serde_json::json!({
                "name": segment.name,
                "range": segment.range,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "resolvedAccountPathUsages": focused_instruction_resolved_account_path_usages(document, instruction, workspace_index),
        "cpiProgramUsages": instruction.cpi_program_usages.iter().map(account_usage_summary).collect::<Vec<_>>(),
        "signerUsages": instruction.signer_usages.iter().map(account_usage_summary).collect::<Vec<_>>(),
        "signerChecks": instruction.signer_checks.iter().map(account_usage_summary).collect::<Vec<_>>(),
        "functionCalls": instruction.function_calls.iter().map(|call| serde_json::json!({
            "name": call.name,
            "range": call.range,
        })).collect::<Vec<_>>(),
        "diagnostics": diagnostics
            .iter()
            .filter(|diagnostic| ranges_touch(instruction.range, diagnostic.range))
            .map(diagnostic_summary)
            .collect::<Vec<_>>(),
    })
}

pub(super) fn focused_instruction_resolved_account_path_usages(
    document: &ParsedDocument,
    instruction: &document::InstructionSymbol,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> Vec<serde_json::Value> {
    let Some(context_name) = instruction
        .context
        .as_ref()
        .map(|context| context.name.as_str())
    else {
        return Vec::new();
    };

    instruction
        .account_path_usages
        .iter()
        .map(|usage| {
            let segment_names = usage
                .segments
                .iter()
                .map(|segment| segment.name.clone())
                .collect::<Vec<_>>();
            serde_json::json!({
                "path": segment_names.join("."),
                "mutable": usage.mutable,
                "segments": usage
                    .segments
                    .iter()
                    .enumerate()
                    .map(|(segment_index, segment)| {
                        resolved_account_path_segment_summary(
                            document,
                            workspace_index,
                            context_name,
                            &segment_names,
                            segment_index,
                            segment.range,
                        )
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect()
}

pub(super) fn resolved_account_path_segment_summary(
    document: &ParsedDocument,
    workspace_index: Option<&workspace::WorkspaceIndex>,
    context_name: &str,
    segments: &[String],
    segment_index: usize,
    usage_range: tower_lsp::lsp_types::Range,
) -> serde_json::Value {
    let Some(name) = segments.get(segment_index) else {
        return serde_json::Value::Null;
    };
    let path = navigation::AccountPathPosition {
        context: context_name.to_string(),
        segments: segments.to_vec(),
        segment_index,
        field: name.clone(),
    };

    if let Some(target) =
        navigation::account_field_path_definition_target_for_position(document, &path)
    {
        if let Some(field) = document
            .symbols()
            .accounts_structs
            .get(&target.container)
            .and_then(|accounts| {
                accounts
                    .fields
                    .iter()
                    .find(|field| field.name == target.field)
            })
        {
            return serde_json::json!({
                "name": name,
                "usageRange": usage_range,
                "declared": true,
                "container": target.container,
                "declarationRange": field.selection_range,
                "type": field.type_name,
                "source": "localDocument",
            });
        }
    }

    if let Some(resolved) = workspace_index.and_then(|workspace_index| {
        workspace_index.resolve_account_field_path(context_name, segments, segment_index)
    }) {
        return serde_json::json!({
            "name": name,
            "usageRange": usage_range,
            "declared": true,
            "container": resolved.container_name,
            "declarationUri": resolved.field_info.location.uri,
            "declarationRange": resolved.field_info.location.range,
            "type": resolved.field_info.type_display,
            "source": "workspaceIndex",
        });
    }

    serde_json::json!({
        "name": name,
        "usageRange": usage_range,
        "declared": false,
    })
}

pub(super) fn focused_instruction_resolved_account_usages(
    document: &ParsedDocument,
    instruction: &document::InstructionSymbol,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> Vec<serde_json::Value> {
    let Some(context_name) = instruction
        .context
        .as_ref()
        .map(|context| context.name.as_str())
    else {
        return Vec::new();
    };

    let mut usages = instruction
        .account_usages
        .iter()
        .map(|usage| (usage.name.as_str(), usage.mutable, usage.range))
        .collect::<Vec<_>>();

    for path_usage in &instruction.account_path_usages {
        let Some(first_segment) = path_usage.segments.first() else {
            continue;
        };
        if usages
            .iter()
            .any(|(name, _, range)| *name == first_segment.name && *range == first_segment.range)
        {
            continue;
        }
        usages.push((
            first_segment.name.as_str(),
            path_usage.mutable,
            first_segment.range,
        ));
    }

    usages
        .into_iter()
        .map(|(name, mutable, range)| {
            resolved_account_usage_summary(
                document,
                workspace_index,
                context_name,
                name,
                mutable,
                range,
            )
        })
        .collect()
}

pub(super) fn resolved_account_usage_summary(
    document: &ParsedDocument,
    workspace_index: Option<&workspace::WorkspaceIndex>,
    context_name: &str,
    name: &str,
    mutable: bool,
    range: tower_lsp::lsp_types::Range,
) -> serde_json::Value {
    let local_field = document
        .symbols()
        .accounts_structs
        .get(context_name)
        .and_then(|accounts| accounts.fields.iter().find(|field| field.name == name));
    if let Some(field) = local_field {
        return serde_json::json!({
            "name": name,
            "mutable": mutable,
            "usageRange": range,
            "declared": true,
            "declarationRange": field.selection_range,
            "type": field.type_name,
            "source": "localDocument",
        });
    }

    if let Some(field_info) = workspace_index
        .and_then(|workspace_index| workspace_index.field_info_in_container(name, context_name))
    {
        return serde_json::json!({
            "name": name,
            "mutable": mutable,
            "usageRange": range,
            "declared": true,
            "declarationUri": field_info.location.uri,
            "declarationRange": field_info.location.range,
            "type": field_info.type_display,
            "source": "workspaceIndex",
        });
    }

    serde_json::json!({
        "name": name,
        "mutable": mutable,
        "usageRange": range,
        "declared": false,
    })
}

pub(super) fn focused_instruction_context_fields(
    document: &ParsedDocument,
    instruction: &document::InstructionSymbol,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> Vec<serde_json::Value> {
    let Some(context_name) = instruction
        .context
        .as_ref()
        .map(|context| context.name.as_str())
    else {
        return Vec::new();
    };

    if let Some(accounts) = document.symbols().accounts_structs.get(context_name) {
        return accounts
            .fields
            .iter()
            .map(|field| {
                let mut value = account_field_summary(accounts, field);
                if let Some(object) = value.as_object_mut() {
                    object.insert("source".to_string(), serde_json::json!("localDocument"));
                }
                value
            })
            .collect();
    }

    workspace_index
        .map(|workspace_index| {
            workspace_index
                .account_context_fields(context_name)
                .into_iter()
                .map(|field| {
                    serde_json::json!({
                        "name": field.name,
                        "type": field.type_display,
                        "source": "workspaceIndex",
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn focused_accounts_context_analysis(
    document: &ParsedDocument,
    diagnostics: &[tower_lsp::lsp_types::Diagnostic],
    focus_context: Option<&str>,
    workspace_index: Option<&workspace::WorkspaceIndex>,
) -> serde_json::Value {
    let Some(context_name) = focus_context else {
        return serde_json::Value::Null;
    };
    let Some(accounts) = document.symbols().accounts_structs.get(context_name) else {
        return serde_json::json!({
            "kind": "anchor.accountsContext",
            "context": context_name,
            "found": false,
        });
    };
    let mut instructions = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == context_name)
        })
        .map(|instruction| {
            serde_json::json!({
                "name": instruction.name,
                "range": instruction.range,
                "selectionRange": instruction.selection_range,
            })
        })
        .collect::<Vec<_>>();
    if instructions.is_empty() {
        if let Some(workspace_index) = workspace_index {
            instructions = workspace_index
                .program_instructions_for_context(context_name)
                .into_iter()
                .map(|instruction| {
                    serde_json::json!({
                        "name": instruction.name,
                        "uri": instruction.location.uri,
                        "range": instruction.location.range,
                        "selectionRange": instruction.location.range,
                        "source": if instruction.is_open { "openDocument" } else { "workspaceIndex" },
                    })
                })
                .collect();
        }
    }

    serde_json::json!({
        "kind": "anchor.accountsContext",
        "found": true,
        "context": accounts.name,
        "range": accounts.range,
        "selectionRange": accounts.selection_range,
        "deriveAccountsRange": accounts.derive_accounts_range,
        "instructionArguments": accounts.instruction_arguments.iter().map(|argument| serde_json::json!({
            "name": argument.name,
            "type": argument.type_name,
            "range": argument.range,
        })).collect::<Vec<_>>(),
        "instructions": instructions,
        "fields": accounts
            .fields
            .iter()
            .map(|field| account_field_summary(accounts, field))
            .collect::<Vec<_>>(),
        "diagnostics": diagnostics
            .iter()
            .filter(|diagnostic| {
                ranges_touch(accounts.range, diagnostic.range)
                    || accounts
                        .fields
                        .iter()
                        .any(|field| ranges_touch(field.range, diagnostic.range))
            })
            .map(diagnostic_summary)
            .collect::<Vec<_>>(),
    })
}

pub(super) fn symbol_range_summary(symbol: &document::SymbolRange) -> serde_json::Value {
    serde_json::json!({
        "name": symbol.name,
        "range": symbol.range,
        "selectionRange": symbol.selection_range,
        "type": symbol.type_name,
        "typeRange": symbol.type_range,
        "genericTypes": symbol.generic_type_names,
        "isOptional": symbol.is_optional,
        "constraints": symbol.account_constraints.iter().map(|constraint| serde_json::json!({
            "text": constraint.text,
            "range": constraint.range,
            "pda": constraint.pda.as_ref().map(pda_constraint_summary),
        })).collect::<Vec<_>>(),
    })
}

pub(super) fn account_field_summary(
    accounts: &document::SymbolRange,
    field: &document::SymbolRange,
) -> serde_json::Value {
    let mut value = symbol_range_summary(field);
    if let Some(expected) =
        account_semantics::expected_account_inner_type_for_document_field_in_accounts(
            accounts, field,
        )
    {
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "expectedInnerType".to_string(),
                serde_json::json!({
                    "generic": expected.generic,
                    "evidence": match expected.evidence {
                        account_semantics::AccountTypeEvidence::Constraint => "constraint",
                        account_semantics::AccountTypeEvidence::AccountReference => "accountReference",
                        account_semantics::AccountTypeEvidence::TokenAccountProperty => {
                            "tokenAccountProperty"
                        }
                    },
                }),
            );
        }
    }
    value
}

pub(super) fn pda_constraint_summary(pda: &document::PdaConstraint) -> serde_json::Value {
    serde_json::json!({
        "isInit": pda.is_init,
        "seeds": match &pda.seeds {
            document::PdaSeeds::List(seeds) => serde_json::json!({
                "kind": "list",
                "values": seeds,
            }),
            document::PdaSeeds::Expr(expr) => serde_json::json!({
                "kind": "expr",
                "value": expr,
            }),
        },
        "bump": match &pda.bump {
            document::PdaBump::Canonical => serde_json::json!({ "kind": "canonical" }),
            document::PdaBump::Explicit(expr) => serde_json::json!({
                "kind": "explicit",
                "value": expr,
            }),
            document::PdaBump::Missing => serde_json::json!({ "kind": "missing" }),
        },
        "programSeed": pda.program_seed,
    })
}

pub(super) fn account_usage_summary(usage: &document::AccountUsage) -> serde_json::Value {
    serde_json::json!({
        "name": usage.name,
        "mutable": usage.mutable,
        "range": usage.range,
    })
}
