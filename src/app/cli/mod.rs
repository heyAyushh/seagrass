mod analyze;
mod diagnostics;
mod sarif;

use {
    clap::{Parser, Subcommand},
    std::error::Error,
};

pub const USAGE_ERROR_EXIT_CODE: i32 = 2;
const FINDINGS_ERROR_EXIT_CODE: i32 = 1;

pub async fn run_from_env() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() <= 1 {
        crate::server::run_stdio().await;
        return Ok(());
    }

    let has_error = Cli::parse_from(args).run()?;
    if has_error {
        std::process::exit(FINDINGS_ERROR_EXIT_CODE);
    }
    Ok(())
}

#[derive(Debug, Parser)]
#[command(
    name = "seagrass",
    bin_name = "seagrass",
    version = env!("CARGO_PKG_VERSION"),
    about = "Seagrass Anchor language tooling",
    after_help = "Examples:\n  seagrass\n  seagrass diagnostics programs/demo/src/lib.rs --json\n  seagrass diagnostics --help"
)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

impl Cli {
    fn run(self) -> Result<bool, Box<dyn Error>> {
        match self.command {
            CliCommand::Analyze(command) => command.run(),
            CliCommand::Diagnostics(command) => command.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Print a Solana codebase intelligence report for a Rust file or directory.
    #[command(after_help = analyze::ANALYZE_HELP)]
    Analyze(analyze::AnalyzeCommand),

    /// Run Seagrass diagnostics on a Rust file or directory and print JSON.
    #[command(after_help = diagnostics::DIAGNOSTICS_HELP)]
    Diagnostics(diagnostics::DiagnosticsCommand),
}
