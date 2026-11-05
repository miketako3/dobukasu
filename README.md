# dobukasu

A minimal meta-search HTTP API for AI agents. JSON in, JSON out, no UI.

`dobukasu` does one thing: take a query, ask several search engines in
parallel, merge their rankings into one list, and return it. It is the part of
[SearXNG](https://github.com/searxng/searxng) an agent actually needs, with the
UI, categories, preferences and engine plugin zoo removed — and rebuilt in Rust
around an everything-is-a-plugin core.

[日本語](README.ja.md)

```
$ curl -s 'http://127.0.0.1:8080/search?q=rust+async+runtime&limit=2' | jq
{
  "query": "rust async runtime",
  "results": [
    {
      "title": "Tokio - An asynchronous Rust runtime",
      "url": "https://tokio.rs/",
      "snippet": "Tokio is an asynchronous runtime for the Rust programming language.",
      "engines": ["bing", "duckduckgo"],
      "score": 0.03278688524590164
    },
    {
      "title": "Async in depth - Tokio",
      "url": "https://tokio.rs/tokio/tutorial/async",
      "snippet": "Async Rust is a cooperative multitasking model.",
      "engines": ["bing"],
      "score": 0.016129032258064516
    }
  ],
  "engines": [
    {"id": "bing", "status": "ok", "count": 10, "duration_ms": 412},
    {"id": "duckduckgo", "status": "ok", "count": 10, "duration_ms": 388},
    {"id": "google", "status": "error", "count": 0, "duration_ms": 201,
     "error": "upstream blocked this client: Google served the JavaScript interstitial; set api_key and cx for the CSE backend"}
  ],
  "took_ms": 430
}
```

## Why

An agent that wants to search the web needs three things: a stable JSON
contract, results merged from more than one engine, and honest failure
reporting. It does not need facets, an image proxy, an opensearch description,
or a settings page. `dobukasu` is that reduced surface.

- **One endpoint.** `GET /search?q=` and `POST /search`. Nothing else to learn.
- **Engines are plugins.** The core crate does not name a single engine. Each
  engine is its own crate that submits itself to a compile-time registry.
- **Rank fusion, not concatenation.** Reciprocal rank fusion means a page two
  engines agree on outranks a page that is merely first in one.
- **Partial failure is normal.** One engine timing out or getting blocked does
  not fail the request; it shows up in `engines[]` with a status and a message.
- **Offline test suite.** Parsers are tested against captured pages, and the
  HTTP contract is tested through the real router with only the network
  boundary stubbed.

## Quick start

```sh
cargo run --release
# or
cargo install --path crates/server
dobukasu
```

The server listens on `127.0.0.1:8080` by default.

```sh
curl -s 'http://127.0.0.1:8080/search?q=hello'
curl -s 'http://127.0.0.1:8080/search?q=hello&engines=bing&limit=5'
curl -s -X POST http://127.0.0.1:8080/search \
  -H 'content-type: application/json' \
  -d '{"q": "hello", "limit": 5, "lang": "ja"}'
curl -s 'http://127.0.0.1:8080/engines'
curl -s 'http://127.0.0.1:8080/healthz'
```

## HTTP API

### `GET /search` / `POST /search`

| Parameter    | Type            | Default            | Meaning                                      |
| ------------ | --------------- | ------------------ | -------------------------------------------- |
| `q`          | string          | required           | Query text, at most 1024 characters.         |
| `engines`    | comma list      | every enabled one  | `bing,duckduckgo`                            |
| `limit`      | integer         | `10`               | Merged results, clamped to `search.max_limit`. |
| `lang`       | string          | none               | Passed to engines (`hl`/`setlang`/`kl`).     |
| `region`     | string          | none               | Country hint (`gl`/`cc`).                    |
| `time_range` | `day\|week\|month\|year` | none      | Freshness filter where supported.            |
| `safesearch` | boolean         | `false`            | Safe-search flag where supported.            |
| `dedup`      | boolean         | `true`             | Merge identical URLs across engines.         |
| `timeout_ms` | integer         | `8000`             | Per-engine deadline, clamped to 100–30000.   |
| `format`     | —               | —                  | Not supported: the response is always JSON.  |

`POST` takes the same fields as a JSON object and accepts numbers and booleans
natively (`{"limit": 5, "safesearch": true}`). Unknown fields are rejected.

Response:

```jsonc
{
  "query": "…",           // the query as received
  "results": [            // merged, de-duplicated, best first
    {
      "title": "…",
      "url": "…",
      "snippet": "…",
      "engines": ["bing", "duckduckgo"],  // first engine first
      "score": 0.0327                     // sum of 1/(60 + rank)
    }
  ],
  "engines": [            // one report per engine that was asked
    {
      "id": "bing",
      "status": "ok",     // ok | timeout | error
      "count": 10,
      "duration_ms": 412,
      "error": "…"        // present only for error
    }
  ],
  "took_ms": 430
}
```

Status codes: `200` even when some engines fail, `400` for a bad request,
`502` when every engine failed. Errors always use the same shape:

```json
{"error": {"code": "unknown_engine", "message": "unknown engine: nope"}}
```

Codes: `invalid_request`, `unknown_engine`, `no_engine_enabled`,
`all_engines_failed`, `config`.

### `GET /engines`

Lists the engines this process can call, whether each is in the default set,
and the plug-in description. Use it to discover what is available before
building a query.

### `GET /healthz`

`{"status":"ok","version":"0.1.0"}`.

## Engines

| id           | keyless | notes                                                                 |
| ------------ | ------- | --------------------------------------------------------------------- |
| `bing`       | yes     | Scrapes `bing.com/search`; decodes `ck/a` redirects.                  |
| `duckduckgo` | yes     | Scrapes `html.duckduckgo.com`; decodes `uddg` redirects.              |
| `google`     | partly  | HTML by default, but Google usually serves a JavaScript interstitial. Set `api_key` and `cx` (Custom Search JSON API) to make it reliable. |

All three are compiled in and enabled by default. Google is the one that will
report `error` out of the box on most hosts; that is deliberate — claiming
"no results" would be a lie. Either give it Custom Search credentials or turn
it off:

```toml
[engines.google]
enabled = false
```

DuckDuckGo rate-limits bursts. When it does, it returns a block page with
status 202; the plug-in reports an empty result set, and `engines[]` shows
`count: 0` rather than inventing results.

## Configuration

Precedence: built-in defaults → `dobukasu.toml` (or `$DOBUKASU_CONFIG`) →
environment variables. See [`dobukasu.example.toml`](dobukasu.example.toml).

| Variable               | Overrides                |
| ---------------------- | ------------------------ |
| `DOBUKASU_BIND`        | `server.bind`            |
| `DOBUKASU_LIMIT`       | `search.limit`           |
| `DOBUKASU_MAX_LIMIT`   | `search.max_limit`       |
| `DOBUKASU_TIMEOUT_MS`  | `search.timeout_ms`      |
| `DOBUKASU_DEDUP`       | `search.dedup`           |
| `DOBUKASU_USER_AGENT`  | `search.user_agent`      |
| `DOBUKASU_ENGINES`     | `search.engines`         |
| `DOBUKASU_CONFIG`      | config file path         |
| `GOOGLE_API_KEY`       | Google `api_key`         |
| `GOOGLE_CX`            | Google `cx`              |
| `RUST_LOG`             | log filter (`dobukasu=info`) |

## Writing an engine plug-in

An engine is a crate. It implements `Engine`, submits a `Plugin` with
`inventory`, and is linked into the server binary. The core never imports it.

```rust
use std::sync::Arc;
use async_trait::async_trait;
use dobukasu_core::{Engine, EngineContext, EngineError, Hit, Plugin, PluginOptions, SearchQuery};

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

fn build(_options: &PluginOptions) -> Result<Arc<dyn Engine>, EngineError> {
    Ok(Arc::new(Example))
}

pub struct Example;

#[async_trait]
impl Engine for Example {
    fn id(&self) -> &'static str { ID }

    async fn search(&self, query: &SearchQuery, ctx: &EngineContext) -> Result<Vec<Hit>, EngineError> {
        let mut url = url::Url::parse("https://example.com/search")?;
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
it is what the offline fixture tests exercise.

Register it by adding the crate to `crates/server/Cargo.toml` and linking it in
`main.rs`:

```rust
use dobukasu_engine_example as _;
```

## Architecture

```
crates/core            engine trait, compile-time registry, config, rank fusion, HTTP API
crates/engine-bing     one plug-in each
crates/engine-duckduckgo
crates/engine-google
crates/server          the `dobukasu` binary: links every bundled plug-in
```

Everything is a plugin in the sense that matters: `core` has no dependency on
any engine, `Registry` is built from whatever `inventory` collected, and
disabling an engine is configuration, not a code change. Rank fusion and the
HTTP contract live in core and are shared by every engine.

A request flows: parse parameters → resolve engines → fan out with a per-engine
timeout → canonicalise URLs and fuse ranks (`fuse`) → truncate → serialise.

## Testing

```sh
cargo test --workspace          # offline: fixtures, HTTP contract, fusion, config
cargo test --workspace -- --ignored   # live smoke tests (needs network)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The suite follows one rule: assert observable behaviour through the public API.
Engines are the only process-external dependency, so they are the only thing
stubbed; URL identity, ranking, parameter parsing, error shapes and status
codes are all exercised through the real router.

```sh
cargo install cargo-mutants
cargo mutants -p dobukasu-core
```

Mutation testing is the quality gate for weak assertions: a surviving mutant on
URL canonicalisation or fusion scoring means the suite is not actually pinning
the behaviour it claims to. The reasoning behind the suite's shape is written
down in [docs/testing.md](docs/testing.md).

## Known limits

- **Scraped engines are best effort.** Under bot mitigation, Bing and
  DuckDuckGo sometimes answer `200` with unrelated pages instead of an error.
  `dobukasu` reports what the upstream returned; it cannot tell relevance from
  the markup. Treat `engines[]` as a status report, not a quality guarantee.
- **No pagination.** One page per engine, no `start`/`page` parameter. An agent
  that wants more should rephrase the query.
- **No caching.** Every request fans out. Put it behind your own cache if you
  call it in a loop.
- **No authentication.** Bind it to loopback or put a reverse proxy in front
  before exposing it to a network.

## Legal and etiquette

`dobukasu` fetches public search result pages the same way a browser does. That
is not a licence to ignore the terms of service of the engines you enable, nor
`robots.txt`, nor rate limits. Run it for your own use, keep the request rate
low, deploy it behind authentication if it is reachable from the internet, and
prefer an engine's official API (as the Google plug-in does) when one exists.
The authors are not responsible for how you use it.

## License

MIT. See [LICENSE](LICENSE).
