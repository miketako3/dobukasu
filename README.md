# dobukasu

A minimal meta-search CLI for AI agents. One query in, one merged JSON list out.

[![CI](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml/badge.svg)](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`dobukasu` does one thing: take a query, ask several search engines in
parallel, fuse their rankings into one list, and print it as JSON. It is the
part of [SearXNG](https://github.com/searxng/searxng) an agent actually needs —
no UI, no categories, no preferences, no server — rebuilt in Rust around an
everything-is-a-plugin core.

[日本語](README.ja.md)

```
$ dobukasu "rust async runtime" --limit 2
{
  "query": "rust async runtime",
  "results": [
    {
      "title": "The Async Ecosystem",
      "url": "https://rust-lang.github.io/async-book/",
      "snippet": "An asynchronous runtime, or executor, …",
      "engines": ["bing", "duckduckgo"],
      "score": 0.03278688524590164
    }
  ],
  "engines": [
    {"id": "bing", "status": "ok", "count": 10, "duration_ms": 412},
    {"id": "duckduckgo", "status": "ok", "count": 10, "duration_ms": 388},
    {"id": "google", "status": "error", "count": 0, "duration_ms": 201,
     "error": "upstream blocked this client: …"}
  ],
  "took_ms": 430
}
```

## Why a CLI and not an API

An agent that wants to search needs one call and a stable contract, not a
service to babysit. A process that prints JSON and exits is the smallest
possible surface: no port, no lifecycle, no auth, nothing left running between
questions. Every invocation is independent, so a wedged run cannot poison the
next one.

- **Everything is a plugin.** The core crate never names an engine. Each engine
  is its own crate that submits itself to a compile-time registry.
- **Rank fusion, not concatenation.** Reciprocal rank fusion means a page two
  engines agree on outranks a page that is merely first in one.
- **Partial failure is normal.** One engine blocked or timing out does not fail
  the search; it shows up in `engines[]` with a status and a reason.
- **Offline test suite.** Parsers are pinned to captured pages, and the CLI
  contract is tested through its real argument parser and output.

## Install

```sh
cargo install --path crates/cli     # installs `dobukasu` into ~/.cargo/bin
# or
cargo build --release               # ./target/release/dobukasu
```

## Use

```sh
dobukasu "query"                          # merged results
dobukasu "query" --limit 5                # keep five
dobukasu "query" --engines bing           # only ask Bing
dobukasu "query" --timeout-ms 4000        # per-engine deadline
dobukasu "query" --no-dedup               # same page once per engine
dobukasu engines                          # what this build can ask
dobukasu --help
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `--limit N` | `10` | merged results to keep |
| `--engines a,b` | every default engine | only ask these |
| `--timeout-ms N` | `8000` | per-engine deadline |
| `--no-dedup` | off | keep the same page once per engine |

Output is JSON on stdout: `{query, results[], engines[], took_ms}`, where each
result carries the `engines` that returned it and its fusion `score`.

| Exit code | Meaning |
| --- | --- |
| `0` | at least one engine answered |
| `1` | every engine failed |
| `2` | bad usage (unknown flag, unknown engine, empty query) |

## Use it as an agent skill

[`skills/dobukasu/SKILL.md`](skills/dobukasu/SKILL.md) is a ready-made skill:
it tells an agent when to search, how to run the CLI, how to read `engines[]`,
and what to do when a search comes back empty.

```sh
mkdir -p ~/.agents/skills
cp -r skills/dobukasu ~/.agents/skills/
```

That is the whole integration. No MCP server, no tool schema to keep in sync.

## Engines

| id | keyless | how |
| --- | --- | --- |
| `bing` | yes | scrapes `bing.com/search`; decodes `ck/a` redirects |
| `duckduckgo` | yes | scrapes `html.duckduckgo.com`; decodes `uddg` redirects |
| `google` | partly | scrapes `www.google.com/wml/search` with Symbian Nokia user agents — the trick SearXNG uses to get a JavaScript-free layout — and switches to the Custom Search JSON API when `GOOGLE_API_KEY` and `GOOGLE_CX` are set |

All three are compiled in and enabled by default.

**About Google.** The ordinary `/search` endpoint now answers automated clients
with a JavaScript interstitial or a 403, which is why scraping it directly
fails. The `/wml/search` layout is the one Google still serves to very old
mobile user agents, and it is what SearXNG parses today. It is still a scraped
endpoint: if Google refuses this host, the engine reports `blocked` with a
message saying so rather than returning an empty list. Set `GOOGLE_API_KEY` and
`GOOGLE_CX` (a free Custom Search key) for a contract that does not depend on
Google's goodwill.

**About DuckDuckGo.** It rate-limits bursts, answering with a 202 challenge
page. That page is recognised and reported as `blocked`; "no hits" and "we were
turned away" are different answers.

## Write an engine plugin

An engine is a crate: it implements `Engine`, submits a `Plugin` with
`inventory`, and gets linked into the binary. The core never imports it.

```rust
pub const ID: &str = "example";

inventory::submit! {
    Plugin {
        id: ID,
        label: "Example",
        description: "Keyless. Uses example.com/search.",
        enabled_by_default: false,
        build,
    }
}

fn build() -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Example))
}

#[async_trait]
impl Engine for Example {
    fn id(&self) -> &'static str { ID }

    async fn search(&self, query: &SearchQuery, ctx: &EngineContext) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse("https://example.com/search")?;
        url.query_pairs_mut().append_pair("q", &query.text);
        let html = ctx.fetch_text(url).await?;
        Ok(parse(&html))
    }
}

#[must_use]
pub fn parse(html: &str) -> Vec<Hit> { /* selectors */ }
```

Keep the parser a pure `&str -> Vec<Hit>` function. That is the engine's real
contract with its upstream, it survives any refactor of the HTTP plumbing, and
it is what the fixture tests exercise. Then link it in `crates/cli/src/main.rs`:

```rust
use dobukasu_engine_example as _;
```

## Architecture

```
crates/core             Engine trait, compile-time registry, rank fusion, runner
crates/engine-bing      one plug-in each
crates/engine-duckduckgo
crates/engine-google
crates/cli              the `dobukasu` binary: links every bundled plug-in
skills/dobukasu         the agent skill that calls it
```

A run flows: parse arguments → resolve engines → fan out with a per-engine
timeout → canonicalise URLs and fuse ranks → truncate → print.

## Testing

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The suite asserts observable behaviour through public seams. Engines are the
only process-external dependency, so they are the only thing stubbed; URL
identity, ranking, argument parsing, exit codes and error reporting are all
exercised for real. [`docs/testing.md`](docs/testing.md) explains the shape of
the suite and what mutation testing has caught.

```sh
cargo install cargo-mutants
cargo mutants --workspace
```

## Known limits

- **Scraped engines are best effort.** Under bot mitigation, Bing sometimes
  answers `200` with unrelated pages instead of an error. `dobukasu` reports
  what the upstream returned; it cannot judge relevance from the markup.
- **No pagination.** One page per engine. Reformulate the query instead.
- **No caching.** Every invocation fans out; wrap it if you call it in a loop.
- **No server.** Three engines per query, then exit. That is deliberate.

## Legal and etiquette

`dobukasu` fetches public search result pages the way a browser does. That is
not a licence to ignore the terms of service of the engines you enable, nor
`robots.txt`, nor rate limits. Run it for your own use, keep the request rate
low, and prefer an official API (as the Google plug-in does) when one exists.
The authors are not responsible for how you use it.

## License

MIT. See [LICENSE](LICENSE).
