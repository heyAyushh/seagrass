use super::*;

#[test]
fn parsed_document_captures_ctx_account_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.count += 1;
        let authority = ctx.accounts.authority.key();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "counter" && usage.mutable));
    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "authority" && !usage.mutable));
    assert!(increment
        .account_data_field_usages
        .iter()
        .any(|usage| { usage.account == "counter" && usage.field == "count" && usage.mutable }));
}

#[test]
fn parsed_document_captures_ctx_accounts_alias_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let accounts = &mut ctx.accounts;
        accounts.counter.count += 1;
        let authority = accounts.authority.key();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "counter" && usage.mutable));
    assert!(increment
        .account_usages
        .iter()
        .any(|usage| usage.name == "authority" && !usage.mutable));
    assert!(increment
        .account_data_field_usages
        .iter()
        .any(|usage| { usage.account == "counter" && usage.field == "count" && usage.mutable }));
    assert!(increment.account_path_usages.iter().any(|usage| {
        usage
            .segments
            .iter()
            .map(|segment| segment.name.as_str())
            .collect::<Vec<_>>()
            == ["counter", "count"]
    }));
}

#[test]
fn parsed_document_captures_account_field_alias_data_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.count += 1;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment.account_data_field_usages.iter().any(|usage| {
        usage.account == "counter"
            && usage.source_account == "counter"
            && usage.field == "count"
            && usage.mutable
    }));
}

#[test]
fn parsed_document_preserves_source_alias_for_account_data_usages() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let account = &mut ctx.accounts.counter;
        account.count += 1;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let increment = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "increment")
        .unwrap();

    assert!(increment.account_data_field_usages.iter().any(|usage| {
        usage.account == "counter"
            && usage.source_account == "account"
            && usage.field == "count"
            && usage.mutable
    }));
}

#[test]
fn parsed_document_captures_composite_account_alias_paths() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let wrapper = &ctx.accounts.wrapper;
        wrapper.inner.fake;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read")
        .unwrap();

    assert!(read.account_path_usages.iter().any(|usage| {
        usage
            .segments
            .iter()
            .map(|segment| segment.name.as_str())
            .collect::<Vec<_>>()
            == ["wrapper", "inner", "fake"]
    }));
}

#[test]
fn parsed_document_captures_nested_ctx_account_usage() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read_token(ctx: Context<ReadToken>) -> Result<()> {
        let data = ctx.accounts.vaultish.data.borrow();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read_token = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read_token")
        .unwrap();

    assert!(read_token
        .account_usages
        .iter()
        .any(|usage| usage.name == "vaultish"));
    assert!(read_token
        .account_data_field_usages
        .iter()
        .any(|usage| usage.account == "vaultish" && usage.field == "data"));
}

#[test]
fn parsed_document_captures_full_ctx_account_paths() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read_nested(ctx: Context<ReadNested>) -> Result<()> {
        let key = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read_nested = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read_nested")
        .unwrap();
    let usage = read_nested
        .account_path_usages
        .iter()
        .find(|usage| {
            usage
                .segments
                .iter()
                .map(|segment| segment.name.as_str())
                .eq(["wrapper", "inner"])
        })
        .expect("nested account path");

    assert!(!usage.mutable);
    assert_eq!(usage.segments[0].range.start.line, 4);
    assert_eq!(usage.segments[1].range.start.line, 4);
}

#[test]
fn parsed_document_captures_cpi_program_usage() {
    let source = r#"
use anchor_lang::solana_program::instruction::Instruction;

#[program]
pub mod demo {
    pub fn call_external(ctx: Context<CallExternal>) -> Result<()> {
        let ix = Instruction {
            program_id: ctx.accounts.metadata_program.key(),
            accounts: vec![],
            data: vec![],
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
        let signed_cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.associated_token_program.to_account_info(),
            (),
            signer_seeds,
        );
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let call_external = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "call_external")
        .unwrap();

    assert!(call_external
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == "metadata_program"));
    assert!(call_external
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == "token_program"));
    assert!(call_external
        .cpi_program_usages
        .iter()
        .any(|usage| usage.name == "associated_token_program"));
}

#[test]
fn parsed_document_captures_account_meta_signer_usage() {
    let source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[program]
pub mod demo {
    pub fn call_external(ctx: Context<CallExternal>) -> Result<()> {
        let metas = vec![
            AccountMeta::new(ctx.accounts.authority.key(), true),
            AccountMeta {
                pubkey: ctx.accounts.readonly_authority.key(),
                is_signer: true,
                is_writable: false,
            },
        ];
        require!(ctx.accounts.checked_authority.is_signer, ErrorCode::MissingSigner);
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let call_external = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "call_external")
        .unwrap();

    assert!(call_external
        .signer_usages
        .iter()
        .any(|usage| usage.name == "authority"));
    assert!(call_external
        .signer_usages
        .iter()
        .any(|usage| usage.name == "readonly_authority"));
    assert!(call_external
        .signer_checks
        .iter()
        .any(|usage| usage.name == "checked_authority"));
}

#[test]
fn parsed_document_captures_account_key_comparisons() {
    let source = r#"
#[program]
pub mod demo {
    pub fn update(ctx: Context<Update>) -> Result<()> {
        if ctx.accounts.user_a.key() == ctx.accounts.user_b.key() {
            return err!(ErrorCode::DuplicateAccount);
        }
        if ctx.accounts.metadata_program.key() != mpl_token_metadata::ID {
            return err!(ErrorCode::InvalidProgram);
        }
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let update = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "update")
        .unwrap();

    assert!(update
        .account_key_comparisons
        .iter()
        .any(|comparison| comparison.matches("user_a", "user_b")));
    assert!(update
        .account_key_comparisons
        .iter()
        .any(|comparison| comparison.compares_account_to_static_program_id("metadata_program")));
    assert!(update.account_usages.iter().all(|usage| !usage.mutable));
}

#[test]
fn parsed_document_captures_token_account_unpack_usage() {
    let source = r#"
use spl_token::state::Account as SplTokenAccount;

#[program]
pub mod demo {
    pub fn read_token(ctx: Context<ReadToken>) -> Result<()> {
        let token = SplTokenAccount::unpack(&ctx.accounts.vaultish.data.borrow())?;
        Ok(())
    }
}
"#;

    let document = ParsedDocument::parse(source).unwrap();
    let read_token = document
        .symbols()
        .instructions
        .iter()
        .find(|instruction| instruction.name == "read_token")
        .unwrap();

    assert!(read_token
        .token_account_unpack_usages
        .iter()
        .any(|usage| usage.name == "vaultish"));
}
