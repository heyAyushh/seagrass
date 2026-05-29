use {
    super::*,
    anchor_syn::ConstraintToken,
    std::{
        collections::HashSet,
        fs,
        path::{Path, PathBuf},
    },
    syn::parse::Parser,
};

#[test]
fn catalog_labels_parse_as_anchor_constraint_tokens() {
    for spec in CONSTRAINTS {
        let sample = sample_constraint(spec);
        syn::parse_str::<ConstraintToken>(&sample)
            .unwrap_or_else(|err| panic!("failed to parse `{}`: {err}", spec.label));
    }
}

#[test]
fn catalog_labels_are_unique() {
    let mut labels = HashSet::new();
    for spec in CONSTRAINTS {
        assert!(labels.insert(spec.label), "duplicate label {}", spec.label);
    }
}

#[test]
fn catalog_exposes_semantic_metadata_for_anchor_families() {
    let init = by_key("init").unwrap();
    assert_eq!(init.family, ConstraintFamily::Core);
    assert!(init.required_companions.contains(&"payer"));
    assert!(init.required_companions.contains(&"space"));
    assert!(init
        .parser_rules
        .iter()
        .any(|rule| rule.message == "init already provided"));

    let token_authority = by_key("token::authority").unwrap();
    assert_eq!(token_authority.family, ConstraintFamily::TokenAccount);

    let associated_token_program = by_key("associated_token::token_program").unwrap();
    assert_eq!(
        associated_token_program.family,
        ConstraintFamily::AssociatedTokenAccount
    );
    assert!(associated_token_program
        .required_companions
        .contains(&"associated_token::mint"));
    assert!(associated_token_program
        .required_companions
        .contains(&"associated_token::authority"));

    let rent_exempt = by_key("rent_exempt").unwrap();
    assert_eq!(rent_exempt.value_kind, ConstraintValueKind::Keyword);

    let seeds_program = by_key("seeds::program").unwrap();
    assert_eq!(seeds_program.family, ConstraintFamily::Pda);
    assert!(seeds_program.conflicts_with.contains(&"init"));
}

#[test]
fn catalog_links_every_constraint_to_reference_docs() {
    let mut missing = Vec::new();
    for spec in CONSTRAINTS {
        let key = key(spec.label);
        let Some(url) = documentation_url_for_key(key) else {
            missing.push(key.to_string());
            continue;
        };
        assert!(
            url.starts_with(ACCOUNT_CONSTRAINT_DOCS)
                || url.starts_with(ACCOUNT_SPACE_DOCS)
                || url.starts_with("https://docs.rs/anchor-derive-accounts/"),
            "constraint `{key}` resolved to unexpected docs URL: {url}"
        );
    }

    assert!(
        missing.is_empty(),
        "constraints missing docs URLs: {}",
        missing.join(", ")
    );
}

#[test]
fn catalog_finds_generated_parser_rule_by_message() {
    let (spec, rule) = parser_rule_for_message("init already provided").unwrap();
    assert_eq!(key(spec.label), "init");
    assert_eq!(rule.kind, ConstraintParserRuleKind::Duplicate);
    assert_eq!(parser_rule_kind_name(rule.kind), "duplicate");
}

#[test]
fn support_matrix_exposes_editor_and_semantic_coverage() {
    let matrix = support_matrix();
    let init = matrix
        .iter()
        .find(|entry| entry["key"] == "init")
        .expect("init support entry");
    assert!(init["editorFeatures"]
        .as_array()
        .unwrap()
        .iter()
        .any(|feature| feature == "hover"));
    assert!(init["semanticChecks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check == "companionConstraintShape"));

    let has_one = matrix
        .iter()
        .find(|entry| entry["key"] == "has_one")
        .expect("has_one support entry");
    assert!(has_one["semanticChecks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check == "accountDataFieldResolution"));
    assert!(has_one["parserRules"]
        .as_array()
        .unwrap()
        .iter()
        .any(|rule| rule["kind"] == "duplicate"));
    assert!(has_one["codeActions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action == "replaceHasOneTarget"));
    assert_eq!(has_one["supportDepth"], "rich");

    let constraint = matrix
        .iter()
        .find(|entry| entry["key"] == "constraint")
        .expect("constraint support entry");
    assert!(constraint["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|gap| gap == "expressionSemanticsNotEvaluated"));
}

#[test]
fn anchor_program_sources_parse_through_document_model() {
    let mut failures = Vec::new();
    for path in anchor_program_rust_files() {
        let source = fs::read_to_string(&path).unwrap();
        if let Err(err) = crate::document::ParsedDocument::parse(source) {
            failures.push(format!("{}: {err}", path.display()));
        }
    }

    assert!(
        failures.is_empty(),
        "Anchor program files failed LSP document parsing:\n{}",
        failures.join("\n")
    );
}

#[test]
fn catalog_covers_all_anchor_program_account_constraints() {
    let catalog = CONSTRAINTS
        .iter()
        .map(|spec| spec.label)
        .collect::<HashSet<_>>();
    let mut missing = Vec::new();
    let mut parse_failures = Vec::new();

    for path in anchor_program_rust_files() {
        let source = fs::read_to_string(&path).unwrap();
        let Ok(file) = syn::parse_file(&source) else {
            continue;
        };
        collect_account_constraint_labels(
            &file,
            &mut |label| {
                if !catalog.contains(label) {
                    missing.push(format!("{}: {label}", path.display()));
                }
            },
            &mut |err| {
                parse_failures.push(format!("{}: {err}", path.display()));
            },
        );
    }

    assert!(
        parse_failures.is_empty(),
        "Anchor account constraints failed parser coverage:\n{}",
        parse_failures.join("\n")
    );
    assert!(
        missing.is_empty(),
        "Anchor account constraints missing from LSP catalog:\n{}",
        missing.join("\n")
    );
}

fn sample_constraint(spec: &ConstraintSpec) -> String {
    let Some(key) = constraint_key(spec.label) else {
        return spec.label.to_string();
    };
    let value = match spec.value_kind {
        ConstraintValueKind::None => unreachable!("none constraints have no value"),
        ConstraintValueKind::Boolean => "false",
        ConstraintValueKind::Keyword => "enforce",
        ConstraintValueKind::Seeds => "[payer.key().as_ref()]",
        ConstraintValueKind::Space => "8 + State::INIT_SPACE",
        ConstraintValueKind::InstructionArgument => "token_decimals",
        ConstraintValueKind::AccountReference
        | ConstraintValueKind::SignerReference
        | ConstraintValueKind::ProgramReference
        | ConstraintValueKind::AnyExpression => "payer",
    };
    format!("{key} = {value}")
}

fn collect_account_constraint_labels(
    file: &syn::File,
    on_label: &mut impl FnMut(&str),
    on_error: &mut impl FnMut(String),
) {
    for item in &file.items {
        match item {
            syn::Item::Struct(item_struct) => {
                for field in &item_struct.fields {
                    collect_attr_labels(&field.attrs, on_label, on_error);
                }
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, items)) = &item_mod.content {
                    collect_account_constraint_labels(
                        &syn::File {
                            shebang: None,
                            attrs: Vec::new(),
                            items: items.clone(),
                        },
                        on_label,
                        on_error,
                    );
                }
            }
            _ => {}
        }
    }
}

fn collect_attr_labels(
    attrs: &[syn::Attribute],
    on_label: &mut impl FnMut(&str),
    on_error: &mut impl FnMut(String),
) {
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("account")) {
        let parser =
            syn::punctuated::Punctuated::<ConstraintToken, syn::Token![,]>::parse_terminated;
        let list = attr.meta.require_list().unwrap();
        match parser.parse2(list.tokens.clone()) {
            Ok(_) => {
                for label in constraint_labels_from_attr_tokens(list.tokens.clone()) {
                    on_label(&label);
                }
            }
            Err(err) => {
                if collect_legacy_string_constraint_labels(attr, on_label) {
                    continue;
                }
                on_error(format!("{}: {err}", quote::ToTokens::to_token_stream(attr)));
                continue;
            }
        }
    }
}

fn collect_legacy_string_constraint_labels(
    attr: &syn::Attribute,
    on_label: &mut impl FnMut(&str),
) -> bool {
    let Ok(list) = attr.meta.require_list() else {
        return false;
    };
    let parser = syn::punctuated::Punctuated::<syn::LitStr, syn::Token![,]>::parse_terminated;
    let Ok(strings) = parser.parse2(list.tokens.clone()) else {
        return false;
    };
    if strings.is_empty() {
        return false;
    }
    for _ in strings {
        on_label("constraint =");
    }
    true
}

fn constraint_labels_from_attr_tokens(tokens: proc_macro2::TokenStream) -> Vec<String> {
    split_top_level_constraints(&tokens.to_string())
        .into_iter()
        .filter_map(|constraint| label_from_constraint_syntax(&constraint))
        .collect()
}

fn split_top_level_constraints(input: &str) -> Vec<String> {
    let mut constraints = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                constraints.push(input[start..index].trim().to_string());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        constraints.push(tail.to_string());
    }

    constraints
}

fn label_from_constraint_syntax(constraint: &str) -> Option<String> {
    let constraint = constraint.trim();
    if constraint.is_empty() {
        return None;
    }
    if constraint.starts_with('"') {
        return Some("constraint =".to_string());
    }

    let before_error = split_once_top_level(constraint, '@')
        .map_or(constraint, |(before, _)| before)
        .trim();
    let Some((key, _)) = split_once_top_level(before_error, '=') else {
        return Some(normalize_constraint_key(before_error));
    };
    Some(format!("{} =", normalize_constraint_key(key)))
}

fn split_once_top_level(input: &str, needle: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == needle && depth == 0 => {
                return Some((&input[..index], &input[index + ch.len_utf8()..]));
            }
            _ => {}
        }
    }

    None
}

fn normalize_constraint_key(key: &str) -> String {
    key.split_whitespace().collect::<Vec<_>>().join("")
}

fn anchor_program_rust_files() -> Vec<PathBuf> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("lsp crate should live under repo root");
    let mut files = Vec::new();
    for root in ["examples", "tests"] {
        collect_program_rust_files(&repo_root.join(root), &mut files);
    }
    files.sort();
    files
}

fn collect_program_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if ignored_dir(&path) {
                continue;
            }
            collect_program_rust_files(&path, files);
        } else if is_program_rust_file(&path) {
            files.push(path);
        }
    }
}

fn is_program_rust_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "rs")
        && path
            .components()
            .any(|component| component.as_os_str() == "programs")
        && path
            .components()
            .any(|component| component.as_os_str() == "src")
}

fn ignored_dir(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        matches!(
            name.to_str(),
            Some("target" | ".anchor" | "node_modules" | "deps")
        )
    })
}
