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
    identifier_at_start(&rhs[accounts_start..]).map(str::to_string)
}

fn alias_account_field_from_rhs(rhs: &str, accounts_alias: &str) -> Option<String> {
    let rhs = rhs
        .trim_start_matches('&')
        .trim_start()
        .strip_prefix("mut ")
        .unwrap_or_else(|| rhs.trim_start_matches('&').trim_start());
    let tail = rhs.strip_prefix(accounts_alias)?.strip_prefix('.')?;
    identifier_at_start(tail).map(str::to_string)
}

fn is_accounts_container_assignment(rhs: &str) -> bool {
    let rhs = rhs.trim_start_matches('&').trim_start();
    let rhs = rhs.strip_prefix("mut ").unwrap_or(rhs).trim_start();
    rhs.ends_with(".accounts")
}

fn identifier_at_start(value: &str) -> Option<&str> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!is_identifier_char(ch)).then_some(idx))
        .unwrap_or(value.len());
    let identifier = &value[..end];
    is_identifier(identifier).then_some(identifier)
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
