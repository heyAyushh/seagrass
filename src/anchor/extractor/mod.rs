mod accounts_struct;
mod constraints;
mod instruction;

use crate::{
    document::ParsedDocument,
    semantic::{
        ErrorCode, ErrorType, ExtractionConfidence, Populated, Program, ProgramId, SemanticModel,
    },
};

pub use accounts_struct::{extract_account_field, extract_accounts_structs, pda_seed_set};
pub use constraints::extract_constraints;

pub fn extract(document: &ParsedDocument) -> SemanticModel {
    let symbols = document.symbols();
    SemanticModel {
        program: symbols.declared_program_id.as_ref().map(|declared| {
            Populated::new(
                Program {
                    id: ProgramId {
                        value: declared.value.clone(),
                        source_range: declared.range,
                    },
                },
                ExtractionConfidence::MacroAnnounced,
            )
        }),
        instructions: symbols
            .instructions
            .iter()
            .map(|instruction| instruction::extract_instruction(instruction, symbols))
            .collect(),
        accounts_structs: accounts_struct::extract_accounts_structs(document),
        error_types: symbols
            .error_codes
            .iter()
            .map(|error_code| {
                Populated::new(
                    ErrorType {
                        name: error_code.name.clone(),
                        codes: error_code
                            .variants
                            .iter()
                            .map(|variant| ErrorCode {
                                name: variant.clone(),
                                discriminant: None,
                            })
                            .collect(),
                    },
                    ExtractionConfidence::MacroAnnounced,
                )
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests;
