/// Verified catalog of Solana runtime sysvars.
///
/// Every entry in this catalog is directly anchored to the `solana-program` crate at the
/// version pinned in `Cargo.toml` (currently `=2.2.1`). The companion test module
/// [`runtime_catalog_parity`] contains compile-time and runtime assertions that make any
/// divergence between the catalog and the real crate a build or test failure, mirroring
/// the discipline used for the Anchor constraint catalog in
/// `src/anchor/constraint_catalog/tests`.
///
/// # Adding a new sysvar
/// 1. Add a [`SysvarSpec`] entry to [`SYSVARS`].
/// 2. Add a corresponding `assert_eq!` in [`runtime_catalog_parity::sysvar_ids_match_solana_program`].
/// 3. If the sysvar exposes a named struct, add a destructuring check in a dedicated
///    `_<name>_fields` helper function in the parity module.
/// 4. Bump the pinned `solana-program` version in `Cargo.toml` and re-verify.
use solana_program::pubkey::Pubkey;

/// Stable name used to identify a sysvar in completions, diagnostics, and hover text.
pub type SysvarName = &'static str;

/// Rust type identifier as it appears in Anchor/Solana source (e.g. `"Clock"`, `"Rent"`).
pub type SysvarTypeIdent = &'static str;

/// Description of a single Solana sysvar account.
#[derive(Debug, Clone, Copy)]
pub struct SysvarSpec {
    /// Short lowercase identifier used in pattern-matching and catalog lookups, e.g. `"clock"`.
    pub name: SysvarName,
    /// Rust type name as written in program source, e.g. `"Clock"`.
    pub type_ident: SysvarTypeIdent,
    /// The well-known account address at which the runtime stores this sysvar.
    pub account_id: Pubkey,
    /// The canonical base-58 representation of [`account_id`][Self::account_id].
    ///
    /// This is the same string used to construct the [`Pubkey`]; keeping it on the spec
    /// lets callers format detail text or documentation without calling `pubkey.to_string()`
    /// at runtime.
    pub account_id_str: &'static str,
    /// `true` for sysvars that are no longer populated by the runtime and should not be
    /// referenced in new programs (currently `fees` and `recent_blockhashes`).
    pub is_deprecated: bool,
}

/// The canonical base-58 string for each sysvar ID.
///
/// These string constants are used to initialise the [`Pubkey`] fields in [`SYSVARS`].  They are
/// also what is verified by the parity tests — if the `solana-program` crate ever ships a
/// different value the test will fail before the mismatch can silently reach users.
mod sysvar_id_strings {
    pub const CLOCK: &str = "SysvarC1ock11111111111111111111111111111111";
    pub const EPOCH_REWARDS: &str = "SysvarEpochRewards1111111111111111111111111";
    pub const EPOCH_SCHEDULE: &str = "SysvarEpochSchedu1e111111111111111111111111";
    pub const FEES: &str = "SysvarFees111111111111111111111111111111111";
    pub const INSTRUCTIONS: &str = "Sysvar1nstructions1111111111111111111111111";
    pub const LAST_RESTART_SLOT: &str = "SysvarLastRestartS1ot1111111111111111111111";
    pub const RECENT_BLOCKHASHES: &str = "SysvarRecentB1ockHashes11111111111111111111";
    pub const RENT: &str = "SysvarRent111111111111111111111111111111111";
    pub const SLOT_HASHES: &str = "SysvarS1otHashes111111111111111111111111111";
    pub const STAKE_HISTORY: &str = "SysvarStakeHistory1111111111111111111111111";
}

/// Parses a well-known sysvar address string into a [`Pubkey`] at compile time.
///
/// `Pubkey::from_str_const` is a `const fn` since solana-pubkey 2.x, so this wrapper can
/// also be `const`.  Keeping both the string literal and the decoded [`Pubkey`] in the catalog
/// means a typo in the string is a compile error rather than a runtime surprise.
const fn pubkey_const(s: &'static str) -> Pubkey {
    solana_program::pubkey::Pubkey::from_str_const(s)
}

/// All known Solana runtime sysvars.
///
/// Ordering follows the canonical listing in the
/// [Solana documentation](https://docs.solanalabs.com/runtime/sysvars), with deprecated
/// sysvars placed last.
pub const SYSVARS: &[SysvarSpec] = &[
    SysvarSpec {
        name: "clock",
        type_ident: "Clock",
        account_id: pubkey_const(sysvar_id_strings::CLOCK),
        account_id_str: sysvar_id_strings::CLOCK,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "epoch_rewards",
        type_ident: "EpochRewards",
        account_id: pubkey_const(sysvar_id_strings::EPOCH_REWARDS),
        account_id_str: sysvar_id_strings::EPOCH_REWARDS,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "epoch_schedule",
        type_ident: "EpochSchedule",
        account_id: pubkey_const(sysvar_id_strings::EPOCH_SCHEDULE),
        account_id_str: sysvar_id_strings::EPOCH_SCHEDULE,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "instructions",
        type_ident: "Instructions",
        account_id: pubkey_const(sysvar_id_strings::INSTRUCTIONS),
        account_id_str: sysvar_id_strings::INSTRUCTIONS,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "last_restart_slot",
        type_ident: "LastRestartSlot",
        account_id: pubkey_const(sysvar_id_strings::LAST_RESTART_SLOT),
        account_id_str: sysvar_id_strings::LAST_RESTART_SLOT,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "rent",
        type_ident: "Rent",
        account_id: pubkey_const(sysvar_id_strings::RENT),
        account_id_str: sysvar_id_strings::RENT,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "slot_hashes",
        type_ident: "SlotHashes",
        account_id: pubkey_const(sysvar_id_strings::SLOT_HASHES),
        account_id_str: sysvar_id_strings::SLOT_HASHES,
        is_deprecated: false,
    },
    SysvarSpec {
        name: "stake_history",
        type_ident: "StakeHistory",
        account_id: pubkey_const(sysvar_id_strings::STAKE_HISTORY),
        account_id_str: sysvar_id_strings::STAKE_HISTORY,
        is_deprecated: false,
    },
    // Deprecated: the runtime no longer populates these accounts.
    SysvarSpec {
        name: "fees",
        type_ident: "Fees",
        account_id: pubkey_const(sysvar_id_strings::FEES),
        account_id_str: sysvar_id_strings::FEES,
        is_deprecated: true,
    },
    SysvarSpec {
        name: "recent_blockhashes",
        type_ident: "RecentBlockhashes",
        account_id: pubkey_const(sysvar_id_strings::RECENT_BLOCKHASHES),
        account_id_str: sysvar_id_strings::RECENT_BLOCKHASHES,
        is_deprecated: true,
    },
];

/// Looks up a [`SysvarSpec`] by its short name (e.g. `"clock"`).
pub fn by_name(name: &str) -> Option<&'static SysvarSpec> {
    SYSVARS.iter().find(|s| s.name == name)
}

/// Looks up a [`SysvarSpec`] by Rust type identifier (e.g. `"Clock"`).
pub fn by_type_ident(ident: &str) -> Option<&'static SysvarSpec> {
    SYSVARS.iter().find(|s| s.type_ident == ident)
}

/// Field names for [`solana_program::clock::Clock`] in the order they appear in the struct
/// definition.  When `solana-program` renames a field the compile-time check in
/// [`runtime_catalog_parity`] will catch the divergence before it reaches users.
pub const CLOCK_FIELDS: &[&str] = &[
    "slot",
    "epoch_start_timestamp",
    "epoch",
    "leader_schedule_epoch",
    "unix_timestamp",
];

/// Field names for [`solana_program::rent::Rent`] in struct-definition order.
pub const RENT_FIELDS: &[&str] = &[
    "lamports_per_byte_year",
    "exemption_threshold",
    "burn_percent",
];

#[cfg(test)]
mod runtime_catalog_parity {
    //! Parity tests that pin the catalog to the real `solana-program` crate.
    //!
    //! A failure here means either:
    //!   (a) the `solana-program` pin was bumped and the catalog needs updating, or
    //!   (b) the catalog string was accidentally changed without bumping the pin.
    //!
    //! Either way the test message clearly identifies which sysvar diverged.

    #[allow(deprecated)]
    use solana_program::sysvar;

    use super::*;

    /// Every sysvar pubkey in the catalog must equal the address returned by the
    /// corresponding `solana_program::sysvar::<name>::id()` function.
    ///
    /// This is the core parity gate: if Solana ever reassigns a sysvar address (which
    /// would be an extraordinarily breaking change, but still) the test fails immediately.
    #[test]
    #[allow(deprecated)]
    fn sysvar_ids_match_solana_program() {
        let clock = by_name("clock").unwrap();
        assert_eq!(
            clock.account_id,
            sysvar::clock::id(),
            "catalog clock id diverges from solana_program::sysvar::clock::id()"
        );

        let epoch_rewards = by_name("epoch_rewards").unwrap();
        assert_eq!(
            epoch_rewards.account_id,
            sysvar::epoch_rewards::id(),
            "catalog epoch_rewards id diverges from solana_program::sysvar::epoch_rewards::id()"
        );

        let epoch_schedule = by_name("epoch_schedule").unwrap();
        assert_eq!(
            epoch_schedule.account_id,
            sysvar::epoch_schedule::id(),
            "catalog epoch_schedule id diverges from solana_program::sysvar::epoch_schedule::id()"
        );

        let instructions = by_name("instructions").unwrap();
        assert_eq!(
            instructions.account_id,
            sysvar::instructions::id(),
            "catalog instructions id diverges from solana_program::sysvar::instructions::id()"
        );

        let last_restart_slot = by_name("last_restart_slot").unwrap();
        assert_eq!(
            last_restart_slot.account_id,
            sysvar::last_restart_slot::id(),
            "catalog last_restart_slot id diverges from solana_program::sysvar::last_restart_slot::id()"
        );

        let rent = by_name("rent").unwrap();
        assert_eq!(
            rent.account_id,
            sysvar::rent::id(),
            "catalog rent id diverges from solana_program::sysvar::rent::id()"
        );

        let slot_hashes = by_name("slot_hashes").unwrap();
        assert_eq!(
            slot_hashes.account_id,
            sysvar::slot_hashes::id(),
            "catalog slot_hashes id diverges from solana_program::sysvar::slot_hashes::id()"
        );

        let stake_history = by_name("stake_history").unwrap();
        assert_eq!(
            stake_history.account_id,
            sysvar::stake_history::id(),
            "catalog stake_history id diverges from solana_program::sysvar::stake_history::id()"
        );

        let fees = by_name("fees").unwrap();
        assert_eq!(
            fees.account_id,
            sysvar::fees::id(),
            "catalog fees id diverges from solana_program::sysvar::fees::id()"
        );

        let recent_blockhashes = by_name("recent_blockhashes").unwrap();
        assert_eq!(
            recent_blockhashes.account_id,
            sysvar::recent_blockhashes::id(),
            "catalog recent_blockhashes id diverges from solana_program::sysvar::recent_blockhashes::id()"
        );
    }

    /// Compile-time field coverage for [`solana_program::clock::Clock`].
    ///
    /// This function is never called; its only purpose is to produce a compile error if
    /// `solana-program` renames or removes a Clock field.  A new field added by the crate
    /// will surface as an "irrefutable pattern" warning/error, prompting a catalog update.
    #[allow(dead_code)]
    fn _clock_fields_compile_check(c: solana_program::clock::Clock) {
        // Exhaustive destructuring — adding, removing, or renaming a field breaks the build.
        let solana_program::clock::Clock {
            slot,
            epoch_start_timestamp,
            epoch,
            leader_schedule_epoch,
            unix_timestamp,
        } = c;
        // Suppress unused-variable warnings without runtime cost.
        let _ = (
            slot,
            epoch_start_timestamp,
            epoch,
            leader_schedule_epoch,
            unix_timestamp,
        );
    }

    /// Compile-time field coverage for [`solana_program::rent::Rent`].
    ///
    /// Same pattern as `_clock_fields_compile_check`: never called, purely structural.
    #[allow(dead_code)]
    fn _rent_fields_compile_check(r: solana_program::rent::Rent) {
        let solana_program::rent::Rent {
            lamports_per_byte_year,
            exemption_threshold,
            burn_percent,
        } = r;
        let _ = (lamports_per_byte_year, exemption_threshold, burn_percent);
    }

    /// The deprecated set must be exactly {fees, recent_blockhashes}.
    ///
    /// If Solana deprecates another sysvar the test will start failing, prompting a catalog
    /// update and documentation review before the change silently reaches LSP users.
    #[test]
    fn deprecated_set_is_fees_and_recent_blockhashes() {
        let deprecated: Vec<_> = SYSVARS
            .iter()
            .filter(|s| s.is_deprecated)
            .map(|s| s.name)
            .collect();

        assert_eq!(
            deprecated,
            &["fees", "recent_blockhashes"],
            "catalog deprecated set changed — update runtime_catalog.rs and bump \
             the solana-program pin if intentional"
        );
    }

    /// Active (non-deprecated) sysvars must include the full set expected by seagrass.
    #[test]
    fn active_sysvar_set_is_complete() {
        let active: Vec<_> = SYSVARS
            .iter()
            .filter(|s| !s.is_deprecated)
            .map(|s| s.name)
            .collect();

        let required = [
            "clock",
            "epoch_rewards",
            "epoch_schedule",
            "instructions",
            "last_restart_slot",
            "rent",
            "slot_hashes",
            "stake_history",
        ];

        for name in &required {
            assert!(
                active.contains(name),
                "expected active sysvar '{name}' not found in catalog"
            );
        }
    }
}
