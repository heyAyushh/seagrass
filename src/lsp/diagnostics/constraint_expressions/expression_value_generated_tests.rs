use {super::collect, crate::document::ParsedDocument, proptest::prelude::*};

prop_compose! {
    fn generated_ident()(tail in "[a-z0-9_]{1,10}") -> String {
        format!("sg_{tail}")
    }
}

proptest! {
    #[test]
    fn reports_generated_unresolved_constraint_identifiers(
        account_field in generated_ident(),
        missing_ident in generated_ident(),
    ) {
        prop_assume!(account_field != missing_ident);
        let source = format!(
            r#"
#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(constraint = {missing_ident})]
    pub {account_field}: UncheckedAccount<'info>,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();
        let diagnostics = collect(&document);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains(&format!("`{missing_ident}` does not resolve"))),
            "expected unresolved identifier diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn accepts_generated_account_and_instruction_constraint_identifiers(
        account_field in generated_ident(),
        instruction_arg in generated_ident(),
    ) {
        prop_assume!(account_field != instruction_arg);
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>, {instruction_arg}: bool) -> Result<()> {{ Ok(()) }}
}}

#[derive(Accounts)]
#[instruction({instruction_arg}: bool)]
pub struct Run<'info> {{
    #[account(constraint = {instruction_arg})]
    pub {account_field}: UncheckedAccount<'info>,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();
        let diagnostics = collect(&document);

        prop_assert!(
            diagnostics.iter().all(|diagnostic| {
                !diagnostic
                    .message
                    .contains(&format!("`{instruction_arg}` does not resolve"))
                    && !diagnostic
                        .message
                        .contains(&format!("`{account_field}` does not resolve"))
            }),
            "bound account and instruction identifiers should resolve, got {diagnostics:#?}"
        );
    }

    #[test]
    fn reports_generated_unknown_account_data_members(
        account_field in generated_ident(),
        known_member in generated_ident(),
        missing_member in generated_ident(),
    ) {
        prop_assume!(account_field != known_member && account_field != missing_member);
        prop_assume!(known_member != missing_member);
        let source = format!(
            r#"
#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(constraint = {account_field}.{missing_member} == Pubkey::default())]
    pub {account_field}: Account<'info, AccountData>,
}}

#[account]
pub struct AccountData {{
    pub {known_member}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();
        let diagnostics = collect(&document);

        prop_assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains(&format!(
                "`{account_field}.{missing_member}` does not resolve"
            ))),
            "expected unknown member diagnostic, got {diagnostics:#?}"
        );
    }

    #[test]
    fn accepts_generated_known_account_data_members(
        account_field in generated_ident(),
        known_member in generated_ident(),
    ) {
        prop_assume!(account_field != known_member);
        let source = format!(
            r#"
#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(constraint = {account_field}.{known_member} == Pubkey::default())]
    pub {account_field}: Account<'info, AccountData>,
}}

#[account]
pub struct AccountData {{
    pub {known_member}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();
        let diagnostics = collect(&document);

        prop_assert!(
            diagnostics.iter().all(|diagnostic| !diagnostic.message.contains(&format!(
                "`{account_field}.{known_member}` does not resolve"
            ))),
            "known account-data member should resolve, got {diagnostics:#?}"
        );
    }
}
