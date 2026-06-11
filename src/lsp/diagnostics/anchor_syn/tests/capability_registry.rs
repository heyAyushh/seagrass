use super::*;

const MODULAR_CLOCK_ALIAS_SOURCE: &str = r#"
use anchor_lang::prelude::*;
use solana_clock::Clock as SolanaClock;

#[derive(Accounts)]
pub struct ReadClock<'info> {
    pub clock: Sysvar<'info, SolanaClock>,
}
"#;

#[test]
fn modular_crate_alias_sysvar_generic_is_accepted_with_dep() {
    let document = ParsedDocument::parse(MODULAR_CLOCK_ALIAS_SOURCE).unwrap();
    let manifest_deps =
        crate::solana_project::parse_manifest_deps("[dependencies]\nsolana-clock = \"2\"");
    let diagnostics = collect_with_context(&document, None, &manifest_deps);

    assert!(
        diagnostics.is_empty(),
        "declared modular sysvar imports should stay quiet: {diagnostics:#?}"
    );
}
