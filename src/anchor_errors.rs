#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorErrorCategory {
    Instruction,
    Event,
    Constraint,
    Require,
    Account,
    Misc,
    Deprecated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorErrorCoverage {
    StaticCovered,
    StaticCatchableMissing,
    BuildProjectCatchable,
    RuntimeOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorErrorSpec {
    pub name: &'static str,
    pub code: u32,
    pub message: &'static str,
    pub category: AnchorErrorCategory,
    pub coverage: AnchorErrorCoverage,
}

pub const ERRORS: &[AnchorErrorSpec] = include!("generated/anchor_error_catalog_generated.rs");

pub fn by_name(name: &str) -> Option<&'static AnchorErrorSpec> {
    ERRORS.iter().find(|spec| spec.name == name)
}

pub fn coverage_summary() -> serde_json::Value {
    let mut static_covered = 0usize;
    let mut static_catchable_missing = 0usize;
    let mut build_project_catchable = 0usize;
    let mut runtime_only = 0usize;

    for spec in ERRORS {
        match spec.coverage {
            AnchorErrorCoverage::StaticCovered => static_covered += 1,
            AnchorErrorCoverage::StaticCatchableMissing => static_catchable_missing += 1,
            AnchorErrorCoverage::BuildProjectCatchable => build_project_catchable += 1,
            AnchorErrorCoverage::RuntimeOnly => runtime_only += 1,
        }
    }

    serde_json::json!({
        "total": ERRORS.len(),
        "summary": {
            "staticCovered": static_covered,
            "staticCatchableMissing": static_catchable_missing,
            "buildProjectCatchable": build_project_catchable,
            "runtimeOnly": runtime_only,
        },
        "errors": ERRORS.iter().map(error_json).collect::<Vec<_>>(),
    })
}

pub fn diagnostic_error_data(names: &[&str]) -> Vec<serde_json::Value> {
    names
        .iter()
        .filter_map(|name| by_name(name))
        .map(error_json)
        .collect()
}

fn error_json(spec: &AnchorErrorSpec) -> serde_json::Value {
    serde_json::json!({
        "name": spec.name,
        "code": spec.code,
        "message": spec.message,
        "category": coverage_name(spec.category),
        "coverage": coverage_status(spec.coverage),
    })
}

fn coverage_name(category: AnchorErrorCategory) -> &'static str {
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

fn coverage_status(coverage: AnchorErrorCoverage) -> &'static str {
    match coverage {
        AnchorErrorCoverage::StaticCovered => "static-covered",
        AnchorErrorCoverage::StaticCatchableMissing => "static-catchable-missing",
        AnchorErrorCoverage::BuildProjectCatchable => "build-project-catchable",
        AnchorErrorCoverage::RuntimeOnly => "runtime-only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_catalog_contains_anchor_framework_errors() {
        assert!(by_name("ConstraintMut").is_some());
        assert!(by_name("AccountNotEnoughKeys").is_some());
        assert!(by_name("TryingToInitPayerAsProgramAccount").is_some());
    }

    #[test]
    fn generated_catalog_assigns_current_anchor_codes() {
        assert_eq!(by_name("ConstraintMut").unwrap().code, 2000);
        assert_eq!(by_name("AccountNotEnoughKeys").unwrap().code, 3005);
        assert_eq!(by_name("DeclaredProgramIdMismatch").unwrap().code, 4100);
    }

    #[test]
    fn coverage_summary_accounts_for_every_generated_error() {
        let summary = coverage_summary();
        assert_eq!(summary["total"].as_u64(), Some(ERRORS.len() as u64));
        assert_eq!(summary["errors"].as_array().unwrap().len(), ERRORS.len());
    }
}
