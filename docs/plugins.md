# Writing an engine plug-in

An engine does not have to be compiled into `dobukasu`. A plug-in is **any
executable** named `dobukasu-engine-<id>` that speaks one JSON request and one
JSON response over stdin/stdout. Python, Go, Node, Rust, a shell script — the
host does not care.

That is the whole ABI. There is no shared library, no Rust version coupling, and
nothing to rebuild when the host updates.

## The contract

**Host sends** (one line of JSON on stdin):

```json
{
  "v": 1,
  "query": {
    "text": "rust async runtime",
    "limit": 10,
    "lang": "en",
    "region": null,
    "time_range": null,
    "safe_search": false
  },
  "user_agent": "dobukasu/0.1.0"
}
```

`lang`, `region` and `time_range` (`day|week|month|year`) are hints; ignore what
you cannot honour. `v` is the contract version and is currently always `1`.

**Plug-in answers** (one JSON document on stdout, exit `0`):

```json
{
  "v": 1,
  "hits": [
    { "title": "…", "url": "https://…", "snippet": "…" }
  ]
}
```

To report a failure, either write `{"v": 1, "error": "quota exceeded"}` and exit
`0`, or print a message on stderr and exit non-zero. Both surface in
`engines[]` as `status: "error"` with your message; a crash never takes the
search down with it.

Rules the host applies:

- the process is killed if it runs past the deadline (`--timeout-ms`, default 8000)
- hits without a title or an absolute `http(s)` URL are dropped
- order matters: hits are fused by rank, so emit your best result first
- no network access is provided or restricted — the plug-in owns its HTTP client

## Write one

```python
#!/usr/bin/env python3
import json, sys, urllib.parse, urllib.request

request = json.load(sys.stdin)
term = request["query"]["text"]
limit = request["query"]["limit"]

url = "https://en.wikipedia.org/w/api.php?" + urllib.parse.urlencode({
    "action": "opensearch", "format": "json", "limit": limit, "search": term,
})
with urllib.request.urlopen(url, timeout=8) as response:
    _term, titles, snippets, urls = json.load(response)

json.dump({"v": 1, "hits": [
    {"title": t, "url": u, "snippet": s}
    for t, u, s in zip(titles, urls, snippets) if u
]}, sys.stdout)
```

A complete, runnable version lives at
[`examples/plugins/dobukasu-engine-wikipedia`](../examples/plugins/dobukasu-engine-wikipedia).

Test it without installing anything:

```sh
chmod +x dobukasu-engine-wikipedia
echo '{"v":1,"query":{"text":"rust","limit":3}}' | ./dobukasu-engine-wikipedia
```

## Install one

Copy the executable into the plug-in directory and it is live:

```sh
mkdir -p ~/.config/dobukasu/plugins
cp dobukasu-engine-wikipedia ~/.config/dobukasu/plugins/
chmod +x ~/.config/dobukasu/plugins/dobukasu-engine-wikipedia

dobukasu engines                      # wikipedia now appears, source: external
dobukasu "rust" --engines wikipedia
```

`DOBUKASU_PLUGIN_PATH` (colon-separated) replaces the default directory. The id
comes from the file name: `dobukasu-engine-wikipedia` answers to `wikipedia`.

External engines are **not** in the default set, because the default set is the
free bundled engines. To make yours part of a plain `dobukasu <query>`:

```sh
export DOBUKASU_ENGINES=bing,duckduckgo,wikipedia
```

## Publish one

The plug-in is just a file, so publish it like any binary:

1. build per platform (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, …)
2. attach the binaries to a GitHub release
3. tell people to download and drop it in the plug-in directory

A one-line installer is enough:

```sh
curl -fsSL https://github.com/you/dobukasu-engine-thing/releases/latest/download/dobukasu-engine-thing-$(uname -s)-$(uname -m).tar.gz \
  | tar -xz -C ~/.config/dobukasu/plugins
chmod +x ~/.config/dobukasu/plugins/dobukasu-engine-thing
```

If you write it in a language with a package manager, `pip install`, `go install`
or `cargo install` work too — as long as the resulting binary is named
`dobukasu-engine-<id>` and lands in the plug-in directory (or on
`DOBUKASU_PLUGIN_PATH`).

## Why not a linked crate?

Bundled engines (`bing`, `duckduckgo`, `google`) are Rust crates linked at build
time. That is the right shape for engines that ship with the binary: the
compiler checks them, and they cost nothing at startup.

It is the wrong shape for third parties, who would have to fork the host,
rebuild it, and re-do that for every release. The executable contract trades a
process spawn (~5 ms) for a plug-in that anyone can write, ship and install
without touching this repository.
