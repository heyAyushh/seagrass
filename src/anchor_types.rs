#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorFieldCompletionKind {
    AccountType,
    Program,
    Sysvar,
    SplAccount,
}

#[derive(Debug, Clone, Copy)]
pub struct AnchorFieldCompletion {
    pub kind: AnchorFieldCompletionKind,
    pub label: &'static str,
    pub insert_text: &'static str,
    pub field_label: &'static str,
    pub field_insert_text: &'static str,
    pub detail: &'static str,
    pub source_path: &'static str,
}

const FIELD_COMPLETIONS: &[AnchorFieldCompletion] =
    include!("generated/anchor_field_completions_generated.rs");

pub fn field_completions() -> &'static [AnchorFieldCompletion] {
    FIELD_COMPLETIONS
}

pub fn by_label(label: &str) -> Option<&'static AnchorFieldCompletion> {
    FIELD_COMPLETIONS
        .iter()
        .find(|completion| completion.label == label)
}

pub fn field_type_for_field_name(field_name: &str) -> Option<&'static str> {
    FIELD_COMPLETIONS
        .iter()
        .find(|completion| completion_field_name(completion) == Some(field_name))
        .map(|completion| completion.insert_text)
}

pub fn core_account_type(label: &str) -> Option<&'static str> {
    FIELD_COMPLETIONS
        .iter()
        .find(|completion| {
            completion.kind == AnchorFieldCompletionKind::AccountType && completion.label == label
        })
        .map(|completion| completion.insert_text)
}

pub fn sysvar_type_for_field(field_name: &str) -> Option<&'static str> {
    FIELD_COMPLETIONS
        .iter()
        .find(|completion| {
            completion.kind == AnchorFieldCompletionKind::Sysvar
                && completion_field_name(completion) == Some(field_name)
        })
        .map(|completion| completion.insert_text)
}

pub fn sysvar_generic_for_field(field_name: &str) -> Option<&'static str> {
    FIELD_COMPLETIONS
        .iter()
        .find(|completion| {
            completion.kind == AnchorFieldCompletionKind::Sysvar
                && completion_field_name(completion) == Some(field_name)
        })
        .and_then(|completion| last_generic_argument(completion.label))
}

pub fn spl_account_type(account_type: &str) -> Option<&'static str> {
    let label = format!("Account<'info, {account_type}>");
    FIELD_COMPLETIONS
        .iter()
        .find(|completion| {
            completion.kind == AnchorFieldCompletionKind::SplAccount
                && completion.label == label.as_str()
        })
        .map(|completion| completion.insert_text)
}

pub fn field_type_completion(
    type_name: &str,
    generic_type_names: &[String],
) -> Option<&'static AnchorFieldCompletion> {
    let label = field_type_label(type_name, generic_type_names)?;
    by_label(&label)
}

pub fn field_type_label(type_name: &str, generic_type_names: &[String]) -> Option<String> {
    match type_name {
        "AccountInfo" | "Signer" | "SystemAccount" | "UncheckedAccount" => {
            Some(format!("{type_name}<'info>"))
        }
        "Account" | "AccountLoader" | "Interface" | "InterfaceAccount" | "Program" | "Sysvar"
            if !generic_type_names.is_empty() =>
        {
            Some(format!(
                "{type_name}<'info, {}>",
                generic_type_names.join(", ")
            ))
        }
        "Migration" if generic_type_names.len() >= 2 => Some(format!(
            "Migration<'info, {}, {}>",
            generic_type_names[0], generic_type_names[1]
        )),
        _ => None,
    }
}

fn completion_field_name(completion: &AnchorFieldCompletion) -> Option<&str> {
    completion
        .field_label
        .split_once(':')
        .map(|(name, _)| name.trim())
}

fn last_generic_argument(label: &str) -> Option<&str> {
    let start = label.rfind(',')? + 1;
    let end = label.rfind('>')?;
    (start < end).then(|| label[start..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_anchor_types_include_core_sysvar_and_spl_shapes() {
        assert_eq!(core_account_type("Signer<'info>"), Some("Signer<'info>"));
        assert_eq!(sysvar_type_for_field("rent"), Some("Sysvar<'info, Rent>"));
        assert_eq!(sysvar_generic_for_field("rent"), Some("Rent"));
        assert_eq!(
            field_type_for_field_name("rent"),
            Some("Sysvar<'info, Rent>")
        );
        assert_eq!(
            field_type_for_field_name("system_program"),
            Some("Program<'info, System>")
        );
        assert_eq!(sysvar_generic_for_field("fees"), Some("Fees"));
        assert_eq!(
            sysvar_generic_for_field("recent_blockhashes"),
            Some("RecentBlockhashes")
        );
        assert_eq!(
            spl_account_type("TokenAccount"),
            Some("Account<'info, TokenAccount>")
        );
        assert!(field_type_completion("Sysvar", &["Rent".to_string()]).is_some());
    }
}
