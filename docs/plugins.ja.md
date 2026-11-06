# エンジン・プラグインの作り方

エンジンは本体に組み込む必要はありません。`dobukasu-engine-<id>` という名前の
**実行ファイル**が、標準入力から JSON を1つ読み、標準出力に JSON を1つ返す。
それだけが契約です。Python でも Go でも Node でも Rust でも構いません。

共有ライブラリも、Rust のバージョン一致も、本体の再ビルドも要りません。

## 契約

**本体が渡すもの**（stdin に JSON 1行）:

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

`lang`、`region`、`time_range`（`day|week|month|year`）はヒントです。扱えないものは無視してよい。
`v` は契約のバージョンで、現在は常に `1`。

**プラグインが返すもの**（stdout に JSON 1つ、終了コード `0`）:

```json
{
  "v": 1,
  "hits": [
    { "title": "…", "url": "https://…", "snippet": "…" }
  ]
}
```

失敗を伝える方法は2つ。`{"v": 1, "error": "quota exceeded"}` を書いて終了 `0` で終わるか、
stderr にメッセージを出して非ゼロで終わるか。どちらも `engines[]` に
`status: "error"` とメッセージとして出る。プラグインが落ちても検索全体は落ちない。

本体側の決まりごと:

- 制限時間（`--timeout-ms`、既定 8000）を超えたプロセスは kill される
- タイトルか絶対 URL が欠けたヒットは捨てる
- 順序に意味がある。順位統合に使うので、良い結果から順に出す
- ネットワークは提供も制限もしない。HTTP クライアントはプラグインの持ち物

## 作る

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

動く完全版は [`examples/plugins/dobukasu-engine-wikipedia`](../examples/plugins/dobukasu-engine-wikipedia) にあります。
インストールせずに試す:

```sh
chmod +x dobukasu-engine-wikipedia
echo '{"v":1,"query":{"text":"rust","limit":3}}' | ./dobukasu-engine-wikipedia
```

## 入れる

実行ファイルをプラグイン用ディレクトリに置けば、そのまま有効になります。

```sh
mkdir -p ~/.config/dobukasu/plugins
cp dobukasu-engine-wikipedia ~/.config/dobukasu/plugins/
chmod +x ~/.config/dobukasu/plugins/dobukasu-engine-wikipedia

dobukasu engines                      # wikipedia が source: external で出る
dobukasu "rust" --engines wikipedia
```

`DOBUKASU_PLUGIN_PATH`（コロン区切り）で置き場所を変えられる。id はファイル名から取るので、
`dobukasu-engine-wikipedia` は `wikipedia` として呼べる。

外部プラグインは**既定の検索対象には入りません**。既定は無料の同梱エンジンだけ、という原則を守るためです。
自分のプラグインを通常の `dobukasu <query>` に加えたいときは:

```sh
export DOBUKASU_ENGINES=bing,duckduckgo,wikipedia
```

## 公開する

プラグインはただのファイルなので、バイナリと同じように配れます。

1. プラットフォームごとにビルドする（`x86_64-unknown-linux-gnu`、`aarch64-apple-darwin` など）
2. GitHub のリリースに添付する
3. 利用者には「ダウンロードしてプラグイン用ディレクトリに置く」と伝える

一行インストーラで足ります。

```sh
curl -fsSL https://github.com/you/dobukasu-engine-thing/releases/latest/download/dobukasu-engine-thing-$(uname -s)-$(uname -m).tar.gz \
  | tar -xz -C ~/.config/dobukasu/plugins
chmod +x ~/.config/dobukasu/plugins/dobukasu-engine-thing
```

`pip install`、`go install`、`cargo install` でも構いません。条件は2つだけで、
生成されるバイナリが `dobukasu-engine-<id>` という名前であること、プラグイン用ディレクトリか
`DOBUKASU_PLUGIN_PATH` に入ることです。

## なぜクレートにしないのか

同梱エンジン（`bing`、`duckduckgo`、`google`）はビルド時にリンクする Rust クレートです。
本体に同梱するエンジンには適した形で、コンパイラが型を検査し、起動コストもゼロです。

第三者のエンジンには向きません。本体を fork して、ビルドし直して、リリースのたびにそれを繰り返す必要が出ます。
実行ファイル契約はプロセス起動（約5ms）と引き換えに、このリポジトリに触らずに書けて、配れて、入れられる形を選んでいます。
