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
| Runner | `crates/core/tests/search.rs` | Partial failure, timeouts, the all-failed flag, limit, cross-engine merging. |
| HTTP adapter | `crates/core/tests/fetch.rs` | How an upstream status becomes an `EngineError`. |
| Upstream pages | `crates/*/tests/contract.rs` | `&str -> Vec<Hit>` for captured pages and API payloads. |
| Plug-in selection | `crates/core/tests/registry.rs` | Which plug-ins load, and what an unusable one reports. |
| CLI contract | `crates/cli/tests/cli.rs` | Arguments, JSON on stdout, stderr, exit codes. |
| Arguments | `crates/cli/src/lib.rs` (`mod tests`) | Parsing rules the binary relies on. |

Two conventions follow from the rule.

**Expectations are written out, never recomputed.** The RRF test spells out
`1 / (60 + rank)` instead of calling the same helper the production code calls.
The parser tests list titles and URLs by hand instead of re-parsing a second
way. If the code and the test shared a helper, both would be wrong together.

**Doubles only at the process boundary.** Engines are the only thing this
program talks to over the network, so they are the only thing replaced. The
argument parser, the runner, fusion and the JSON rendering all run for real;
the CLI tests build a `Cli` from stub engines and read its `Output`.

## Fixtures

- `crates/engine-bing/tests/fixtures/results.html` — a captured SERP.
- `crates/engine-duckduckgo/tests/fixtures/results.html` — a captured SERP.
- `crates/engine-duckduckgo/tests/fixtures/throttled.html` — the block page
  DuckDuckGo serves when it rate-limits a client.
- `crates/engine-google/tests/fixtures/blocked.html` — the real JavaScript
  interstitial.
- `crates/engine-brave/tests/fixtures/results.json` — a Brave API payload,
  including the HTML Brave puts inside titles and snippets.
- `crates/engine-google/tests/fixtures/wml.html` — a shape fixture: Google
  answers this host with 403 or a JavaScript interstitial instead of results,
  so this file reproduces the `/wml/search` layout the backend targets.

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

## What the gate has actually found

The first scoped run over the fusion module (25 mutants, `rank_fusion` and
`url_identity` only) caught 24 and left one unviable. Before that, an earlier
run of the same module reported eight survivors, all in the "which occurrence
supplies the title" rule:

- a page a later engine ranks higher did not take that engine's title, and
- the tie-break between two engines at the same rank was not pinned at all.

Fixing it showed the tie-break's second clause was dead code: engines are
visited in order, so `best_engine` can never exceed the current engine index.
The clause was removed and the two behaviours are now asserted directly
(`a_page_a_later_engine_ranks_higher_keeps_that_engines_title`,
`a_tie_keeps_the_earlier_engines_spelling`).

A separate survivor in `canonical_url` (the `path.len() > 1` guard before
trimming a trailing slash) turned out to be an equivalent mutant, because the
URL serialiser normalises an empty path back to `/`. The guard was replaced
with `strip_suffix('/')` so no unobservable branch remains.
