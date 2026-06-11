#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Program,
    Instruction,
    AccountsStruct,
    AccountField,
    CompositeRef,
    Constraint,
    PdaSeedSet,
    CpiCall,
    Check,
    ErrorType,
    ErrorCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Populated<T> {
    pub inner: T,
    pub populated_fields: PopulatedFields,
    pub extraction_confidence: ExtractionConfidence,
}

impl<T> Populated<T> {
    pub fn new(inner: T, extraction_confidence: ExtractionConfidence) -> Self {
        Self {
            inner,
            populated_fields: PopulatedFields::default(),
            extraction_confidence,
        }
    }

    pub fn with_fields(
        inner: T,
        extraction_confidence: ExtractionConfidence,
        populated_fields: PopulatedFields,
    ) -> Self {
        Self {
            inner,
            populated_fields,
            extraction_confidence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionConfidence {
    MacroAnnounced,
    IdiomBased,
    Partial,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PopulatedFields(pub u64);

impl PopulatedFields {
    pub const COMPOSITE_REFS_RESOLVED: u64 = 1 << 0;
    pub const SIGNER_CHECKS: u64 = 1 << 1;
    pub const OWNER_CHECKS: u64 = 1 << 2;
    pub const DISCRIMINATOR_CHECKS: u64 = 1 << 3;
    pub const PDA_SEEDS: u64 = 1 << 4;
    pub const CPI_CALLS: u64 = 1 << 5;
    pub const ERROR_DISCRIMINANTS: u64 = 1 << 6;

    pub fn has(&self, flag: u64) -> bool {
        self.0 & flag != 0
    }

    pub fn set(&mut self, flag: u64) {
        self.0 |= flag;
    }
}
