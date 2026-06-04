use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

const ANCHOR_PATH_ENV: &str = "SEAGRASS_REGEN_ANCHOR_PATH";
const CORPUS_PATH_ENV: &str = "SEAGRASS_REGEN_CORPUS_PATH";
const FAMILY_ENV: &str = "SEAGRASS_REGEN_FAMILY";
const OUT_DIR_ENV: &str = "SEAGRASS_REGEN_OUT_DIR";

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ValueKind {
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
enum ConstraintFamily {
    Core,
    Pda,
    TokenAccount,
    AssociatedTokenAccount,
    Mint,
    MintExtension,
    Realloc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ParserRuleKind {
    Duplicate,
    Ordering,
    Conflict,
    TypeRequirement,
    FeatureGate,
    Parser,
}

fn main() {
    let config = GeneratorConfig::from_env();
    let anchor_root = &config.anchor_path;
    let out_dir = &config.out_dir;
    fs::create_dir_all(&out_dir).unwrap();

    let workspace_manifest_path = anchor_root.join("Cargo.toml");
    let parser_path = anchor_root.join("lang/syn/src/parser/accounts/constraints.rs");
    let accounts_parser_path = anchor_root.join("lang/syn/src/parser/accounts/mod.rs");
    let error_path = first_existing_path(anchor_root, &["lang/error/src/lib.rs", "lang/src/error.rs"]);
    let error_source_path = anchor_relative_path(anchor_root, &error_path);
    let prelude_path = anchor_root.join("lang/src/lib.rs");
    let spl_paths = [
        anchor_root.join("spl/src/associated_token.rs"),
        anchor_root.join("spl/src/token.rs"),
        anchor_root.join("spl/src/token_2022.rs"),
        anchor_root.join("spl/src/token_interface.rs"),
    ];
    let source = fs::read_to_string(&parser_path).unwrap();
    let parser_fingerprint = fingerprint(&source);
    let parser_label_map = extract_constraint_label_map(&source);
    let parser_labels = parser_label_map.values().cloned().collect::<BTreeSet<_>>();
    let parser_rules = extract_parser_rules(&source, &parser_label_map);
    let parser_rule_count = parser_rules.values().map(Vec::len).sum::<usize>();
    let mut labels = parser_labels.clone();
    let mut corpus_summary = CorpusSummary::default();
    for source_root in config.corpus_roots(&anchor_root) {
        collect_program_constraint_labels(&source_root, &mut labels, &mut corpus_summary);
    }
    let out = generate_catalog(&labels, &parser_rules);
    fs::write(out_dir.join("constraint_catalog_generated.rs"), out).unwrap();

    fs::write(
        out_dir.join("constraint_keys_generated.rs"),
        generate_constraint_key_lists(&labels),
    )
    .unwrap();

    let error_source = fs::read_to_string(&error_path).unwrap();
    let error_fingerprint = fingerprint(&error_source);
    let errors = extract_anchor_errors(&error_source);
    fs::write(
        out_dir.join("anchor_error_catalog_generated.rs"),
        generate_error_catalog(&errors),
    )
    .unwrap();

    let field_completion_fingerprint =
        field_completion_source_fingerprint(&prelude_path, &accounts_parser_path, &spl_paths);
    let field_completions =
        anchor_field_completions(&prelude_path, &accounts_parser_path, &spl_paths);
    fs::write(
        out_dir.join("anchor_field_completions_generated.rs"),
        generate_field_completions(&field_completions),
    )
    .unwrap();

    let workspace_manifest = fs::read_to_string(&workspace_manifest_path).unwrap();
    fs::write(
        out_dir.join("anchor_support_generated.rs"),
        generate_anchor_support(
            config.family,
            &workspace_version(&workspace_manifest),
            parser_labels.len(),
            parser_rule_count,
            labels.len(),
            field_completions.len(),
            corpus_summary.file_count,
            corpus_summary.constraint_labels.len(),
            errors.len(),
            parser_fingerprint,
            error_fingerprint,
            field_completion_fingerprint,
            corpus_summary.fingerprint,
            &error_source_path,
        ),
    )
    .unwrap();
}

fn first_existing_path(anchor_root: &Path, candidates: &[&str]) -> PathBuf {
    candidates
        .iter()
        .map(|candidate| anchor_root.join(candidate))
        .find(|path| path.exists())
        .unwrap_or_else(|| panic!("none of the expected paths exist: {candidates:?}"))
}

fn anchor_relative_path(anchor_root: &Path, path: &Path) -> String {
    path.strip_prefix(anchor_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

struct GeneratorConfig {
    anchor_path: PathBuf,
    out_dir: PathBuf,
    family: RequestedFamily,
    corpus_path: Option<PathBuf>,
}

impl GeneratorConfig {
    fn from_env() -> Self {
        Self {
            anchor_path: required_path_env(ANCHOR_PATH_ENV),
            out_dir: required_path_env(OUT_DIR_ENV),
            family: RequestedFamily::parse(&required_env(FAMILY_ENV)),
            corpus_path: env::var_os(CORPUS_PATH_ENV).map(PathBuf::from),
        }
    }

    fn corpus_roots(&self, anchor_root: &Path) -> Vec<PathBuf> {
        self.corpus_path
            .as_ref()
            .map(|path| vec![path.clone()])
            .unwrap_or_else(|| vec![anchor_root.join("examples"), anchor_root.join("tests")])
    }
}

#[derive(Debug, Clone, Copy)]
enum RequestedFamily {
    V1,
    V2Preview,
}

impl RequestedFamily {
    fn parse(value: &str) -> Self {
        match value {
            "v1" => Self::V1,
            "v2-preview" => Self::V2Preview,
            _ => panic!("{FAMILY_ENV} must be v1 or v2-preview"),
        }
    }

    fn support_level(self) -> AnchorSupportLevel {
        match self {
            Self::V1 => AnchorSupportLevel::AnchorV1,
            Self::V2Preview => AnchorSupportLevel::AnchorV2Preview,
        }
    }

    fn version_family(self) -> &'static str {
        match self {
            Self::V1 => "anchor-v1",
            Self::V2Preview => "anchor-v2-preview",
        }
    }
}

fn required_path_env(name: &str) -> PathBuf {
    PathBuf::from(required_env(name))
}

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

#[derive(Debug)]
struct CorpusSummary {
    file_count: usize,
    constraint_labels: BTreeSet<String>,
    fingerprint: u64,
}

impl Default for CorpusSummary {
    fn default() -> Self {
        Self {
            file_count: 0,
            constraint_labels: BTreeSet::new(),
            fingerprint: FNV_OFFSET,
        }
    }
}

#[derive(Debug)]
struct AnchorError {
    name: String,
    code: u32,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FieldCompletion {
    kind: FieldCompletionKind,
    label: String,
    insert_text: String,
    field_label: String,
    field_insert_text: String,
    detail: String,
    source_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FieldCompletionKind {
    AccountType,
    Program,
    Sysvar,
    SplAccount,
}

fn extract_anchor_errors(source: &str) -> Vec<AnchorError> {
    let Some(enum_start) = source.find("pub enum ErrorCode") else {
        return Vec::new();
    };
    let Some(body_start) = source[enum_start..]
        .find('{')
        .map(|idx| enum_start + idx + 1)
    else {
        return Vec::new();
    };
    let Some(body_end) = matching_delimiter(source, body_start - 1, '{', '}') else {
        return Vec::new();
    };

    let mut errors = Vec::new();
    let mut next_code = 0u32;
    let mut pending_msg: Option<String> = None;

    for line in source[body_start..body_end].lines() {
        let trimmed = line.trim();
        if let Some(message) = msg_attr(trimmed) {
            pending_msg = Some(message);
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with("//")
            || trimmed.starts_with("#[")
            || !trimmed.ends_with(',')
        {
            continue;
        }

        let variant = trimmed.trim_end_matches(',').trim();
        let (name, code) = if let Some((name, code)) = variant.split_once('=') {
            let code = code.trim().replace('_', "").parse::<u32>().unwrap();
            (name.trim(), code)
        } else {
            (variant, next_code)
        };

        if name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            errors.push(AnchorError {
                name: name.to_string(),
                code,
                message: pending_msg.take().unwrap_or_default(),
            });
            next_code = code + 1;
        }
    }

    errors
}

fn msg_attr(line: &str) -> Option<String> {
    let rest = line.strip_prefix("#[msg(")?;
    let literal_start = rest.find('"')? + 1;
    let after_start = &rest[literal_start..];
    let literal_end = after_start.find('"')?;
    Some(after_start[..literal_end].to_string())
}

fn anchor_field_completions(
    prelude_path: &Path,
    parser_path: &Path,
    spl_paths: &[PathBuf],
) -> Vec<FieldCompletion> {
    let mut completions = BTreeSet::new();
    let prelude_source = fs::read_to_string(prelude_path).unwrap_or_default();
    let prelude_body = module_body(&prelude_source, "prelude").unwrap_or(&prelude_source);
    let prelude_idents = exported_idents(prelude_body);
    let prelude_path = "lang/src/lib.rs";
    let parser_source = fs::read_to_string(parser_path).unwrap_or_default();
    let parser_path = "lang/syn/src/parser/accounts/mod.rs";

    for ty in [
        "Account",
        "AccountInfo",
        "AccountLoader",
        "Interface",
        "InterfaceAccount",
        "Migration",
        "Program",
        "Signer",
        "SystemAccount",
        "Sysvar",
        "UncheckedAccount",
    ] {
        if prelude_idents.contains(ty) {
            completions.insert(core_field_completion(ty, prelude_path));
        }
    }

    if prelude_idents.contains("System") {
        completions.insert(program_completion("System", prelude_path));
    }

    let mut sysvars = extract_sysvar_types(prelude_body);
    sysvars.extend(extract_parser_sysvar_types(&parser_source));
    for sysvar in sysvars {
        completions.insert(sysvar_completion(&sysvar, parser_path));
    }

    for path in spl_paths {
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let source_path = normalized_source_path(path);
        let module = path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        for item in extract_zero_lifetime_structs(&source) {
            match module {
                "associated_token" | "token_2022"
                    if item == "AssociatedToken" || item == "Token2022" =>
                {
                    completions.insert(program_completion(&item, &source_path));
                }
                "token" if item == "Token" => {
                    completions.insert(program_completion(&item, &source_path));
                }
                "token_interface" if item == "TokenInterface" => {
                    completions.insert(program_completion(&item, &source_path));
                }
                "token" if item == "Mint" || item == "TokenAccount" => {
                    completions.insert(spl_account_completion("Account", &item, &source_path));
                }
                "token_interface" if item == "Mint" || item == "TokenAccount" => {
                    completions.insert(spl_account_completion(
                        "InterfaceAccount",
                        &item,
                        &source_path,
                    ));
                }
                _ => {}
            }
        }
    }

    completions.into_iter().collect()
}

fn core_field_completion(ty: &str, source_path: &str) -> FieldCompletion {
    let (label, insert_text, field_name, detail) = match ty {
        "Account" => (
            "Account<'info, T>".to_string(),
            "Account<'info, ${1:T}>".to_string(),
            "account".to_string(),
            "Anchor account container that checks ownership and deserializes data.".to_string(),
        ),
        "AccountInfo" => (
            "AccountInfo<'info>".to_string(),
            "AccountInfo<'info>".to_string(),
            "account_info".to_string(),
            "Raw Solana account info; prefer UncheckedAccount when no checks are performed."
                .to_string(),
        ),
        "AccountLoader" => (
            "AccountLoader<'info, T>".to_string(),
            "AccountLoader<'info, ${1:T}>".to_string(),
            "account_loader".to_string(),
            "Anchor zero-copy account loader.".to_string(),
        ),
        "Interface" => (
            "Interface<'info, T>".to_string(),
            "Interface<'info, ${1:T}>".to_string(),
            "program".to_string(),
            "Anchor program interface account.".to_string(),
        ),
        "InterfaceAccount" => (
            "InterfaceAccount<'info, T>".to_string(),
            "InterfaceAccount<'info, ${1:T}>".to_string(),
            "account".to_string(),
            "Anchor account container for interface-owned account data.".to_string(),
        ),
        "Migration" => (
            "Migration<'info, From, To>".to_string(),
            "Migration<'info, ${1:From}, ${2:To}>".to_string(),
            "migration".to_string(),
            "Anchor account migration container.".to_string(),
        ),
        "Program" => (
            "Program<'info, T>".to_string(),
            "Program<'info, ${1:T}>".to_string(),
            "program".to_string(),
            "Anchor executable program account.".to_string(),
        ),
        "Signer" => (
            "Signer<'info>".to_string(),
            "Signer<'info>".to_string(),
            "signer".to_string(),
            "Anchor signer account.".to_string(),
        ),
        "SystemAccount" => (
            "SystemAccount<'info>".to_string(),
            "SystemAccount<'info>".to_string(),
            "system_account".to_string(),
            "Anchor account owned by the system program.".to_string(),
        ),
        "Sysvar" => (
            "Sysvar<'info, T>".to_string(),
            "Sysvar<'info, ${1:T}>".to_string(),
            "sysvar".to_string(),
            "Anchor sysvar account.".to_string(),
        ),
        "UncheckedAccount" => (
            "UncheckedAccount<'info>".to_string(),
            "UncheckedAccount<'info>".to_string(),
            "account".to_string(),
            "Explicit unchecked account wrapper.".to_string(),
        ),
        _ => (
            format!("{ty}<'info>"),
            format!("{ty}<'info>"),
            lower_snake(ty),
            "Anchor account type.".to_string(),
        ),
    };

    FieldCompletion {
        kind: FieldCompletionKind::AccountType,
        field_label: format!("{field_name}: {label}"),
        field_insert_text: format!("{field_name}: {insert_text},"),
        label,
        insert_text,
        detail,
        source_path: source_path.to_string(),
    }
}

fn program_completion(program: &str, source_path: &str) -> FieldCompletion {
    let label = format!("Program<'info, {program}>");
    let insert_text = label.clone();
    let field_name = format!(
        "{}_program",
        lower_snake(program).trim_end_matches("_program")
    );
    FieldCompletion {
        kind: FieldCompletionKind::Program,
        field_label: format!("{field_name}: {label}"),
        field_insert_text: format!("{field_name}: {insert_text},"),
        label,
        insert_text,
        detail: format!("Anchor program account for `{program}`."),
        source_path: source_path.to_string(),
    }
}

fn sysvar_completion(sysvar: &str, source_path: &str) -> FieldCompletion {
    let label = format!("Sysvar<'info, {sysvar}>");
    let insert_text = label.clone();
    let field_name = lower_snake(sysvar);
    FieldCompletion {
        kind: FieldCompletionKind::Sysvar,
        field_label: format!("{field_name}: {label}"),
        field_insert_text: format!("{field_name}: {insert_text},"),
        label,
        insert_text,
        detail: format!("Anchor sysvar account for `{sysvar}`."),
        source_path: source_path.to_string(),
    }
}

fn spl_account_completion(wrapper: &str, account: &str, source_path: &str) -> FieldCompletion {
    let label = format!("{wrapper}<'info, {account}>");
    let insert_text = label.clone();
    let mut field_name = lower_snake(account);
    if let Some(stripped) = field_name.strip_suffix("_account") {
        field_name = stripped.to_string();
    }
    FieldCompletion {
        kind: FieldCompletionKind::SplAccount,
        field_label: format!("{field_name}: {label}"),
        field_insert_text: format!("{field_name}: {insert_text},"),
        label,
        insert_text,
        detail: format!("Anchor SPL account wrapper for `{account}`."),
        source_path: source_path.to_string(),
    }
}

fn extract_sysvar_types(prelude_body: &str) -> BTreeSet<String> {
    let mut sysvars = BTreeSet::new();
    let mut in_solana_sysvar_block = false;

    for line in prelude_body.lines() {
        let trimmed = line.trim();
        if trimmed.contains("solana_sysvar::{") {
            in_solana_sysvar_block = true;
        }

        if trimmed.contains("solana_clock::")
            || trimmed.contains("solana_instructions_sysvar::")
            || trimmed.contains("solana_stake_interface::")
            || in_solana_sysvar_block
        {
            for piece in trimmed.split(',') {
                if let Some(ident) = use_piece_ident(piece) {
                    if ident != "Sysvar" && ident != "SolanaSysvar" {
                        sysvars.insert(ident);
                    }
                } else if in_solana_sysvar_block {
                    if let Some(ident) = sysvar_module_type(piece) {
                        sysvars.insert(ident);
                    }
                }
            }
        }

        if in_solana_sysvar_block && trimmed.contains('}') {
            in_solana_sysvar_block = false;
        }
    }

    sysvars
}

fn extract_parser_sysvar_types(source: &str) -> BTreeSet<String> {
    let Some(fn_start) = source.find("fn parse_sysvar") else {
        return BTreeSet::new();
    };
    let Some(body_start) = source[fn_start..].find('{').map(|idx| fn_start + idx) else {
        return BTreeSet::new();
    };
    let Some(body_end) = matching_delimiter(source, body_start, '{', '}') else {
        return BTreeSet::new();
    };

    source[body_start..body_end]
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let ident = trimmed
                .strip_prefix('"')
                .and_then(|rest| rest.find('"').map(|end| &rest[..end]))?;
            trimmed.contains("=> SysvarTy::").then(|| ident.to_string())
        })
        .collect()
}

fn exported_idents(prelude_body: &str) -> BTreeSet<String> {
    prelude_body
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|part| {
            part.chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
        .map(ToString::to_string)
        .collect()
}

fn use_piece_ident(piece: &str) -> Option<String> {
    let piece = piece
        .trim()
        .trim_start_matches("pub use")
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim_end_matches(';')
        .trim();
    let piece = piece.split(" as ").next().unwrap_or(piece).trim();
    let ident = piece.rsplit("::").next()?.trim();
    ident
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        .then(|| ident.to_string())
}

fn sysvar_module_type(piece: &str) -> Option<String> {
    let piece = piece
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim_end_matches(';')
        .trim();
    let ident = piece.rsplit("::").next()?.trim();
    if ident.is_empty()
        || ident.starts_with("is_")
        || !ident.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_')
    {
        return None;
    }

    Some(upper_camel(ident))
}

fn upper_camel(value: &str) -> String {
    let mut result = String::new();
    let mut uppercase_next = true;
    for ch in value.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            result.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn extract_zero_lifetime_structs(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("pub struct ")?;
            let name = rest
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .next()?;
            (!rest.contains("<'info") && !name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

fn module_body<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let module_start = source.find(&format!("pub mod {name}"))?;
    let open = module_start + source[module_start..].find('{')?;
    let close = matching_delimiter(source, open, '{', '}')?;
    source.get(open + 1..close)
}

fn lower_snake(value: &str) -> String {
    let mut result = String::new();
    let mut previous_lower = false;
    for ch in value.chars() {
        if ch.is_ascii_uppercase() {
            if previous_lower {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
            previous_lower = false;
        } else {
            result.push(ch);
            previous_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    result
}

fn normalized_source_path(path: &Path) -> String {
    path.components()
        .collect::<Vec<_>>()
        .windows(3)
        .find_map(|components| {
            (components[0].as_os_str() == "spl" && components[1].as_os_str() == "src").then(|| {
                components
                    .iter()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
        })
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn generate_field_completions(completions: &[FieldCompletion]) -> String {
    let mut out = String::new();
    out.push_str("&[\n");
    for completion in completions {
        out.push_str("    AnchorFieldCompletion {\n");
        out.push_str(&format!(
            "        kind: AnchorFieldCompletionKind::{:?},\n",
            completion.kind
        ));
        out.push_str(&format!(
            "        label: {},\n",
            rust_string(&completion.label)
        ));
        out.push_str(&format!(
            "        insert_text: {},\n",
            rust_string(&completion.insert_text)
        ));
        out.push_str(&format!(
            "        field_label: {},\n",
            rust_string(&completion.field_label)
        ));
        out.push_str(&format!(
            "        field_insert_text: {},\n",
            rust_string(&completion.field_insert_text)
        ));
        out.push_str(&format!(
            "        detail: {},\n",
            rust_string(&completion.detail)
        ));
        out.push_str(&format!(
            "        source_path: {},\n",
            rust_string(&completion.source_path)
        ));
        out.push_str("    },\n");
    }
    out.push_str("]\n");
    out
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn workspace_version(manifest: &str) -> String {
    let mut in_workspace_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }
        if !in_workspace_package {
            continue;
        }
        let Some(value) = trimmed.strip_prefix("version") else {
            continue;
        };
        let Some((_, value)) = value.split_once('=') else {
            continue;
        };
        return value.trim().trim_matches('"').to_string();
    }
    "unknown".to_string()
}

#[derive(Debug)]
struct AnchorVersion {
    raw: String,
    major: u32,
    minor: u32,
    patch: u32,
}

impl AnchorVersion {
    fn parse(version: &str) -> Self {
        let mut parts = version
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.'))
            .next()
            .unwrap_or(version)
            .split('.');
        Self {
            raw: version.to_string(),
            major: parse_version_part(parts.next()),
            minor: parse_version_part(parts.next()),
            patch: parse_version_part(parts.next()),
        }
    }
}

fn parse_version_part(part: Option<&str>) -> u32 {
    part.and_then(|part| part.parse::<u32>().ok()).unwrap_or(0)
}

#[derive(Debug)]
enum AnchorSupportLevel {
    AnchorV1,
    AnchorV2Preview,
}

fn fingerprint(source: &str) -> u64 {
    fnv_update(FNV_OFFSET, source.as_bytes())
}

fn combined_fingerprint(parts: &[u64]) -> u64 {
    parts.iter().fold(FNV_OFFSET, |hash, part| {
        fnv_update(hash, &part.to_le_bytes())
    })
}

fn fnv_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn field_completion_source_fingerprint(
    prelude_path: &Path,
    parser_path: &Path,
    spl_paths: &[PathBuf],
) -> u64 {
    let mut hash = FNV_OFFSET;
    for path in std::iter::once(prelude_path)
        .chain(std::iter::once(parser_path))
        .chain(spl_paths.iter().map(PathBuf::as_path))
    {
        hash = fnv_update(hash, path.to_string_lossy().as_bytes());
        if let Ok(source) = fs::read_to_string(path) {
            hash = fnv_update(hash, source.as_bytes());
        }
    }
    hash
}

fn fingerprint_hex(hash: u64) -> String {
    format!("{hash:016x}")
}

#[allow(clippy::too_many_arguments)]
fn generate_anchor_support(
    requested_family: RequestedFamily,
    version: &str,
    parser_constraint_count: usize,
    parser_rule_count: usize,
    constraint_count: usize,
    field_completion_count: usize,
    corpus_file_count: usize,
    corpus_constraint_count: usize,
    error_count: usize,
    parser_fingerprint: u64,
    error_fingerprint: u64,
    field_completion_fingerprint: u64,
    corpus_fingerprint: u64,
    error_source_path: &str,
) -> String {
    let version = AnchorVersion::parse(version);
    let source_fingerprint = combined_fingerprint(&[
        parser_fingerprint,
        error_fingerprint,
        field_completion_fingerprint,
        corpus_fingerprint,
    ]);
    let mut out = String::new();
    out.push_str("AnchorSupportManifest {\n");
    out.push_str(&format!("    anchor_version: {:?},\n", version.raw));
    out.push_str(&format!(
        "    support_level: AnchorSupportLevel::{:?},\n",
        requested_family.support_level()
    ));
    out.push_str("    generator_version: 2,\n");
    out.push_str("    profile: AnchorSupportProfile {\n");
    out.push_str(&format!("        version_major: {},\n", version.major));
    out.push_str(&format!("        version_minor: {},\n", version.minor));
    out.push_str(&format!("        version_patch: {},\n", version.patch));
    out.push_str(&format!(
        "        version_family: {:?},\n",
        requested_family.version_family()
    ));
    out.push_str(&format!(
        "        source_fingerprint: {:?},\n",
        fingerprint_hex(source_fingerprint)
    ));
    out.push_str(&format!(
        "        parser_fingerprint: {:?},\n",
        fingerprint_hex(parser_fingerprint)
    ));
    out.push_str(&format!(
        "        error_fingerprint: {:?},\n",
        fingerprint_hex(error_fingerprint)
    ));
    out.push_str(&format!(
        "        field_completion_fingerprint: {:?},\n",
        fingerprint_hex(field_completion_fingerprint)
    ));
    out.push_str(&format!(
        "        corpus_fingerprint: {:?},\n",
        fingerprint_hex(corpus_fingerprint)
    ));
    out.push_str(&format!(
        "        parser_constraint_count: {parser_constraint_count},\n"
    ));
    out.push_str(&format!(
        "        parser_rule_count: {parser_rule_count},\n"
    ));
    out.push_str(&format!(
        "        program_corpus_file_count: {corpus_file_count},\n"
    ));
    out.push_str(&format!(
        "        program_corpus_constraint_count: {corpus_constraint_count},\n"
    ));
    out.push_str("        generation_mode: \"source-driven\",\n");
    out.push_str("        regeneration_strategy: \"regenerate from the checked-out Anchor parser, framework errors, examples, and tests; compare fingerprints and support gaps before claiming a newer version is covered\",\n");
    out.push_str(
        "        anchor_v1_policy: \"supported when generated from Anchor v1 sources and the generated parser/error/corpus profile remains internally consistent\",\n",
    );
    out.push_str(
        "        anchor_v2_policy: \"preview only until a v2 checkout generates a stable parser/error/corpus profile and the semantic analyzers are audited against it\",\n",
    );
    out.push_str("    },\n");
    out.push_str(&format!(
        "    generated_constraint_count: {constraint_count},\n"
    ));
    out.push_str(&format!(
        "    generated_field_completion_count: {field_completion_count},\n"
    ));
    out.push_str(&format!("    generated_error_count: {error_count},\n"));
    out.push_str("    sources: &[\n");
    out.push_str("        AnchorSupportSource { kind: \"constraint-parser\", path: \"lang/syn/src/parser/accounts/constraints.rs\" },\n");
    out.push_str(&format!(
        "        AnchorSupportSource {{ kind: \"error-codes\", path: {:?} }},\n",
        error_source_path
    ));
    out.push_str(
        "        AnchorSupportSource { kind: \"field-completions\", path: \"lang/src/lib.rs\" },\n",
    );
    out.push_str(
        "        AnchorSupportSource { kind: \"field-completions\", path: \"spl/src/{associated_token,token,token_2022,token_interface}.rs\" },\n",
    );
    out.push_str(
        "        AnchorSupportSource { kind: \"program-corpus\", path: \"examples/**/programs/**/src/**/*.rs\" },\n",
    );
    out.push_str(
        "        AnchorSupportSource { kind: \"program-corpus\", path: \"tests/**/programs/**/src/**/*.rs\" },\n",
    );
    out.push_str("    ],\n");
    out.push_str("    notes: &[\n");
    out.push_str("        \"Anchor v1 support is generated from this checkout's parser, framework error enum, examples, and tests.\",\n");
    out.push_str("        \"The source fingerprint changes when Anchor parser/error/corpus inputs change, forcing the support matrix to be re-evaluated for newer Anchor v1 releases.\",\n");
    out.push_str("        \"Anchor v2 is treated as preview until the parser and generated support manifest prove stable coverage.\",\n");
    out.push_str("    ],\n");
    out.push_str("}\n");
    out
}

fn collect_program_constraint_labels(
    path: &Path,
    labels: &mut BTreeSet<String>,
    summary: &mut CorpusSummary,
) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            if ignored_dir(&path) {
                continue;
            }
            collect_program_constraint_labels(&path, labels, summary);
        } else if is_program_rust_file(&path) {
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            summary.file_count += 1;
            summary.fingerprint = combined_fingerprint(&[
                summary.fingerprint,
                fingerprint(path.to_string_lossy().as_ref()),
                fingerprint(&source),
            ]);
            let source = strip_line_comments(&source);
            for label in extract_account_attribute_labels(&source) {
                summary.constraint_labels.insert(label.clone());
                labels.insert(label);
            }
        }
    }
}

fn ignored_dir(path: &Path) -> bool {
    path.file_name().is_some_and(|name| {
        matches!(
            name.to_str(),
            Some("target" | ".anchor" | "node_modules" | "deps")
        )
    })
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

fn extract_constraint_label_map(source: &str) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let Some((literal, variant)) = constraint_token_arm(trimmed) else {
            continue;
        };
        let Some(label) = label_for_arm(source, trimmed, literal, variant) else {
            continue;
        };
        labels.entry(variant.to_string()).or_insert(label);
    }
    labels
}

fn constraint_token_arm(line: &str) -> Option<(&str, &str)> {
    let quote_start = line.find('"')?;
    let rest = &line[quote_start + 1..];
    let quote_end = rest.find('"')?;
    let literal = &rest[..quote_end];
    let after_literal = &rest[quote_end + 1..];
    if !after_literal.contains("=> ConstraintToken::") {
        return None;
    }
    let variant_start = after_literal.find("ConstraintToken::")? + "ConstraintToken::".len();
    let variant_rest = &after_literal[variant_start..];
    let variant_end = variant_rest
        .find(|ch: char| !ch.is_ascii_alphanumeric())
        .unwrap_or(variant_rest.len());
    Some((literal, &variant_rest[..variant_end]))
}

fn extract_account_attribute_labels(source: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    let mut search_start = 0;
    while let Some(relative_start) = source[search_start..].find("#[account(") {
        let open_paren = search_start + relative_start + "#[account".len();
        let Some(close_paren) = matching_delimiter(source, open_paren, '(', ')') else {
            search_start = open_paren + 1;
            continue;
        };
        if is_account_container_attribute(&source[close_paren + 1..]) {
            search_start = close_paren + 1;
            continue;
        }
        let attr_body = strip_line_comments(&source[open_paren + 1..close_paren]);
        for constraint in split_top_level_constraints(&attr_body) {
            if let Some(label) = label_from_constraint_syntax(&constraint) {
                labels.insert(label);
            }
        }
        search_start = close_paren + 1;
    }
    labels
}

fn is_account_container_attribute(after_close_paren: &str) -> bool {
    let mut after_attr = after_close_paren
        .strip_prefix(']')
        .unwrap_or(after_close_paren)
        .trim_start();
    loop {
        after_attr = after_attr.trim_start();
        if let Some(rest) = after_attr.strip_prefix("//") {
            let Some(newline) = rest.find('\n') else {
                return false;
            };
            after_attr = &rest[newline + 1..];
        } else if let Some(rest) = after_attr.strip_prefix("/*") {
            let Some(comment_end) = rest.find("*/") else {
                return false;
            };
            after_attr = &rest[comment_end + 2..];
        } else if after_attr.starts_with("#[") {
            let Some(close_attr) = matching_delimiter(after_attr, 1, '[', ']') else {
                break;
            };
            after_attr = &after_attr[close_attr + 1..];
        } else {
            break;
        }
    }
    after_attr.starts_with("pub struct")
        || after_attr.starts_with("struct")
        || after_attr.starts_with("pub enum")
        || after_attr.starts_with("enum")
}

fn strip_line_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }

    out
}

fn matching_delimiter(source: &str, open_index: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;

    for (offset, ch) in source[open_index..].char_indices() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
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

        let index = open_index + offset;
        match ch {
            '/' if source[index..].starts_with("//") => line_comment = true,
            '"' => in_string = true,
            _ if ch == open => depth += 1,
            _ if ch == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_constraints(input: &str) -> Vec<String> {
    let mut constraints = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;

    for (index, ch) in input.char_indices() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
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
            '/' if input[index..].starts_with("//") => line_comment = true,
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
    let mut line_comment = false;

    for (index, ch) in input.char_indices() {
        if line_comment {
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
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
            '/' if input[index..].starts_with("//") => line_comment = true,
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

fn label_for_arm(source: &str, line: &str, literal: &str, variant: &str) -> Option<String> {
    let prefix = namespace_prefix(source, line, variant);
    let key = match prefix.as_deref() {
        Some(prefix) => format!("{prefix}::{literal}"),
        None => literal.to_string(),
    };
    if variant_takes_value(variant) {
        Some(format!("{key} ="))
    } else {
        Some(key)
    }
}

fn namespace_prefix(source: &str, line: &str, variant: &str) -> Option<String> {
    if matches!(
        variant,
        "Init"
            | "Zeroed"
            | "Mut"
            | "Dup"
            | "Signer"
            | "Executable"
            | "Bump"
            | "HasOne"
            | "Raw"
            | "Owner"
            | "RentExempt"
            | "Payer"
            | "Space"
            | "Close"
            | "Address"
    ) {
        return None;
    }
    let line_index = source.find(line)?;
    let before = &source[..line_index];

    if variant.starts_with("Mint") {
        return Some("mint".to_string());
    }
    if variant.starts_with("Token") {
        return Some("token".to_string());
    }
    if variant.starts_with("AssociatedToken") {
        return Some("associated_token".to_string());
    }
    if variant == "ProgramSeed" {
        return Some("seeds".to_string());
    }
    if variant.starts_with("Realloc") {
        return Some("realloc".to_string());
    }
    if variant.starts_with("Extension") {
        let group = nearest_extension_group(before)?;
        return Some(format!("extensions::{group}"));
    }
    None
}

fn nearest_extension_group(before: &str) -> Option<&'static str> {
    [
        ("group_member_pointer", "group_member_pointer"),
        ("metadata_pointer", "metadata_pointer"),
        ("close_authority", "close_authority"),
        ("permanent_delegate", "permanent_delegate"),
        ("transfer_hook", "transfer_hook"),
        ("group_pointer", "group_pointer"),
        ("pausable", "pausable"),
    ]
    .into_iter()
    .filter_map(|(needle, group)| {
        before
            .rfind(&format!("\"{needle}\" =>"))
            .map(|idx| (idx, group))
    })
    .max_by_key(|(idx, _)| *idx)
    .map(|(_, group)| group)
}

fn variant_takes_value(variant: &str) -> bool {
    !matches!(
        variant,
        "Init" | "Zeroed" | "Mut" | "Dup" | "Signer" | "Executable" | "Bump"
    )
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ParserRule {
    method: String,
    kind: ParserRuleKind,
    message: String,
}

fn extract_parser_rules(
    source: &str,
    variant_labels: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<ParserRule>> {
    let method_labels = extract_add_method_labels(source, variant_labels);
    let mut rules = BTreeMap::<String, Vec<ParserRule>>::new();

    for (method, label) in method_labels {
        let Some(body) = function_body(source, &format!("fn {method}")) else {
            continue;
        };
        for message in string_literals(body) {
            if !looks_like_parser_rule(&message) {
                continue;
            }
            let rule = ParserRule {
                method: method.clone(),
                kind: parser_rule_kind(&message),
                message,
            };
            rules.entry(label.clone()).or_default().push(rule);
        }
    }

    for label_rules in rules.values_mut() {
        label_rules.sort();
        label_rules.dedup();
    }

    rules
}

fn extract_add_method_labels(
    source: &str,
    variant_labels: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let Some(add_body) = function_body(source, "pub fn add") else {
        return BTreeMap::new();
    };
    let mut methods = BTreeMap::new();
    let mut current_variant = None;

    for line in add_body.lines() {
        if let Some(variant) = constraint_token_variant(line) {
            current_variant = Some(variant);
        }
        let Some(method) = add_method_call(line) else {
            continue;
        };
        let Some(variant) = current_variant.as_ref() else {
            continue;
        };
        let Some(label) = variant_labels.get(variant) else {
            continue;
        };
        methods.insert(method, label.clone());
        current_variant = None;
    }

    methods
}

fn constraint_token_variant(line: &str) -> Option<String> {
    let start = line.find("ConstraintToken::")? + "ConstraintToken::".len();
    let rest = &line[start..];
    let end = rest
        .find(|ch: char| !ch.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn add_method_call(line: &str) -> Option<String> {
    let start = line.find("self.add_")? + "self.".len();
    let rest = &line[start..];
    let end = rest.find('(').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn function_body<'a>(source: &'a str, signature: &str) -> Option<&'a str> {
    let signature_start = source.find(signature)?;
    let open_brace = source[signature_start..]
        .find('{')
        .map(|idx| signature_start + idx)?;
    let close_brace = matching_delimiter(source, open_brace, '{', '}')?;
    Some(&source[open_brace + 1..close_brace])
}

fn string_literals(source: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut literal = String::new();
        let mut escaped = false;
        for (_, ch) in chars.by_ref() {
            if escaped {
                literal.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                break;
            } else {
                literal.push(ch);
            }
        }
        literals.push(literal.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    literals
}

fn looks_like_parser_rule(message: &str) -> bool {
    message.contains("already provided")
        || message.contains("must be provided before")
        || message.contains("cannot be used with")
        || message.contains("must be on")
        || message.contains("requires")
        || message.contains("feature")
}

fn parser_rule_kind(message: &str) -> ParserRuleKind {
    if message.contains("already provided") {
        ParserRuleKind::Duplicate
    } else if message.contains("must be provided before") {
        ParserRuleKind::Ordering
    } else if message.contains("cannot be used with") {
        ParserRuleKind::Conflict
    } else if message.contains("feature") {
        ParserRuleKind::FeatureGate
    } else if message.contains("must be on") || message.contains("requires") {
        ParserRuleKind::TypeRequirement
    } else {
        ParserRuleKind::Parser
    }
}

fn value_kind(label: &str) -> ValueKind {
    if !label.ends_with(" =") {
        return ValueKind::None;
    }
    let key = label.trim_end_matches(" =");
    if key == "seeds" {
        ValueKind::Seeds
    } else if key == "space" || key == "realloc" {
        ValueKind::Space
    } else if key == "realloc::zero" {
        ValueKind::Boolean
    } else if key == "mint::decimals" || key == "bump" {
        ValueKind::InstructionArgument
    } else if key == "rent_exempt" {
        ValueKind::Keyword
    } else if key == "owner" || key == "address" || key == "constraint" {
        ValueKind::AnyExpression
    } else if key.ends_with("token_program")
        || key.ends_with("program_id")
        || key == "seeds::program"
    {
        ValueKind::ProgramReference
    } else if key.contains("authority") || key.contains("delegate") || key.ends_with("payer") {
        ValueKind::SignerReference
    } else {
        ValueKind::AccountReference
    }
}

fn detail(label: &str, kind: ValueKind) -> &'static str {
    if let Some(detail) = match label {
        "init" => Some("Create and initialize an account."),
        "init_if_needed" => {
            Some("Create the account if needed, otherwise validate the existing account.")
        }
        "seeds =" => Some("PDA seed expression list."),
        "seeds::program =" => Some("Program id used to derive PDA seeds."),
        "bump" | "bump =" => Some("PDA bump seed."),
        "payer =" => Some("Account that pays for account creation or allocation."),
        "space =" => Some("Byte space to allocate for an initialized account."),
        "realloc =" => Some("New byte space for account reallocation."),
        "realloc::payer =" => Some("Account that pays additional rent for reallocation."),
        "realloc::zero =" => Some("Whether newly allocated account bytes are zeroed."),
        "rent_exempt =" => Some("Rent exemption mode: `skip` or `enforce`."),
        "mint::decimals =" => Some("Token mint decimals value."),
        "token::authority =" | "associated_token::authority =" | "mint::authority =" => {
            Some("Authority account for this token constraint.")
        }
        "token::token_program ="
        | "associated_token::token_program ="
        | "mint::token_program =" => Some("Token program account or program id."),
        _ => None,
    } {
        return detail;
    }
    if label.starts_with("extensions::") {
        return "Anchor Token-2022 extension constraint.";
    }
    match kind {
        ValueKind::None => "Anchor account constraint.",
        ValueKind::AnyExpression => "Anchor expression constraint.",
        ValueKind::AccountReference => "Anchor account reference constraint.",
        ValueKind::SignerReference => "Anchor signer or authority account constraint.",
        ValueKind::ProgramReference => "Anchor program id or program account constraint.",
        ValueKind::InstructionArgument => "Anchor instruction argument or expression constraint.",
        ValueKind::Keyword => "Anchor keyword constraint value.",
        ValueKind::Boolean => "Anchor boolean constraint value.",
        ValueKind::Space => "Anchor account space expression.",
        ValueKind::Seeds => "Anchor PDA seed expression list.",
    }
}

fn constraint_family(label: &str) -> ConstraintFamily {
    let key = label.trim_end_matches(" =");
    if key.starts_with("extensions::") {
        ConstraintFamily::MintExtension
    } else if key.starts_with("associated_token::") {
        ConstraintFamily::AssociatedTokenAccount
    } else if key.starts_with("token::") {
        ConstraintFamily::TokenAccount
    } else if key.starts_with("mint::") {
        ConstraintFamily::Mint
    } else if key.starts_with("realloc") {
        ConstraintFamily::Realloc
    } else if matches!(key, "seeds" | "seeds::program" | "bump") {
        ConstraintFamily::Pda
    } else {
        ConstraintFamily::Core
    }
}

fn required_companions(label: &str) -> &'static [&'static str] {
    match label.trim_end_matches(" =") {
        "init" | "init_if_needed" => &["payer", "space", "system_program"],
        "seeds" => &["bump"],
        "bump" => &["seeds"],
        "seeds::program" => &["seeds"],
        "realloc" => &["mut", "realloc::payer", "realloc::zero"],
        "close" => &["mut"],
        "token::mint" | "token::authority" => &["TokenAccount"],
        "associated_token::mint" => &["associated_token::authority", "TokenAccount"],
        "associated_token::authority" => &["associated_token::mint", "TokenAccount"],
        "associated_token::token_program" => {
            &["associated_token::mint", "associated_token::authority"]
        }
        "mint::decimals" | "mint::authority" => &["Mint"],
        _ => &[],
    }
}

fn conflicts_with(label: &str) -> &'static [&'static str] {
    match label.trim_end_matches(" =") {
        "seeds::program" => &["init"],
        // Known pure-constraint conflicts (expanded for source-driven diagnostics)
        "init" => &["seeds::program"], // reverse direction for symmetry in conflict reporting
        "zero" => &["mut"],
        "mut" => &["zero"],
        _ => &[],
    }
}

/// Returns true when the presence of this constraint on an account field
/// means the account must be declared mutable (or is created/realloced/closed).
fn implies_mutability(label: &str) -> bool {
    matches!(
        label.trim_end_matches(" ="),
        "mut"
            | "init"
            | "init_if_needed"
            | "realloc"
            | "realloc::payer"
            | "realloc::zero"
            | "close"
            | "zero"
    )
}

/// Returns true for constraints that cause an account to be created or
/// reinitialized (the "init-like" family used by initialization diagnostics).
fn is_init_like(label: &str) -> bool {
    matches!(label.trim_end_matches(" ="), "init" | "init_if_needed")
}

/// For keyword-valued constraints, the set of literal values the Anchor
/// parser accepts. Empty for non-keyword constraints.
fn allowed_keyword_values(label: &str) -> &'static [&'static str] {
    match label.trim_end_matches(" =") {
        "rent_exempt" => &["skip", "enforce"],
        _ => &[],
    }
}

fn generate_catalog(
    labels: &BTreeSet<String>,
    parser_rules: &BTreeMap<String, Vec<ParserRule>>,
) -> String {
    let mut out = String::from("&[\n");
    for label in labels {
        let kind = value_kind(label);
        out.push_str("    ConstraintSpec {\n");
        out.push_str(&format!("        label: {:?},\n", label));
        out.push_str(&format!("        detail: {:?},\n", detail(label, kind)));
        out.push_str(&format!(
            "        value_kind: ConstraintValueKind::{kind:?},\n"
        ));
        out.push_str(&format!(
            "        family: ConstraintFamily::{:?},\n",
            constraint_family(label)
        ));
        out.push_str(&format!(
            "        required_companions: &{:?},\n",
            required_companions(label)
        ));
        out.push_str(&format!(
            "        conflicts_with: &{:?},\n",
            conflicts_with(label)
        ));
        out.push_str("        parser_rules: ");
        out.push_str(&format_parser_rules(
            parser_rules.get(label).map(Vec::as_slice).unwrap_or(&[]),
        ));
        out.push_str(",\n");
        out.push_str(&format!(
            "        implies_mutability: {},\n",
            implies_mutability(label)
        ));
        out.push_str(&format!("        is_init_like: {},\n", is_init_like(label)));
        out.push_str(&format!(
            "        allowed_keyword_values: &{:?},\n",
            allowed_keyword_values(label)
        ));
        out.push_str("    },\n");
    }
    out.push_str("]\n");
    out
}

fn generate_constraint_key_lists(labels: &BTreeSet<String>) -> String {
    let mut out = String::new();

    let mutability_keys: Vec<String> = labels
        .iter()
        .filter(|label| implies_mutability(label))
        .map(|label| label.trim_end_matches(" =").to_string())
        .collect();

    let init_like_keys: Vec<String> = labels
        .iter()
        .filter(|label| is_init_like(label))
        .map(|label| label.trim_end_matches(" =").to_string())
        .collect();

    out.push_str("#[allow(dead_code)]\npub const MUTABILITY_IMPLYING_KEYS: &[&str] = &[");
    for key in &mutability_keys {
        out.push_str(&format!("{:?}, ", key));
    }
    out.push_str("];\n\n");

    out.push_str("pub const INIT_LIKE_KEYS: &[&str] = &[");
    for key in &init_like_keys {
        out.push_str(&format!("{:?}, ", key));
    }
    out.push_str("];\n");

    out
}

fn format_parser_rules(rules: &[ParserRule]) -> String {
    if rules.is_empty() {
        return "&[]".to_string();
    }

    let mut out = String::from("&[\n");
    for rule in rules {
        out.push_str("            ConstraintParserRule {\n");
        out.push_str(&format!(
            "                kind: ConstraintParserRuleKind::{:?},\n",
            rule.kind
        ));
        out.push_str(&format!("                message: {:?},\n", rule.message));
        out.push_str(&format!(
            "                source_method: {:?},\n",
            rule.method
        ));
        out.push_str("            },\n");
    }
    out.push_str("        ]");
    out
}

fn generate_error_catalog(errors: &[AnchorError]) -> String {
    let mut out = String::from("&[\n");
    for error in errors {
        out.push_str("    AnchorErrorSpec {\n");
        out.push_str(&format!("        name: {:?},\n", error.name));
        out.push_str(&format!("        code: {},\n", error.code));
        out.push_str(&format!("        message: {:?},\n", error.message));
        out.push_str(&format!(
            "        category: AnchorErrorCategory::{:?},\n",
            error_category(error.code)
        ));
        out.push_str(&format!(
            "        coverage: AnchorErrorCoverage::{:?},\n",
            error_coverage(&error.name, error.code)
        ));
        out.push_str("    },\n");
    }
    out.push_str("]\n");
    out
}

#[derive(Debug)]
enum AnchorErrorCategory {
    Instruction,
    Event,
    Constraint,
    Require,
    Account,
    Misc,
    Deprecated,
    Custom,
}

fn error_category(code: u32) -> AnchorErrorCategory {
    match code {
        100..=999 => AnchorErrorCategory::Instruction,
        1500..=1999 => AnchorErrorCategory::Event,
        2000..=2499 => AnchorErrorCategory::Constraint,
        2500..=2999 => AnchorErrorCategory::Require,
        3000..=4099 => AnchorErrorCategory::Account,
        4100..=4999 => AnchorErrorCategory::Misc,
        5000 => AnchorErrorCategory::Deprecated,
        _ => AnchorErrorCategory::Custom,
    }
}

#[derive(Debug)]
enum AnchorErrorCoverage {
    StaticCovered,
    PreflightCovered,
    RuntimeOnly,
}

fn error_coverage(name: &str, code: u32) -> AnchorErrorCoverage {
    match name {
        "ConstraintMut"
        | "ConstraintHasOne"
        | "ConstraintSeeds"
        | "ConstraintClose"
        | "ConstraintAssociated"
        | "ConstraintAssociatedInit"
        | "ConstraintTokenMint"
        | "ConstraintTokenOwner"
        | "ConstraintMintMintAuthority"
        | "ConstraintMintFreezeAuthority"
        | "ConstraintMintDecimals"
        | "ConstraintSpace"
        | "ConstraintTokenTokenProgram"
        | "ConstraintMintTokenProgram"
        | "ConstraintAssociatedTokenTokenProgram"
        | "ConstraintDuplicateMutableAccount"
        | "InvalidProgramExecutable"
        | "TryingToInitPayerAsProgramAccount"
        | "ConstraintSigner"
        | "ConstraintRaw"
        | "ConstraintOwner"
        | "ConstraintRentExempt"
        | "ConstraintExecutable"
        | "ConstraintAddress"
        | "ConstraintZero"
        | "ConstraintAccountIsNone"
        | "AccountNotMutable"
        | "AccountNotSigner"
        | "AccountNotSystemOwned"
        | "AccountNotProgramData"
        | "AccountNotAssociatedTokenAccount"
        | "AccountSysvarMismatch"
        | "AccountDuplicateReallocs" => AnchorErrorCoverage::StaticCovered,
        "DeclaredProgramIdMismatch" | "EventInstructionStub" | "Deprecated" => {
            AnchorErrorCoverage::StaticCovered
        }
        "InstructionMissing"
        | "InstructionFallbackNotFound"
        | "InstructionDidNotDeserialize"
        | "AccountDiscriminatorAlreadySet"
        | "AccountDiscriminatorNotFound"
        | "AccountDiscriminatorMismatch"
        | "AccountDidNotDeserialize"
        | "AccountNotEnoughKeys"
        | "AccountOwnedByWrongProgram"
        | "InvalidProgramId"
        | "AccountNotInitialized"
        | "AccountReallocExceedsLimit" => AnchorErrorCoverage::PreflightCovered,
        _ if (100..=199).contains(&code)
            || (2500..=4099).contains(&code)
            || name.starts_with("Instruction")
            || name.starts_with("Require") =>
        {
            AnchorErrorCoverage::RuntimeOnly
        }
        _ => AnchorErrorCoverage::StaticCovered,
    }
}
