#[tokio::main]
async fn main() {
    if let Err(error) = seagrass_core::cli::run_from_env().await {
        eprintln!("{error}");
        std::process::exit(seagrass_core::cli::USAGE_ERROR_EXIT_CODE);
    }
}
