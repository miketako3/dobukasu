# dobukasu

**SearXNG の仕組みを、エージェント前提・プラグイン優先・最小構成で Rust に作り直した。**

[![CI](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml/badge.svg)](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml) [![Release](https://img.shields.io/github/v/release/miketako3/dobukasu)](https://github.com/miketako3/dobukasu/releases/latest) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

SearXNG がやっていること——複数の検索エンジンに同時に問い合わせ、順位を統合する——を、制約を三つに絞って作り直した。**エージェント前提**（JSON を出して終了するコマンドとして動く）、**プラグイン優先**（コアはエンジン名を一つも知らず、各エンジンは自分を登録するクレート）、**最小構成**（UI もカテゴリも設定画面もデータベースも持たない）。クエリを一つ渡すと、統合された一覧が返る。アカウントもキーもサーバも要らない。

[English](README.md)

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

## 名前の由来

禪院直哉。『呪術廻戦』で自他ともに認める「最速の呪術師」と呼ばれた男。彼の術式は一秒という時間を細かいコマに刻み、その一つひとつを辿ることで誰より速く動く。

あの構図には名前がついている。**ドブカスラッシュ**。術式が右腕を勝手に振り抜き、左手は髪を直し、表情は最後まで退屈なままだ。

`dobukasu` がクエリに対してやることも同じだ。一つの問いを複数のエンジンに刻み、すべてを並行に走らせ、一つの答えに束ねる。単独のエンジンより速く、より多くの裏付けを持つ答えが出る。

これはオマージュであって、作品との関係はない。漫画・作者・出版社とは一切関係がない。

## 使い方

```sh
dobukasu "query"                    # 統合した結果を JSON で
dobukasu "query" --limit 5          # 5 件に絞る
dobukasu "query" --engines bing     # エンジンを指定する
dobukasu "query" --timeout-ms 4000  # エンジンごとの制限時間
dobukasu engines                    # このビルドが聞けるエンジン一覧
```

| フラグ | 既定値 | 意味 |
| --- | --- | --- |
| `--limit N` | `10` | 統合後に残す件数 |
| `--engines a,b` | 無料のエンジン全部 | 指定したものだけに聞く |
| `--timeout-ms N` | `8000` | エンジンごとの制限時間 |
| `--no-dedup` | 無効 | 同じページをエンジンごとに残す |

終了コードはエージェント向けに決めてある。誰か答えたら `0`、全滅したら `1`、使い方の誤りは `2`。エンジンの失敗は検索の失敗にしない。`engines[]` に状態と理由を出す。

## インストール

```sh
curl -fsSL https://raw.githubusercontent.com/miketako3/dobukasu/main/install.sh | sh
```

macOS（arm64 / x86_64）と Linux（x86_64）のビルド済みバイナリを取ってきて `~/.local/bin` に置く。`DOBUKASU_INSTALL_DIR` と `DOBUKASU_VERSION` で置き場所とバージョンを変えられる。

ソースからビルドするなら `cargo install --path crates/cli`。

## エージェントスキルとして使う

[`skills/dobukasu/SKILL.md`](skills/dobukasu/SKILL.md) がエージェント側の半分。いつ検索するか、`engines[]` をどう読むか、どのエンジンを信じるか、結果が空だったときに何をするかが書いてある。

```sh
mkdir -p ~/.agents/skills
cp -r skills/dobukasu ~/.agents/skills/
```

MCP サーバも、同期し続けるスキーマも要らない。JSON を出して終了するプロセスが一つあるだけ。

## 作り直して何が変わったか

SearXNG はよくできている。ここで借りているのは思想で、画面は借りていない。呼び出し手がエージェントになったとき、効いてくる違いはこの四点。

- **サーバが要らない。** 起動も監視も要塞化も不要。実行は毎回独立していて、固まった検索が次の検索を壊さない。
- **足し算ではなく順位統合。** Reciprocal Rank Fusion なので、2 エンジンが一致して上位に出すページは、1 エンジンで 1 位のページより上に来る。
- **失敗を正直に返す。** 「ブロックされた」「時間切れ」「ヒットなし」は別の答えで、`engines[]` がそれを区別する。空の一覧を返してごまかさない。
- **全部プラグイン。** コアはエンジン名を一つも知らない。

## 同梱エンジン

| id | キー不要 | 取得方法 |
| --- | --- | --- |
| `bing` | はい | `bing.com/search` を解析。市場（`setlang`、`cc`）を明示しないと Bing が無関係なページを返すため、常に指定する。`ck/a` リダイレクトを復号する |
| `duckduckgo` | はい | `html.duckduckgo.com` を解析。レート制限のチャレンジページを見分けてブロックとして報告する。`uddg` リダイレクトを復号する |
| `google` | はい\* | Symbian の Nokia ユーザーエージェントで `www.google.com/wml/search` を解析する。SearXNG が使っている JavaScript なしのレイアウト。`GOOGLE_API_KEY` と `GOOGLE_CX` があれば Custom Search JSON API に切り替える |

\* 理屈のうえではキー不要。ただし Google は多くの回線をそもそも拒否する。そのときは空の一覧を返さず、`blocked` と理由を返す。資格情報を入れるか、Google が拒否していないホストで動かすのが対処になる。

既定の検索対象は無料のエンジンだけ。アカウントが要るものは既定に入らず、設定したうえで `--engines` で名指しする。無料で動く道が常に確保される。

```sh
dobukasu engines   # 使えるもの、既定かどうか、使えないものはその理由
```

## プラグイン

エンジンは、本体に組み込むクレートにも、置くだけの実行ファイルにもできます。

**実行ファイル型プラグイン**が、第三者がエンジンを足すときの道。`dobukasu-engine-<id>` という名前で、標準入力の JSON を1つ読み、標準出力に JSON を1つ返すプログラムなら何でも構いません。Rust も再ビルドも ABI も要りません。

```sh
mkdir -p ~/.config/dobukasu/plugins
cp dobukasu-engine-wikipedia ~/.config/dobukasu/plugins/ && chmod +x ~/.config/dobukasu/plugins/*
dobukasu "rust" --engines wikipedia
```

書き方・試し方・入れ方・公開の仕方と、リクエストとレスポンスの正確な形は [docs/plugins.ja.md](docs/plugins.ja.md) に。動く例は [`examples/plugins/dobukasu-engine-wikipedia`](examples/plugins/dobukasu-engine-wikipedia)。

**同梱エンジン**は Rust クレート。`Engine` を実装し、`inventory` で `Plugin` を登録する。コアはそのクレートを import しないので、追加で触るのはクレート、`crates/cli/Cargo.toml`、`crates/cli/src/main.rs` の 1 行の 3 か所だけ。

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
pub fn parse(html: &str) -> Vec<Hit> { /* セレクタ */ }
```

`parse` は `&str -> Vec<Hit>` の純粋関数にしておく。これが上流との実際の契約であり、HTTP 周りをどう作り替えても生き残る。キーが要るエンジンは `enabled_by_default: false` にして、既定の検索を無料のまま保つ。リンクを忘れると静かに落ちるので、`crates/cli/tests/bundled.rs` が実際のバイナリを起動して同梱を検証する。

## 設計

```
crates/core             Engine trait、コンパイル時レジストリ、順位統合、実行部
crates/engine-bing      エンジンはそれぞれ 1 クレート
crates/engine-duckduckgo
crates/engine-google
crates/cli              `dobukasu` バイナリ。同梱プラグインを全部リンクする
skills/dobukasu         それを呼ぶエージェントスキル
```

1 回の実行は、引数の解析 → エンジンの解決 → 制限時間付きの並行実行 → URL の正規化と順位統合 → 件数の切り詰め → 出力、の順に流れる。

URL は読み手が同一と見なす単位で比較する。大文字小文字、`www.`、フラグメント、トラッキングパラメータ、末尾スラッシュの違いは別のページにしない。スコアは `Σ 1 / (60 + 順位)` で、同点は最良順位、エンジンの順序、URL の順に解く。同じ入力なら同じ順序になる。

## テスト

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

公開された継ぎ目から観察できる振る舞いだけを検証する。プロセス外の依存はエンジンだけなので、差し替えるのもエンジンだけ。引数解析も実行部も順位統合も JSON 出力も、本物を通す。[docs/testing.md](docs/testing.md) に、テストの形とミューテーションテストが見つけたものを書いた。

```sh
cargo install cargo-mutants
cargo mutants --workspace
```

## 限界

- **スクレイピングするエンジンはベストエフォート。** ボット対策が効いていると、Bing は無関係なページを返し、DuckDuckGo は連打を制限し、Google は回線ごと拒否することがある。dobukasu は返ってきたものをそのまま報告し、拒否は拒否として伝えるが、マークアップから関連性は判断できない。
- **ページングはない。** エンジンごとに 1 ページだけ。続きが欲しいときはクエリを言い換える。
- **キャッシュはない。** 実行のたびに毎回問い合わせる。

## 法務

dobukasu は公開されている検索結果ページをブラウザと同じように取得する。だからといって、有効にしたエンジンの利用規約や `robots.txt`、レート制限を無視してよい理由にはならない。低い頻度で動かし、公式 API があるならそちらを優先すること。

## ライセンス

MIT。[LICENSE](LICENSE) を参照。
