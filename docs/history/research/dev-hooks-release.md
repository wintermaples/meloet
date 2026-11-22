# 開発の仕込みを release から外す（Cargo の feature、Tauri、Vite）

2026-11-22。調べた人: meloet-research。入れた版: tauri 2.12.5、tauri-build 2.7.4、tauri-codegen 2.7.4、tauri-utils 2.10.2、vite 8.3.8、rolldown 1.2.19（どれも `~/.cargo/registry/src/` と `ui/node_modules/` の手元のソースを読んだ）。

## 1. 結論（短く）

1. **Rust 側は、自分の feature（例 `dev-hooks`）に `#[cfg(feature = "dev-hooks")]` をつけるのがよい。** 確かさ A。`cfg(debug_assertions)` は、release で測りたい meloet には合わない（release では常に切れる）。Tauri が持つ `cfg(dev)` と `tauri::is_dev()` は、「release か debug か」ではなく「画面を devUrl から読むか、実行ファイルに入れた物から読むか」の印。これも合わない。確かさ A。
2. **`cargo build --release` でも、meloet は本番の動き。** `meloet-app` が `default = ["custom-protocol"]` を持つから。`--no-default-features` のときだけ開発の動き（devUrl）になる。確かさ A（ソースを読んだ）。
3. **Tauri は `ui/dist` を brotli で圧縮して実行ファイルに入れる。** だから実行ファイルに `strings` をかけても JS の中の文字は見えない。画面側の見はりは、`ui/dist` を grep するのが本筋。さらに、実行ファイルの中の物を試験で取り出して調べる方法もある（7 節）。確かさ A。
4. **画面側（Vite 8）は、`import.meta.env.*` と `define` が「作るときに文字として置きかわる」ことまでは A で確かめた。ただし「その `if` の中の動的 import の塊（chunk）が必ず消える」とは、文書に書いてなかった。** いまの `ui/dist` にも `start-*.js`（仕込みの塊）が残っている。消えるかは、実際に作って `ui/dist` を grep して決める。確かさ: 置きかえ A、消えるかは未確認。
5. **見はりの試験は 2 本立て。** Rust は `strings`（名前とパス）、画面は `ui/dist` の grep。どちらも「仕込みの名前の一覧」を決めて探す（`MELOET_` の頭だけで探すと、本物の設定の名前に当たる）。

## 2. Rust 側: 3 つの形のくらべ

| 形 | release で測れるか | 配る物に入らない保証 | ひとこと |
|---|---|---|---|
| `#[cfg(feature = "dev-hooks")]`（自分の feature） | 測れる（`--features dev-hooks` をつければ） | feature なしなら、コードも文字も入らない | 軸が「入れるか」だけ。いちばん素直 |
| `#[cfg(debug_assertions)]` | 測れない（release は常に false） | 切れる | meloet は release で測るので不向き。release の測りが取れなくなる |
| `#[cfg(dev)]` / `tauri::is_dev()` | 軸がちがう | — | Tauri の「devUrl か実行ファイル内の画面か」の印 |

- `debug_assertions` は、Cargo の既定で dev は true、release は false（Cargo の本、profiles）。確かさ A。Rust の本にも「最適化なしのとき既定で入る」とある。確かさ A。
- `tauri::is_dev()` の中身は `!cfg!(feature = "custom-protocol")`（tauri-2.12.5/src/lib.rs 317〜319 行）。`cfg(dev)` は tauri の build.rs が `custom-protocol` が無いときに出す印で（build.rs 252〜255 行）、`tauri_build::build()` が自分のクレートにも `dev` の印を出す（tauri-build-2.7.4/src/lib.rs 596 行、714 行）。確かさ A。meloet の `build.rs` は `tauri_build::build()` を呼んでいるので `cfg(dev)` は使えるが、上のとおり意味がちがう。
- **注意: `cfg!(...)`（マクロ）は真偽の値になるだけで、どちらの枝もコンパイルされ、文字も残る。** 外したいときは `#[cfg(...)]`（属性）を使う。Rust の Reference に、`#[cfg]` は偽なら「ソースから消える」、`cfg!` は `true`/`false` の値になる、とある。確かさ A（「`cfg!` の枝も残る」は、その 2 つの文からの読みとり）。
- Cargo の本は、feature は足すだけの形（additive）にせよ、と書く。また、同じ依存が複数から使われると feature は「和」になる。確かさ A。meloet への意味: `cargo build --workspace --features …` や、テストと同じ呼び出しで build すると、`dev-hooks` が配る実行ファイルにも入る恐れがある。配るビルドは `cargo build --release -p meloet-app` のように 1 つだけ指す（resolver は `"3"` なので `-p` と `--features` の組みあわせは使える。Cargo の本 A）。
- feature をクレートをまたいで流すには `dev-hooks = ["meloet-core/dev-hooks"]` のように書く（Cargo の本、features A）。

### 仕込みがある場所（リポジトリで読んだ。確かさ A、自分のリポジトリ）
- `crates/meloet-app/src/devhooks.rs`: `MELOET_WINDOW`、`MELOET_SNAPSHOT`（と `_AFTER_MS`）、`MELOET_COMMANDS`、`MELOET_EVAL`、`MELOET_PIPE`、`MELOET_EXIT_AFTER_MS`、`MELOET_CLOSE_AFTER_MS`、`MELOET_PCWINDOW_SHOT`、`MELOET_OPEN_SONG`、`MELOET_UI_SCRIPT`。
- `crates/meloet-app/src/main.rs`: `MELOET_DEBUG_SCREEN`（253 行）。Tauri の命令 `measure_enabled`、`measure_report`、`ui_script`（99〜115 行、`generate_handler!` の 387〜389 行）。`devhooks::read_env()`、`apply_window_size`、`take_open_song`、`on_page_load` の呼び出し。
- `crates/meloet-app/src/pcwindow.rs` 145 行: `devhooks::after_pc_window`。
- `crates/meloet-core/src/measure.rs`: `MELOET_MEASURE_DIR`（`measure_dir` を読む）。
- テスト以外で、本番のクレートの中にあるほかの環境変数: `MELOET_INPUT_SINK_MONITOR`（core/service.rs 425）、`MELOET_ACCEPT_VIRTUAL`、`MELOET_ONLY_VIRTUAL`、`MELOET_VKEYS_NAME`、`MELOET_MIDI_FAIL_AFTER`（midi/lib.rs 119〜128）、`MELOET_PLUGHOST_FAKE`（plughost/formats/fake.rs）、`MELOET_PLUGHOST_NO_DUMP`、`MELOET_PLUGHOST_RT`（plughost/main.rs、rt.rs）。これらを外すかは棟梁（とオーナーさん）が決めること。
- 本物の設定として残すか決める物: `MELOET_STATE_DIR`、`MELOET_CONFIG_DIR`、`MELOET_SF2_DIR`、`MELOET_SONGS_DIR`、`MELOET_SOUNDS_DIR`（置き場の差し替え）。

### `#[tauri::command]` と `generate_handler!`
`generate_handler!` の中の各項目は、外側の属性（`#[cfg(...)]`）を受けとれる（tauri-macros-2.7.4/src/command/handler.rs 155〜177 行。`attrs` を各項目の頭に並べて出している）。確かさ A。だから、
```rust
#[cfg(feature = "dev-hooks")] measure_enabled,
```
の形で外せる。命令の関数の側にも同じ `#[cfg]` を、`#[tauri::command]` より上に書く（C: 並べかたは私の推論。コンパイルで確かめること）。

## 3. Tauri 2.12.5 の「開発か本番か」

- `tauri` の feature `custom-protocol`（既定では入っていない）が、開発か本番かのただ 1 つの印。lib.rs の説明にも「Tauri CLI が管理する feature。入っていると本番とみなす」とある（lib.rs 32 行）。確かさ A。
- `tauri build`: CLI が `tauri/custom-protocol` を足す。`tauri dev`: 足さず、Cargo.toml の既定にあればはずす（tauri-cli の `interface/rust.rs`、GitHub の dev ブランチを読んだ。版を固定していないので、確かさは A だが「入れた版そのもの」ではない）。
- `custom-protocol` なしで devUrl があると、画面は埋めこまれず、devUrl から読む（codegen は assets を空にする。tauri-codegen-2.7.4/src/context.rs 176〜181 行。`manager/mod.rs` 345〜359 行）。確かさ A。
- `custom-protocol` ありだと `frontendDist`（meloet は `../../ui/dist`）が埋めこまれる（context.rs 182〜193 行）。確かさ A。
- 食いちがい: Tauri の公式リリースノート（tauri-build v2.0.0-beta.5）は「`custom-protocol` feature はアプリに必要なくなり、無視される。本番かは `#[cfg(not(dev))]` で見よ」と書く（確かさ A、ただし 2024-02 で古い）。ところが 2.12.5 のソースでは、feature が `dev` の印を決めている。つまり「アプリ自身が feature を持たなくてよくなった」という意味で、tauri 側の feature は今も使われている、と読める（C: 読みとり）。高いほう（入れた版のソース）を採る。
- `cargo build --release` と `tauri build` のちがい: `tauri build` は `custom-protocol` を足し、そのあと bundle を作る。`cargo build --release` は feature を足さない。meloet は `meloet-app` の既定で入れているので、どちらでも本番の動き。`tauri build --debug` は debug のビルドでも本番の動き（Tauri の debug の文書。A）。
- Tauri の devtools は「開発と debug のビルドでだけ。本番で使うには `devtools` feature」（v2.tauri.app/develop/debug。ソースでも `any(debug_assertions, feature = "devtools")`。確かさ A）。meloet は入れていないので、release には devtools の窓も入らない（`tauri` の feature は既定のまま。meloet の workspace の `tauri = "=2.12.5"`。確かさ A）。

## 4. 画面側（Vite 8.3.8 / Rolldown 1.2.19）

- Vite 8 の本番ビルドは Rolldown と Oxc で動く（移行の文書。vite.dev/guide/migration。確かさ A）。
- `import.meta.env.DEV` ・ `PROD` ・ `MODE` は、本番では「作るときに文字として置きかわる。tree-shaking が効くように」（vite.dev/guide/env-and-mode。確かさ A）。`if (import.meta.env.DEV)` の中は本番で消える、と文書の例にある。
- ソースで確かめた（`ui/node_modules/vite/dist/node/chunks/node.js` 17059〜17126 行、37711・37827 行）: ビルドのとき、`config.env`（`DEV: !isProduction`、`PROD`、`MODE`）と、使う人の `define` が、Rolldown の `transform.define` に渡される。`isProduction` は `NODE_ENV === "production"`。文書にも「`vite build --mode development` でも `NODE_ENV` は production のまま」とある。確かさ A。
- `define` は「ビルドのときは文字として置きかわる（JSON にできる値か識別子）」（vite.dev/config/shared-options。確かさ A）。
- **未確認:** 置きかわった `if (false) { await import("./x") }` の中の動的 import の塊が出力されないか。Rolldown の文書（in-depth/dead-code-elimination）は「使われず副作用のないコードは消す。確かでないときは残す」とあり、`if (false)` の枝の動的 import は書いていない。探しても一次情報は見つからなかった。Webpack では「枝は消えたが、動的 import した先のモジュールが残る」報告もある（C、ほかの道具の話）。→ 作って `ui/dist` を見れば、すぐわかる。
- いまの `ui/dist` の事実（読んだ。確かさ A）: `ui/dist/assets/start-BIuY9ziL.js` があり、`MELOET_` などの名前が 1 つの塊の中にある。`index-Y9exDC4T.js` には `measure_report`・`measure_enabled`・`ui_script` の文字がある（`bridge/tauri.ts` 26〜31 行）。`ui/src/gl/Surface.svelte` 40 行の `import("../dev/start")` は、Rust から台本が来たときだけ動くが、`if` が実行時の値なので、塊はいつも出る。`ui/src/gl/measure.ts`・`profile.ts` は、ふつうの import で本体の塊に入っている。
- 確かめかた（`ui/dist` を grep）: `grep -rlE 'MELOET_|measure_report|measure_enabled|ui_script|hist-|pr-scroll' ui/dist`。`ls ui/dist/assets | grep -E '^start-'`。

### 画面側の作りのすすめ（推論なので確かさ C。試験で確かめること）
- 「測るとき」は release でも画面の仕込みが要るので、`import.meta.env.DEV` では足りない。`vite build --mode measure` にして `import.meta.env.MODE === "measure"` か、`define` で `__DEV_HOOKS__` を決める（`process.env.MELOET_DEV_HOOKS === "1"`）。
- 仕込みへの入口を 1 つにする（`ui/src/dev/index.ts` のようなファイルを、`if (__DEV_HOOKS__) { await import(...) }` の中でだけ読む）。`bridge/tauri.ts` の `measure_*`・`ui_script` の呼び出しと、`gl/measure.ts`・`profile.ts` の使いも、その入口の中に移す。
- 動的 import の塊が DCE で消えないと出たら、「木から落ちるのを当てにしない」形にする: Vite の `resolve.alias`（または小さなプラグイン）で、本番では `dev/index.ts` を空の `dev/index.off.ts` に差しかえる。

## 5. Tauri が画面を実行ファイルに入れる方法

- `tauri-codegen` が `ui/dist` の各ファイルを読み、内容の hash でファイル名を決め、**brotli で圧縮**して `OUT_DIR` に書く。`include_bytes!` で実行ファイルに入れる（tauri-codegen-2.7.4/src/embedded_assets.rs 289〜391 行、394〜407 行）。feature `compression` が既定で有効で、無効ならそのまま入る（tauri の `Cargo.toml` の default、76〜91 行）。確かさ A。
- 圧縮の質は `cfg!(debug_assertions)` なら 2、でなければ 9（同 296〜300 行）。確かさ A。
- 読むとき（`tauri-utils-2.10.2/src/assets.rs` 168〜176 行）は、`BrotliDecompress` で戻す。確かさ A。
- 結論: 圧縮された塊を `strings` で見ても JS の文字は読めない（brotli の出力は文字の形で残らない。C: 推論）。見るべきは `ui/dist`（入れる前の物。入れた物はそのコピー）。

## 6. ほかのアプリの例（一次情報の範囲）

- Electron: `app.isPackaged`（配った物かどうかの真偽）。確かさ A（electronjs.org/docs/latest/api/app）。「Fuses」は、パッケージのとき、実行ファイルの中のビットを書きかえ、`runAsNode` や `nodeCliInspect` など使わない機能を切る。これは「コードは残して使えなくする」やり方で、meloet の「コードごと外す」より弱い。確かさ A（electronjs.org/docs/latest/tutorial/fuses、tutorial/security）。
- Tauri: `devtools` を feature にして、本番では入れない。確かさ A（v2.tauri.app/develop/debug）。
- 「仕込みの名前を実行ファイルから探して見はる試験」の例は、一次情報では見つからなかった（探したが出なかった）。7 節は私の案（C）。

## 7. 見はりの試験の形（案。確かさ C: 手順は推論、材料は上の A の事実）

1. **Rust の実行ファイル**（`cargo build --release -p meloet-app`、feature なし）に `strings target/release/meloet | grep -E '<仕込みの名前>'` で何も出ないこと。名前は、環境変数（`MELOET_PIPE` など）に加えて、パスと記号（`devhooks`、`crates/meloet-app/src/devhooks.rs`）も。release の既定は `strip = "none"` なので記号の名前が残る（Cargo の本、profiles。meloet の workspace は `[profile.release]` を持たない。確かさ A）。仕込みを外すと、これらも消える。
2. **画面**: `vite build`（測りの mode でない）の `ui/dist` に `grep -rE` で名前が無いこと、`start-*.js` が無いこと。Rust の `build.rs` ではなく、`scripts/check-all.sh` に足す形が素直。
3. **実際に入った物**: `meloet-app` の試験（feature なしでビルド）で `tauri::generate_context!()` を呼び、`context.assets().iter()` で各ファイルの key を取り、`context.assets().get(&key)` で brotli を戻して、名前を探す。`Context::assets()` は `&dyn Assets`、`get` は戻した中身を返す（tauri-2.12.5 lib.rs 329〜332 行、444 行。確かさ A）。「配る実行ファイルに入ったもの」を直接見られるので、`ui/dist` の grep が古い、という穴を塞げる。
4. **逆の試験**: `--features dev-hooks`（と `vite build --mode measure`）のときは、名前が「ある」こと。見はりの試験が空振りしていない証になる。
5. 「`tauri::is_dev()` が false」の確認は要らない（meloet は `custom-protocol` が既定。`cargo build --release` でも本番）。

## 8. meloet へのすすめ

- feature の名前: `dev-hooks`。`meloet-app` に `dev-hooks = ["meloet-core/dev-hooks", …]` と置き、`default` に入れない。`custom-protocol` はそのまま既定。
- `cfg` をつける所:
  - `devhooks.rs` のモジュールごと（`main.rs` の `#[cfg(feature = "dev-hooks")] mod devhooks;`）。呼び出し側（`read_env`、`apply_window_size`、`take_open_song`、`on_page_load`、`pcwindow.rs` の `after_pc_window`）は、`#[cfg(feature)]` と `#[cfg(not(feature))]` の 2 つの小さな関数（なにもしない版）か、呼び出しの行ごと cfg にする。
  - Tauri の命令 `measure_enabled`、`measure_report`、`ui_script`（関数と `generate_handler!` の項目の両方）。`MELOET_DEBUG_SCREEN` の `if`。
  - `meloet-core/src/measure.rs`（`MELOET_MEASURE_DIR`）、`meloet-midi` の `MELOET_*` の試験用の鍵盤の設定、`meloet-plughost` の `MELOET_PLUGHOST_FAKE`・`_NO_DUMP`・`_RT`、`MELOET_INPUT_SINK_MONITOR`。置き場の差し替え（`STATE_DIR` など）は、本番に残すかを先に決める。
  - 画面: `__DEV_HOOKS__`（または `MODE === "measure"`）の `if` の中でだけ、仕込みの入口を動的に読む。Rust の `dev-hooks` と同じ時に立てる（`tools/measure/` の道具が両方を一緒に作るようにする）。
- 順序: Rust 側と、画面側を別々に直せる。画面側は、まず `ui/dist` の grep の試験を足してから、直すと、DCE が効くかどうかが試験で見える。
- 見はりの試験: 上の 7 節の 1〜4。配る手順（Release のビルド）は `cargo build --release -p meloet-app`（`--features` なし）を 1 つだけ呼ぶ。
- やってはいけない（C）: `cfg(debug_assertions)` で外す（測りが release でできなくなる）。`cfg!(...)` で外す（文字が残る）。`--workspace --features dev-hooks` を配る build と同じ呼び出しに入れる（feature が和になる）。

## 9. 出どころ（見た日はすべて 2026-11-22）

| 出どころ | 書いてあった事 | 確かさ |
|---|---|---|
| `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-2.12.5/build.rs` 250〜257 行、`src/lib.rs` 32・317〜319・329〜332・444 行、`Cargo.toml` 76〜91 行、`src/manager/mod.rs` 345〜373 行 | `custom-protocol` が `dev` を決める、`is_dev()`、assets の iter と get、`compression` が既定 | A |
| 同 `tauri-build-2.7.4/src/lib.rs` 590〜610・714 行 | `is_dev()` と `cfg(dev)` の印 | A |
| 同 `tauri-codegen-2.7.4/src/embedded_assets.rs` 289〜407 行、`src/context.rs` 162〜199 行 | brotli の圧縮、質 2 か 9、`include_bytes!`、dev + devUrl では空 | A |
| 同 `tauri-utils-2.10.2/src/assets.rs` 168〜192 行 | 読むとき brotli で戻す | A |
| 同 `tauri-macros-2.7.4/src/command/handler.rs` 155〜177 行 | `generate_handler!` の項目の `#[cfg]` を受ける | A |
| `/srv/meloet-agent/meloet/ui/node_modules/vite/dist/node/chunks/node.js`（vite 8.3.8）17059〜17126、37711、37827 行 | `DEV`・`PROD`・`define` が Rolldown の `transform.define` に渡る | A |
| `/srv/meloet-agent/meloet` の `crates/meloet-app/Cargo.toml`、`src/devhooks.rs`、`src/main.rs`、`ui/vite.config.ts`、`ui/src/gl/Surface.svelte`、`ui/src/bridge/tauri.ts`、`ui/src/dev/start.ts`、`ui/dist/assets/` | 仕込みの場所、`default = ["custom-protocol"]`、`ui/dist` に `start-*.js` が残っている | A（自分のリポジトリ） |
| https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-cli/src/interface/rust.rs | `tauri build` は `tauri/custom-protocol` を足し、`tauri dev` は足さない | A（dev ブランチ。入れた版そのものではない） |
| https://v2.tauri.app/develop/debug/ | devtools は開発と debug のビルドだけ、`devtools` feature、`cfg(dev)`、`tauri::is_dev()`、`tauri build --debug` | A |
| https://v2.tauri.app/release/tauri-build/v2.0.0-beta.5/ | `custom-protocol` はアプリに不要になり無視、`cfg(not(dev))` を使え（2024-02 の古い記述。2.12.5 のソースと食いちがう） | A（古い。1 段下げて B 相当） |
| https://vite.dev/guide/env-and-mode | `import.meta.env.DEV` などは作るとき置きかわる、`MODE`、`--mode`、`NODE_ENV` とは別 | A |
| https://vite.dev/config/shared-options | `define` は dev では global、build では置きかえ、Oxc | A |
| https://vite.dev/guide/build 、https://vite.dev/guide/migration | Vite 8 は Rolldown と Oxc | A |
| https://rolldown.rs/in-depth/dead-code-elimination | 副作用のないコードを消す、確かでなければ残す | A |
| https://doc.rust-lang.org/cargo/reference/features.html | feature は和になる、additive、resolver 2 と `-p`、`CARGO_FEATURE_*` | A |
| https://doc.rust-lang.org/cargo/reference/profiles.html | `strip` の既定は none、`debug-assertions` は dev で true、release で false | A |
| https://doc.rust-lang.org/reference/conditional-compilation.html | `debug_assertions`、`#[cfg]` と `cfg!` | A |
| https://www.electronjs.org/docs/latest/api/app 、/tutorial/fuses 、/tutorial/security | `app.isPackaged`、Fuses はパッケージのときに書きかえる、`runAsNode` と `nodeCliInspect` | A |
| 検索の要約（`custom-protocol` の旧版の話、Webpack の動的 import 残りの報告） | 参考だけ。結論には使っていない | C |

## 10. わからなかったこと、追加で調べるとよいこと

- **Vite 8.3.8 + Rolldown 1.2.19 で `if (定数 false) { import() }` の塊が消えるか。** 文書にも検索にも一次情報が無かった。実際に `vite build` して `ui/dist` を見るのがいちばん早い（私は Bash が使えず、ビルドは走らせていない）。
- `#[cfg]` を `#[tauri::command]` の関数につけたときの順序の細かい動き（コンパイルで確かめる）。
- `tauri build` が `ui/dist` を作る前に走らせる `beforeBuildCommand`（meloet の `tauri.conf.json` には無い。Release の手順で `vite build` をどう呼ぶかは決める事）。
- Tauri 2.12.5 で `generate_context!()` の `assets()` を試験の中で呼べるか（理屈は A のソースで足りているが、動かしていない）。
- 本番に残す環境変数（`MELOET_STATE_DIR` など、置き場の差し替え）の線引きは、棟梁とオーナーさんが決める。
