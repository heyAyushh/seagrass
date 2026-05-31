use tower_lsp::lsp_types::Position;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LocalAccountAliasPath {
    pub(super) account_field: String,
    pub(super) member_chain: Vec<String>,
    pub(super) member_prefix: String,
}

pub(super) fn local_account_alias_path_at(
    source: &str,
    position: Position,
) -> Option<LocalAccountAliasPath> {
    let offset = crate::range::byte_offset_at(source, position)?;
    let line = crate::range::line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let expression = dotted_expression_tail(&line[..cursor])?;
    let (receiver_expression, member_prefix) = expression.rsplit_once('.')?;
    let mut receiver_segments = receiver_expression.split('.');
    let alias = receiver_segments.next()?;
    if !is_identifier(alias) {
        return None;
    }
    let account_field = account_field_alias_target(source, offset, alias)?;
    let member_chain = receiver_segments.map(str::to_string).collect::<Vec<_>>();
    if !member_chain.iter().all(|segment| is_identifier(segment)) {
        return None;
    }

    Some(LocalAccountAliasPath {
        account_field,
        member_chain,
        member_prefix: member_prefix.to_string(),
    })
}

fn dotted_expression_tail(prefix: &str) -> Option<&str> {
    let tail = prefix
        .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        .next()
        .unwrap_or_default();
    tail.contains('.').then_some(tail)
}

fn account_field_alias_target(source: &str, offset: usize, alias: &str) -> Option<String> {
    let before_cursor = &source[..offset.min(source.len())];
    let accounts_aliases = accounts_aliases_before_cursor(before_cursor);
    before_cursor.lines().rev().find_map(|line| {
        let (candidate, rhs) = local_assignment(line)?;
        (candidate == alias)
            .then(|| account_field_from_assignment_rhs(rhs, &accounts_aliases))
            .flatten()
    })
}

fn accounts_aliases_before_cursor(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let (alias, rhs) = local_assignment(line)?;
            is_accounts_container_assignment(rhs).then_some(alias.to_string())
        })
        .collect()
}

fn local_assignment(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("let ")?;
    let (left, right) = rest.split_once('=')?;
    let left = left.trim().strip_prefix("mut ").unwrap_or(left.trim());
    let alias = left.split_once(':').map_or(left, |(name, _)| name).trim();
    is_identifier(alias).then_some((alias, right.trim().trim_end_matches(';').trim()))
}

fn account_field_from_assignment_rhs(rhs: &str, accounts_aliases: &[String]) -> Option<String> {
    direct_account_field_from_rhs(rhs).or_else(|| {
        accounts_aliases
            .iter()
            .find_map(|alias| alias_account_field_from_rhs(rhs, alias))
    })
}

fn direct_account_field_from_rhs(rhs: &str) -> Option<String> {
    let accounts_start = rhs.find(".accounts.")? + ".accounts.".len();
    let (account, rest) = identifier_at_start(&rhs[accounts_start..])?;
    is_direct_account_alias_remainder(rest).then(|| account.to_string())
}

fn alias_account_field_from_rhs(rhs: &str, accounts_alias: &str) -> Option<String> {
    let rhs = rhs
        .trim_start_matches('&')
        .trim_start()
        .strip_prefix("mut ")
        .unwrap_or_else(|| rhs.trim_start_matches('&').trim_start());
    let tail = rhs.strip_prefix(accounts_alias)?.strip_prefix('.')?;
    let (account, rest) = identifier_at_start(tail)?;
    is_direct_account_alias_remainder(rest).then(|| account.to_string())
}

fn is_accounts_container_assignment(rhs: &str) -> bool {
    let rhs = rhs.trim_start_matches('&').trim_start();
    let rhs = rhs.strip_prefix("mut ").unwrap_or(rhs).trim_start();
    rhs.ends_with(".accounts")
}

fn identifier_at_start(value: &str) -> Option<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!is_identifier_char(ch)).then_some(idx))
        .unwrap_or(value.len());
    let identifier = &value[..end];
    is_identifier(identifier).then_some((identifier, &value[end..]))
}

fn is_direct_account_alias_remainder(value: &str) -> bool {
    value.trim().is_empty()
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(is_identifier_char)
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use {
        super::*, crate::lsp::completions::proptest_support::rust_identifier, proptest::prelude::*,
    };

    fn position_after(source: &str, needle: &str) -> Position {
        let offset = source.find(needle).expect("needle") + needle.len();
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix
            .rsplit('\n')
            .next()
            .map(|line| line.chars().count())
            .unwrap_or_default() as u32;
        Position { line, character }
    }

    proptest! {
        #[test]
        fn resolves_direct_ctx_account_alias_variants(
            alias in rust_identifier(),
            account in rust_identifier(),
            member_prefix in "[a-z_]{0,8}",
            mutable in any::<bool>(),
            reference in any::<bool>(),
        ) {
            prop_assume!(alias != "ctx" && alias != account);
            let mutability = mutable.then_some("mut ").unwrap_or_default();
            let reference = reference.then_some("&").unwrap_or_default();
            let source = format!(
                "pub fn handler(ctx: Context<Run>) -> Result<()> {{\n    let {alias} = {reference}{mutability}ctx.accounts.{account};\n    {alias}.{member_prefix}\n}}\n"
            );
            let completion_line = format!("    {alias}.{member_prefix}");
            let path = local_account_alias_path_at(
                &source,
                position_after(&source, &completion_line),
            )
            .expect("alias member path should resolve");

            prop_assert_eq!(path.account_field, account);
            prop_assert!(path.member_chain.is_empty());
            prop_assert_eq!(path.member_prefix, member_prefix);
        }

        #[test]
        fn resolves_intermediate_accounts_alias_variants(
            accounts_alias in rust_identifier(),
            account_alias in rust_identifier(),
            account in rust_identifier(),
            member_prefix in "[a-z_]{0,8}",
        ) {
            prop_assume!(accounts_alias != account_alias);
            prop_assume!(accounts_alias != "ctx" && account_alias != "ctx");
            prop_assume!(account_alias != account);
            let source = format!(
                "pub fn handler(ctx: Context<Run>) -> Result<()> {{\n    let {accounts_alias} = &mut ctx.accounts;\n    let {account_alias} = &mut {accounts_alias}.{account};\n    {account_alias}.{member_prefix}\n}}\n"
            );
            let completion_line = format!("    {account_alias}.{member_prefix}");
            let path = local_account_alias_path_at(
                &source,
                position_after(&source, &completion_line),
            )
            .expect("accounts alias member path should resolve");

            prop_assert_eq!(path.account_field, account);
            prop_assert!(path.member_chain.is_empty());
            prop_assert_eq!(path.member_prefix, member_prefix);
        }
    }

    #[test]
    fn resolves_short_alias_from_longer_accounts_alias() {
        let source = "pub fn handler(ctx: Context<Run>) -> Result<()> {\n    let afn = &mut ctx.accounts;\n    let a = &mut afn.a0;\n    a.\n}\n";
        let path = local_account_alias_path_at(source, position_after(source, "    a."))
            .expect("short alias should not be confused with accounts alias prefix");

        assert_eq!(path.account_field, "a0");
        assert!(path.member_chain.is_empty());
        assert_eq!(path.member_prefix, "");
    }

    #[test]
    fn ignores_method_call_on_account_alias_rhs() {
        let source = r#"
pub fn handler(ctx: Context<Run>) -> Result<()> {
    let loaded = ctx.accounts.position.load()?;
    loaded.position_
}
"#;

        assert!(
            local_account_alias_path_at(source, position_after(source, "    loaded.position_"))
                .is_none(),
            "loaded account data should be handled by the typed member resolver"
        );
    }
}
