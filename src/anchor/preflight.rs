use crate::anchor_errors::{self, AnchorErrorSpec};

const ANCHOR_DISCRIMINATOR_BYTES: usize = 8;
const MAX_PERMITTED_ACCOUNT_DATA_INCREASE_BYTES: usize = 10_240;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InstructionEvidence<'a> {
    pub data_len: Option<usize>,
    pub deserializes: Option<bool>,
    pub discriminator_matches_known_instruction: Option<bool>,
    pub fallback_supported: Option<bool>,
    pub expected_program_id: Option<&'a str>,
    pub provided_program_id: Option<&'a str>,
    pub expected_account_count: Option<usize>,
    pub provided_account_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AccountEvidence<'a> {
    pub data_len: Option<usize>,
    pub expected_discriminator: Option<[u8; ANCHOR_DISCRIMINATOR_BYTES]>,
    pub actual_discriminator: Option<[u8; ANCHOR_DISCRIMINATOR_BYTES]>,
    pub expected_owner: Option<&'a str>,
    pub actual_owner: Option<&'a str>,
    pub initialized: Option<bool>,
    pub deserializes: Option<bool>,
    pub expected_zero_discriminator_for_init: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReallocEvidence {
    pub requested_data_increase: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InvocationEvidence<'a> {
    pub instruction: InstructionEvidence<'a>,
    pub accounts: &'a [AccountEvidence<'a>],
    pub reallocs: &'a [ReallocEvidence],
}

pub fn detected_errors(evidence: &InvocationEvidence<'_>) -> Vec<&'static AnchorErrorSpec> {
    let mut errors = Vec::new();
    collect_instruction_errors(evidence.instruction, &mut errors);
    collect_account_errors(evidence.accounts, &mut errors);
    collect_realloc_errors(evidence.reallocs, &mut errors);
    errors
}

fn collect_instruction_errors(
    evidence: InstructionEvidence<'_>,
    errors: &mut Vec<&'static AnchorErrorSpec>,
) {
    if evidence
        .data_len
        .is_some_and(|len| len < ANCHOR_DISCRIMINATOR_BYTES)
    {
        push_error(errors, "InstructionMissing");
        return;
    }

    if evidence.discriminator_matches_known_instruction == Some(false)
        && evidence.fallback_supported == Some(false)
    {
        push_error(errors, "InstructionFallbackNotFound");
    }

    if evidence.deserializes == Some(false) {
        push_error(errors, "InstructionDidNotDeserialize");
    }

    if let (Some(expected), Some(provided)) =
        (evidence.expected_program_id, evidence.provided_program_id)
    {
        if expected != provided {
            push_error(errors, "InvalidProgramId");
        }
    }

    if let (Some(expected), Some(provided)) = (
        evidence.expected_account_count,
        evidence.provided_account_count,
    ) {
        if provided < expected {
            push_error(errors, "AccountNotEnoughKeys");
        }
    }
}

fn collect_account_errors(
    accounts: &[AccountEvidence<'_>],
    errors: &mut Vec<&'static AnchorErrorSpec>,
) {
    for account in accounts {
        if account.expected_zero_discriminator_for_init == Some(true)
            && account
                .actual_discriminator
                .is_some_and(|actual| actual != [0; ANCHOR_DISCRIMINATOR_BYTES])
        {
            push_error(errors, "AccountDiscriminatorAlreadySet");
        }

        if account.initialized == Some(false) {
            push_error(errors, "AccountNotInitialized");
        }

        if let (Some(expected), Some(actual)) = (account.expected_owner, account.actual_owner) {
            if expected != actual {
                push_error(errors, "AccountOwnedByWrongProgram");
            }
        }

        if account.expected_discriminator.is_some() && missing_discriminator(account) {
            push_error(errors, "AccountDiscriminatorNotFound");
        }

        if let (Some(expected), Some(actual)) =
            (account.expected_discriminator, account.actual_discriminator)
        {
            if expected != actual {
                push_error(errors, "AccountDiscriminatorMismatch");
            }
        }

        if account.deserializes == Some(false) {
            push_error(errors, "AccountDidNotDeserialize");
        }
    }
}

fn collect_realloc_errors(
    reallocs: &[ReallocEvidence],
    errors: &mut Vec<&'static AnchorErrorSpec>,
) {
    for realloc in reallocs {
        if realloc.requested_data_increase > MAX_PERMITTED_ACCOUNT_DATA_INCREASE_BYTES {
            push_error(errors, "AccountReallocExceedsLimit");
        }
    }
}

fn missing_discriminator(account: &AccountEvidence<'_>) -> bool {
    account
        .data_len
        .is_some_and(|len| len < ANCHOR_DISCRIMINATOR_BYTES)
        || account.actual_discriminator.is_none()
}

fn push_error(errors: &mut Vec<&'static AnchorErrorSpec>, name: &str) {
    if errors.iter().any(|error| error.name == name) {
        return;
    }
    if let Some(error) = anchor_errors::by_name(name) {
        errors.push(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM_ID: &str = "Prog1111111111111111111111111111111111111";
    const OTHER_PROGRAM_ID: &str = "Other11111111111111111111111111111111111";
    const EXPECTED_DISCRIMINATOR: [u8; ANCHOR_DISCRIMINATOR_BYTES] = [1, 2, 3, 4, 5, 6, 7, 8];
    const ACTUAL_DISCRIMINATOR: [u8; ANCHOR_DISCRIMINATOR_BYTES] = [8, 7, 6, 5, 4, 3, 2, 1];

    #[test]
    fn detects_instruction_preflight_errors_from_invocation_shape() {
        let evidence = InvocationEvidence {
            instruction: InstructionEvidence {
                data_len: Some(ANCHOR_DISCRIMINATOR_BYTES),
                deserializes: Some(false),
                discriminator_matches_known_instruction: Some(false),
                fallback_supported: Some(false),
                expected_program_id: Some(PROGRAM_ID),
                provided_program_id: Some(OTHER_PROGRAM_ID),
                expected_account_count: Some(3),
                provided_account_count: Some(2),
            },
            ..InvocationEvidence::default()
        };

        assert_detected(
            &evidence,
            &[
                "InstructionFallbackNotFound",
                "InstructionDidNotDeserialize",
                "InvalidProgramId",
                "AccountNotEnoughKeys",
            ],
        );
    }

    #[test]
    fn detects_missing_instruction_discriminator_before_deserialization() {
        let evidence = InvocationEvidence {
            instruction: InstructionEvidence {
                data_len: Some(ANCHOR_DISCRIMINATOR_BYTES - 1),
                deserializes: Some(false),
                ..InstructionEvidence::default()
            },
            ..InvocationEvidence::default()
        };

        assert_detected(&evidence, &["InstructionMissing"]);
    }

    #[test]
    fn detects_account_preflight_errors_from_account_bytes_and_metadata() {
        let accounts = [AccountEvidence {
            data_len: Some(ANCHOR_DISCRIMINATOR_BYTES),
            expected_discriminator: Some(EXPECTED_DISCRIMINATOR),
            actual_discriminator: Some(ACTUAL_DISCRIMINATOR),
            expected_owner: Some(PROGRAM_ID),
            actual_owner: Some(OTHER_PROGRAM_ID),
            initialized: Some(false),
            deserializes: Some(false),
            expected_zero_discriminator_for_init: Some(true),
        }];
        let evidence = InvocationEvidence {
            accounts: &accounts,
            ..InvocationEvidence::default()
        };

        assert_detected(
            &evidence,
            &[
                "AccountDiscriminatorAlreadySet",
                "AccountNotInitialized",
                "AccountOwnedByWrongProgram",
                "AccountDiscriminatorMismatch",
                "AccountDidNotDeserialize",
            ],
        );
    }

    #[test]
    fn detects_missing_discriminator_and_realloc_limit() {
        let accounts = [AccountEvidence {
            data_len: Some(ANCHOR_DISCRIMINATOR_BYTES - 1),
            expected_discriminator: Some(EXPECTED_DISCRIMINATOR),
            ..AccountEvidence::default()
        }];
        let reallocs = [ReallocEvidence {
            requested_data_increase: MAX_PERMITTED_ACCOUNT_DATA_INCREASE_BYTES + 1,
        }];
        let evidence = InvocationEvidence {
            accounts: &accounts,
            reallocs: &reallocs,
            ..InvocationEvidence::default()
        };

        assert_detected(
            &evidence,
            &["AccountDiscriminatorNotFound", "AccountReallocExceedsLimit"],
        );
    }

    fn assert_detected(evidence: &InvocationEvidence<'_>, expected: &[&str]) {
        let names = detected_errors(evidence)
            .iter()
            .map(|error| error.name)
            .collect::<Vec<_>>();
        for expected_name in expected {
            assert!(
                names.contains(expected_name),
                "missing {expected_name}; detected {names:?}"
            );
        }
    }
}
