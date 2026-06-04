use {
    crate::{
        anchor_errors::{AnchorErrorCategory, AnchorErrorCoverage, AnchorErrorSpec},
        anchor_preflight::{
            self, AccountEvidence, InstructionEvidence, InvocationEvidence, ReallocEvidence,
        },
    },
    clap::Args,
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeMap,
        error::Error,
        fmt, fs,
        path::{Path, PathBuf},
    },
};

const PREFLIGHT_SCHEMA_VERSION: u8 = 1;
const ANCHOR_DISCRIMINATOR_BYTES: usize = 8;
const HEX_DIGITS_PER_BYTE: usize = 2;
const ANCHOR_DISCRIMINATOR_HEX_CHARS: usize = ANCHOR_DISCRIMINATOR_BYTES * HEX_DIGITS_PER_BYTE;
const PREFLIGHT_FIXTURE_EXAMPLE: &str =
    "seagrass preflight fixtures/preflight/anchor-errors.json --json";

pub(super) const PREFLIGHT_HELP: &str = "\
Examples:
  seagrass preflight fixtures/preflight/anchor-errors.json --json

Input:
  Reads checked invocation/account evidence as JSON. This does not compile a
  program, run a validator, or infer live runtime state.

Output:
  Prints a JSON object with detected Anchor preflight errors, invocation indexes,
  coverage tiers, and explicit runtimeEvidence = notConfigured. Exits 1 when
  preflight errors are detected; exits 2 for usage or malformed evidence.";

#[derive(Debug, Args)]
pub(super) struct PreflightCommand {
    /// JSON file containing invocation/account evidence.
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// Print structured JSON. This is the default output format.
    #[arg(long = "json")]
    json: bool,
}

impl PreflightCommand {
    pub(super) fn run(self) -> Result<bool, Box<dyn Error>> {
        let report = preflight_report_for_path(&self.path)?;
        let has_errors = report.summary.detected_errors > 0;
        serde_json::to_writer_pretty(std::io::stdout(), &report)?;
        println!();
        Ok(has_errors)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreflightFixture {
    #[serde(default)]
    invocations: Vec<InvocationFixture>,
    #[serde(default)]
    instruction: Option<InstructionFixture>,
    #[serde(default)]
    accounts: Vec<AccountFixture>,
    #[serde(default)]
    reallocs: Vec<ReallocFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvocationFixture {
    #[serde(default)]
    instruction: InstructionFixture,
    #[serde(default)]
    accounts: Vec<AccountFixture>,
    #[serde(default)]
    reallocs: Vec<ReallocFixture>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstructionFixture {
    data_len: Option<usize>,
    deserializes: Option<bool>,
    discriminator_matches_known_instruction: Option<bool>,
    fallback_supported: Option<bool>,
    expected_program_id: Option<String>,
    provided_program_id: Option<String>,
    expected_account_count: Option<usize>,
    provided_account_count: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountFixture {
    data_len: Option<usize>,
    expected_discriminator: Option<String>,
    actual_discriminator: Option<String>,
    expected_owner: Option<String>,
    actual_owner: Option<String>,
    initialized: Option<bool>,
    deserializes: Option<bool>,
    expected_zero_discriminator_for_init: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReallocFixture {
    requested_data_increase: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreflightReport {
    schema_version: u8,
    path: String,
    layer: &'static str,
    runtime_evidence: RuntimeEvidence,
    summary: PreflightSummary,
    errors: Vec<DetectedPreflightError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEvidence {
    status: &'static str,
    reason: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreflightSummary {
    invocations: usize,
    detected_errors: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DetectedPreflightError {
    name: &'static str,
    code: u32,
    message: &'static str,
    category: &'static str,
    coverage: &'static str,
    invocations: Vec<usize>,
}

#[derive(Debug)]
struct PreflightInputError {
    message: String,
}

impl PreflightInputError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PreflightInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for PreflightInputError {}

fn preflight_report_for_path(path: &Path) -> Result<PreflightReport, Box<dyn Error>> {
    let fixture = read_fixture(path)?;
    let invocations = invocation_fixtures(fixture)?;
    let mut detected = BTreeMap::<&'static str, DetectedPreflightError>::new();

    for (index, invocation) in invocations.iter().enumerate() {
        let accounts = invocation
            .accounts
            .iter()
            .map(account_evidence)
            .collect::<Result<Vec<_>, _>>()?;
        let reallocs = invocation
            .reallocs
            .iter()
            .map(realloc_evidence)
            .collect::<Vec<_>>();
        let evidence = InvocationEvidence {
            instruction: instruction_evidence(&invocation.instruction),
            accounts: &accounts,
            reallocs: &reallocs,
        };
        for error in anchor_preflight::detected_errors(&evidence) {
            upsert_detected_error(&mut detected, error, index);
        }
    }

    Ok(PreflightReport {
        schema_version: PREFLIGHT_SCHEMA_VERSION,
        path: path.display().to_string(),
        layer: "preflight",
        runtime_evidence: RuntimeEvidence {
            status: "notConfigured",
            reason: "preflight evidence does not include telemetry, validator execution, or runtime coverage artifacts",
        },
        summary: PreflightSummary {
            invocations: invocations.len(),
            detected_errors: detected.len(),
        },
        errors: detected.into_values().collect(),
    })
}

fn read_fixture(path: &Path) -> Result<PreflightFixture, Box<dyn Error>> {
    let contents = fs::read_to_string(path).map_err(|error| {
        PreflightInputError::new(format!(
            "Error: cannot read preflight evidence {}: {error}\nExample:\n  {PREFLIGHT_FIXTURE_EXAMPLE}",
            path.display()
        ))
    })?;
    serde_json::from_str(&contents).map_err(|error| {
        PreflightInputError::new(format!(
            "Error: malformed preflight evidence {}: {error}\nExample:\n  {PREFLIGHT_FIXTURE_EXAMPLE}",
            path.display()
        ))
        .into()
    })
}

fn invocation_fixtures(
    fixture: PreflightFixture,
) -> Result<Vec<InvocationFixture>, PreflightInputError> {
    if !fixture.invocations.is_empty() {
        return Ok(fixture.invocations);
    }
    if fixture.instruction.is_some() || !fixture.accounts.is_empty() || !fixture.reallocs.is_empty()
    {
        return Ok(vec![InvocationFixture {
            instruction: fixture.instruction.unwrap_or_default(),
            accounts: fixture.accounts,
            reallocs: fixture.reallocs,
        }]);
    }
    Err(PreflightInputError::new(format!(
        "Error: preflight evidence must contain invocations or top-level instruction/account evidence.\nExample:\n  {PREFLIGHT_FIXTURE_EXAMPLE}"
    )))
}

fn instruction_evidence(fixture: &InstructionFixture) -> InstructionEvidence<'_> {
    InstructionEvidence {
        data_len: fixture.data_len,
        deserializes: fixture.deserializes,
        discriminator_matches_known_instruction: fixture.discriminator_matches_known_instruction,
        fallback_supported: fixture.fallback_supported,
        expected_program_id: fixture.expected_program_id.as_deref(),
        provided_program_id: fixture.provided_program_id.as_deref(),
        expected_account_count: fixture.expected_account_count,
        provided_account_count: fixture.provided_account_count,
    }
}

fn account_evidence(fixture: &AccountFixture) -> Result<AccountEvidence<'_>, PreflightInputError> {
    Ok(AccountEvidence {
        data_len: fixture.data_len,
        expected_discriminator: optional_discriminator(
            fixture.expected_discriminator.as_deref(),
            "expectedDiscriminator",
        )?,
        actual_discriminator: optional_discriminator(
            fixture.actual_discriminator.as_deref(),
            "actualDiscriminator",
        )?,
        expected_owner: fixture.expected_owner.as_deref(),
        actual_owner: fixture.actual_owner.as_deref(),
        initialized: fixture.initialized,
        deserializes: fixture.deserializes,
        expected_zero_discriminator_for_init: fixture.expected_zero_discriminator_for_init,
    })
}

fn realloc_evidence(fixture: &ReallocFixture) -> ReallocEvidence {
    ReallocEvidence {
        requested_data_increase: fixture.requested_data_increase,
    }
}

fn optional_discriminator(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<[u8; ANCHOR_DISCRIMINATOR_BYTES]>, PreflightInputError> {
    value
        .map(|hex| parse_discriminator(hex, field_name).map(Some))
        .unwrap_or(Ok(None))
}

fn parse_discriminator(
    hex: &str,
    field_name: &str,
) -> Result<[u8; ANCHOR_DISCRIMINATOR_BYTES], PreflightInputError> {
    if hex.len() != ANCHOR_DISCRIMINATOR_HEX_CHARS {
        return Err(PreflightInputError::new(format!(
            "Error: {field_name} must be {ANCHOR_DISCRIMINATOR_HEX_CHARS} hex characters"
        )));
    }

    let mut bytes = [0u8; ANCHOR_DISCRIMINATOR_BYTES];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let start = index * HEX_DIGITS_PER_BYTE;
        let end = start + HEX_DIGITS_PER_BYTE;
        *slot = u8::from_str_radix(&hex[start..end], 16).map_err(|_| {
            PreflightInputError::new(format!("Error: {field_name} must contain only hex digits"))
        })?;
    }
    Ok(bytes)
}

fn upsert_detected_error(
    detected: &mut BTreeMap<&'static str, DetectedPreflightError>,
    error: &'static AnchorErrorSpec,
    invocation_index: usize,
) {
    detected
        .entry(error.name)
        .and_modify(|entry| entry.invocations.push(invocation_index))
        .or_insert_with(|| DetectedPreflightError {
            name: error.name,
            code: error.code,
            message: error.message,
            category: category_name(error.category),
            coverage: coverage_name(error.coverage),
            invocations: vec![invocation_index],
        });
}

fn category_name(category: AnchorErrorCategory) -> &'static str {
    match category {
        AnchorErrorCategory::Instruction => "instruction",
        AnchorErrorCategory::Event => "event",
        AnchorErrorCategory::Constraint => "constraint",
        AnchorErrorCategory::Require => "require",
        AnchorErrorCategory::Account => "account",
        AnchorErrorCategory::Misc => "misc",
        AnchorErrorCategory::Deprecated => "deprecated",
    }
}

fn coverage_name(coverage: AnchorErrorCoverage) -> &'static str {
    match coverage {
        AnchorErrorCoverage::StaticCovered => "static-covered",
        AnchorErrorCoverage::PreflightCovered => "preflight-covered",
        AnchorErrorCoverage::RuntimeOnly => "runtime-only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_reports_all_preflight_covered_anchor_errors() {
        let report = preflight_report_for_path(Path::new("fixtures/preflight/anchor-errors.json"))
            .expect("fixture should parse");
        let names = report
            .errors
            .iter()
            .map(|error| error.name)
            .collect::<Vec<_>>();

        assert_eq!(report.layer, "preflight");
        assert_eq!(report.runtime_evidence.status, "notConfigured");
        assert_eq!(report.summary.detected_errors, 12);
        assert_eq!(
            names,
            [
                "AccountDidNotDeserialize",
                "AccountDiscriminatorAlreadySet",
                "AccountDiscriminatorMismatch",
                "AccountDiscriminatorNotFound",
                "AccountNotEnoughKeys",
                "AccountNotInitialized",
                "AccountOwnedByWrongProgram",
                "AccountReallocExceedsLimit",
                "InstructionDidNotDeserialize",
                "InstructionFallbackNotFound",
                "InstructionMissing",
                "InvalidProgramId",
            ]
        );
        assert!(report
            .errors
            .iter()
            .all(|error| error.coverage == "preflight-covered"));
    }

    #[test]
    fn rejects_bad_discriminator_hex() {
        let error = parse_discriminator("not-hex", "expectedDiscriminator")
            .expect_err("invalid discriminator should fail");

        assert!(error.to_string().contains("16 hex characters"));
    }
}
