# dobukasu

AI エージェントのための最小限のメタ検索 CLI。クエリを一つ渡すと、統合された JSON の一覧が返る。

[![CI](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml/badge.svg)](https://github.com/miketako3/dobukasu/actions/workflows/ci.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

やることは一つ。クエリを受け取り、複数の検索エンジンに並行して問い合わせ、順位を一つに統合し、JSON として出力する。SearXNG のうちエージェントが実際に使う部分だけを取り出し、UI・カテゴリ・設定画面・サーバを削り、Rust で「全部プラグイン」の構成に組み直したもの。

[English](README.md)

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

## API ではなく CLI にした理由

検索したいエージェントに必要なのは、呼び出し一回分と安定した契約であって、面倒を見続けるサービスではない。JSON を出して終了するプロセスは、考えうる最小の表面積になる。ポートも、起動と停止の管理も、認証も、質問と質問の間に残り続けるものもない。呼び出しは毎回独立しているので、一度固まった実行が次の実行を壊すこともない。

- **全部プラグイン。** コアはエンジン名を一つも知らない。各エンジンは独立したクレートで、コンパイル時のレジストリに自分を登録する。
- **足し算ではなく順位統合。** Reciprocal Rank Fusion なので、2 エンジンが一致して上位に出すページは、1 エンジンで 1 位のページより上に来る。
- **部分失敗が正常。** 一つのエンジンがブロックされてもタイムアウトしても、検索は失敗しない。`engines[]` に状態と理由が出る。
- **テストはオフラインで回る。** パーサは捕捉したページで固定し、CLI の契約は本物の引数解析と出力を通して検証する。

## インストール

```sh
cargo install --path crates/cli     # ~/.cargo/bin に dobukasu が入る
# または
cargo build --release               # ./target/release/dobukasu
```

## 使い方

```sh
dobukasu "query"                          # 統合した結果
dobukasu "query" --limit 5                # 5 件に絞る
dobukasu "query" --engines bing           # Bing だけに聞く
dobukasu "query" --timeout-ms 4000        # エンジンごとの制限時間
dobukasu "query" --no-dedup               # 同じページをエンジンごとに残す
dobukasu engines                          # このビルドが聞けるエンジン一覧
dobukasu --help
```

| フラグ | 既定値 | 意味 |
| --- | --- | --- |
| `--limit N` | `10` | 統合後に残す件数 |
| `--engines a,b` | 既定の全エンジン | 指定したものだけに聞く |
| `--timeout-ms N` | `8000` | エンジンごとの制限時間 |
| `--no-dedup` | 無効 | 同じページをエンジンごとに残す |

出力は標準出力の JSON で、形は `{query, results[], engines[], took_ms}`。各結果は、それを返した `engines` と統合スコア `score` を持つ。

| 終了コード | 意味 |
| --- | --- |
| `0` | 少なくとも 1 エンジンが答えた |
| `1` | 全エンジンが失敗した |
| `2` | 使い方の誤り（未知のフラグ、未知のエンジン、クエリなし） |

## エージェントスキルとして使う

[`skills/dobukasu/SKILL.md`](skills/dobukasu/SKILL.md) がそのまま使えるスキルになっている。いつ検索するか、CLI をどう呼ぶか、`engines[]` をどう読むか、結果が空だったときに何をするかが書いてある。

```sh
mkdir -p ~/.agents/skills
cp -r skills/dobukasu ~/.agents/skills/
```

統合はこれだけ。MCP サーバも、同期し続けるツール定義も要らない。

## 同梱エンジン

| id | キー不要 | 取得方法 |
| --- | --- | --- |
| `bing` | はい | `bing.com/search` を解析。`ck/a` リダイレクトを復号する |
| `duckduckgo` | はい | `html.duckduckgo.com` を解析。`uddg` リダイレクトを復号する |
| `google` | 一部 | Symbian の Nokia ユーザーエージェントで `www.google.com/wml/search` を解析する。これは SearXNG が使っている、JavaScript なしのレイアウトを引き出す手法。`GOOGLE_API_KEY` と `GOOGLE_CX` があれば Custom Search JSON API に切り替える |

3 つともビルドに含まれ、既定で有効。

**Google について。** 通常の `/search` は、自動クライアントには JavaScript 必須の画面か 403 を返す。だから直接叩くと失敗する。`/wml/search` は、ごく古い携帯のユーザーエージェントにだけ Google が返しているレイアウトで、現在の SearXNG もこれを解析している。ただしこれもスクレイピングであることに変わりはない。Google がこのホストを拒否したら、エンジンは空の一覧を返さず、`blocked` とその理由を報告する。Google の機嫌に依存しない契約が欲しいなら、無料の Custom Search キーを `GOOGLE_API_KEY` と `GOOGLE_CX` に入れる。

**DuckDuckGo について。** 短時間の連打で制限をかけ、202 とチャレンジページを返す。そのページを見分けて `blocked` として報告する。「結果がゼロだった」と「門前払いされた」は別の答えなので、混ぜない。

## エンジンを追加する

エンジンはクレートにする。`Engine` を実装し、`inventory` で `Plugin` を登録し、バイナリにリンクする。コアはそのクレートを import しない。

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
pub fn parse(html: &str) -> Vec<Hit> { /* セレクタ */ }
```

パーサは `&str -> Vec<Hit>` の純粋関数にしておく。これが上流 HTML との実際の契約であり、HTTP 周りのリファクタリングに影響されず、フィクスチャテストが叩く対象になる。登録は `crates/cli/src/main.rs` でリンクするだけ。

```rust
use dobukasu_engine_example as _;
```

## 構成

```
crates/core             Engine trait、コンパイル時レジストリ、順位統合、実行部
crates/engine-bing      エンジンはそれぞれ 1 クレート
crates/engine-duckduckgo
crates/engine-google
crates/cli              `dobukasu` バイナリ。同梱プラグインを全部リンクする
skills/dobukasu         それを呼ぶエージェントスキル
```

1 回の実行は、引数の解析 → エンジンの解決 → エンジンごとのタイムアウト付き並行実行 → URL の正規化と順位統合 → 件数の切り詰め → 出力、の順に流れる。

## テスト

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

方針は一つ。公開された継ぎ目から観察できる振る舞いだけを検証する。プロセス外の依存はエンジンだけなので、差し替えるのもエンジンだけ。URL の同一性、順位、引数解析、終了コード、エラー報告は、すべて本物を通して検証する。テストの形をこう決めた理由と、ミューテーションテストが見つけたものは [docs/testing.md](docs/testing.md) に書いた。

```sh
cargo install cargo-mutants
cargo mutants --workspace
```

## 既知の限界

- **スクレイピングするエンジンはベストエフォート。** ボット対策が効いていると、Bing はエラーではなく無関係なページを `200` で返すことがある。dobukasu は上流が返したものをそのまま報告する。マークアップから関連性は判断できない。
- **ページングはない。** エンジンごとに 1 ページだけ。続きが欲しいときはクエリを言い換える。
- **キャッシュはない。** 実行のたびに毎回問い合わせる。ループで叩くなら手前にキャッシュを置く。
- **サーバはない。** 1 回の実行で 3 エンジンに聞いて終わる。これは意図した設計。

## 法務とマナー

dobukasu は公開されている検索結果ページをブラウザと同じように取得する。だからといって、有効にしたエンジンの利用規約や `robots.txt`、レート制限を無視してよい理由にはならない。自分の用途で、低い頻度で動かすこと。公式 API があるならそちらを優先すること（Google プラグインはそうしている）。使い方について作者は責任を負わない。

## ライセンス

MIT。[LICENSE](LICENSE) を参照。
