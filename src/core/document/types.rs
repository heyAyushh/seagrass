use tower_lsp::lsp_types::Range;

#[derive(Debug, Clone)]
pub struct SymbolRange {
    pub name: String,
    pub range: Range,
    pub selection_range: Range,
    pub fields: Vec<SymbolRange>,
    pub variants: Vec<SymbolRange>,
    pub type_name: Option<String>,
    pub type_range: Option<Range>,
    /// Whitespace-normalized full Rust type text, retained for layout-sensitive
    /// users such as account-space estimation.
    pub type_signature: Option<String>,
    pub generic_type_names: Vec<String>,
    pub generic_type_ranges: Vec<NamedRange>,
    pub is_optional: bool,
    pub max_len_args: Vec<String>,
    pub account_constraints: Vec<AccountConstraint>,
    pub pda_constraint: Option<PdaConstraint>,
    pub instruction_arguments: Vec<InstructionAttributeArgument>,
    pub derive_attribute_range: Option<Range>,
    pub derive_accounts_range: Option<Range>,
    pub derive_init_space_range: Option<Range>,
    pub is_zero_copy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFieldTypeSummary {
    pub type_name: Option<String>,
    pub generic_type_names: Vec<String>,
    pub is_optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedRange {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct AccountConstraint {
    pub text: String,
    pub range: Range,
    pub pda: Option<PdaConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdaConstraint {
    pub is_init: bool,
    pub seeds: PdaSeeds,
    pub bump: PdaBump,
    pub program_seed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaSeeds {
    List(Vec<String>),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaBump {
    Canonical,
    Explicit(String),
    Missing,
}

#[derive(Debug, Clone)]
pub struct InstructionAttributeArgument {
    pub name: String,
    pub range: Range,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InstructionSymbol {
    pub name: String,
    pub range: Range,
    pub selection_range: Range,
    pub context: Option<ContextReference>,
    pub arguments: Vec<InstructionArgument>,
    pub function_calls: Vec<FunctionCall>,
    pub account_usages: Vec<AccountUsage>,
    pub account_data_field_usages: Vec<AccountDataFieldUsage>,
    pub account_path_usages: Vec<AccountPathUsage>,
    pub cpi_program_usages: Vec<AccountUsage>,
    pub signer_usages: Vec<AccountUsage>,
    pub signer_checks: Vec<AccountUsage>,
    pub account_key_comparisons: Vec<AccountKeyComparison>,
    pub token_account_unpack_usages: Vec<AccountUsage>,
}

#[derive(Debug, Clone)]
pub struct ContextReference {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct InstructionArgument {
    pub name: String,
    pub range: Range,
    /// Last path segment of the type (e.g. `Pubkey`, `Vec`) — used for the
    /// common scalar seed cases and member resolution.
    pub type_name: Option<String>,
    /// Whitespace-normalized full type (e.g. `Vec<u8>`, `[u8;32]`, `&[u8]`).
    /// Needed to distinguish byte containers that share a head segment.
    pub type_signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCall {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUsage {
    pub name: String,
    pub range: Range,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDataFieldUsage {
    pub account: String,
    pub source_account: String,
    pub field: String,
    pub range: Range,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPathUsage {
    pub segments: Vec<NamedRange>,
    pub mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountKeyComparison {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredProgramId {
    pub value: String,
    pub range: Range,
}
