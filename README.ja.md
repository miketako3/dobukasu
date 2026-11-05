# dobukasu

AI エージェントのための最小限のメタ検索 HTTP API。JSON を投げると JSON が返る。UI はない。

やることは一つ。クエリを受け取り、複数の検索エンジンに並行して問い合わせ、順位を一つに統合して返す。SearXNG のうちエージェントが実際に使う部分だけを取り出し、UI・カテゴリ・設定画面・大量のエンジンを削り、Rust で「全部プラグイン」の構成に組み直したもの。

[English](README.md)

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
    }
  ],
  "engines": [
    {"id": "bing", "status": "ok", "count": 10, "duration_ms": 412},
    {"id": "duckduckgo", "status": "ok", "count": 10, "duration_ms": 388},
    {"id": "google", "status": "error", "count": 0, "duration_ms": 201,
     "error": "upstream blocked this client: Google served the JavaScript interstitial; …"}
  ],
  "took_ms": 430
}
```

## なぜ作ったか

エージェントが Web を検索するときに必要なのは、安定した JSON の契約、複数エンジンの統合、そして失敗の正直な報告の三つ。ファセットも画像プロキシも OpenSearch 記述も要らない。dobukasu はその範囲だけを実装する。

- **エンドポイントは一つ。** `GET /search?q=` と `POST /search` だけ。
- **エンジンはプラグイン。** コアはエンジン名を一つも知らない。各エンジンは独立したクレートで、コンパイル時のレジストリに自分を登録する。
- **足し算ではなく順位統合。** Reciprocal Rank Fusion なので、2 エンジンが一致して上位に出すページは、1 エンジンで 1 位のページより上に来る。
- **部分失敗が正常。** 一つのエンジンがタイムアウトしてもブロックされても、リクエストは失敗しない。`engines[]` に状態と理由が出る。
- **テストはオフラインで回る。** パーサは捕捉した実ページで検証し、HTTP の契約は本物のルータを通して検証する。差し替えるのはネットワーク境界だけ。

## 使い方

```sh
cargo run --release
# または
cargo install --path crates/server
dobukasu
```

既定では `127.0.0.1:8080` で待ち受ける。

```sh
curl -s 'http://127.0.0.1:8080/search?q=hello'
curl -s 'http://127.0.0.1:8080/search?q=hello&engines=bing&limit=5'
curl -s -X POST http://127.0.0.1:8080/search \
  -H 'content-type: application/json' \
  -d '{"q": "hello", "limit": 5, "lang": "ja"}'
curl -s 'http://127.0.0.1:8080/engines'
```

## HTTP API

### `GET /search` / `POST /search`

| パラメータ   | 型                        | 既定値          | 意味                                             |
| ------------ | ------------------------- | --------------- | ------------------------------------------------ |
| `q`          | string                    | 必須            | クエリ。1024 文字まで                            |
| `engines`    | カンマ区切り              | 有効な全エンジン | `bing,duckduckgo`                                |
| `limit`      | integer                   | `10`            | 統合後の件数。`search.max_limit` で頭打ち        |
| `lang`       | string                    | なし            | 言語ヒント（`hl`/`setlang`/`kl` に渡す）         |
| `region`     | string                    | なし            | 国ヒント（`gl`/`cc`）                            |
| `time_range` | `day\|week\|month\|year`  | なし            | 対応エンジンでのみ効く鮮度フィルタ               |
| `safesearch` | boolean                   | `false`         | 対応エンジンでのみ効くセーフサーチ               |
| `dedup`      | boolean                   | `true`          | エンジン間で同一 URL を統合する                  |
| `timeout_ms` | integer                   | `8000`          | エンジンごとの制限時間。100〜30000 に丸める      |

`POST` は同じ項目を JSON オブジェクトで受け取る。数値と真偽値はネイティブに受け付ける（`{"limit": 5, "safesearch": true}`）。未知のフィールドは拒否する。

応答はこうなる。

```jsonc
{
  "query": "…",           // 受け取ったクエリ
  "results": [            // 統合・重複排除済み。上位から
    {
      "title": "…",
      "url": "…",
      "snippet": "…",
      "engines": ["bing", "duckduckgo"],  // 先に見つけた順
      "score": 0.0327                     // 1/(60 + 順位) の和
    }
  ],
  "engines": [            // 問い合わせたエンジンごとの報告
    {
      "id": "bing",
      "status": "ok",     // ok | timeout | error
      "count": 10,
      "duration_ms": 412,
      "error": "…"        // error のときだけ
    }
  ],
  "took_ms": 430
}
```

状態コードは、一部のエンジンが失敗しても `200`、リクエストが不正なら `400`、全エンジンが失敗したら `502`。エラーは常に同じ形で返す。

```json
{"error": {"code": "unknown_engine", "message": "unknown engine: nope"}}
```

コードは `invalid_request`、`unknown_engine`、`no_engine_enabled`、`all_engines_failed`、`config`。

### `GET /engines`

このプロセスが呼べるエンジンの一覧。既定の検索対象に入っているかどうかと、プラグインの説明も返す。クエリを組み立てる前に何が使えるかを知るために使う。

### `GET /healthz`

`{"status":"ok","version":"0.1.0"}`。

## 同梱エンジン

| id           | キー不要 | 備考                                                                                     |
| ------------ | -------- | ---------------------------------------------------------------------------------------- |
| `bing`       | はい     | `bing.com/search` を解析。`ck/a` リダイレクトを復号する                                  |
| `duckduckgo` | はい     | `html.duckduckgo.com` を解析。`uddg` リダイレクトを復号する                              |
| `google`     | 一部     | 既定は HTML だが、Google は多くの場合 JavaScript 必須の画面を返す。`api_key` と `cx`（Custom Search API）を設定すると安定する |

3 つともビルドに含まれ、既定で有効。Google は多くのホストで最初から `error` を返すが、これは意図的で、「結果なし」と言って嘘をつかないため。Custom Search の資格情報を与えるか、無効にする。

```toml
[engines.google]
enabled = false
```

DuckDuckGo は短時間の連打で制限をかける。そのときは 202 とチャレンジページが返る。プラグインはそのページを見分けて `status: "error"` と `Blocked` のメッセージを返す。結果がゼロだったことにはしない。

## 設定

優先順位は、組み込みの既定値 → `dobukasu.toml`（または `$DOBUKASU_CONFIG`）→ 環境変数。[`dobukasu.example.toml`](dobukasu.example.toml) に全項目の例がある。

| 環境変数              | 上書きする項目      |
| --------------------- | ------------------- |
| `DOBUKASU_BIND`       | `server.bind`       |
| `DOBUKASU_LIMIT`      | `search.limit`      |
| `DOBUKASU_MAX_LIMIT`  | `search.max_limit`  |
| `DOBUKASU_TIMEOUT_MS` | `search.timeout_ms` |
| `DOBUKASU_DEDUP`      | `search.dedup`      |
| `DOBUKASU_USER_AGENT` | `search.user_agent` |
| `DOBUKASU_ENGINES`    | `search.engines`    |
| `DOBUKASU_CONFIG`     | 設定ファイルのパス  |
| `GOOGLE_API_KEY`      | Google の `api_key` |
| `GOOGLE_CX`           | Google の `cx`      |
| `RUST_LOG`            | ログフィルタ        |

## エンジンを追加する

エンジンはクレートにする。`Engine` を実装し、`inventory` で `Plugin` を登録し、サーバのバイナリにリンクする。コアはそのクレートを import しない。

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

パーサは `&str -> Vec<Hit>` の純粋関数にしておく。これが上流 HTML との実際の契約であり、HTTP 周りのリファクタリングに影響されず、オフラインのフィクスチャテストが叩く対象になる。

登録は `crates/server/Cargo.toml` にクレートを足し、`main.rs` でリンクするだけ。

```rust
use dobukasu_engine_example as _;
```

## 構成

```
crates/core             Engine trait、コンパイル時レジストリ、設定、順位統合、HTTP API
crates/engine-bing      エンジンはそれぞれ 1 クレート
crates/engine-duckduckgo
crates/engine-google
crates/server           `dobukasu` バイナリ。同梱プラグインを全部リンクする
```

コアはどのエンジンにも依存しない。`Registry` は `inventory` が集めたものから組み立てる。エンジンの無効化はコードではなく設定で行う。順位統合と HTTP の契約はコアにあり、全エンジンで共有する。

リクエストは、パラメータの解析 → エンジンの解決 → エンジンごとのタイムアウト付き並行実行 → URL の正規化と順位統合 → 件数の切り詰め → シリアライズ、の順に流れる。

## テスト

```sh
cargo test --workspace                  # オフライン: フィクスチャ、HTTP 契約、順位統合、設定
cargo test --workspace -- --ignored     # ネットワークを要するライブ確認
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

方針は一つ。公開 API から観察できる振る舞いだけを検証する。プロセス外の依存はエンジンだけなので、差し替えるのもエンジンだけ。URL の同一性、順位、パラメータ解析、エラーの形、状態コードは、すべて本物のルータを通して検証する。

```sh
cargo install cargo-mutants
cargo mutants -p dobukasu-core
```

ミューテーションテストを品質ゲートにする。URL の正規化や順位計算で変異体が生き残るなら、そのテストは主張している振る舞いを実際には固定できていない。テストの形をこう決めた理由は [docs/testing.md](docs/testing.md) に書いた。

## 既知の限界

- **スクレイピングするエンジンはベストエフォート。** ボット対策が効いていると、Bing や DuckDuckGo はエラーではなく無関係なページを `200` で返すことがある。dobukasu は上流が返したものをそのまま報告する。マークアップから関連性は判断できない。`engines[]` は状態の報告であって品質の保証ではない。
- **ページングはない。** エンジンごとに 1 ページだけ。続きが欲しいエージェントはクエリを言い換える。
- **キャッシュはない。** リクエストごとに毎回問い合わせる。ループで叩くなら手前にキャッシュを置く。
- **認証はない。** ループバックに束縛するか、ネットワークに出す前にリバースプロキシを置く。

## 法務とマナー

dobukasu は公開されている検索結果ページをブラウザと同じように取得する。だからといって、有効にしたエンジンの利用規約や `robots.txt`、レート制限を無視してよい理由にはならない。自分の用途で、低い頻度で動かすこと。インターネットから到達できる場所に置くなら認証をかけること。公式 API があるならそちらを優先すること（Google プラグインはそうしている）。使い方について作者は責任を負わない。

## ライセンス

MIT。[LICENSE](LICENSE) を参照。
