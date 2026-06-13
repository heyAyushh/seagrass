use {super::Populated, tower_lsp::lsp_types::Range};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticModel {
    pub program: Option<Populated<Program>>,
    pub instructions: Vec<Populated<Instruction>>,
    pub accounts_structs: Vec<Populated<AccountsStruct>>,
    pub error_types: Vec<Populated<ErrorType>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub id: ProgramId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramId {
    pub value: String,
    pub source_range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub name: String,
    pub context_type: Option<String>,
    pub parameters: Vec<InstructionParam>,
    pub cpi_calls: Vec<CpiCall>,
    pub signer_checks: Vec<Check>,
    pub owner_checks: Vec<Check>,
    pub discriminator_checks: Vec<Check>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountsStruct {
    pub name: String,
    pub fields: Vec<AccountField>,
    pub composite_refs: Vec<CompositeRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeRef {
    pub field_name: String,
    pub target_struct_name: String,
    pub resolved: Option<Box<AccountsStruct>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountField {
    pub name: String,
    pub source_range: Range,
    pub account_type: AccountType,
    pub constraints: Vec<Constraint>,
    pub token_interface_candidate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    RawAccountInfo,
    Account,
    UncheckedAccount,
    Interface,
    InterfaceAccount,
    Program,
    Signer,
    SystemAccount,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub key: String,
    pub value: ConstraintValue,
    pub source_range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintValue {
    AccountRef(String),
    Expression(String),
    Bool(bool),
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdaSeedSet {
    pub seeds: Vec<PdaSeed>,
    pub bump: Option<ConstraintValue>,
    pub program_id: Option<ConstraintValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdaSeed {
    Literal(Vec<u8>),
    AccountRef(String),
    Expression(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionParam {
    pub name: String,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpiCall {
    pub callee_program_ref: Option<String>,
    pub source_range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub kind: CheckKind,
    pub subject_ref: Option<String>,
    pub source_range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    Signer,
    Owner,
    Discriminator,
    KeyEquality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorType {
    pub name: String,
    pub codes: Vec<ErrorCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCode {
    pub name: String,
    pub discriminant: Option<u32>,
}
