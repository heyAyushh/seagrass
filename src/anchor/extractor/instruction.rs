use crate::{
    document::{AccountUsage, AnchorSymbols, InstructionSymbol},
    semantic::{
        Check, CheckKind, CpiCall, ExtractionConfidence, Instruction, InstructionParam, Populated,
        PopulatedFields,
    },
};

pub fn extract_instruction(
    instruction: &InstructionSymbol,
    _symbols: &AnchorSymbols,
) -> Populated<Instruction> {
    let signer_checks = checks_from_usages(&instruction.signer_checks, CheckKind::Signer);
    let owner_checks = checks_from_usages(&[], CheckKind::Owner);
    let discriminator_checks = checks_from_usages(&[], CheckKind::Discriminator);
    let cpi_calls = instruction
        .cpi_program_usages
        .iter()
        .map(|usage| CpiCall {
            callee_program_ref: Some(usage.name.clone()),
            source_range: usage.range,
        })
        .collect::<Vec<_>>();
    let mut populated_fields = PopulatedFields::default();
    populated_fields.set(PopulatedFields::SIGNER_CHECKS);
    if !cpi_calls.is_empty() {
        populated_fields.set(PopulatedFields::CPI_CALLS);
    }
    Populated::with_fields(
        Instruction {
            name: instruction.name.clone(),
            context_type: instruction
                .context
                .as_ref()
                .map(|context| context.name.clone()),
            parameters: instruction
                .arguments
                .iter()
                .map(|argument| InstructionParam {
                    name: argument.name.clone(),
                    type_name: argument
                        .type_signature
                        .clone()
                        .or(argument.type_name.clone()),
                })
                .collect(),
            cpi_calls,
            signer_checks,
            owner_checks,
            discriminator_checks,
        },
        ExtractionConfidence::MacroAnnounced,
        populated_fields,
    )
}

fn checks_from_usages(usages: &[AccountUsage], kind: CheckKind) -> Vec<Check> {
    usages
        .iter()
        .map(|usage| Check {
            kind,
            subject_ref: Some(usage.name.clone()),
            source_range: usage.range,
        })
        .collect()
}
