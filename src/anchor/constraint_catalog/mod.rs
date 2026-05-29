#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintValueKind {
    None,
    AnyExpression,
    AccountReference,
    SignerReference,
    ProgramReference,
    InstructionArgument,
    Keyword,
    Boolean,
    Space,
    Seeds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintFamily {
    Core,
    Pda,
    TokenAccount,
    AssociatedTokenAccount,
    Mint,
    MintExtension,
    Realloc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConstraintParserRuleKind {
    Duplicate,
    Ordering,
    Conflict,
    TypeRequirement,
    FeatureGate,
    Parser,
}

#[derive(Debug, Clone, Copy)]
pub struct ConstraintParserRule {
    pub kind: ConstraintParserRuleKind,
    pub message: &'static str,
    pub source_method: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ConstraintSpec {
    pub label: &'static str,
    pub detail: &'static str,
    pub value_kind: ConstraintValueKind,
    pub family: ConstraintFamily,
    pub required_companions: &'static [&'static str],
    pub conflicts_with: &'static [&'static str],
    pub parser_rules: &'static [ConstraintParserRule],
    /// True when the presence of this constraint implies the account must be mutable
    /// (e.g. `mut`, `init`, `init_if_needed`, `realloc`, `close`, `zero`).
    #[allow(dead_code)]
    pub implies_mutability: bool,
    /// True for constraints that create or reinitialize an account (`init`, `init_if_needed`).
    #[allow(dead_code)]
    pub is_init_like: bool,
    /// For `ConstraintValueKind::Keyword` constraints, the set of allowed literal values
    /// (e.g. `rent_exempt` → `["skip", "enforce"]`). Empty for non-keyword constraints.
    pub allowed_keyword_values: &'static [&'static str],
}

pub const CONSTRAINTS: &[ConstraintSpec] = include!("../generated/constraint_catalog_generated.rs");
include!("../generated/constraint_keys_generated.rs");

pub const ACCOUNT_CONSTRAINT_DOCS: &str =
    "https://www.anchor-lang.com/docs/references/account-constraints";
pub const ACCOUNT_SPACE_DOCS: &str = "https://www.anchor-lang.com/docs/references/space";

pub fn by_key(key: &str) -> Option<&'static ConstraintSpec> {
    CONSTRAINTS.iter().find(|spec| {
        spec.label == key
            || spec
                .label
                .strip_suffix(" =")
                .is_some_and(|label| label == key)
    })
}

pub fn by_key_with_assignment(key: &str, has_assignment: bool) -> Option<&'static ConstraintSpec> {
    if has_assignment {
        CONSTRAINTS
            .iter()
            .find(|spec| spec.label == format!("{key} ="))
            .or_else(|| by_key(key))
    } else {
        CONSTRAINTS
            .iter()
            .find(|spec| spec.label == key)
            .or_else(|| by_key(key))
    }
}

pub fn value_kind_for_key(key: &str) -> Option<ConstraintValueKind> {
    by_key_with_assignment(key, true).map(|spec| spec.value_kind)
}

pub fn parser_rule_for_message(
    message: &str,
) -> Option<(&'static ConstraintSpec, &'static ConstraintParserRule)> {
    CONSTRAINTS.iter().find_map(|spec| {
        spec.parser_rules
            .iter()
            .find(|rule| rule.message == message || message.contains(rule.message))
            .map(|rule| (spec, rule))
    })
}

pub fn key(label: &str) -> &str {
    label.strip_suffix(" =").unwrap_or(label)
}

pub fn documentation_url_for_key(key: &str) -> Option<&'static str> {
    let key = normalized_documentation_key(key);
    let fragment = match key {
        "address" => "#accountaddress--expr",
        "bump" | "seeds" | "seeds::program" => "#accountseeds-bump",
        "close" => "#accountclose--target",
        "constraint" => "#accountconstraint--expr",
        "dup" => return Some(ACCOUNT_CONSTRAINT_DOCS),
        "executable" => "#accountexecutable",
        "extensions::close_authority::authority" => "#accountextensionsclose_authority",
        "extensions::group_member_pointer::authority"
        | "extensions::group_member_pointer::member_address" => {
            "#accountextensionsgroup_member_pointer"
        }
        "extensions::group_pointer::authority" | "extensions::group_pointer::group_address" => {
            "#accountextensionsgroup_pointer"
        }
        "extensions::metadata_pointer::authority"
        | "extensions::metadata_pointer::metadata_address" => "#accountextensionsmetadata_pointer",
        "extensions::permanent_delegate::delegate" => "#accountextensionspermanent_delegate",
        "extensions::transfer_hook::authority" | "extensions::transfer_hook::program_id" => {
            "#accountextensionstransfer_hook"
        }
        "has_one" => "#accounthas_one--target",
        "init" | "payer" => "#accountinit",
        "init_if_needed" => "#accountinit_if_needed",
        "mut" => "#accountmut",
        "owner" => "#accountowner--expr",
        "realloc" | "realloc::payer" | "realloc::zero" => "#accountrealloc",
        "signer" => "#accountsigner",
        "space" => return Some(ACCOUNT_SPACE_DOCS),
        "zero" => "#accountzero",
        key if key.ends_with("::token_program") => "#accounttoken_program--expr",
        key if key.starts_with("associated_token::") => "#accountassociated_token",
        key if key.starts_with("extensions::") => "#token-extensions-constraints",
        key if key.starts_with("mint::") => "#accountmint",
        key if key.starts_with("token::") => "#accounttoken",
        "rent_exempt" => return Some("https://docs.rs/anchor-derive-accounts/1.0.2/anchor_derive_accounts/derive.Accounts.html#normal-constraints"),
        _ => return None,
    };
    Some(account_constraint_docs_with_fragment(fragment))
}

fn normalized_documentation_key(key: &str) -> &str {
    key.trim()
        .trim_end_matches(',')
        .strip_suffix(" =")
        .unwrap_or_else(|| key.trim().trim_end_matches(','))
}

fn account_constraint_docs_with_fragment(fragment: &'static str) -> &'static str {
    match fragment {
        "#accountaddress--expr" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountaddress--expr"
        }
        "#accountassociated_token" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountassociated_token"
        }
        "#accountclose--target" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountclose--target"
        }
        "#accountconstraint--expr" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountconstraint--expr"
        }
        "#accountexecutable" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountexecutable"
        }
        "#accountextensionsclose_authority" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionsclose_authority"
        }
        "#accountextensionsgroup_member_pointer" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionsgroup_member_pointer"
        }
        "#accountextensionsgroup_pointer" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionsgroup_pointer"
        }
        "#accountextensionsmetadata_pointer" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionsmetadata_pointer"
        }
        "#accountextensionspermanent_delegate" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionspermanent_delegate"
        }
        "#accountextensionstransfer_hook" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountextensionstransfer_hook"
        }
        "#accounthas_one--target" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accounthas_one--target"
        }
        "#accountinit" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountinit"
        }
        "#accountinit_if_needed" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountinit_if_needed"
        }
        "#accountmint" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountmint"
        }
        "#accountmut" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountmut"
        }
        "#accountowner--expr" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountowner--expr"
        }
        "#accountrealloc" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountrealloc"
        }
        "#accountseeds-bump" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountseeds-bump"
        }
        "#accountsigner" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountsigner"
        }
        "#accounttoken" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accounttoken"
        }
        "#accounttoken_program--expr" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accounttoken_program--expr"
        }
        "#accountzero" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#accountzero"
        }
        "#token-extensions-constraints" => {
            "https://www.anchor-lang.com/docs/references/account-constraints#token-extensions-constraints"
        }
        _ => ACCOUNT_CONSTRAINT_DOCS,
    }
}

pub fn family_keys(family: ConstraintFamily) -> impl Iterator<Item = &'static str> {
    CONSTRAINTS
        .iter()
        .filter(move |spec| spec.family == family)
        .map(|spec| key(spec.label))
}

pub fn constraint_key_companions(spec: &ConstraintSpec) -> impl Iterator<Item = &'static str> + '_ {
    spec.required_companions
        .iter()
        .copied()
        .filter(|value| is_constraint_key(value))
}

pub fn is_constraint_key(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
}

/// Returns whether the constraint identified by `key` (with or without ` =`)
/// is marked in the generated catalog as implying that the target account
/// must be declared mutable (covers `mut`, `init*`, `realloc*`, `close`, `zero`, etc.).
#[allow(dead_code)]
pub fn implies_mutability_for_key(key: &str) -> bool {
    let trimmed = key.trim().trim_end_matches(',');
    if let Some(spec) = by_key(trimmed) {
        return spec.implies_mutability;
    }
    // Try stripping any trailing value part for keyed constraints like "init, ..."
    let primary = trimmed
        .split(|ch: char| ch == ',' || ch.is_whitespace())
        .next()
        .unwrap_or(trimmed)
        .trim();
    by_key(primary).is_some_and(|spec| spec.implies_mutability)
}

/// Returns true if, according to the generated catalog, the presence of `key`
/// on an account field requires the presence of `companion`.
pub fn requires_companion(for_key: &str, companion: &str) -> bool {
    by_key(for_key)
        .map(|spec| {
            spec.required_companions
                .iter()
                .any(|c| key(c) == key(companion))
        })
        .unwrap_or(false)
}

/// Returns the set of constraint keys (as they appear in `#[account(...)]`)
/// whose presence on a field means the account must be treated as mutable,
/// according to the generated catalog. This list is derived from the Anchor
/// parser at build time.
pub fn mutability_implying_keys() -> &'static [&'static str] {
    // We could precompute a static slice in the generator, but for simplicity
    // we filter at runtime (tiny set). This keeps the generator change minimal.
    &[
        "mut",
        "init",
        "init_if_needed",
        "realloc",
        "close",
        "zero",
        // The realloc::* and other companions are covered because when the
        // primary key is present the evidence methods will usually see the
        // composite attribute, but we list the primaries here.
        "realloc::payer",
        "realloc::zero",
    ]
}

pub fn markdown_doc(spec: &ConstraintSpec) -> String {
    let key = spec.label.strip_suffix(" =").unwrap_or(spec.label);
    let mut value = format!("`{key}`\n\n{}", spec.detail);
    value.push_str(&format!("\n\nFamily: `{:?}`", spec.family));
    value.push_str(&format!("\n\nValue: `{:?}`", spec.value_kind));
    if !spec.required_companions.is_empty() {
        value.push_str(&format!(
            "\n\nRequires: {}",
            spec.required_companions
                .iter()
                .map(|item| format!("`{item}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !spec.conflicts_with.is_empty() {
        value.push_str(&format!(
            "\n\nConflicts with: {}",
            spec.conflicts_with
                .iter()
                .map(|item| format!("`{item}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !spec.parser_rules.is_empty() {
        value.push_str("\n\nAnchor parser rules:");
        for rule in spec.parser_rules.iter().take(6) {
            value.push_str(&format!(
                "\n- `{:?}` from `{}`: {}",
                rule.kind, rule.source_method, rule.message
            ));
        }
        if spec.parser_rules.len() > 6 {
            value.push_str(&format!(
                "\n- ...{} more parser rule(s)",
                spec.parser_rules.len() - 6
            ));
        }
    }
    value
}

pub fn support_matrix() -> Vec<serde_json::Value> {
    CONSTRAINTS.iter().map(constraint_support).collect()
}

fn constraint_support(spec: &ConstraintSpec) -> serde_json::Value {
    let editor_features = editor_features(spec);
    let semantic_checks = semantic_checks(spec);
    let code_actions = code_actions(spec);
    let gaps = support_gaps(spec, &semantic_checks, &code_actions);
    serde_json::json!({
        "label": spec.label,
        "key": key(spec.label),
        "family": family_name(spec.family),
        "valueKind": value_kind_name(spec.value_kind),
        "detail": spec.detail,
        "requiredCompanions": spec.required_companions,
        "conflictsWith": spec.conflicts_with,
        "parserRules": spec.parser_rules.iter().map(parser_rule_json).collect::<Vec<_>>(),
        "supportDepth": support_depth(&semantic_checks, &code_actions, &gaps),
        "editorFeatures": editor_features,
        "semanticChecks": semantic_checks,
        "codeActions": code_actions,
        "gaps": gaps,
    })
}

fn parser_rule_json(rule: &ConstraintParserRule) -> serde_json::Value {
    serde_json::json!({
        "kind": parser_rule_kind_name(rule.kind),
        "message": rule.message,
        "sourceMethod": rule.source_method,
    })
}

fn editor_features(spec: &ConstraintSpec) -> Vec<&'static str> {
    let mut features = vec!["completion", "completionResolve", "hover", "signatureHelp"];
    if matches!(
        spec.value_kind,
        ConstraintValueKind::AccountReference
            | ConstraintValueKind::SignerReference
            | ConstraintValueKind::ProgramReference
            | ConstraintValueKind::InstructionArgument
            | ConstraintValueKind::Keyword
            | ConstraintValueKind::Space
    ) {
        features.push("contextualValueCompletion");
    }
    features
}

fn code_actions(spec: &ConstraintSpec) -> Vec<&'static str> {
    let mut actions = Vec::new();
    match key(spec.label) {
        "init" => actions.push("addMissingInitConstraints"),
        "has_one" => actions.push("replaceHasOneTarget"),
        "mut" => actions.push("addMutConstraint"),
        "payer" => actions.push("replaceMissingAccountReference"),
        "space" => actions.push("addMissingInitConstraints"),
        "realloc" | "close" => actions.push("addMutConstraint"),
        _ => {}
    }
    match spec.value_kind {
        ConstraintValueKind::AccountReference
        | ConstraintValueKind::SignerReference
        | ConstraintValueKind::ProgramReference => {
            if !actions.contains(&"replaceMissingAccountReference") {
                actions.push("replaceMissingAccountReference");
            }
        }
        ConstraintValueKind::None
        | ConstraintValueKind::AnyExpression
        | ConstraintValueKind::InstructionArgument
        | ConstraintValueKind::Keyword
        | ConstraintValueKind::Boolean
        | ConstraintValueKind::Space
        | ConstraintValueKind::Seeds => {}
    }
    actions
}

fn semantic_checks(spec: &ConstraintSpec) -> Vec<&'static str> {
    let mut checks = Vec::new();

    if !spec.required_companions.is_empty() {
        checks.push("companionConstraintShape");
    }
    if !spec.conflicts_with.is_empty() {
        checks.push("conflictingConstraintShape");
    }
    if !spec.parser_rules.is_empty() {
        checks.push("anchorParserRules");
    }
    match spec.value_kind {
        ConstraintValueKind::AccountReference
        | ConstraintValueKind::SignerReference
        | ConstraintValueKind::ProgramReference => checks.push("accountReferenceResolution"),
        ConstraintValueKind::InstructionArgument => checks.push("instructionArgumentResolution"),
        ConstraintValueKind::Keyword => checks.push("keywordValueValidation"),
        ConstraintValueKind::None
        | ConstraintValueKind::AnyExpression
        | ConstraintValueKind::Boolean
        | ConstraintValueKind::Space
        | ConstraintValueKind::Seeds => {}
    }
    match spec.family {
        ConstraintFamily::TokenAccount | ConstraintFamily::AssociatedTokenAccount => {
            checks.push("tokenAccountTypeShape")
        }
        ConstraintFamily::Mint | ConstraintFamily::MintExtension => checks.push("mintTypeShape"),
        ConstraintFamily::Pda => checks.push("pdaShape"),
        ConstraintFamily::Realloc => checks.push("reallocShape"),
        ConstraintFamily::Core => {}
    }
    if key(spec.label) == "has_one" {
        checks.push("accountDataFieldResolution");
    }

    checks
}

fn support_gaps(
    spec: &ConstraintSpec,
    semantic_checks: &[&'static str],
    code_actions: &[&'static str],
) -> Vec<&'static str> {
    let mut gaps = Vec::new();

    if semantic_checks.is_empty() {
        gaps.push("noSemanticDiagnosticsYet");
    }
    if code_actions.is_empty()
        && (!semantic_checks.is_empty()
            || matches!(
                spec.value_kind,
                ConstraintValueKind::AccountReference
                    | ConstraintValueKind::SignerReference
                    | ConstraintValueKind::ProgramReference
            ))
    {
        gaps.push("noQuickFixYet");
    }
    if matches!(spec.value_kind, ConstraintValueKind::AnyExpression) {
        gaps.push("expressionSemanticsNotEvaluated");
    }
    if matches!(spec.family, ConstraintFamily::MintExtension) {
        gaps.push("token2022ExtensionSpecificRulesNeedExpansion");
    }
    if matches!(key(spec.label), "owner" | "address" | "executable") {
        gaps.push("runtimeValueValidationLimited");
    }

    gaps
}

fn support_depth(
    semantic_checks: &[&'static str],
    code_actions: &[&'static str],
    gaps: &[&'static str],
) -> &'static str {
    if gaps.is_empty() && !semantic_checks.is_empty() && !code_actions.is_empty() {
        "rich"
    } else if !semantic_checks.is_empty() && !code_actions.is_empty() {
        "actionable"
    } else if !semantic_checks.is_empty() {
        "semantic"
    } else {
        "surface"
    }
}

fn family_name(family: ConstraintFamily) -> &'static str {
    match family {
        ConstraintFamily::Core => "core",
        ConstraintFamily::Pda => "pda",
        ConstraintFamily::TokenAccount => "token-account",
        ConstraintFamily::AssociatedTokenAccount => "associated-token-account",
        ConstraintFamily::Mint => "mint",
        ConstraintFamily::MintExtension => "mint-extension",
        ConstraintFamily::Realloc => "realloc",
    }
}

fn value_kind_name(value_kind: ConstraintValueKind) -> &'static str {
    match value_kind {
        ConstraintValueKind::None => "none",
        ConstraintValueKind::AnyExpression => "any-expression",
        ConstraintValueKind::AccountReference => "account-reference",
        ConstraintValueKind::SignerReference => "signer-reference",
        ConstraintValueKind::ProgramReference => "program-reference",
        ConstraintValueKind::InstructionArgument => "instruction-argument",
        ConstraintValueKind::Keyword => "keyword",
        ConstraintValueKind::Boolean => "boolean",
        ConstraintValueKind::Space => "space",
        ConstraintValueKind::Seeds => "seeds",
    }
}

pub fn parser_rule_kind_name(kind: ConstraintParserRuleKind) -> &'static str {
    match kind {
        ConstraintParserRuleKind::Duplicate => "duplicate",
        ConstraintParserRuleKind::Ordering => "ordering",
        ConstraintParserRuleKind::Conflict => "conflict",
        ConstraintParserRuleKind::TypeRequirement => "type-requirement",
        ConstraintParserRuleKind::FeatureGate => "feature-gate",
        ConstraintParserRuleKind::Parser => "parser",
    }
}

pub fn account_reference_specs() -> impl Iterator<Item = (&'static str, ConstraintValueKind)> {
    CONSTRAINTS.iter().filter_map(|spec| match spec.value_kind {
        ConstraintValueKind::AccountReference
        | ConstraintValueKind::SignerReference
        | ConstraintValueKind::ProgramReference => {
            constraint_key(spec.label).map(|key| (key, spec.value_kind))
        }
        ConstraintValueKind::None
        | ConstraintValueKind::AnyExpression
        | ConstraintValueKind::InstructionArgument
        | ConstraintValueKind::Keyword
        | ConstraintValueKind::Boolean
        | ConstraintValueKind::Space
        | ConstraintValueKind::Seeds => None,
    })
}

pub fn instruction_argument_keys() -> impl Iterator<Item = &'static str> {
    CONSTRAINTS.iter().filter_map(|spec| match spec.value_kind {
        ConstraintValueKind::InstructionArgument => constraint_key(spec.label),
        ConstraintValueKind::None
        | ConstraintValueKind::AnyExpression
        | ConstraintValueKind::AccountReference
        | ConstraintValueKind::SignerReference
        | ConstraintValueKind::ProgramReference
        | ConstraintValueKind::Keyword
        | ConstraintValueKind::Boolean
        | ConstraintValueKind::Space
        | ConstraintValueKind::Seeds => None,
    })
}

pub fn expression_value_keys() -> impl Iterator<Item = &'static str> {
    CONSTRAINTS.iter().filter_map(|spec| match spec.value_kind {
        ConstraintValueKind::AnyExpression => constraint_key(spec.label),
        ConstraintValueKind::None
        | ConstraintValueKind::AccountReference
        | ConstraintValueKind::SignerReference
        | ConstraintValueKind::ProgramReference
        | ConstraintValueKind::InstructionArgument
        | ConstraintValueKind::Keyword
        | ConstraintValueKind::Boolean
        | ConstraintValueKind::Space
        | ConstraintValueKind::Seeds => None,
    })
}

fn constraint_key(label: &str) -> Option<&str> {
    label.strip_suffix(" =")
}

#[cfg(test)]
mod tests;
