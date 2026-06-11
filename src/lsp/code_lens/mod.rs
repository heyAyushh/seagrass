use {
    crate::{
        anchor::space::{self, SpaceEstimate},
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{CodeLens, Command, Url},
};

const ANALYZE_COMMAND: &str = "seagrass/analyze";

pub fn code_lenses(
    document: &ParsedDocument,
    uri: &Url,
    workspace: &WorkspaceIndex,
) -> Vec<CodeLens> {
    let account_context_lenses =
        document
            .symbols()
            .accounts_structs
            .values()
            .filter_map(|accounts| {
                let count = instruction_count(document, workspace, &accounts.name);
                (count > 0).then(|| CodeLens {
                    range: accounts.selection_range,
                    command: Some(Command {
                        title: instruction_count_title(count),
                        command: ANALYZE_COMMAND.to_string(),
                        arguments: Some(vec![serde_json::json!({
                            "uri": uri,
                            "context": accounts.name,
                        })]),
                    }),
                    data: Some(serde_json::json!({
                        "kind": "anchor.accountsContext",
                        "context": accounts.name,
                        "instructionCount": count,
                    })),
                })
            });

    let instruction_context_lenses =
        document
            .symbols()
            .instructions
            .iter()
            .filter_map(|instruction| {
                let context = instruction.context.as_ref()?;
                Some(CodeLens {
                    range: instruction.selection_range,
                    command: Some(Command {
                        title: instruction_context_title(&context.name),
                        command: ANALYZE_COMMAND.to_string(),
                        arguments: Some(vec![serde_json::json!({
                            "uri": uri,
                            "instruction": instruction.name,
                        })]),
                    }),
                    data: Some(serde_json::json!({
                        "kind": "anchor.programInstruction",
                        "instruction": instruction.name,
                        "context": context.name,
                    })),
                })
            });

    let account_space_lenses =
        document
            .symbols()
            .account_data_structs
            .values()
            .filter_map(|account| {
                let estimate =
                    space::account_space_with_document(account, document, Some(workspace));
                let title = account_space_title(&estimate)?;
                let total_bytes = match estimate {
                    SpaceEstimate::Exact(data_bytes) => Some(8u64.saturating_add(data_bytes)),
                    SpaceEstimate::Formula { .. } | SpaceEstimate::Unknown(_) => None,
                };
                let mut data = serde_json::json!({
                    "kind": "anchor.accountSpace",
                    "accountType": account.name,
                });
                if let (serde_json::Value::Object(map), Some(bytes)) = (&mut data, total_bytes) {
                    map.insert("bytes".to_string(), serde_json::json!(bytes));
                }

                Some(CodeLens {
                    range: account.selection_range,
                    command: Some(Command {
                        title,
                        command: ANALYZE_COMMAND.to_string(),
                        arguments: Some(vec![serde_json::json!({
                            "uri": uri,
                            "accountType": account.name,
                        })]),
                    }),
                    data: Some(data),
                })
            });

    account_context_lenses
        .chain(instruction_context_lenses)
        .chain(account_space_lenses)
        .collect()
}

fn instruction_count(
    document: &ParsedDocument,
    workspace: &WorkspaceIndex,
    context: &str,
) -> usize {
    let local_count = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|reference| reference.name == context)
        })
        .count();

    if local_count > 0 {
        return local_count;
    }

    workspace
        .implementation_locations_for_context(context)
        .len()
}

fn instruction_count_title(count: usize) -> String {
    match count {
        1 => "1 Anchor instruction".to_string(),
        count => format!("{count} Anchor instructions"),
    }
}

fn instruction_context_title(context: &str) -> String {
    format!("Context<{context}>")
}

fn account_space_title(estimate: &SpaceEstimate) -> Option<String> {
    match estimate {
        SpaceEstimate::Exact(data_bytes) => {
            let total = 8u64.saturating_add(*data_bytes);
            Some(format!("space: 8 + {data_bytes} = {total} bytes"))
        }
        SpaceEstimate::Formula { fixed, symbolic } => {
            let data_expr = space::formula_expr(*fixed, symbolic);
            Some(format!("space: 8 + {data_expr} bytes (set max_len)"))
        }
        SpaceEstimate::Unknown(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    };

    #[test]
    fn accounts_context_lens_reports_program_instruction_count() {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }

    pub fn update(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let workspace = WorkspaceIndex::build(&[], [(uri.clone(), source.to_string())]);
        let lenses = code_lenses(&document, &uri, &workspace);

        let accounts_lens = lenses
            .iter()
            .find(|lens| lens.data.as_ref().unwrap()["kind"] == "anchor.accountsContext")
            .unwrap();
        assert_eq!(
            accounts_lens
                .command
                .as_ref()
                .map(|command| command.title.as_str()),
            Some("2 Anchor instructions")
        );
        assert_eq!(accounts_lens.data.as_ref().unwrap()["context"], "Create");
        assert_eq!(accounts_lens.data.as_ref().unwrap()["instructionCount"], 2);
        assert!(lenses.iter().any(|lens| {
            lens.command
                .as_ref()
                .is_some_and(|command| command.title == "Context<Create>")
                && lens.data.as_ref().unwrap()["kind"] == "anchor.programInstruction"
                && lens.data.as_ref().unwrap()["instruction"] == "initialize"
        }));
    }

    #[test]
    fn accounts_context_lens_stays_quiet_for_unused_context() {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let workspace = WorkspaceIndex::build(&[], [(uri.clone(), source.to_string())]);

        assert!(code_lenses(&document, &uri, &workspace).is_empty());
    }

    #[test]
    fn accounts_context_lens_prefers_local_instruction_count_over_workspace_name_collisions() {
        let local_uri = Url::parse("file:///tmp/local.rs").unwrap();
        let other_uri = Url::parse("file:///tmp/other.rs").unwrap();
        let local_source = r#"
#[program]
pub mod local {
    pub fn create(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
}
"#;
        let other_source = r#"
#[program]
pub mod other {
    pub fn create_one(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }

    pub fn create_two(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(local_source).unwrap();
        let workspace = WorkspaceIndex::build(
            &[],
            [
                (local_uri.clone(), local_source.to_string()),
                (other_uri, other_source.to_string()),
            ],
        );
        let lenses = code_lenses(&document, &local_uri, &workspace);

        let accounts_lens = lenses
            .iter()
            .find(|lens| lens.data.as_ref().unwrap()["kind"] == "anchor.accountsContext")
            .unwrap();
        assert_eq!(
            accounts_lens
                .command
                .as_ref()
                .map(|command| command.title.as_str()),
            Some("1 Anchor instruction")
        );
        assert_eq!(accounts_lens.data.as_ref().unwrap()["instructionCount"], 1);
    }

    #[test]
    fn program_instruction_lens_reports_context_type() {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let workspace = WorkspaceIndex::build(&[], [(uri.clone(), source.to_string())]);
        let lenses = code_lenses(&document, &uri, &workspace);

        assert_eq!(lenses.len(), 1);
        assert_eq!(
            lenses[0]
                .command
                .as_ref()
                .map(|command| command.title.as_str()),
            Some("Context<Create>")
        );
        assert_eq!(
            lenses[0].data.as_ref().unwrap()["kind"],
            "anchor.programInstruction"
        );
        assert_eq!(
            lenses[0].data.as_ref().unwrap()["instruction"],
            "initialize"
        );
        assert_eq!(lenses[0].data.as_ref().unwrap()["context"], "Create");
    }

    #[test]
    fn account_space_lens_reports_exact_account_data_size() {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let source = r#"
#[account]
pub struct Vault {
    pub authority: Pubkey,
    pub amount: u64,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let workspace = WorkspaceIndex::build(&[], [(uri.clone(), source.to_string())]);
        let lenses = code_lenses(&document, &uri, &workspace);

        assert_eq!(lenses.len(), 1);
        let lens = &lenses[0];
        assert_eq!(
            lens.command.as_ref().map(|command| command.title.as_str()),
            Some("space: 8 + 40 = 48 bytes")
        );
        assert_eq!(lens.data.as_ref().unwrap()["kind"], "anchor.accountSpace");
        assert_eq!(lens.data.as_ref().unwrap()["accountType"], "Vault");
        assert_eq!(lens.data.as_ref().unwrap()["bytes"], 48);
    }

    #[test]
    fn account_space_lens_ignores_derive_accounts_contexts() {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let workspace = WorkspaceIndex::build(&[], [(uri.clone(), source.to_string())]);

        assert!(code_lenses(&document, &uri, &workspace).is_empty());
    }

    #[test]
    fn account_space_lens_stays_quiet_for_unknown_size() {
        let uri = Url::parse("file:///tmp/lib.rs").unwrap();
        let source = r#"
#[account]
pub struct Vault {
    pub missing: Missing,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let workspace = WorkspaceIndex::build(&[], [(uri.clone(), source.to_string())]);

        assert!(code_lenses(&document, &uri, &workspace).is_empty());
    }
}
