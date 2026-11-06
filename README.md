# dobukasu

**SearXNG's architecture, rebuilt for agents: agent-native, plugin-first, minimal — in Rust.**

[![CI](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml/badge.svg)](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml) [![Release](https://img.shields.io/github/v/release/miketako3/dobukasu)](https://github.com/miketako3/dobukasu/releases/latest) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`dobukasu` does what SearXNG does — ask several search engines at once and fuse
their rankings — but rebuilt from the ground up with three constraints:
**agent-native** (a command that prints JSON and exits, not a service),
**plugin-first** (the core names no engine; every engine is a crate that
registers itself), and **minimal** (no UI, no categories, no preferences, no
database). One query in, one merged list out. No account, no key, no server.

```
$ dobukasu "rust async runtime" --limit 3
{
  "query": "rust async runtime",
  "results": [
    {
      "title": "The Async Ecosystem",
      "url": "https://rust-lang.github.io/async-book/",
      "snippet": "An asynchronous runtime, or executor, is responsible for …",
      "engines": ["bing", "duckduckgo"],
      "score": 0.03278688524590164
    },
    …
  ],
  "engines": [
    {"id": "bing",       "status": "ok",    "count": 10, "duration_ms": 412},
    {"id": "duckduckgo", "status": "ok",    "count": 10, "duration_ms": 388},
    {"id": "google",     "status": "error", "count": 0,  "duration_ms": 201,
     "error": "upstream blocked this client: …"}
  ],
  "took_ms": 430
}
```

## The name

禪院直哉 — Naoya Zen'in, the self-declared fastest sorcerer of *Jujutsu Kaisen*.
His technique carves a second into frames, and he moves faster than anyone by
tracing every one of them.

The pose has a name in the fandom — ドブカスラッシュ, the dobukasu rush: the
technique swings his right arm for him, the left hand fixes his hair, and the
face stays bored the whole time.

`dobukasu` does the same to a query. Split it across engines, run every frame in
parallel, and merge them into one answer that is faster and better sourced than
any single engine could give you.

A homage, not a product tie-in: this project is not affiliated with the manga,
its author or its publishers.

## Usage

```sh
dobukasu "query"                    # merged results as JSON
dobukasu "query" --limit 5          # keep five
dobukasu "query" --engines bing     # ask one engine
dobukasu "query" --timeout-ms 4000  # per-engine deadline
dobukasu engines                    # what this build can ask
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `--limit N` | `10` | merged results to keep |
| `--engines a,b` | the free engines | ask only these |
| `--timeout-ms N` | `8000` | per-engine deadline |
| `--no-dedup` | off | keep the same page once per engine |

Exit codes are designed for agents: `0` when at least one engine answered, `1`
when every engine failed, `2` on a usage error. A failing engine never fails the
search — it is reported in `engines[]` with a status and a reason.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/miketako3/dobukasu/main/install.sh | sh
```

Prebuilt for macOS (arm64, x86_64) and Linux (x86_64). Set
`DOBUKASU_INSTALL_DIR` or `DOBUKASU_VERSION` to change where and which.

From source: `cargo install --path crates/cli`.

## Use it as an agent skill

[`skills/dobukasu/SKILL.md`](skills/dobukasu/SKILL.md) is the agent-facing half:
when to search, how to read `engines[]`, which engine to trust, and what to do
when a search comes back empty.

```sh
mkdir -p ~/.agents/skills
cp -r skills/dobukasu ~/.agents/skills/
```

No MCP server, no schema to keep in sync. A process that prints JSON and exits.

## What "rebuilt" changes

SearXNG is excellent and this borrows its architecture, not its surface. The
differences that matter when the caller is an agent rather than a person:

- **No server.** Nothing to start, watch, or secure. Each run is independent, so
  a wedged search cannot poison the next one.
- **Rank fusion, not concatenation.** Reciprocal rank fusion: a page two engines
  agree on outranks a page that is merely first in one.
- **Honest failure.** "Blocked", "timed out" and "no hits" are three different
  answers, and `engines[]` tells them apart instead of returning an empty list.
- **Everything is a plugin.** The core crate never names an engine.

## Engines

| id | keyless | how |
| --- | --- | --- |
| `bing` | yes | scrapes `bing.com/search`, naming the market (`setlang`, `cc`) so Bing answers with a real ranking; decodes `ck/a` redirects |
| `duckduckgo` | yes | scrapes `html.duckduckgo.com`; recognises the rate-limit challenge page and reports it as a block; decodes `uddg` redirects |
| `google` | yes\* | scrapes `www.google.com/wml/search` with Symbian Nokia user agents — the JavaScript-free layout SearXNG parses — and switches to the Custom Search JSON API when `GOOGLE_API_KEY` and `GOOGLE_CX` are set |

\* Keyless in principle, but Google blocks many networks outright. When it does,
the engine says `blocked` with the reason rather than pretending it found
nothing. Credentials, or a host Google has not blacklisted, are what fix it.

The default set is the free engines. Anything that needs an account stays out of
the default set and is reachable with `--engines` once configured — the free path
is the one that always works.

```sh
dobukasu engines   # what is available, which are default, and what is not, and why
```

## Plug-ins

An engine can be a compiled-in crate or an executable you drop in a directory.

**Executable plug-ins** are the way third parties add engines: any program named
`dobukasu-engine-<id>` that reads one JSON request on stdin and writes one JSON
response on stdout. No Rust, no rebuild, no ABI.

```sh
mkdir -p ~/.config/dobukasu/plugins
cp dobukasu-engine-wikipedia ~/.config/dobukasu/plugins/ && chmod +x ~/.config/dobukasu/plugins/*
dobukasu "rust" --engines wikipedia
```

Write, test, install and publish instructions, plus the exact request/response
shape: [`docs/plugins.md`](docs/plugins.md). A runnable example:
[`examples/plugins/dobukasu-engine-wikipedia`](examples/plugins/dobukasu-engine-wikipedia).

**Bundled engines** are Rust crates. One implements `Engine` and submits a
`Plugin` with `inventory`; the core never imports it, so adding one touches
three files: the crate, `crates/cli/Cargo.toml`, and one line in
`crates/cli/src/main.rs`.

```rust
pub const ID: &str = "example";

inventory::submit! {
    Plugin {
        id: ID,
        label: "Example",
        description: "Keyless. Uses example.com/search.",
        enabled_by_default: true,
        build,
    }
}

#[async_trait]
impl Engine for Example {
    fn id(&self) -> &'static str { ID }

    async fn search(&self, query: &SearchQuery, ctx: &EngineContext) -> Result<Vec<Hit>, EngineError> {
        let mut url = Url::parse("https://example.com/search")?;
        url.query_pairs_mut().append_pair("q", &query.text);
        Ok(parse(&ctx.fetch_text(url).await?))
    }
}

#[must_use]
pub fn parse(html: &str) -> Vec<Hit> { /* selectors */ }
```

Keep `parse` a pure `&str -> Vec<Hit>` function: that is the engine's real
contract with its upstream, and it survives any change to the HTTP plumbing.
Set `enabled_by_default: false` for engines that need a key, so the default
search stays free. `crates/cli/tests/bundled.rs` runs the shipped binary and
fails if a bundled engine was never linked.

## Design

```
crates/core             Engine trait, compile-time registry, rank fusion, runner
crates/engine-bing      one plug-in each
crates/engine-duckduckgo
crates/engine-google
crates/cli              the `dobukasu` binary: links every bundled plug-in
skills/dobukasu         the agent skill that calls it
```

One run: parse arguments → resolve engines → fan out under a deadline →
canonicalise URLs and fuse ranks → truncate → print.

URLs are compared the way a reader would compare them: case, `www.`, fragments,
tracking parameters and trailing slashes do not make a second page. The score is
`Σ 1 / (60 + rank)`, and ties break by best rank, then engine order, then URL, so
the same inputs always produce the same order.

## Testing

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

The suite asserts observable behaviour through public seams. Engines are the
only process-external dependency, so they are the only thing stubbed; argument
parsing, the runner, fusion and the JSON output all run for real.
[`docs/testing.md`](docs/testing.md) explains the shape of the suite, and what
mutation testing caught.

```sh
cargo install cargo-mutants
cargo mutants --workspace
```

## Limits

- **Scraped engines are best effort.** Under bot mitigation, Bing sometimes
  returns a decoy SERP, DuckDuckGo throttles bursts, and Google may refuse a
  network entirely. `dobukasu` reports what came back and flags refusals; it
  cannot judge relevance from the markup.
- **No pagination.** One page per engine. Reformulate the query instead.
- **No caching.** Every run fans out.

## Legal

`dobukasu` fetches public search result pages the way a browser does. That is not
a licence to ignore the terms of service of the engines you enable, nor
`robots.txt`, nor rate limits. Keep the request rate low, and prefer an official
API when one exists.

## License

MIT. See [LICENSE](LICENSE).
