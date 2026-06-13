use {
    crate::{solana::runtime_catalog, solana_project::CargoManifestDeps},
    std::collections::HashMap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Concept {
    SysvarClock,
    SysvarEpochRewards,
    SysvarEpochSchedule,
    SysvarInstructions,
    SysvarLastRestartSlot,
    SysvarRent,
    SysvarSlotHashes,
    SysvarStakeHistory,
    SysvarFees,
    SysvarRecentBlockhashes,
    AccountInfo,
}

impl Concept {
    pub fn is_sysvar(self) -> bool {
        self.sysvar_name().is_some()
    }

    pub fn sysvar_name(self) -> Option<&'static str> {
        match self {
            Self::SysvarClock => Some("clock"),
            Self::SysvarEpochRewards => Some("epoch_rewards"),
            Self::SysvarEpochSchedule => Some("epoch_schedule"),
            Self::SysvarInstructions => Some("instructions"),
            Self::SysvarLastRestartSlot => Some("last_restart_slot"),
            Self::SysvarRent => Some("rent"),
            Self::SysvarSlotHashes => Some("slot_hashes"),
            Self::SysvarStakeHistory => Some("stake_history"),
            Self::SysvarFees => Some("fees"),
            Self::SysvarRecentBlockhashes => Some("recent_blockhashes"),
            Self::AccountInfo => None,
        }
    }

    pub fn catalog_sysvar(self) -> Option<&'static runtime_catalog::SysvarSpec> {
        self.sysvar_name().and_then(runtime_catalog::by_name)
    }
}

pub struct CrateCapability {
    pub crate_name: &'static str,
    pub min_version: &'static str,
    pub provides: &'static [(Concept, &'static str)],
}

const ANCHOR_LANG_PROVIDES: &[(Concept, &str)] = &[
    (Concept::AccountInfo, "AccountInfo"),
    (Concept::SysvarClock, "Clock"),
    (Concept::SysvarEpochSchedule, "EpochSchedule"),
    (Concept::SysvarInstructions, "Instructions"),
    (Concept::SysvarRent, "Rent"),
    (Concept::SysvarSlotHashes, "SlotHashes"),
    (Concept::SysvarStakeHistory, "StakeHistory"),
];
const PINOCCHIO_PROVIDES: &[(Concept, &str)] = &[
    (Concept::AccountInfo, "AccountInfo"),
    (Concept::SysvarClock, "Clock"),
    (Concept::SysvarFees, "Fees"),
    (Concept::SysvarInstructions, "Instructions"),
    (Concept::SysvarRent, "Rent"),
    (Concept::SysvarSlotHashes, "SlotHashes"),
];
const SOLANA_CLOCK_PROVIDES: &[(Concept, &str)] = &[(Concept::SysvarClock, "Clock")];
const SOLANA_PROGRAM_PROVIDES: &[(Concept, &str)] = &[
    (Concept::AccountInfo, "AccountInfo"),
    (Concept::SysvarClock, "Clock"),
    (Concept::SysvarEpochRewards, "EpochRewards"),
    (Concept::SysvarEpochSchedule, "EpochSchedule"),
    (Concept::SysvarInstructions, "Instructions"),
    (Concept::SysvarLastRestartSlot, "LastRestartSlot"),
    (Concept::SysvarRent, "Rent"),
    (Concept::SysvarSlotHashes, "SlotHashes"),
    (Concept::SysvarStakeHistory, "StakeHistory"),
    (Concept::SysvarFees, "Fees"),
    (Concept::SysvarRecentBlockhashes, "RecentBlockhashes"),
];
const SOLANA_RENT_PROVIDES: &[(Concept, &str)] = &[(Concept::SysvarRent, "Rent")];
const SOLANA_SYSVAR_PROVIDES: &[(Concept, &str)] = &[
    (Concept::SysvarClock, "Clock"),
    (Concept::SysvarEpochRewards, "EpochRewards"),
    (Concept::SysvarEpochSchedule, "EpochSchedule"),
    (Concept::SysvarLastRestartSlot, "LastRestartSlot"),
    (Concept::SysvarRent, "Rent"),
    (Concept::SysvarSlotHashes, "SlotHashes"),
    (Concept::SysvarStakeHistory, "StakeHistory"),
    (Concept::SysvarFees, "Fees"),
    (Concept::SysvarRecentBlockhashes, "RecentBlockhashes"),
];

pub const CAPABILITY_REGISTRY: &[CrateCapability] = &[
    CrateCapability {
        crate_name: "anchor-lang",
        min_version: "0.29",
        provides: ANCHOR_LANG_PROVIDES,
    },
    // Pinocchio is chain-activated by explicit import + manifest evidence. It is
    // reviewed from local crate sources rather than compiled as a dev-dependency.
    CrateCapability {
        crate_name: "pinocchio",
        min_version: "0.1",
        provides: PINOCCHIO_PROVIDES,
    },
    CrateCapability {
        crate_name: "solana-clock",
        min_version: "0.1",
        provides: SOLANA_CLOCK_PROVIDES,
    },
    CrateCapability {
        crate_name: "solana-program",
        min_version: "1.0",
        provides: SOLANA_PROGRAM_PROVIDES,
    },
    CrateCapability {
        crate_name: "solana-rent",
        min_version: "0.1",
        provides: SOLANA_RENT_PROVIDES,
    },
    CrateCapability {
        crate_name: "solana-sysvar",
        min_version: "2.2",
        provides: SOLANA_SYSVAR_PROVIDES,
    },
];

pub fn resolve_concept(
    ident: &str,
    import_origins: &HashMap<String, String>,
    declared_deps: &CargoManifestDeps,
) -> Option<Concept> {
    resolve_concept_with_aliases(ident, import_origins, &HashMap::new(), declared_deps)
}

pub fn resolve_concept_with_aliases(
    ident: &str,
    import_origins: &HashMap<String, String>,
    import_aliases: &HashMap<String, String>,
    declared_deps: &CargoManifestDeps,
) -> Option<Concept> {
    let origin = import_origins.get(ident)?;
    let exported_ident = import_aliases
        .get(ident)
        .map(String::as_str)
        .unwrap_or(ident);

    CAPABILITY_REGISTRY.iter().find_map(|capability| {
        if !declared_deps.import_root_matches_crate(origin, capability.crate_name) {
            return None;
        }
        capability
            .provides
            .iter()
            .find_map(|(concept, type_ident)| (*type_ident == exported_ident).then_some(*concept))
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{document::ParsedDocument, solana_project},
    };

    #[test]
    fn registry_concepts_all_backed_by_catalog() {
        for capability in CAPABILITY_REGISTRY {
            for (concept, type_ident) in capability.provides {
                if let Some(sysvar) = concept.catalog_sysvar() {
                    assert_eq!(
                        sysvar.type_ident, *type_ident,
                        "{} provides stale sysvar type for {:?}",
                        capability.crate_name, concept
                    );
                }
            }
        }
    }

    #[test]
    fn registry_crate_names_unique_and_sorted() {
        let names = CAPABILITY_REGISTRY
            .iter()
            .map(|capability| capability.crate_name)
            .collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();

        assert_eq!(names, sorted);
    }

    #[test]
    fn local_struct_never_resolves() {
        let origins = HashMap::from([("Clock".to_string(), "crate".to_string())]);
        let deps = solana_project::parse_manifest_deps("[dependencies]\nsolana-clock = \"2\"");

        assert_eq!(resolve_concept("Clock", &origins, &deps), None);
    }

    #[test]
    fn dep_missing_never_resolves() {
        let origins = HashMap::from([("Clock".to_string(), "solana_clock".to_string())]);
        let deps = solana_project::parse_manifest_deps("[dependencies]\n");

        assert_eq!(resolve_concept("Clock", &origins, &deps), None);
    }

    #[test]
    fn alias_resolves_to_exported_concept() {
        let document = ParsedDocument::parse("use solana_clock::Clock as SolanaClock;\n").unwrap();
        let deps = solana_project::parse_manifest_deps("[dependencies]\nsolana-clock = \"2\"");

        assert_eq!(
            resolve_concept_with_aliases(
                "SolanaClock",
                &document.symbols().import_origins,
                &document.symbols().import_aliases,
                &deps,
            ),
            Some(Concept::SysvarClock)
        );
    }

    #[test]
    fn package_rename_resolves_to_exported_concept() {
        let document = ParsedDocument::parse("use chain_clock::Clock;\n").unwrap();
        let deps = solana_project::parse_manifest_deps(
            r#"
[dependencies]
chain-clock = { package = "solana-clock", version = "2" }
"#,
        );

        assert_eq!(
            resolve_concept("Clock", &document.symbols().import_origins, &deps),
            Some(Concept::SysvarClock)
        );
    }

    #[test]
    fn modular_fixture_resolves_clock_concept() {
        let document = ParsedDocument::parse(include_str!(
            "../../tests/fixtures/capability_resolution/modular/src/lib.rs"
        ))
        .unwrap();
        let deps = solana_project::parse_manifest_deps(include_str!(
            "../../tests/fixtures/capability_resolution/modular/Cargo.toml"
        ));

        assert_eq!(
            resolve_concept("Clock", &document.symbols().import_origins, &deps),
            Some(Concept::SysvarClock)
        );
    }

    #[test]
    fn no_dep_fixture_stays_silent_without_declared_dep() {
        let document = ParsedDocument::parse(include_str!(
            "../../tests/fixtures/capability_resolution/no_dep/src/lib.rs"
        ))
        .unwrap();
        let deps = solana_project::parse_manifest_deps(include_str!(
            "../../tests/fixtures/capability_resolution/no_dep/Cargo.toml"
        ));

        assert_eq!(
            resolve_concept("Clock", &document.symbols().import_origins, &deps),
            None
        );
    }

    #[test]
    fn pinocchio_fixture_resolves_clock_concept() {
        let document = ParsedDocument::parse(include_str!(
            "../../tests/fixtures/capability_resolution/pinocchio/src/lib.rs"
        ))
        .unwrap();
        let deps = solana_project::parse_manifest_deps(include_str!(
            "../../tests/fixtures/capability_resolution/pinocchio/Cargo.toml"
        ));

        assert_eq!(
            resolve_concept("Clock", &document.symbols().import_origins, &deps),
            Some(Concept::SysvarClock)
        );
    }
}
