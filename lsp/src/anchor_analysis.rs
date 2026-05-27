//! Deepened AnchorAnalysis seam — owns all Anchor semantics (PDA seeds, constraints,
//! account relationships, security smells, instruction account info) behind a narrow,
//! query-based interface. Callers ask questions; no tree walks or duplicated logic outside
//! this module.

use std::{collections::HashMap, sync::Arc};

use crate::document::{AnchorSymbols, ParsedDocument, PdaConstraint, PdaSeeds, SymbolRange};

/// One seed in a PDA derivation path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdaSeed {
    pub name: String,
    pub kind: PdaSeedKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaSeedKind {
    /// A literal byte string: `b"seed"` or `"seed"`
    Static(String),
    /// A program account reference: `some_account.key()`
    AccountKey(String),
    /// The canonical `bump` seed
    Bump,
    /// The current program id
    ProgramId,
    /// A variable or expression reference
    Variable(String),
}

/// One Anchor constraint on a accounts-struct field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorConstraint {
    pub kind: ConstraintKind,
    pub field_name: String,
    pub struct_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    Init {
        payer: Option<String>,
        space: Option<String>,
    },
    InitIfNeeded {
        payer: Option<String>,
        space: Option<String>,
    },
    Mut,
    HasOne {
        target: String,
    },
    BelongsTo {
        target: String,
    },
    Seeds {
        seeds: Vec<PdaSeed>,
    },
    Close {
        target: String,
    },
    Realloc {
        payer: Option<String>,
        space: Option<String>,
    },
    Signer,
    Address,
    Owner,
    Executable,
    TokenMint {
        mint: Option<String>,
    },
    TokenAuthority {
        authority: Option<String>,
    },
    AssociatedToken {
        mint: Option<String>,
        authority: Option<String>,
    },
    Zero,
}

/// Directed edge between two account fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRelationship {
    pub from: String,
    pub to: String,
    pub kind: RelationshipKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipKind {
    /// `payer = some_account`
    Payer,
    /// `has_one = some_account` or `belongs_to = some_account`
    HasOne,
    /// `close = some_account`
    Close,
    /// Token program / associated token program reference
    TokenProgram,
    /// `mint::token_program` or `token::token_program`
    AssociatedToken,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountRelationshipGraph {
    pub relationships: Vec<AccountRelationship>,
}

/// A potential security issue surfaced from account constraint analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct SecuritySmell {
    pub kind: SecuritySmellKind,
    pub account: String,
    pub instruction: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SecuritySmellKind {
    MissingSigner,
    MissingMut,
    UncheckedAccount,
    DuplicateAccountType,
    InvalidSysvar,
    CpiWithoutValidation,
    PdaWithoutSeeds,
}

/// Per-field account info for an instruction, derived from constraints.
#[derive(Debug, Clone, PartialEq)]
pub struct InstructionAccountInfo {
    pub is_signer: bool,
    pub is_mutable: bool,
    pub constraint_kinds: Vec<ConstraintKind>,
    pub pda_seeds: Option<Vec<PdaSeed>>,
}

/// Narrow interface for all Anchor analysis. Each query method delegates to the
/// private [`Model`] which is built once from [`ParsedDocument`].
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorAnalysis {
    model: Arc<Model>,
}

// Salsa requires an Update impl for tracked-function return types.
// AnchorAnalysis is an opaque cache keyed by its contents; always accept a new value.
unsafe impl salsa::Update for AnchorAnalysis {
    unsafe fn maybe_update(_old_pointer: *mut Self, _new_value: Self) -> bool {
        true
    }
}

#[allow(dead_code)]
impl AnchorAnalysis {
    /// Creates the analysis by parsing `source` and extracting all Anchor semantics.
    /// This is intended to be called from a Salsa tracked function in [`crate::salsa_db`].
    pub fn from_source(source: Arc<str>) -> Arc<Self> {
        let document = match ParsedDocument::parse(source.as_ref()) {
            Ok(doc) => doc,
            Err(_) => {
                return Arc::new(Self {
                    model: Arc::new(Model::empty()),
                });
            }
        };
        let model = Model::build(&document);
        Arc::new(Self {
            model: Arc::new(model),
        })
    }

    /// Returns all PDA seeds declared for an account field (matched by name).
    pub fn pda_seeds_for(&self, account: &str) -> Vec<PdaSeed> {
        self.model.pda_seeds_for(account)
    }

    /// Returns all constraints on a given accounts struct (keyed by struct name).
    pub fn constraints_for(&self, struct_name: &str) -> Vec<AnchorConstraint> {
        self.model.constraints_for(struct_name)
    }

    /// Returns the complete account relationship graph across all structs.
    pub fn account_relationships(&self) -> AccountRelationshipGraph {
        self.model.account_relationships()
    }

    /// Returns security smells detected from constraint and usage analysis.
    pub fn security_smells(&self) -> Vec<SecuritySmell> {
        self.model.security_smells.clone()
    }

    /// Returns per-field info for a specific account in a specific instruction.
    pub fn instruction_account_info(
        &self,
        instruction: &str,
        account: &str,
    ) -> Option<InstructionAccountInfo> {
        self.model
            .instruction_account_info
            .get(&(instruction.to_string(), account.to_string()))
            .cloned()
    }
}

#[derive(Debug, PartialEq)]
struct Model {
    /// PDA seeds per account field, keyed by (struct_name, field_name)
    pda_seeds: HashMap<(String, String), Vec<PdaSeed>>,
    /// All constraints per struct
    constraints: HashMap<String, Vec<AnchorConstraint>>,
    /// Account relationships across all structs
    relationships: AccountRelationshipGraph,
    /// Detected security smells
    security_smells: Vec<SecuritySmell>,
    /// Instruction account info, keyed by (instruction_name, field_name)
    instruction_account_info: HashMap<(String, String), InstructionAccountInfo>,
}

impl Model {
    fn empty() -> Self {
        Self {
            pda_seeds: HashMap::new(),
            constraints: HashMap::new(),
            relationships: AccountRelationshipGraph::default(),
            security_smells: Vec::new(),
            instruction_account_info: HashMap::new(),
        }
    }

    fn build(document: &ParsedDocument) -> Self {
        let mut model = Self::empty();
        let symbols = document.symbols();
        model.extract_from_symbols(symbols);
        model
    }

    fn pda_seeds_for(&self, account_field: &str) -> Vec<PdaSeed> {
        self.pda_seeds
            .iter()
            .filter(|((_, field), _)| field.as_str() == account_field)
            .flat_map(|(_, seeds)| seeds.clone())
            .collect()
    }

    fn constraints_for(&self, struct_name: &str) -> Vec<AnchorConstraint> {
        self.constraints
            .get(struct_name)
            .cloned()
            .unwrap_or_default()
    }

    fn account_relationships(&self) -> AccountRelationshipGraph {
        self.relationships.clone()
    }

    // ── extraction ───────────────────────────────────────────────────────

    fn extract_from_symbols(&mut self, symbols: &AnchorSymbols) {
        for (struct_name, struct_symbol) in &symbols.accounts_structs {
            let mut struct_constraints: Vec<AnchorConstraint> = Vec::new();

            for field in &struct_symbol.fields {
                let field_name = field.name.clone();

                // Parse constraints from #[account(...)] attributes
                for constraint in &field.account_constraints {
                    let text = &constraint.text;
                    let kinds = parse_constraint_kinds(
                        text,
                        &field_name,
                        struct_name,
                        &mut self.relationships,
                    );
                    for kind in kinds {
                        struct_constraints.push(AnchorConstraint {
                            kind: kind.clone(),
                            field_name: field_name.clone(),
                            struct_name: struct_name.clone(),
                        });
                    }
                }

                // Collect PDA seeds from parser-backed PdaConstraint
                if let Some(pda) = &field.pda_constraint {
                    let seeds = convert_pda_seeds(pda);
                    if !seeds.is_empty() {
                        self.pda_seeds
                            .insert((struct_name.clone(), field_name.clone()), seeds);
                    }
                }

                // Record signer flags
                let is_signer = field
                    .account_constraints
                    .iter()
                    .any(|c| is_signer_constraint(&c.text));

                if field_has_type_marker(field, "Signer") && !is_signer {
                    self.security_smells.push(SecuritySmell {
                        kind: SecuritySmellKind::MissingSigner,
                        account: field_name.clone(),
                        instruction: None,
                        message: format!(
                            "`{}` is typed `Signer` in `{}` but has no `#[account(signer)]` constraint",
                            field_name, struct_name
                        ),
                    });
                }
            }

            if !struct_constraints.is_empty() {
                self.constraints
                    .insert(struct_name.clone(), struct_constraints);
            }
        }

        // Populate instruction_account_info from context references
        for instruction in &symbols.instructions {
            let Some(ctx) = &instruction.context else {
                continue;
            };
            let Some(accounts_struct) = symbols.accounts_structs.get(&ctx.name) else {
                continue;
            };

            for field in &accounts_struct.fields {
                let is_signer = field
                    .account_constraints
                    .iter()
                    .any(|c| is_signer_constraint(&c.text));
                let is_mutable = field
                    .account_constraints
                    .iter()
                    .any(|c| is_mut_constraint(&c.text));
                let pda_seeds: Option<Vec<PdaSeed>> =
                    field.pda_constraint.as_ref().map(convert_pda_seeds);
                let constraint_kinds: Vec<ConstraintKind> = field
                    .account_constraints
                    .iter()
                    .flat_map(|c| {
                        parse_constraint_kinds(
                            &c.text,
                            &field.name,
                            &ctx.name,
                            &mut AccountRelationshipGraph::default(),
                        )
                    })
                    .collect();

                self.instruction_account_info.insert(
                    (instruction.name.clone(), field.name.clone()),
                    InstructionAccountInfo {
                        is_signer,
                        is_mutable,
                        constraint_kinds,
                        pda_seeds,
                    },
                );
            }
        }
    }
}

/// Parses all constraint kinds from a single `#[account(...)]` text blob.
fn parse_constraint_kinds(
    text: &str,
    field_name: &str,
    _struct_name: &str,
    relationships: &mut AccountRelationshipGraph,
) -> Vec<ConstraintKind> {
    let mut kinds = Vec::new();

    if has_token(text, "init") {
        let payer = extract_simple_value(text, "payer");
        let space = extract_simple_value(text, "space");
        kinds.push(ConstraintKind::Init { payer, space });
    }
    if has_token(text, "init_if_needed") {
        let payer = extract_simple_value(text, "payer");
        let space = extract_simple_value(text, "space");
        kinds.push(ConstraintKind::InitIfNeeded { payer, space });
    }
    if has_token(text, "mut") {
        kinds.push(ConstraintKind::Mut);
    }
    if has_token(text, "zero") {
        kinds.push(ConstraintKind::Zero);
    }
    if has_token(text, "signer") {
        kinds.push(ConstraintKind::Signer);
    }
    if has_token(text, "address") {
        kinds.push(ConstraintKind::Address);
    }
    if has_token(text, "owner") {
        kinds.push(ConstraintKind::Owner);
    }
    if has_token(text, "executable") {
        kinds.push(ConstraintKind::Executable);
    }
    if has_token(text, "has_one") {
        if let Some(target) = extract_simple_value(text, "has_one") {
            kinds.push(ConstraintKind::HasOne {
                target: target.clone(),
            });
            relationships.relationships.push(AccountRelationship {
                from: field_name.to_string(),
                to: target,
                kind: RelationshipKind::HasOne,
            });
        }
    }
    if has_token(text, "belongs_to") {
        if let Some(target) = extract_simple_value(text, "belongs_to") {
            kinds.push(ConstraintKind::BelongsTo {
                target: target.clone(),
            });
            relationships.relationships.push(AccountRelationship {
                from: field_name.to_string(),
                to: target,
                kind: RelationshipKind::HasOne,
            });
        }
    }
    if has_token(text, "close") {
        if let Some(target) = extract_simple_value(text, "close") {
            kinds.push(ConstraintKind::Close {
                target: target.clone(),
            });
            relationships.relationships.push(AccountRelationship {
                from: field_name.to_string(),
                to: target,
                kind: RelationshipKind::Close,
            });
        }
    }
    if has_token(text, "seeds") {
        let seeds = extract_pda_seeds(text);
        if !seeds.is_empty() {
            kinds.push(ConstraintKind::Seeds {
                seeds: seeds.clone(),
            });
        }
    }
    if has_token(text, "realloc") {
        let payer = extract_simple_value(text, "realloc::payer")
            .or_else(|| extract_simple_value(text, "payer"));
        let space = extract_simple_value(text, "realloc");
        kinds.push(ConstraintKind::Realloc { payer, space });
    }
    if has_token(text, "token::mint") {
        let mint = extract_simple_value(text, "token::mint");
        kinds.push(ConstraintKind::TokenMint { mint });
    }
    if has_token(text, "token::authority") {
        let authority = extract_simple_value(text, "token::authority");
        kinds.push(ConstraintKind::TokenAuthority { authority });
    }
    if has_token(text, "associated_token::mint") {
        let mint = extract_simple_value(text, "associated_token::mint");
        kinds.push(ConstraintKind::AssociatedToken {
            mint,
            authority: None,
        });
    }
    if has_token(text, "associated_token::authority") {
        let authority = extract_simple_value(text, "associated_token::authority");
        // If we already pushed an AssociatedToken from mint, update it
        if let Some(last) = kinds.last_mut() {
            if let ConstraintKind::AssociatedToken { authority: a, .. } = last {
                *a = authority;
            } else {
                kinds.push(ConstraintKind::AssociatedToken {
                    mint: None,
                    authority,
                });
            }
        }
    }

    // Payer relationship from init/realloc
    if let Some(payer) = extract_simple_value(text, "payer") {
        let already_has_close = kinds
            .iter()
            .any(|k| matches!(k, ConstraintKind::Close { .. }));
        if !already_has_close {
            relationships.relationships.push(AccountRelationship {
                from: field_name.to_string(),
                to: payer,
                kind: RelationshipKind::Payer,
            });
        }
    }
    if let Some(payer) = extract_simple_value(text, "realloc::payer") {
        relationships.relationships.push(AccountRelationship {
            from: field_name.to_string(),
            to: payer,
            kind: RelationshipKind::Payer,
        });
    }

    // Token program relationships
    for key in &["token::token_program", "mint::token_program"] {
        if let Some(prog) = extract_simple_value(text, key) {
            relationships.relationships.push(AccountRelationship {
                from: field_name.to_string(),
                to: prog,
                kind: RelationshipKind::TokenProgram,
            });
        }
    }
    if has_token(text, "associated_token::token_program") {
        if let Some(prog) = extract_simple_value(text, "associated_token::token_program") {
            relationships.relationships.push(AccountRelationship {
                from: field_name.to_string(),
                to: prog,
                kind: RelationshipKind::AssociatedToken,
            });
        }
    }

    kinds
}

/// Detects a bare flag token in constraint text.
fn has_token(text: &str, token: &str) -> bool {
    crate::constraint_text::has_flag_or_key(text, token)
}

/// Extracts a simple identifier value after `key = `, returning only the first
/// identifier before any `,`, `)`, `.`, `::`, or `[`.
fn extract_simple_value(text: &str, key: &str) -> Option<String> {
    let value = crate::constraint_text::value_after_key(text, key)?;
    let end = value.find([',', ')', ']', '.', ':']).unwrap_or(value.len());
    let result = value[..end].trim();
    if result.is_empty() || result == ")" {
        return None;
    }
    Some(result.to_string())
}

/// Extracts PDA seeds from constraint text, returning classified PdaSeed values.
fn extract_pda_seeds(text: &str) -> Vec<PdaSeed> {
    let seeds_text = match crate::constraint_text::bracket_value(text, "seeds") {
        Some(text) => text,
        None => return Vec::new(),
    };

    crate::constraint_text::split_top_level(seeds_text)
        .into_iter()
        .map(classify_seed)
        .collect()
}

fn classify_seed(seed: &str) -> PdaSeed {
    let seed = seed.trim();
    if seed == "bump" {
        return PdaSeed {
            name: "bump".into(),
            kind: PdaSeedKind::Bump,
        };
    }
    if seed == "program_id" || seed == "crate::ID" || seed == "crate::id" || seed == "id" {
        return PdaSeed {
            name: "program_id".into(),
            kind: PdaSeedKind::ProgramId,
        };
    }
    if seed.contains(".key()") {
        let name = seed.split('.').next().unwrap_or(seed).trim().to_string();
        return PdaSeed {
            name: name.clone(),
            kind: PdaSeedKind::AccountKey(name),
        };
    }
    if seed.starts_with("b\"") || seed.starts_with("b'") || seed.starts_with('"') {
        return PdaSeed {
            name: "static".into(),
            kind: PdaSeedKind::Static(seed.to_string()),
        };
    }
    PdaSeed {
        name: seed.to_string(),
        kind: PdaSeedKind::Variable(seed.to_string()),
    }
}

fn convert_pda_seeds(pda: &PdaConstraint) -> Vec<PdaSeed> {
    match &pda.seeds {
        PdaSeeds::List(seeds) => seeds.iter().map(|s| classify_seed(s)).collect(),
        PdaSeeds::Expr(_) => Vec::new(),
    }
}

fn is_signer_constraint(text: &str) -> bool {
    has_token(text, "signer")
}

fn is_mut_constraint(text: &str) -> bool {
    has_token(text, "mut") || has_token(text, "zero")
}

fn field_has_type_marker(field: &SymbolRange, marker: &str) -> bool {
    field.type_name.as_deref() == Some(marker)
        || field
            .generic_type_names
            .iter()
            .any(|g| g.as_str() == marker)
}
