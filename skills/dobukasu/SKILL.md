---
name: dobukasu
description: Web search for agents through the local `dobukasu` CLI, which asks several engines at once and returns one merged, de-duplicated JSON list. Use whenever a task needs current information from the web — "search for", "look up", "find out about", "what's the latest on", "research X" — or when you need several sources on one question, or when a single engine would give you a thin or biased ranking. Prefer this over fetching a search engine's HTML yourself: it fans out to Google, Bing and DuckDuckGo in parallel, reports which engine answered, and tells you honestly when an engine was blocked instead of returning nothing.
license: MIT
category: research
---

# dobukasu

`dobukasu` is a command-line meta-search: one query in, one merged JSON list
out. It asks every enabled engine in parallel and fuses the rankings with
reciprocal rank fusion, so a page two engines agree on outranks a page that is
merely first in one.

## When to use it

- The answer depends on something current (versions, prices, releases, news).
- You need several independent sources, not one engine's opinion.
- A previous search came back thin, and you want a second ranking of the same
  question.
- You want to know *why* a search was empty: blocked engine, no results, or
  everything timed out are different answers.

Do not use it to fetch a page you already have a URL for — use a fetch tool for
that. `dobukasu` finds URLs; it does not read them.

## Running it

```sh
dobukasu "query"                 # merged results, JSON on stdout
dobukasu "query" --limit 5       # keep five
dobukasu "query" --engines bing,duckduckgo
dobukasu "query" --timeout-ms 4000
dobukasu engines                 # what this build can ask, and what is default
```

Multi-word queries can be passed as one quoted argument or as several words.
Every invocation is one process; there is no server to start.

The binary is on `PATH` as `dobukasu`. If it is not, it lives at
`~/.cargo/bin/dobukasu` or in a checkout at `target/release/dobukasu`.

## Reading the output

```jsonc
{
  "query": "rust async runtime",
  "results": [
    {
      "title": "…",
      "url": "https://…",
      "snippet": "…",
      "engines": ["bing", "duckduckgo"],  // who returned this page
      "score": 0.0328                     // sum of 1/(60 + rank)
    }
  ],
  "engines": [
    {"id": "bing", "status": "ok", "count": 10, "duration_ms": 412},
    {"id": "google", "status": "error", "count": 0, "duration_ms": 201,
     "error": "upstream blocked this client: …"}
  ],
  "took_ms": 430
}
```

`status` is `ok`, `timeout` or `error`. A result's `engines` list is evidence:
two engines agreeing is a much stronger signal than one engine's first hit, and
the `score` already reflects that.

## Exit codes

| code | meaning |
| --- | --- |
| 0 | at least one engine answered |
| 1 | every engine failed — read `engines[].error` before retrying |
| 2 | bad usage (unknown flag, unknown engine, empty query) |

## How to use the results well

- **Lead with what two engines agree on.** Sort by `score`, but when two results
  score close together, prefer the one with more `engines`.
- **Do not retry blindly.** If `engines[]` shows `blocked` for one engine and
  `ok` for another, the search succeeded; use what came back. Retrying only the
  blocked engine usually just trips the rate limit again.
- **Reformulate rather than paginate.** There is no page parameter. If the top
  results are off-topic, change the words, add a year, quote a phrase, or add a
  site name.
- **Keep the URL.** When you cite a result, cite `url`; the snippet is the
  engine's summary, not the page's words.
- **DuckDuckGo throttles bursts.** After several quick searches it answers with
  a challenge page and reports `blocked`; wait or lean on the other engines.
- **Google needs credentials to be reliable.** Without `GOOGLE_API_KEY` and
  `GOOGLE_CX` it scrapes a JavaScript-free layout that Google often refuses.
  Its `error` line says so; it is not a bug in your query.

## If a search comes back empty

1. Look at `engines[]`. `blocked` means the upstream refused this client;
   `timeout` means the deadline was too short (`--timeout-ms 15000`).
2. Try `--engines bing,duckduckgo` to skip a blocked engine.
3. Change the query: shorter, more distinctive terms beat full sentences.
4. Only then conclude the web has nothing; report which engines failed.
