//! The `dobukasu` binary: link the bundled plug-ins, then hand over to the CLI.

use std::process::ExitCode;

// The engines register themselves at link time; without these the linker is
// free to drop them and the binary would ship with no engines at all.
use dobukasu_engine_bing as _;
use dobukasu_engine_duckduckgo as _;
use dobukasu_engine_google as _;

use dobukasu::{Cli, plugin_directories};
use dobukasu_core::{Registry, discover};

#[tokio::main]
async fn main() -> ExitCode {
    let mut registry = match Registry::from_plugins() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    // Executable plug-ins: drop a `dobukasu-engine-<id>` file in the plugin
    // directory and it becomes available to `--engines <id>`.
    let configured = std::env::var("DOBUKASU_PLUGIN_PATH").ok();
    let home = std::env::var("HOME").ok();
    for directory in plugin_directories(configured.as_deref(), home.as_deref()) {
        for engine in discover(&directory) {
            registry.add(engine, false);
        }
    }

    // DOBUKASU_ENGINES picks the default set, so a plug-in can be made part of
    // a plain `dobukasu <query>` without touching the binary.
    if let Ok(list) = std::env::var("DOBUKASU_ENGINES") {
        let ids: Vec<String> = list
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if let Err(error) = registry.set_defaults(&ids) {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    }

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
