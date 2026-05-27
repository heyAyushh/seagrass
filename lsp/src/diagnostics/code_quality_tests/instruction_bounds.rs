use super::*;

#[test]
fn reports_instruction_data_bounds_even_with_unrelated_helper() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

fn helper(instruction_data: &[u8]) -> ProgramResult {
    validate_instruction_data(instruction_data)?;
    Ok(())
}

pub fn process(instruction_data: &[u8]) -> ProgramResult {
    let tag = instruction_data[0];
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "instruction-data-bounds");
}

#[test]
fn ignores_instruction_bounds_text_in_comments_strings_and_attributes() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

#[doc = "instruction_data[0]"]
pub fn process(_instruction_data: &[u8]) -> ProgramResult {
    // instruction_data[0]
    let _template = "data[0]";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "instruction-data-bounds");
}

#[test]
fn ignores_unrelated_local_vector_named_data_index() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

pub fn process() -> ProgramResult {
    let data = vec![1_u8];
    let tag = data[0];
    let _ = tag;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "instruction-data-bounds");
}
