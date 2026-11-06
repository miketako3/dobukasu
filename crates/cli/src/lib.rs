//! The command line an agent actually calls. Everything observable is here:
//! argument parsing, the JSON on stdout, the message on stderr, the exit code.

use std::time::Duration;

use dobukasu_core::{Registry, SearchOutcome, SearchQuery, plugins, search as run_search};
use serde::Serialize;

/// Default per-engine deadline.
pub const DEFAULT_TIMEOUT_MS: u64 = 8_000;
/// Default number of merged results.
pub const DEFAULT_LIMIT: usize = 10;

/// One complete run: what the caller sees when the process exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub code: u8,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    fn ok(stdout: String) -> Self {
        Self {
            code: 0,
            stdout,
            stderr: String::new(),
        }
    }

    fn failed(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            stdout: String::new(),
            stderr: format!("error: {}\n", message.into()),
        }
    }
}

/// What the argument list asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Search(SearchArgs),
    Engines,
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchArgs {
    pub query: String,
    pub limit: Option<usize>,
    pub engines: Option<Vec<String>>,
    pub timeout_ms: Option<u64>,
    pub dedup: bool,
}

/// A CLI bound to a registry and an HTTP client. Tests build one from stub
/// engines; the binary builds one from the linked plug-ins.
pub struct Cli {
    registry: Registry,
    client: reqwest::Client,
}

impl Cli {
    #[must_use]
    pub fn new(registry: Registry, client: reqwest::Client) -> Self {
        Self { registry, client }
    }

    /// Run one command line (without the program name).
    pub async fn run(&self, argv: &[String]) -> Output {
        let command = match parse(argv) {
            Ok(command) => command,
            Err(message) => return Output::failed(2, message),
        };

        match command {
            Command::Help => Output::ok(help()),
            Command::Version => Output::ok(format!("dobukasu {}\n", env!("CARGO_PKG_VERSION"))),
            Command::Engines => Output::ok(engines_json(&self.registry)),
            Command::Search(request) => self.search(request).await,
        }
    }

    async fn search(&self, request: SearchArgs) -> Output {
        let engines = match self.registry.resolve(request.engines.as_deref()) {
            Ok(engines) => engines,
            Err(error) => return Output::failed(2, error.to_string()),
        };

        let mut query = SearchQuery::new(request.query.clone());
        query.limit = request.limit.unwrap_or(DEFAULT_LIMIT).max(1);

        let outcome = run_search(
            &self.client,
            engines,
            &query,
            Duration::from_millis(request.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)),
            request.dedup,
        )
        .await;

        let code = u8::from(outcome.all_failed());
        match render(&outcome) {
            Ok(stdout) => Output {
                code,
                stdout,
                stderr: String::new(),
            },
            Err(error) => Output::failed(1, error.to_string()),
        }
    }
}

fn render(outcome: &SearchOutcome) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(outcome).map(|mut text| {
        text.push('\n');
        text
    })
}

#[derive(Serialize)]
struct EngineEntry {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    default: bool,
}

#[derive(Serialize)]
struct UnavailableEntry {
    id: &'static str,
    reason: String,
}

#[derive(Serialize)]
struct EngineList {
    engines: Vec<EngineEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    unavailable: Vec<UnavailableEntry>,
}

fn engines_json(registry: &Registry) -> String {
    let engines = registry
        .ids()
        .iter()
        .map(|id| {
            let plugin = plugins().into_iter().find(|plugin| plugin.id == *id);
            EngineEntry {
                id,
                label: plugin.map_or(id, |plugin| plugin.label),
                description: plugin.map_or("", |plugin| plugin.description),
                default: registry.is_default(id),
            }
        })
        .collect();
    let unavailable = registry
        .unavailable()
        .iter()
        .map(|(id, reason)| UnavailableEntry {
            id,
            reason: reason.clone(),
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&EngineList {
        engines,
        unavailable,
    })
    .unwrap_or_else(|_| "{\"engines\":[]}".to_owned());
    text.push('\n');
    text
}

/// Parse the argument list. Unknown flags are errors, not guesses.
pub fn parse(argv: &[String]) -> Result<Command, String> {
    let mut words: Vec<&str> = Vec::new();
    let mut limit = None;
    let mut engines = None;
    let mut timeout_ms = None;
    let mut dedup = true;
    let mut command = None;

    let mut index = 0;
    while index < argv.len() {
        let argument = argv[index].as_str();
        match argument {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--limit" => {
                let value = next_value(argv, &mut index, "--limit")?;
                limit = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("--limit must be a positive integer, got {value}"))?,
                );
            }
            "--engines" => {
                let value = next_value(argv, &mut index, "--engines")?;
                let ids: Vec<String> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
                if ids.is_empty() {
                    return Err("--engines needs at least one engine id".to_owned());
                }
                engines = Some(ids);
            }
            "--timeout-ms" => {
                let value = next_value(argv, &mut index, "--timeout-ms")?;
                timeout_ms = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("--timeout-ms must be a number, got {value}"))?,
                );
            }
            "--no-dedup" => dedup = false,
            other if other.starts_with("--") => return Err(format!("unknown flag: {other}")),
            other => {
                if command.is_none() && matches!(other, "search" | "engines") {
                    command = Some(other);
                } else {
                    words.push(other);
                }
            }
        }
        index += 1;
    }

    if command == Some("engines") {
        return Ok(Command::Engines);
    }

    let query = words.join(" ").trim().to_owned();
    if query.is_empty() {
        return Err("a query is required: dobukasu \"your query\"".to_owned());
    }
    Ok(Command::Search(SearchArgs {
        query,
        limit,
        engines,
        timeout_ms,
        dedup,
    }))
}

fn next_value(argv: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    argv.get(*index)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn help() -> String {
    format!(
        "\
dobukasu {version} — ask several search engines, get one merged JSON list

USAGE
    dobukasu <query> [--limit N] [--engines a,b] [--timeout-ms N] [--no-dedup]
    dobukasu engines
    dobukasu --help | --version

FLAGS
    --limit N        merged results to keep (default {limit})
    --engines a,b    only ask these engines (default: all, see `dobukasu engines`)
    --timeout-ms N   per-engine deadline (default {timeout})
    --no-dedup       keep the same page once per engine

OUTPUT
    JSON on stdout: {{query, results[], engines[], took_ms}}.
    Exit 0 when at least one engine answered, 1 when all failed, 2 on usage errors.

ENVIRONMENT
    GOOGLE_API_KEY, GOOGLE_CX   use the Google Custom Search JSON API instead of
                                the scraped endpoint (which Google often blocks)
",
        version = env!("CARGO_PKG_VERSION"),
        limit = DEFAULT_LIMIT,
        timeout = DEFAULT_TIMEOUT_MS,
    )
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};

    fn parse_str(text: &str) -> Result<Command, String> {
        let argv: Vec<String> = text.split_whitespace().map(ToOwned::to_owned).collect();
        parse(&argv)
    }

    #[test]
    fn a_bare_query_is_a_search() {
        let parsed = parse_str("rust async runtime").expect("parses");
        let Command::Search(args) = parsed else {
            panic!("expected a search");
        };
        assert_eq!(args.query, "rust async runtime");
    }

    #[test]
    fn flags_are_optional_and_typed() {
        let parsed = parse_str("q --limit 3 --timeout-ms 1500 --no-dedup").expect("parses");
        let Command::Search(args) = parsed else {
            panic!("expected a search");
        };
        assert_eq!(args.limit, Some(3));
        assert_eq!(args.timeout_ms, Some(1500));
        assert!(!args.dedup);
    }

    #[test]
    fn the_engines_list_is_split_on_commas() {
        let parsed = parse_str("q --engines bing,duckduckgo").expect("parses");
        let Command::Search(args) = parsed else {
            panic!("expected a search");
        };
        assert_eq!(
            args.engines,
            Some(vec!["bing".to_owned(), "duckduckgo".to_owned()])
        );
    }

    #[test]
    fn help_and_version_win_over_the_query() {
        assert_eq!(parse_str("--help").expect("parses"), Command::Help);
        assert_eq!(parse_str("--version").expect("parses"), Command::Version);
    }

    #[test]
    fn an_unknown_flag_is_refused() {
        assert!(parse_str("q --verbose").is_err());
    }

    #[test]
    fn a_missing_value_is_refused() {
        assert!(parse_str("q --limit").is_err());
    }

    #[test]
    fn hits_are_not_mistaken_for_queries() {
        assert_eq!(parse_str("engines").expect("parses"), Command::Engines);
    }
}
