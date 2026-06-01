use {
    super::*,
    crate::{
        constraint_catalog::{self, ConstraintValueKind},
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, Url},
};

mod catalog_tests;
mod core_tests;
mod instruction_arg_member_tests;
mod semantic_tests;

struct ReferenceCompletionScenario {
    prefix: &'static str,
    expected_label: &'static str,
}

fn reference_completion_scenario(
    key: &str,
    value_kind: ConstraintValueKind,
) -> ReferenceCompletionScenario {
    match value_kind {
        ConstraintValueKind::AccountReference => match key {
            "token::mint" | "associated_token::mint" => ReferenceCompletionScenario {
                prefix: "mi",
                expected_label: "mint",
            },
            _ => ReferenceCompletionScenario {
                prefix: "rec",
                expected_label: "recipient",
            },
        },
        ConstraintValueKind::SignerReference => ReferenceCompletionScenario {
            prefix: "auth",
            expected_label: "authority",
        },
        ConstraintValueKind::ProgramReference => {
            if key.ends_with("token_program") {
                ReferenceCompletionScenario {
                    prefix: "tok",
                    expected_label: "token_program",
                }
            } else if key == "seeds::program" {
                ReferenceCompletionScenario {
                    prefix: "meta",
                    expected_label: "metadata_program",
                }
            } else {
                ReferenceCompletionScenario {
                    prefix: "hook",
                    expected_label: "hook_program",
                }
            }
        }
        _ => unreachable!("only reference value kinds are passed here"),
    }
}

fn generated_value_completion_source(
    key: &str,
    value_prefix: &str,
    instruction_arguments: &str,
) -> String {
    format!(
        r#"
#[program]
pub mod demo {{
    pub fn initialize(ctx: Context<Create>, {instruction_arguments}) -> Result<()> {{
        Ok(())
    }}
}}

#[derive(Accounts)]
pub struct Create<'info> {{
    #[account({key} = {value_prefix})]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub authority: Signer<'info>,
    pub recipient: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub metadata_program: Program<'info, Metadata>,
    pub hook_program: Program<'info, HookProgram>,
}}

#[account]
pub struct State {{
    pub recipient: Pubkey,
}}
"#
    )
}

fn position_after(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle present") + needle.len();
    let prefix = &source[..offset];
    let line = u32::try_from(prefix.chars().filter(|ch| *ch == '\n').count()).unwrap();
    let line_start = prefix.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    Position {
        line,
        character: u32::try_from(prefix[line_start..].chars().count()).unwrap(),
    }
}

fn top_labels(items: &[CompletionItem], count: usize) -> Vec<&str> {
    items
        .iter()
        .take(count)
        .map(|item| item.label.as_str())
        .collect()
}

struct BrokenAttributeScenario {
    attribute: &'static str,
    cursor: &'static str,
    expected: &'static str,
}

fn broken_attribute_source(attribute: &str) -> String {
    format!(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account({attribute}
    pub state: Account<'info, State>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub vault: Account<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub user: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}}

#[account]
pub struct State {{
    pub authority: Pubkey,
}}
"#
    )
}
