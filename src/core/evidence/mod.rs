use {
    crate::{
        constraint_catalog::{self, ConstraintValueKind},
        document::{
            AccountConstraint, InstructionSymbol, ParsedDocument, PdaConstraint, PdaSeeds,
            SymbolRange,
        },
        range::line_at,
    },
    std::collections::HashSet,
    tower_lsp::lsp_types::{Position, Range},
};

mod summaries;

pub use summaries::summary;

#[derive(Debug)]
pub struct EvidenceGraph<'a> {
    account_sets: Vec<AccountSetEvidence<'a>>,
}

impl<'a> EvidenceGraph<'a> {
    pub fn from_document(document: &'a ParsedDocument) -> Self {
        Self::from_document_with_reachable_functions(document, |_| None)
    }

    pub fn from_document_with_reachable_functions(
        document: &'a ParsedDocument,
        reachable_functions: impl Fn(&str) -> Option<HashSet<String>>,
    ) -> Self {
        let account_sets = document
            .symbols()
            .accounts_structs
            .values()
            .map(|accounts| {
                AccountSetEvidence::new(
                    document,
                    accounts,
                    reachable_functions(&accounts.name).as_ref(),
                )
            })
            .collect();

        Self { account_sets }
    }

    pub fn account_sets(&self) -> &[AccountSetEvidence<'a>] {
        &self.account_sets
    }
}

#[derive(Debug)]
pub struct AccountSetEvidence<'a> {
    pub accounts: &'a SymbolRange,
    fields: Vec<FieldEvidence<'a>>,
    instructions: Vec<&'a InstructionSymbol>,
    account_names: HashSet<&'a str>,
    instruction_argument_names: HashSet<&'a str>,
}

impl<'a> AccountSetEvidence<'a> {
    fn new(
        document: &'a ParsedDocument,
        accounts: &'a SymbolRange,
        workspace_reachable_functions: Option<&HashSet<String>>,
    ) -> Self {
        let instructions =
            reachable_instructions_for_accounts(document, accounts, workspace_reachable_functions);

        let account_names = accounts
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<HashSet<_>>();
        let instruction_argument_names = instructions
            .iter()
            .flat_map(|instruction| instruction.arguments.iter())
            .map(|argument| argument.name.as_str())
            .chain(
                accounts
                    .instruction_arguments
                    .iter()
                    .map(|argument| argument.name.as_str()),
            )
            .collect::<HashSet<_>>();

        Self {
            accounts,
            fields: accounts
                .fields
                .iter()
                .map(|field| FieldEvidence::new(field, &instructions))
                .collect(),
            instructions,
            account_names,
            instruction_argument_names,
        }
    }

    pub fn fields(&self) -> &[FieldEvidence<'a>] {
        &self.fields
    }

    pub fn has_account(&self, name: &str) -> bool {
        self.account_names.contains(name)
    }

    pub fn account_names(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.account_names.iter().copied()
    }

    pub fn has_instruction_argument(&self, name: &str) -> bool {
        self.instruction_argument_names.contains(name)
    }

    pub fn instruction_argument_names(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.instruction_argument_names.iter().copied()
    }

    pub fn instruction_argument_type(&self, name: &str) -> Option<&'a str> {
        self.instructions
            .iter()
            .flat_map(|instruction| instruction.arguments.iter())
            .find(|argument| {
                argument.name == name
                    || argument.name.trim_start_matches('_') == name.trim_start_matches('_')
            })
            .and_then(|argument| argument.type_name.as_deref())
            .or_else(|| {
                self.accounts
                    .instruction_arguments
                    .iter()
                    .find(|argument| {
                        argument.name == name
                            || argument.name.trim_start_matches('_') == name.trim_start_matches('_')
                    })
                    .and_then(|argument| argument.type_name.as_deref())
            })
    }

    pub fn instruction_argument_location(&self, name: &str) -> Option<(&'a str, Range)> {
        self.instructions.iter().find_map(|instruction| {
            instruction
                .arguments
                .iter()
                .find(|argument| {
                    argument.name == name
                        || argument.name.trim_start_matches('_') == name.trim_start_matches('_')
                })
                .map(|argument| (instruction.name.as_str(), argument.range))
        })
    }

    pub fn has_instruction_mapping(&self) -> bool {
        !self.instructions.is_empty()
    }

    pub fn unknown_account_usages(&self) -> Vec<UnknownAccountUsageEvidence<'a>> {
        self.instructions
            .iter()
            .flat_map(|instruction| {
                instruction.account_usages.iter().filter_map(|usage| {
                    (!self.has_account(&usage.name)).then_some(UnknownAccountUsageEvidence {
                        instruction: instruction.name.as_str(),
                        account: usage.name.as_str(),
                        range: usage.range,
                    })
                })
            })
            .collect()
    }

    pub fn init_like_instruction(&self) -> Option<Option<&'a InstructionSymbol>> {
        self.instructions
            .iter()
            .copied()
            .find(|instruction| is_init_like_name(&instruction.name))
            .map(Some)
    }

    pub fn has_payer_candidate(&self) -> bool {
        self.fields.iter().any(|field| {
            field.type_name() == Some("Signer")
                || field
                    .constraints
                    .iter()
                    .any(|constraint| constraint.has_flag_or_key("signer"))
        })
    }

    pub fn has_system_program(&self) -> bool {
        self.system_program_field().is_some_and(|field| {
            field.type_name() == Some("Program") && field.has_generic_type("System")
        })
    }

    pub fn system_program_field(&self) -> Option<&FieldEvidence<'a>> {
        self.fields
            .iter()
            .find(|field| field.field.name == "system_program")
    }
}

fn reachable_instructions_for_accounts<'a>(
    document: &'a ParsedDocument,
    accounts: &SymbolRange,
    workspace_reachable_functions: Option<&HashSet<String>>,
) -> Vec<&'a InstructionSymbol> {
    let program_instructions = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        })
        .collect::<Vec<_>>();

    if let Some(reachable_names) = workspace_reachable_functions {
        let mut reachable = program_instructions;
        reachable.extend(document.symbols().functions.iter().filter(|function| {
            function
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
                && reachable_names.contains(&function.name)
        }));
        return reachable;
    }

    let helpers = document
        .symbols()
        .functions
        .iter()
        .filter(|function| {
            function
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        })
        .collect::<Vec<_>>();

    if program_instructions.is_empty() {
        return helpers;
    }

    let mut reachable = program_instructions;
    let mut seen_names = reachable
        .iter()
        .map(|instruction| instruction.name.as_str())
        .collect::<HashSet<_>>();
    let mut changed = true;
    while changed {
        changed = false;
        let called_names = reachable
            .iter()
            .flat_map(|instruction| instruction.function_calls.iter())
            .map(|call| call.name.as_str())
            .collect::<HashSet<_>>();
        for helper in &helpers {
            if called_names.contains(helper.name.as_str())
                && seen_names.insert(helper.name.as_str())
            {
                reachable.push(*helper);
                changed = true;
            }
        }
    }

    reachable
}

#[derive(Debug)]
pub struct FieldEvidence<'a> {
    pub field: &'a SymbolRange,
    constraints: Vec<ConstraintEvidence<'a>>,
    used_by_instructions: Vec<FieldUsageEvidence<'a>>,
    used_as_cpi_programs: Vec<FieldUsageEvidence<'a>>,
}

impl<'a> FieldEvidence<'a> {
    fn new(field: &'a SymbolRange, instructions: &[&'a InstructionSymbol]) -> Self {
        Self {
            field,
            constraints: field
                .account_constraints
                .iter()
                .map(ConstraintEvidence::new)
                .collect(),
            used_by_instructions: instructions
                .iter()
                .flat_map(|instruction| {
                    instruction
                        .account_usages
                        .iter()
                        .filter_map(|usage| {
                            (usage.name == field.name).then_some(FieldUsageEvidence {
                                instruction: instruction.name.as_str(),
                                range: usage.range,
                                mutable: usage.mutable,
                            })
                        })
                        .chain(
                            instruction
                                .account_data_field_usages
                                .iter()
                                .filter_map(|usage| {
                                    (usage.account == field.name).then_some(FieldUsageEvidence {
                                        instruction: instruction.name.as_str(),
                                        range: usage.range,
                                        mutable: usage.mutable,
                                    })
                                }),
                        )
                })
                .collect(),
            used_as_cpi_programs: instructions
                .iter()
                .flat_map(|instruction| {
                    instruction.cpi_program_usages.iter().filter_map(|usage| {
                        (usage.name == field.name).then_some(FieldUsageEvidence {
                            instruction: instruction.name.as_str(),
                            range: usage.range,
                            mutable: usage.mutable,
                        })
                    })
                })
                .collect(),
        }
    }

    pub fn constraints(&self) -> &[ConstraintEvidence<'a>] {
        &self.constraints
    }

    pub fn used_by_instructions(&self) -> &[FieldUsageEvidence<'a>] {
        &self.used_by_instructions
    }

    pub fn used_as_cpi_programs(&self) -> &[FieldUsageEvidence<'a>] {
        &self.used_as_cpi_programs
    }

    pub fn type_name(&self) -> Option<&str> {
        self.field.type_name.as_deref()
    }

    pub fn is_optional(&self) -> bool {
        self.field.is_optional
    }

    pub fn has_generic_type(&self, name: &str) -> bool {
        self.field
            .generic_type_names
            .iter()
            .any(|generic| generic == name)
    }

    pub fn has_any_constraint(&self, keys: &[&str]) -> bool {
        self.constraints.iter().any(|constraint| {
            keys.iter()
                .any(|key| constraint.has_flag_or_key(key) || constraint.has_key(key))
        })
    }

    pub fn has_init_constraint(&self) -> bool {
        self.constraints
            .iter()
            .any(ConstraintEvidence::has_init_like_constraint)
    }

    pub fn is_unchecked_account(&self) -> bool {
        matches!(
            self.type_name(),
            Some("AccountInfo") | Some("UncheckedAccount")
        )
    }

    pub fn pda(&self) -> Option<&PdaConstraint> {
        self.field.pda_constraint.as_ref()
    }

    pub fn seeds_are_static_only(&self) -> bool {
        if let Some(pda) = self.pda() {
            return pda.seed_values().is_some_and(|seeds| {
                !seeds.is_empty()
                    && seeds
                        .iter()
                        .all(|seed| seed.starts_with("b\"") || seed.starts_with("b'"))
            });
        }

        self.constraints
            .iter()
            .any(ConstraintEvidence::seeds_are_static_only)
    }

    pub fn seed_expressions(
        &self,
        accounts: &AccountSetEvidence<'_>,
        constraint: &ConstraintEvidence<'_>,
    ) -> Vec<SeedExpressionEvidence> {
        if let Some(pda) = self.pda() {
            return pda_seed_expressions(pda, accounts);
        }

        constraint.seed_expressions(accounts)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FieldUsageEvidence<'a> {
    pub instruction: &'a str,
    pub range: Range,
    pub mutable: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct UnknownAccountUsageEvidence<'a> {
    pub instruction: &'a str,
    pub account: &'a str,
    pub range: Range,
}

#[derive(Debug)]
pub struct ConstraintEvidence<'a> {
    pub constraint: &'a AccountConstraint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedExpressionEvidence {
    pub expression: String,
    pub kind: SeedExpressionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedExpressionKind {
    StaticBytes,
    AccountKey,
    InstructionArgument,
    Expression,
}

impl<'a> ConstraintEvidence<'a> {
    pub(crate) fn new(constraint: &'a AccountConstraint) -> Self {
        Self { constraint }
    }

    pub fn text(&self) -> &str {
        &self.constraint.text
    }

    pub fn range(&self) -> Range {
        self.constraint.range
    }

    pub fn pda(&self) -> Option<&PdaConstraint> {
        self.constraint.pda.as_ref()
    }

    pub fn has_key(&self, key: &str) -> bool {
        crate::constraint_text::has_key(self.text(), key)
    }

    pub fn has_flag_or_key(&self, key: &str) -> bool {
        crate::constraint_text::has_flag_or_key(self.text(), key)
    }

    pub fn has_init_like_constraint(&self) -> bool {
        constraint_catalog::CONSTRAINTS.iter().any(|spec| {
            spec.is_init_like && self.has_flag_or_key(constraint_catalog::key(spec.label))
        })
    }

    pub fn init_constraint_key(&self) -> Option<&'static str> {
        constraint_catalog::CONSTRAINTS
            .iter()
            .filter(|spec| spec.is_init_like)
            .map(|spec| constraint_catalog::key(spec.label))
            .find(|key| self.has_flag_or_key(key))
    }

    pub fn implies_mutability(&self) -> bool {
        constraint_catalog::CONSTRAINTS.iter().any(|spec| {
            spec.implies_mutability && self.has_flag_or_key(constraint_catalog::key(spec.label))
        })
    }

    pub fn account_references(&self) -> Vec<ConstraintReference<'_>> {
        constraint_catalog::account_reference_specs()
            .filter_map(|(key, value_kind)| {
                let name = self.account_reference_after_key(key)?;
                Some(ConstraintReference {
                    key,
                    name,
                    value_kind,
                })
            })
            .collect()
    }

    pub fn instruction_argument_references(&self) -> Vec<ConstraintArgumentReference<'_>> {
        constraint_catalog::instruction_argument_keys()
            .filter_map(|key| {
                let name = self.simple_identifier_after_key(key)?;
                Some(ConstraintArgumentReference { key, name })
            })
            .collect()
    }

    pub fn simple_identifier_value(&self, key: &str) -> Option<&str> {
        self.simple_identifier_after_key(key)
    }

    pub fn values_after_key(&self, key: &str) -> Vec<&str> {
        crate::constraint_text::values_after_key(self.text(), key)
    }

    pub fn value_range(&self, source: &str, key: &str, value: &str) -> Option<Range> {
        (self.constraint.range.start.line..=self.constraint.range.end.line)
            .find_map(|line_number| value_range_on_line(source, line_number, key, value))
    }

    pub fn seeds_are_static_only(&self) -> bool {
        if let Some(pda) = self.pda() {
            return pda.seed_values().is_some_and(|seeds| {
                !seeds.is_empty()
                    && seeds
                        .iter()
                        .all(|seed| seed.starts_with("b\"") || seed.starts_with("b'"))
            });
        }

        let Some(seed_list) = self.bracket_value("seeds") else {
            return false;
        };
        let seeds = crate::constraint_text::split_top_level(seed_list);
        !seeds.is_empty()
            && seeds
                .iter()
                .all(|seed| seed.starts_with("b\"") || seed.starts_with("b'"))
    }

    pub fn seed_expressions(
        &self,
        accounts: &AccountSetEvidence<'_>,
    ) -> Vec<SeedExpressionEvidence> {
        if let Some(pda) = self.pda() {
            return pda
                .seed_values()
                .map(|seeds| {
                    seeds
                        .iter()
                        .map(|seed| SeedExpressionEvidence {
                            expression: seed.clone(),
                            kind: classify_seed_expression(seed, accounts),
                        })
                        .collect()
                })
                .unwrap_or_else(|| {
                    vec![SeedExpressionEvidence {
                        expression: pda.seed_expression().unwrap_or_default(),
                        kind: SeedExpressionKind::Expression,
                    }]
                });
        }

        let Some(seed_list) = self.bracket_value("seeds") else {
            return Vec::new();
        };
        crate::constraint_text::split_top_level(seed_list)
            .into_iter()
            .map(|seed| SeedExpressionEvidence {
                expression: seed.to_string(),
                kind: classify_seed_expression(seed, accounts),
            })
            .collect()
    }

    fn value_after_key(&self, key: &str) -> Option<&str> {
        crate::constraint_text::value_after_key(self.text(), key)
    }

    fn account_reference_after_key(&self, key: &str) -> Option<&str> {
        let value = self.value_after_key(key)?;
        let (identifier, rest) = leading_identifier(value)?;
        let rest = rest.trim_start();

        if rest.starts_with("::") || rest.starts_with('(') {
            return None;
        }
        (rest.is_empty()
            || rest.starts_with(',')
            || rest.starts_with(')')
            || rest.starts_with(']')
            || rest.starts_with('.'))
        .then_some(identifier)
    }

    fn simple_identifier_after_key(&self, key: &str) -> Option<&str> {
        let value = self.value_after_key(key)?;
        let (identifier, rest) = leading_identifier(value)?;
        let rest = rest.trim_start();

        (rest.is_empty() || rest.starts_with(',') || rest.starts_with(')') || rest.starts_with(']'))
            .then_some(identifier)
    }

    fn bracket_value(&self, key: &str) -> Option<&str> {
        crate::constraint_text::bracket_value(self.text(), key)
    }
}

impl PdaConstraint {
    fn seed_values(&self) -> Option<&Vec<String>> {
        match &self.seeds {
            PdaSeeds::List(seeds) => Some(seeds),
            PdaSeeds::Expr(_) => None,
        }
    }

    fn seed_expression(&self) -> Option<String> {
        match &self.seeds {
            PdaSeeds::List(seeds) => Some(format!("[{}]", seeds.join(", "))),
            PdaSeeds::Expr(expr) => Some(expr.clone()),
        }
    }
}

fn pda_seed_expressions(
    pda: &PdaConstraint,
    accounts: &AccountSetEvidence<'_>,
) -> Vec<SeedExpressionEvidence> {
    pda.seed_values()
        .map(|seeds| {
            seeds
                .iter()
                .map(|seed| SeedExpressionEvidence {
                    expression: seed.clone(),
                    kind: classify_seed_expression(seed, accounts),
                })
                .collect()
        })
        .unwrap_or_else(|| {
            vec![SeedExpressionEvidence {
                expression: pda.seed_expression().unwrap_or_default(),
                kind: SeedExpressionKind::Expression,
            }]
        })
}

#[derive(Debug, Clone, Copy)]
pub struct ConstraintReference<'a> {
    pub key: &'static str,
    pub name: &'a str,
    pub value_kind: ConstraintValueKind,
}

#[derive(Debug, Clone, Copy)]
pub struct ConstraintArgumentReference<'a> {
    pub key: &'static str,
    pub name: &'a str,
}

fn is_init_like_name(name: &str) -> bool {
    matches!(
        name,
        "Init" | "Initialize" | "Create" | "CreateAccount" | "New"
    ) || name.starts_with("Init")
        || name.starts_with("Initialize")
        || name.starts_with("Create")
        || name.starts_with("init")
        || name.starts_with("initialize")
        || name.starts_with("create")
}

fn leading_identifier(value: &str) -> Option<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!is_ident_char(ch)).then_some(idx))
        .unwrap_or(value.len());
    let identifier = &value[..end];
    if !is_account_identifier(identifier) {
        return None;
    }
    Some((identifier, &value[end..]))
}

fn classify_seed_expression(seed: &str, accounts: &AccountSetEvidence<'_>) -> SeedExpressionKind {
    let seed = seed.trim();
    if seed.starts_with("b\"") || seed.starts_with("b'") {
        return SeedExpressionKind::StaticBytes;
    }

    let Some((identifier, rest)) = leading_identifier(seed) else {
        return SeedExpressionKind::Expression;
    };
    let rest = rest.trim_start();

    if accounts.has_account(identifier) && is_account_seed_path(rest) {
        return SeedExpressionKind::AccountKey;
    }

    if accounts.has_instruction_argument(identifier)
        && (rest.starts_with(".as_ref()")
            || rest.starts_with(".as_bytes()")
            || rest.starts_with(".to_le_bytes().as_ref()"))
    {
        return SeedExpressionKind::InstructionArgument;
    }

    SeedExpressionKind::Expression
}

fn is_account_seed_path(rest: &str) -> bool {
    account_seed_path_root(rest)
        .is_some_and(|root| root.is_empty() || root.split('.').all(is_account_identifier))
}

fn account_seed_path_root(rest: &str) -> Option<&str> {
    for suffix in [".key().as_ref()", ".key().as_ref", ".as_ref()"] {
        if let Some(path) = rest.strip_suffix(suffix) {
            return Some(path.trim_start_matches('.'));
        }
    }
    None
}

fn is_account_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    (first.is_ascii_alphabetic() || first == '_') && chars.all(is_ident_char)
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn value_range_on_line(source: &str, line_number: u32, key: &str, value: &str) -> Option<Range> {
    let line = line_at(source, line_number)?;
    let normalized = normalized_line(line);
    let pattern = format!("{key}={value}");
    let start = normalized.text.find(&pattern)? + key.len() + 1;
    let end = start + value.chars().count();

    Some(Range {
        start: Position {
            line: line_number,
            character: u32::try_from(*normalized.positions.get(start)?).ok()?,
        },
        end: Position {
            line: line_number,
            character: u32::try_from(
                normalized
                    .positions
                    .get(end)
                    .copied()
                    .unwrap_or_else(|| line.chars().count()),
            )
            .ok()?,
        },
    })
}

#[derive(Debug)]
struct NormalizedLine {
    text: String,
    positions: Vec<usize>,
}

fn normalized_line(line: &str) -> NormalizedLine {
    let mut text = String::new();
    let mut positions = Vec::new();

    for (character, ch) in line.chars().enumerate() {
        if !ch.is_whitespace() {
            text.push(ch);
            positions.push(character);
        }
    }

    NormalizedLine { text, positions }
}

#[cfg(test)]
mod tests;
