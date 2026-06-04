use {
    super::{display_path, InstructionIntelligenceReport},
    crate::program_artifacts,
    serde::Serialize,
    std::collections::BTreeSet,
};

const RUNTIME_EVIDENCE_STATUS_NOT_CONFIGURED: &str = "notConfigured";
const COMPUTE_STATUS_SOURCE_ONLY: &str = "sourceOnly";
const COMPUTE_STATUS_STATIC_BYTECODE_AVAILABLE: &str = "staticBytecodeAvailable";
const COMPUTE_STATUS_ARTIFACT_MISSING: &str = "artifactMissing";
const COMPUTE_STATUS_ARTIFACT_INVALID: &str = "artifactInvalid";
const COMPUTE_STATUS_BYTECODE_UNAVAILABLE: &str = "bytecodeUnavailable";
const COMPUTE_STATUS_ARTIFACT_NOT_DISCOVERED: &str = "artifactNotDiscovered";
const SOURCE_ONLY_COMPUTE_REASON: &str =
    "No local SBF deploy artifact was discovered for this source file.";
const RUNTIME_MEASUREMENT_NOT_CONFIGURED_REASON: &str =
    "Runtime CU measurements require simulation, validator logs, traces, or telemetry.";
const MISSING_DEPLOY_ARTIFACT_REASON: &str = "Local SBF deploy artifact does not exist yet.";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ComputeAnalysisReport {
    status: &'static str,
    source_signals: Vec<&'static str>,
    bytecode: BytecodeComputeAnalysisReport,
    runtime_measurement_status: &'static str,
    runtime_measurement_reason: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BytecodeComputeAnalysisReport {
    status: &'static str,
    artifact_path: Option<String>,
    text_section_bytes: Option<u64>,
    sbf_instruction_count: Option<u64>,
    estimated_program_execution_cu_floor: Option<u64>,
    reason: Option<String>,
}

pub(super) fn compute_analysis_report(
    instructions: &[InstructionIntelligenceReport],
    artifact_report: Option<&program_artifacts::ProgramArtifactReport>,
) -> ComputeAnalysisReport {
    let source_signals = compute_source_signals(instructions);
    let bytecode = bytecode_compute_analysis(artifact_report);
    let status = compute_status(bytecode.status);

    ComputeAnalysisReport {
        status,
        source_signals,
        bytecode,
        runtime_measurement_status: RUNTIME_EVIDENCE_STATUS_NOT_CONFIGURED,
        runtime_measurement_reason: RUNTIME_MEASUREMENT_NOT_CONFIGURED_REASON,
    }
}

fn compute_status(bytecode_status: &'static str) -> &'static str {
    if bytecode_status == COMPUTE_STATUS_STATIC_BYTECODE_AVAILABLE {
        COMPUTE_STATUS_STATIC_BYTECODE_AVAILABLE
    } else if bytecode_status == COMPUTE_STATUS_ARTIFACT_NOT_DISCOVERED {
        COMPUTE_STATUS_SOURCE_ONLY
    } else {
        bytecode_status
    }
}

fn compute_source_signals(instructions: &[InstructionIntelligenceReport]) -> Vec<&'static str> {
    instructions
        .iter()
        .flat_map(|instruction| instruction.compute_review_reasons.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn bytecode_compute_analysis(
    artifact_report: Option<&program_artifacts::ProgramArtifactReport>,
) -> BytecodeComputeAnalysisReport {
    let Some(artifact_report) = artifact_report else {
        return unavailable_bytecode_report(
            COMPUTE_STATUS_ARTIFACT_NOT_DISCOVERED,
            None,
            SOURCE_ONLY_COMPUTE_REASON.to_string(),
        );
    };

    match &artifact_report.deploy {
        program_artifacts::DeployArtifactState::Missing => unavailable_bytecode_report(
            COMPUTE_STATUS_ARTIFACT_MISSING,
            Some(display_path(&artifact_report.deploy_path)),
            MISSING_DEPLOY_ARTIFACT_REASON.to_string(),
        ),
        program_artifacts::DeployArtifactState::Invalid { file, reason } => {
            unavailable_bytecode_report(
                COMPUTE_STATUS_ARTIFACT_INVALID,
                Some(display_path(&file.path)),
                reason.clone(),
            )
        }
        program_artifacts::DeployArtifactState::Present { file, .. } => {
            match program_artifacts::sbf_text_instruction_count(&file.path) {
                Ok(count) => BytecodeComputeAnalysisReport {
                    status: COMPUTE_STATUS_STATIC_BYTECODE_AVAILABLE,
                    artifact_path: Some(display_path(&file.path)),
                    text_section_bytes: Some(count.text_section_bytes),
                    sbf_instruction_count: Some(count.instruction_count),
                    estimated_program_execution_cu_floor: Some(count.instruction_count),
                    reason: None,
                },
                Err(reason) => unavailable_bytecode_report(
                    COMPUTE_STATUS_BYTECODE_UNAVAILABLE,
                    Some(display_path(&file.path)),
                    reason,
                ),
            }
        }
    }
}

fn unavailable_bytecode_report(
    status: &'static str,
    artifact_path: Option<String>,
    reason: String,
) -> BytecodeComputeAnalysisReport {
    BytecodeComputeAnalysisReport {
        status,
        artifact_path,
        text_section_bytes: None,
        sbf_instruction_count: None,
        estimated_program_execution_cu_floor: None,
        reason: Some(reason),
    }
}
