use {
    crate::{
        document::AccountConstraint,
        range::range_from_span,
        semantic::{Constraint, ConstraintValue, PdaSeed},
    },
    quote::ToTokens,
    syn::{spanned::Spanned, Attribute},
    tower_lsp::lsp_types::Range,
};

const ACCOUNT_REF_KEYS: &[&str] = &[
    "address",
    "associated_token::authority",
    "associated_token::mint",
    "associated_token::token_program",
    "close",
    "constraint",
    "has_one",
    "mint::authority",
    "mint::freeze_authority",
    "mint::token_program",
    "owner",
    "payer",
    "realloc::payer",
    "seeds::program",
    "token::authority",
    "token::mint",
    "token::token_program",
];

pub fn extract_constraints(attr: &Attribute) -> Vec<Constraint> {
    let text = attr.meta.to_token_stream().to_string();
    constraints_from_text(&text, range_from_span(attr.span()))
}

pub fn constraints_from_account_constraints(
    account_constraints: &[AccountConstraint],
) -> Vec<Constraint> {
    account_constraints
        .iter()
        .flat_map(|constraint| constraints_from_text(&constraint.text, constraint.range))
        .collect()
}

pub fn pda_seeds_from_texts(seed_texts: &[String]) -> Vec<PdaSeed> {
    seed_texts
        .iter()
        .map(|seed| pda_seed_from_text(seed))
        .collect()
}

fn constraints_from_text(text: &str, source_range: Range) -> Vec<Constraint> {
    account_args(text)
        .map(|args| split_top_level(args, ','))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|part| constraint_from_part(&part, source_range))
        .collect()
}

fn account_args(text: &str) -> Option<&str> {
    text.strip_prefix("account(")?.strip_suffix(')')
}

fn constraint_from_part(part: &str, source_range: Range) -> Option<Constraint> {
    let part = part.trim();
    if part.is_empty() {
        return None;
    }
    let (key, value) = part
        .split_once('=')
        .map(|(key, value)| {
            (
                key.trim().to_string(),
                constraint_value(key.trim(), value.trim()),
            )
        })
        .unwrap_or_else(|| (part.to_string(), ConstraintValue::Absent));
    Some(Constraint {
        key,
        value,
        source_range,
    })
}

fn constraint_value(key: &str, value: &str) -> ConstraintValue {
    match value {
        "true" => ConstraintValue::Bool(true),
        "false" => ConstraintValue::Bool(false),
        _ if ACCOUNT_REF_KEYS.contains(&key) && is_path_like(value) => {
            ConstraintValue::AccountRef(value.to_string())
        }
        _ => ConstraintValue::Expression(value.to_string()),
    }
}

fn pda_seed_from_text(seed: &str) -> PdaSeed {
    if let Some(literal) = byte_string_literal(seed) {
        return PdaSeed::Literal(literal);
    }
    if is_path_like(seed) {
        return PdaSeed::AccountRef(seed.to_string());
    }
    PdaSeed::Expression(seed.to_string())
}

fn byte_string_literal(seed: &str) -> Option<Vec<u8>> {
    let inner = seed.strip_prefix("b\"")?.strip_suffix('"')?;
    Some(inner.as_bytes().to_vec())
}

fn is_path_like(value: &str) -> bool {
    let mut saw_segment = false;
    for segment in value.split('.') {
        if segment.is_empty() || !is_ident(segment) {
            return false;
        }
        saw_segment = true;
    }
    saw_segment
}

fn is_ident(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn split_top_level(text: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ if ch == delimiter && depth == 0 => {
                parts.push(text[start..index].to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(text[start..].to_string());
    parts
}
