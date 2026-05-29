pub mod anchor;
mod app;
mod core;
mod lsp;
mod runtime;
pub mod server;
pub mod solana;
mod testing;

pub use app::cli;
pub use core::{document, project, syntax};
pub use runtime::range;
pub use testing::fuzz_harness;

pub(crate) use anchor::{
    account_members, account_semantics, analysis as anchor_analysis, constraint_catalog,
    constraint_ranges, constraint_text, types as anchor_types,
};
pub use anchor::{errors as anchor_errors, support as anchor_support};
pub(crate) use core::{definition_bridge, document_stub, evidence, workspace};
pub(crate) use lsp::{
    actions, assists, code_lens, completions, diagnostics, document_links, folding, hover,
    inlay_hints, navigation, renaming, selection_ranges, semantic_tokens, signature_help,
};
pub(crate) use runtime::{
    debounce, hotpath, query_cache, salsa_db, server_observability, server_types,
};
pub use solana::project as solana_project;
pub(crate) use solana::{ecosystem, program_artifacts};

pub const SEAGRASS_FEEDBACK_URL: &str = "https://github.com/heyAyushh/seagrass/issues";
