use super::EM_BPF;

const TEST_ELF_HEADER_LEN: usize = 64;
const TEST_ELF_SECTION_HEADER_LEN: usize = 64;
const TEST_ELF_SECTION_COUNT: usize = 3;
const TEST_ELF_SECTION_TYPE_PROGBITS: u32 = 1;
const TEST_SBF_INSTRUCTION_BYTES: usize = 8;
const TEST_SECTION_INDEX_SHSTRTAB: u16 = 2;
const TEST_TEXT_SECTION_NAME_OFFSET: u32 = 1;
const TEST_SHSTRTAB_SECTION_NAME_OFFSET: u32 = 7;
const TEST_ELF_SECTION_TYPE_STRTAB: u32 = 3;

pub(crate) fn minimal_sbf_elf() -> Vec<u8> {
    let mut header = vec![0_u8; TEST_ELF_HEADER_LEN];
    header[0..4].copy_from_slice(b"\x7fELF");
    header[4] = 2;
    header[5] = 1;
    header[6] = 1;
    header[16] = 2;
    header[18..20].copy_from_slice(&EM_BPF.to_le_bytes());
    header[60] = TEST_ELF_SECTION_COUNT as u8;
    header
}

pub(crate) fn sbf_elf_with_text_instruction_count(instruction_count: usize) -> Vec<u8> {
    sbf_elf_with_text_bytes(instruction_count * TEST_SBF_INSTRUCTION_BYTES)
}

pub(crate) fn sbf_elf_with_text_bytes(text_len: usize) -> Vec<u8> {
    let shstrtab = b"\0.text\0.shstrtab\0";
    let shstrtab_offset = TEST_ELF_HEADER_LEN;
    let text_offset = shstrtab_offset + shstrtab.len();
    let section_header_offset = text_offset + text_len;
    let mut elf =
        vec![0_u8; section_header_offset + TEST_ELF_SECTION_HEADER_LEN * TEST_ELF_SECTION_COUNT];
    elf[..TEST_ELF_HEADER_LEN].copy_from_slice(&minimal_sbf_elf());
    elf[40..48].copy_from_slice(&(section_header_offset as u64).to_le_bytes());
    elf[58..60].copy_from_slice(&(TEST_ELF_SECTION_HEADER_LEN as u16).to_le_bytes());
    elf[60..62].copy_from_slice(&(TEST_ELF_SECTION_COUNT as u16).to_le_bytes());
    elf[62..64].copy_from_slice(&TEST_SECTION_INDEX_SHSTRTAB.to_le_bytes());
    elf[shstrtab_offset..shstrtab_offset + shstrtab.len()].copy_from_slice(shstrtab);

    write_elf64_section(
        &mut elf,
        section_header_offset + TEST_ELF_SECTION_HEADER_LEN,
        TEST_TEXT_SECTION_NAME_OFFSET,
        TEST_ELF_SECTION_TYPE_PROGBITS,
        text_offset as u64,
        text_len as u64,
    );
    write_elf64_section(
        &mut elf,
        section_header_offset + TEST_ELF_SECTION_HEADER_LEN * 2,
        TEST_SHSTRTAB_SECTION_NAME_OFFSET,
        TEST_ELF_SECTION_TYPE_STRTAB,
        shstrtab_offset as u64,
        shstrtab.len() as u64,
    );
    elf
}

fn write_elf64_section(
    elf: &mut [u8],
    offset: usize,
    name_offset: u32,
    section_type: u32,
    section_offset: u64,
    section_size: u64,
) {
    elf[offset..offset + 4].copy_from_slice(&name_offset.to_le_bytes());
    elf[offset + 4..offset + 8].copy_from_slice(&section_type.to_le_bytes());
    elf[offset + 24..offset + 32].copy_from_slice(&section_offset.to_le_bytes());
    elf[offset + 32..offset + 40].copy_from_slice(&section_size.to_le_bytes());
}
