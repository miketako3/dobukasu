# How this repository tests itself

The suite is written against one rule: **test observable behaviour through the
public API, and let the implementation move freely underneath**.

That rule is what makes the tests survive refactoring. A test that asserts "the
handler called the engine twice" breaks the day the handler stops calling the
engine twice, even though nobody can observe the difference. A test that
asserts "a page two engines returned appears once, listing both engines" keeps
passing for as long as the product keeps that promise.

## Layers

| Layer | File | What it fixes |
| --- | --- | --- |
| URL identity | `crates/core/tests/url_identity.rs` | Which URLs count as the same page. |
| Rank fusion | `crates/core/tests/rank_fusion.rs` | Ordering, de-duplication, truncation, the RRF score. |
| HTTP contract | `crates/core/tests/http_api.rs` | Parameters, status codes, JSON shape, partial failure, timeouts. |
| HTTP adapter | `crates/core/tests/http_fetch.rs` | How an upstream status becomes an `EngineError`. |
| Configuration | `crates/core/tests/config_layers.rs` | Defaults, file, environment, validation. |
| Upstream HTML | `crates/*/tests/contract.rs` | `&str -> Vec<Hit>` for captured pages. |
| Bundling | `crates/server/tests/bundled_engines.rs` | Which plug-ins a built binary actually contains. |

Two conventions follow from the rule.

**Expectations are written out, never recomputed.** The RRF test spells out
`1 / (60 + rank)` instead of calling the same helper the production code calls.
The parser tests list titles and URLs by hand instead of re-parsing a second
way. If the code and the test shared a helper, both would be wrong together.

**Doubles only at the process boundary.** Engines are the only thing this
server talks to over the network, so they are the only thing replaced
(`crates/core/tests/common/mod.rs`). Everything else — the router, parameter
parsing, fusion, serialisation — runs for real in the tests.

## Fixtures

- `crates/engine-bing/tests/fixtures/results.html` — a captured SERP.
- `crates/engine-duckduckgo/tests/fixtures/results.html` — a captured SERP.
- `crates/engine-duckduckgo/tests/fixtures/throttled.html` — the block page
  DuckDuckGo serves when it rate-limits a client.
- `crates/engine-google/tests/fixtures/blocked.html` — the real JavaScript
  interstitial.
- `crates/engine-google/tests/fixtures/results.html` — a shape fixture: Google
  will not serve a real results page to an automated client, so this file
  reproduces the markup the HTML backend targets.

Fixtures are inputs, not snapshots. They never change when the code changes, so
a diff in a fixture means the upstream changed, not that someone edited the
implementation.

## Mutation testing

```sh
cargo install cargo-mutants
cargo mutants --workspace
```

The suite is only as good as its weakest assertion. Mutation testing finds the
assertions that could have been deleted without anyone noticing: if the
production code can be changed and every test still passes, that behaviour was
never actually pinned.

Surviving mutants are fixed by **strengthening an observable assertion**, not
by reaching into internals. Equivalent mutants (behaviour genuinely unchanged,
or an accessor no test can observe) are removed from the surface instead of
being covered with a contrived test.

The scheduled workflow in `.github/workflows/mutants.yml` runs the same gate
weekly and on demand.
