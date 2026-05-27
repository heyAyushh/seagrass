#[tokio::main]
async fn main() {
    if let Err(error) = seagrass::cli::run_from_env().await {
        eprintln!("{error}");
        std::process::exit(seagrass::cli::USAGE_ERROR_EXIT_CODE);
    }
}
