//! The `dobukasu` binary: link the bundled plug-ins, then hand over to the CLI.

use std::process::ExitCode;

// The engines register themselves at link time; without these the linker is
// free to drop them and the binary would ship with no engines at all.
use dobukasu_engine_bing as _;
use dobukasu_engine_brave as _;
use dobukasu_engine_duckduckgo as _;
use dobukasu_engine_google as _;

use dobukasu::Cli;
use dobukasu_core::Registry;

#[tokio::main]
async fn main() -> ExitCode {
    let registry = match Registry::from_plugins() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let client = match reqwest::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            eprintln!("error: cannot build the HTTP client: {error}");
            return ExitCode::from(2);
        }
    };

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let output = Cli::new(registry, client).run(&argv).await;
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);
    ExitCode::from(output.code)
}
