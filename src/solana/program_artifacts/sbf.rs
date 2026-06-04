use {
    super::{parse_elf_summary, read_u16, read_u32, read_u64, ElfClass, ElfEndian},
    std::{fs, path::Path},
};

const BYTES_PER_MEBIBYTE: u64 = 1024 * 1024;
const MAX_SBF_ARTIFACT_BYTES: u64 = 64 * BYTES_PER_MEBIBYTE;
const ELF_SECTION_HEADER_32_LEN: u16 = 40;
const ELF_SECTION_HEADER_64_LEN: u16 = 64;
const ELF_SECTION_TYPE_STRTAB: u32 = 3;
const ELF_SECTION_NAME_TEXT: &str = ".text";
const SBF_INSTRUCTION_BYTES: u64 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbfTextInstructionCount {
    pub text_section_bytes: u64,
    pub instruction_count: u64,
}

pub fn sbf_text_instruction_count(path: &Path) -> Result<SbfTextInstructionCount, String> {
    let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
    if metadata.len() > MAX_SBF_ARTIFACT_BYTES {
        return Err(format!(
            "SBF artifact is {} bytes, above the {} byte analysis limit",
            metadata.len(),
            MAX_SBF_ARTIFACT_BYTES
        ));
    }
    parse_sbf_text_instruction_count(&fs::read(path).map_err(|err| err.to_string())?)
}

pub(super) fn parse_sbf_text_instruction_count(
    bytes: &[u8],
) -> Result<SbfTextInstructionCount, String> {
    parse_elf_summary(bytes)?;
    let section_table = ElfSectionTable::parse(bytes)?;
    let shstrtab = section_table.string_table(bytes)?;
    for section in section_table.sections(bytes)? {
        if section.name(shstrtab)? == ELF_SECTION_NAME_TEXT {
            if section.size % SBF_INSTRUCTION_BYTES != 0 {
                return Err(format!(
                    "SBF .text section is {} bytes, not aligned to {} byte instructions",
                    section.size, SBF_INSTRUCTION_BYTES
                ));
            }
            return Ok(SbfTextInstructionCount {
                text_section_bytes: section.size,
                instruction_count: section.size / SBF_INSTRUCTION_BYTES,
            });
        }
    }
    Err("SBF artifact has no .text section".to_string())
}

#[derive(Debug, Clone, Copy)]
struct ElfSectionTable {
    class: ElfClass,
    endian: ElfEndian,
    offset: u64,
    entry_size: u16,
    entry_count: u16,
    string_table_index: u16,
}

#[derive(Debug, Clone, Copy)]
struct ElfSection {
    name_offset: u32,
    section_type: u32,
    offset: u64,
    size: u64,
}

impl ElfSectionTable {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        let summary = parse_elf_summary(bytes)?;
        let offset = match summary.class {
            ElfClass::Elf32 => u64::from(read_u32(bytes, 32, summary.endian)?),
            ElfClass::Elf64 => read_u64(bytes, 40, summary.endian)?,
        };
        let entry_size = match summary.class {
            ElfClass::Elf32 => read_u16(bytes, 46, summary.endian)?,
            ElfClass::Elf64 => read_u16(bytes, 58, summary.endian)?,
        };
        let string_table_index = match summary.class {
            ElfClass::Elf32 => read_u16(bytes, 50, summary.endian)?,
            ElfClass::Elf64 => read_u16(bytes, 62, summary.endian)?,
        };
        let minimum_entry_size = match summary.class {
            ElfClass::Elf32 => ELF_SECTION_HEADER_32_LEN,
            ElfClass::Elf64 => ELF_SECTION_HEADER_64_LEN,
        };
        if entry_size < minimum_entry_size {
            return Err(format!(
                "ELF section header size {entry_size} is smaller than expected {minimum_entry_size}"
            ));
        }
        Ok(Self {
            class: summary.class,
            endian: summary.endian,
            offset,
            entry_size,
            entry_count: summary.section_count,
            string_table_index,
        })
    }

    fn sections(self, bytes: &[u8]) -> Result<Vec<ElfSection>, String> {
        (0..self.entry_count)
            .map(|index| self.section(bytes, index))
            .collect()
    }

    fn section(self, bytes: &[u8], index: u16) -> Result<ElfSection, String> {
        let offset = self
            .offset
            .checked_add(u64::from(index) * u64::from(self.entry_size))
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or_else(|| "ELF section table offset overflows usize".to_string())?;
        match self.class {
            ElfClass::Elf32 => Ok(ElfSection {
                name_offset: read_u32(bytes, offset, self.endian)?,
                section_type: read_u32(bytes, offset + 4, self.endian)?,
                offset: u64::from(read_u32(bytes, offset + 16, self.endian)?),
                size: u64::from(read_u32(bytes, offset + 20, self.endian)?),
            }),
            ElfClass::Elf64 => Ok(ElfSection {
                name_offset: read_u32(bytes, offset, self.endian)?,
                section_type: read_u32(bytes, offset + 4, self.endian)?,
                offset: read_u64(bytes, offset + 24, self.endian)?,
                size: read_u64(bytes, offset + 32, self.endian)?,
            }),
        }
    }

    fn string_table(self, bytes: &[u8]) -> Result<&[u8], String> {
        if self.string_table_index >= self.entry_count {
            return Err("ELF section string table index is outside the section table".to_string());
        }
        let section = self.section(bytes, self.string_table_index)?;
        if section.section_type != ELF_SECTION_TYPE_STRTAB {
            return Err("ELF section string table is not a STRTAB section".to_string());
        }
        section.bytes(bytes)
    }
}

impl ElfSection {
    fn bytes<'a>(self, bytes: &'a [u8]) -> Result<&'a [u8], String> {
        let start = usize::try_from(self.offset)
            .map_err(|_| "ELF section offset overflows usize".to_string())?;
        let len = usize::try_from(self.size)
            .map_err(|_| "ELF section size overflows usize".to_string())?;
        let end = start
            .checked_add(len)
            .ok_or_else(|| "ELF section range overflows usize".to_string())?;
        bytes
            .get(start..end)
            .ok_or_else(|| "ELF section extends past end of file".to_string())
    }

    fn name(self, shstrtab: &[u8]) -> Result<&str, String> {
        let start = usize::try_from(self.name_offset)
            .map_err(|_| "ELF section name offset overflows usize".to_string())?;
        let name_bytes = shstrtab
            .get(start..)
            .ok_or_else(|| "ELF section name offset is outside the string table".to_string())?;
        let end = name_bytes
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| "ELF section name is not null-terminated".to_string())?;
        std::str::from_utf8(&name_bytes[..end])
            .map_err(|_| "ELF section name is not valid UTF-8".to_string())
    }
}
