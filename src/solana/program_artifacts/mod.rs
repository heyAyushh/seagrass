use {
    crate::{
        document::ParsedDocument,
        ecosystem, project,
        solana_project::{self, SolanaProgram, SolanaProjectKind},
    },
    serde_json::Value,
    std::{
        fs::{self, File},
        io::Read,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    },
    tower_lsp::lsp_types::Url,
};

const MAX_SOURCE_INPUTS: usize = 512;
const MAX_SOURCE_DEPTH: usize = 16;
const EM_BPF: u16 = 247;
const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramArtifactReport {
    pub program: SolanaProgram,
    pub root: PathBuf,
    pub source_root: Option<PathBuf>,
    pub source_inputs: Vec<ArtifactInput>,
    pub deploy_path: PathBuf,
    pub keypair_path: PathBuf,
    pub idl_path: PathBuf,
    pub types_path: PathBuf,
    pub deploy: DeployArtifactState,
    pub keypair: ProgramKeypairState,
    pub idl: IdlArtifactState,
    pub typescript: FilePresence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInput {
    pub path: PathBuf,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFile {
    pub path: PathBuf,
    pub byte_len: u64,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployArtifactState {
    Missing,
    Present { file: ArtifactFile, elf: ElfSummary },
    Invalid { file: ArtifactFile, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramKeypairState {
    Missing,
    Present {
        file: ArtifactFile,
        public_key: String,
    },
    Invalid {
        file: ArtifactFile,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdlArtifactState {
    Missing,
    Present {
        file: ArtifactFile,
        program_name: Option<String>,
        address: Option<String>,
    },
    Invalid {
        file: ArtifactFile,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePresence {
    Missing,
    Present(ArtifactFile),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfSummary {
    pub class: ElfClass,
    pub endian: ElfEndian,
    pub machine: u16,
    pub entry: u64,
    pub section_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfClass {
    Elf32,
    Elf64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfEndian {
    Little,
    Big,
}

impl ProgramArtifactReport {
    pub fn build_surface_exists(&self) -> bool {
        self.root.join("target/deploy").is_dir()
            || self.root.join("target/idl").is_dir()
            || self.root.join("target/types").is_dir()
            || !matches!(self.deploy, DeployArtifactState::Missing)
            || !matches!(self.keypair, ProgramKeypairState::Missing)
            || !matches!(self.idl, IdlArtifactState::Missing)
            || !matches!(self.typescript, FilePresence::Missing)
    }

    pub fn deploy_file(&self) -> Option<&ArtifactFile> {
        match &self.deploy {
            DeployArtifactState::Present { file, .. }
            | DeployArtifactState::Invalid { file, .. } => Some(file),
            DeployArtifactState::Missing => None,
        }
    }

    pub fn idl_file(&self) -> Option<&ArtifactFile> {
        match &self.idl {
            IdlArtifactState::Present { file, .. } | IdlArtifactState::Invalid { file, .. } => {
                Some(file)
            }
            IdlArtifactState::Missing => None,
        }
    }

    pub fn deploy_stale_inputs(&self) -> Vec<&ArtifactInput> {
        match &self.deploy {
            DeployArtifactState::Present { file, .. } => self.inputs_newer_than(file.modified),
            DeployArtifactState::Missing | DeployArtifactState::Invalid { .. } => Vec::new(),
        }
    }

    pub fn idl_stale_inputs(&self) -> Vec<&ArtifactInput> {
        match &self.idl {
            IdlArtifactState::Present { file, .. } => self.inputs_newer_than(file.modified),
            IdlArtifactState::Missing | IdlArtifactState::Invalid { .. } => Vec::new(),
        }
    }

    pub fn typescript_stale_inputs(&self) -> Vec<&ArtifactInput> {
        match &self.typescript {
            FilePresence::Present(file) => self.inputs_newer_than(file.modified),
            FilePresence::Missing => Vec::new(),
        }
    }

    pub fn to_json(&self) -> Value {
        let idl_applicable = self.idl_applicable();
        let typescript_applicable = self.typescript_applicable();
        serde_json::json!({
            "program": {
                "cluster": self.program.cluster,
                "name": self.program.name,
                "id": self.program.id,
                "programId": self.program.id,
                "kind": self.program.kind.as_str(),
                "framework": self.program.kind.label(),
            },
            "root": path_to_string(&self.root),
            "paths": {
                "manifest": self
                    .program
                    .metadata_uri
                    .to_file_path()
                    .ok()
                    .map(|path| path_to_string(&path)),
                "sourceRoot": self.source_root.as_ref().map(|path| path_to_string(path)),
                "deploy": path_to_string(&self.deploy_path),
                "keypair": path_to_string(&self.keypair_path),
                "idl": idl_applicable.then(|| path_to_string(&self.idl_path)),
                "typescript": typescript_applicable.then(|| path_to_string(&self.types_path)),
            },
            "buildSurface": self.build_surface_exists(),
            "inputs": {
                "newestModifiedMs": self.newest_input_modified().and_then(system_time_ms),
                "files": self.source_inputs.iter().map(|input| {
                    serde_json::json!({
                        "path": path_to_string(&input.path),
                        "modifiedMs": system_time_ms(input.modified),
                    })
                }).collect::<Vec<_>>(),
            },
            "deploy": deploy_json(&self.deploy),
            "keypair": keypair_json(&self.keypair),
            "idl": idl_json(&self.idl, idl_applicable),
            "typescript": presence_json(&self.typescript, typescript_applicable),
            "ecosystem": ecosystem::report_for_program(&self.program).to_json(),
            "stale": {
                "deploy": stale_inputs_json(self.deploy_stale_inputs()),
                "idl": stale_inputs_json(self.idl_stale_inputs()),
                "typescript": stale_inputs_json(self.typescript_stale_inputs()),
            },
        })
    }

    fn inputs_newer_than(&self, modified: SystemTime) -> Vec<&ArtifactInput> {
        self.source_inputs
            .iter()
            .filter(|input| input.modified > modified)
            .collect()
    }

    fn newest_input_modified(&self) -> Option<SystemTime> {
        self.source_inputs.iter().map(|input| input.modified).max()
    }

    pub fn idl_applicable(&self) -> bool {
        self.program.kind == SolanaProjectKind::Anchor
            || !matches!(self.idl, IdlArtifactState::Missing)
            || self.root.join("target/idl").is_dir()
    }

    pub fn typescript_applicable(&self) -> bool {
        self.program.kind == SolanaProjectKind::Anchor
            || !matches!(self.typescript, FilePresence::Missing)
            || self.root.join("target/types").is_dir()
    }
}

pub fn report_for_document(uri: &Url, document: &ParsedDocument) -> Option<ProgramArtifactReport> {
    let program = solana_project::detect_for_document(uri, document)?;
    report_for_program(program)
}

pub fn reports_for_roots(roots: &[Url]) -> Vec<ProgramArtifactReport> {
    let mut reports = solana_project::detect_for_roots(roots)
        .into_iter()
        .filter_map(report_for_program)
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| {
        left.program
            .name
            .cmp(&right.program.name)
            .then_with(|| left.program.cluster.cmp(&right.program.cluster))
            .then_with(|| left.root.cmp(&right.root))
    });
    reports.dedup_by(|left, right| {
        left.program.name == right.program.name
            && left.program.cluster == right.program.cluster
            && left.root == right.root
    });
    reports
}

pub fn report_for_program(program: SolanaProgram) -> Option<ProgramArtifactReport> {
    let root = program.root.clone();
    let deploy_path = root
        .join("target")
        .join("deploy")
        .join(format!("{}.so", program.name));
    let keypair_path = root
        .join("target")
        .join("deploy")
        .join(format!("{}-keypair.json", program.name));
    let idl_path = root
        .join("target")
        .join("idl")
        .join(format!("{}.json", program.name));
    let types_path = root
        .join("target")
        .join("types")
        .join(format!("{}.ts", program.name));
    let source_root = program.source_root.clone();
    let source_inputs = source_root
        .as_ref()
        .map(|source_root| collect_source_inputs(source_root, &program.name))
        .unwrap_or_default();

    Some(ProgramArtifactReport {
        program,
        root,
        source_root,
        source_inputs,
        deploy: deploy_state(&deploy_path),
        keypair: keypair_state(&keypair_path),
        idl: idl_state(&idl_path),
        typescript: file_presence(&types_path),
        deploy_path,
        keypair_path,
        idl_path,
        types_path,
    })
}

pub fn parse_elf_summary(bytes: &[u8]) -> Result<ElfSummary, String> {
    if bytes.len() < 20 {
        return Err("ELF header is truncated".to_string());
    }
    if &bytes[..4] != b"\x7fELF" {
        return Err("file does not start with an ELF header".to_string());
    }
    let class = match bytes[4] {
        1 => ElfClass::Elf32,
        2 => ElfClass::Elf64,
        other => return Err(format!("unsupported ELF class {other}")),
    };
    let endian = match bytes[5] {
        1 => ElfEndian::Little,
        2 => ElfEndian::Big,
        other => return Err(format!("unsupported ELF endianness {other}")),
    };
    if bytes.get(6).copied() != Some(1) {
        return Err("unsupported ELF version".to_string());
    }

    let minimum_len = match class {
        ElfClass::Elf32 => 52,
        ElfClass::Elf64 => 64,
    };
    if bytes.len() < minimum_len {
        return Err("ELF header is truncated".to_string());
    }

    let machine = read_u16(bytes, 18, endian)?;
    if machine != EM_BPF {
        return Err(format!(
            "ELF machine {machine} is not eBPF/SBF (expected EM_BPF 247)"
        ));
    }

    let entry = match class {
        ElfClass::Elf32 => u64::from(read_u32(bytes, 24, endian)?),
        ElfClass::Elf64 => read_u64(bytes, 24, endian)?,
    };
    let section_count = match class {
        ElfClass::Elf32 => read_u16(bytes, 48, endian)?,
        ElfClass::Elf64 => read_u16(bytes, 60, endian)?,
    };

    Ok(ElfSummary {
        class,
        endian,
        machine,
        entry,
        section_count,
    })
}

fn deploy_state(path: &Path) -> DeployArtifactState {
    let Some(file) = artifact_file(path.to_path_buf()) else {
        return DeployArtifactState::Missing;
    };

    match read_elf_header(path).and_then(|header| parse_elf_summary(&header)) {
        Ok(elf) => DeployArtifactState::Present { file, elf },
        Err(reason) => DeployArtifactState::Invalid { file, reason },
    }
}

fn idl_state(path: &Path) -> IdlArtifactState {
    let Some(file) = artifact_file(path.to_path_buf()) else {
        return IdlArtifactState::Missing;
    };

    match fs::read_to_string(path)
        .map_err(|err| err.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|err| err.to_string()))
    {
        Ok(value) => IdlArtifactState::Present {
            file,
            program_name: idl_program_name(&value),
            address: idl_address(&value),
        },
        Err(reason) => IdlArtifactState::Invalid { file, reason },
    }
}

fn keypair_state(path: &Path) -> ProgramKeypairState {
    let Some(file) = artifact_file(path.to_path_buf()) else {
        return ProgramKeypairState::Missing;
    };

    match fs::read_to_string(path)
        .map_err(|err| err.to_string())
        .and_then(|text| keypair_public_key(&text))
    {
        Ok(public_key) => ProgramKeypairState::Present { file, public_key },
        Err(reason) => ProgramKeypairState::Invalid { file, reason },
    }
}

fn file_presence(path: &Path) -> FilePresence {
    artifact_file(path.to_path_buf())
        .map(FilePresence::Present)
        .unwrap_or(FilePresence::Missing)
}

fn read_elf_header(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let mut header = [0_u8; 64];
    let len = file.read(&mut header).map_err(|err| err.to_string())?;
    Ok(header[..len].to_vec())
}

fn artifact_file(path: PathBuf) -> Option<ArtifactFile> {
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some(ArtifactFile {
        path,
        byte_len: metadata.len(),
        modified: metadata.modified().ok()?,
    })
}

fn artifact_input(path: PathBuf) -> Option<ArtifactInput> {
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some(ArtifactInput {
        path,
        modified: metadata.modified().ok()?,
    })
}

fn collect_source_inputs(source_root: &Path, program_name: &str) -> Vec<ArtifactInput> {
    let mut paths = Vec::new();
    collect_source_paths(source_root, 0, &mut paths);
    if let Some(program_dir) = source_root.parent() {
        paths.push(program_dir.join("Cargo.toml"));
        paths.push(program_dir.join("Xargo.toml"));
        paths.push(program_dir.join("rust-toolchain.toml"));
        paths.push(program_dir.join("rust-toolchain"));
    }
    paths.sort();
    paths.dedup();

    paths
        .into_iter()
        .take(MAX_SOURCE_INPUTS)
        .filter_map(artifact_input)
        .filter(|input| {
            input
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_none_or(|stem| project::normalize_program_name(stem) != program_name)
                || input.path.extension().and_then(|ext| ext.to_str()) == Some("rs")
        })
        .collect()
}

fn collect_source_paths(dir: &Path, depth: usize, paths: &mut Vec<PathBuf>) {
    if depth > MAX_SOURCE_DEPTH || paths.len() >= MAX_SOURCE_INPUTS {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if paths.len() >= MAX_SOURCE_INPUTS {
            return;
        }
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_source_paths(&path, depth + 1, paths);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "node_modules" | "target" | ".anchor")
    )
}

fn idl_program_name(value: &Value) -> Option<String> {
    value
        .get("metadata")
        .and_then(|metadata| metadata.get("name"))
        .or_else(|| value.get("name"))
        .and_then(|name| name.as_str())
        .map(str::to_string)
}

fn idl_address(value: &Value) -> Option<String> {
    value
        .get("address")
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("address"))
        })
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("programId"))
        })
        .or_else(|| value.get("programId"))
        .and_then(|address| address.as_str())
        .map(str::to_string)
}

pub fn keypair_public_key(text: &str) -> Result<String, String> {
    let value = serde_json::from_str::<Value>(text).map_err(|err| err.to_string())?;
    let bytes = value
        .as_array()
        .ok_or_else(|| "keypair file is not a JSON byte array".to_string())?
        .iter()
        .map(|value| {
            let byte = value
                .as_u64()
                .ok_or_else(|| "keypair array contains a non-byte value".to_string())?;
            u8::try_from(byte).map_err(|_| "keypair byte is outside 0..=255".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if bytes.len() != 64 {
        return Err(format!(
            "keypair has {} bytes, expected 64-byte Solana keypair",
            bytes.len()
        ));
    }
    Ok(base58_encode(&bytes[32..]))
}

fn base58_encode(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let zero_count = bytes.iter().take_while(|byte| **byte == 0).count();
    let mut digits = Vec::<u8>::new();
    for byte in bytes {
        let mut carry = u32::from(*byte);
        for digit in digits.iter_mut() {
            let value = u32::from(*digit) * 256 + carry;
            *digit = u8::try_from(value % 58).unwrap_or_default();
            carry = value / 58;
        }
        while carry > 0 {
            digits.push(u8::try_from(carry % 58).unwrap_or_default());
            carry /= 58;
        }
    }

    let mut encoded = String::with_capacity(zero_count + digits.len());
    encoded.extend(std::iter::repeat_n('1', zero_count));
    for digit in digits.iter().rev() {
        encoded.push(char::from(BASE58_ALPHABET[usize::from(*digit)]));
    }
    encoded
}

fn read_u16(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<u16, String> {
    let bytes = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| "ELF header is truncated".to_string())?;
    Ok(match endian {
        ElfEndian::Little => u16::from_le_bytes([bytes[0], bytes[1]]),
        ElfEndian::Big => u16::from_be_bytes([bytes[0], bytes[1]]),
    })
}

fn read_u32(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<u32, String> {
    let bytes = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "ELF header is truncated".to_string())?;
    Ok(match endian {
        ElfEndian::Little => u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        ElfEndian::Big => u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
    })
}

fn read_u64(bytes: &[u8], offset: usize, endian: ElfEndian) -> Result<u64, String> {
    let bytes = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| "ELF header is truncated".to_string())?;
    Ok(match endian {
        ElfEndian::Little => u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        ElfEndian::Big => u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
    })
}

fn deploy_json(state: &DeployArtifactState) -> Value {
    match state {
        DeployArtifactState::Missing => serde_json::json!({ "status": "missing" }),
        DeployArtifactState::Present { file, elf } => serde_json::json!({
            "status": "present",
            "file": artifact_file_json(file),
            "elf": elf_json(elf),
        }),
        DeployArtifactState::Invalid { file, reason } => serde_json::json!({
            "status": "invalid",
            "file": artifact_file_json(file),
            "reason": reason,
        }),
    }
}

fn keypair_json(state: &ProgramKeypairState) -> Value {
    match state {
        ProgramKeypairState::Missing => serde_json::json!({ "status": "missing" }),
        ProgramKeypairState::Present { file, public_key } => serde_json::json!({
            "status": "present",
            "file": artifact_file_json(file),
            "publicKey": public_key,
        }),
        ProgramKeypairState::Invalid { file, reason } => serde_json::json!({
            "status": "invalid",
            "file": artifact_file_json(file),
            "reason": reason,
        }),
    }
}

fn idl_json(state: &IdlArtifactState, applicable: bool) -> Value {
    if !applicable && matches!(state, IdlArtifactState::Missing) {
        return serde_json::json!({ "status": "not-applicable" });
    }

    match state {
        IdlArtifactState::Missing => serde_json::json!({ "status": "missing" }),
        IdlArtifactState::Present {
            file,
            program_name,
            address,
        } => serde_json::json!({
            "status": "present",
            "file": artifact_file_json(file),
            "programName": program_name,
            "address": address,
        }),
        IdlArtifactState::Invalid { file, reason } => serde_json::json!({
            "status": "invalid",
            "file": artifact_file_json(file),
            "reason": reason,
        }),
    }
}

fn presence_json(state: &FilePresence, applicable: bool) -> Value {
    if !applicable && matches!(state, FilePresence::Missing) {
        return serde_json::json!({ "status": "not-applicable" });
    }

    match state {
        FilePresence::Missing => serde_json::json!({ "status": "missing" }),
        FilePresence::Present(file) => serde_json::json!({
            "status": "present",
            "file": artifact_file_json(file),
        }),
    }
}

fn artifact_file_json(file: &ArtifactFile) -> Value {
    serde_json::json!({
        "path": path_to_string(&file.path),
        "byteLen": file.byte_len,
        "modifiedMs": system_time_ms(file.modified),
    })
}

fn elf_json(elf: &ElfSummary) -> Value {
    serde_json::json!({
        "class": match elf.class {
            ElfClass::Elf32 => "ELF32",
            ElfClass::Elf64 => "ELF64",
        },
        "endian": match elf.endian {
            ElfEndian::Little => "little",
            ElfEndian::Big => "big",
        },
        "machine": elf.machine,
        "machineName": if elf.machine == EM_BPF { "EM_BPF" } else { "unknown" },
        "entry": elf.entry,
        "sectionCount": elf.section_count,
    })
}

fn stale_inputs_json(inputs: Vec<&ArtifactInput>) -> Vec<Value> {
    inputs
        .into_iter()
        .map(|input| {
            serde_json::json!({
                "path": path_to_string(&input.path),
                "modifiedMs": system_time_ms(input.modified),
            })
        })
        .collect()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn system_time_ms(time: SystemTime) -> Option<u128> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
}

#[cfg(test)]
mod tests;
