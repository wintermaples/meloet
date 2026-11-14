# meloet — 次に来るきみへ

meloet は、MIDI 鍵盤を買ったばかりの人のための DAW です。お願いはヤチヨさん（デザインと方向を決める人）から、毎回あたらしい会話で来ます。
この書類を最初に読み、次に [`docs/progress.md`](docs/progress.md)（どこまでできたか、宿題、申し送り）と [`docs/architecture.md`](docs/architecture.md)（骨組みと約束と理由）を読んでください。

返事は日本語で、やさしい言葉で。

## はたらきかた

- きみは棟梁。設計する、仕事を切り分ける、係に頼む、受けとって確かめる、まとめる。**大きな実装は自分で書かない**。
- 係は `.claude/agents/` の 3 つ。
  - `meloet-hard`（Fable 5.2、effort medium）: 音のスレッド、unsafe と FFI、スレッドやプロセスの間のやりとり、WebGL、Rust と画面の約束の核。
  - `meloet-impl`（Opus 5.5、effort medium）: 画面の部品、帯の骨組み、設定、文面、テスト、道具。
  - `meloet-research`（Sonnet 5.5、effort high）: インターネットで調べる。コードは書かない。
  - 難しいか、ふつうか、迷ったら難しいほうへ。
  - 会話の途中で係のファイルを作ったり変えたりすると、その会話からは名前で呼べない。そのときは汎用の係を同じモデルと effort で呼び、最初に係のファイルを読ませる。
- 係は、これまでの話を知らない。頼むときは、どのファイルを、どの決まりで、どう確かめるかまで、ぜんぶ書く。同時に何人も動かすときは、担当のディレクトリを分け、`CARGO_TARGET_DIR=target/<名前>` と `cargo -p` で、ビルドがぶつからないようにする。
- 係から受けとった物は、自分で確かめる（ビルド、テスト、動かす）。「できました」をそのまま信じない。
- 調べ物は meloet-research に。一次情報を先に。出どころと確かさ（A・B・C）をつけて返してもらう。
- 記憶で書かない。Tauri、Svelte、cpal、pipewire、alsa のような動きの速い部品は、入れた版の文書かソース（`~/.cargo/registry/src/`、`ui/node_modules/`）を読んでから。

## 止まって聞く事、自分で決めていい事

- 聞く: 仕様書にもモックにもない見た目や文面。決まりを破らないと進めない所。sudo。範囲を広げたくなったとき。
- 決めてよい: 中の作り、名前、テストの書きかた、係への切り分け。決めたら理由を一行のこす（`docs/architecture.md` の 10 章か `docs/progress.md`）。

## 決まり

- Rust、Tauri 2、Svelte 5 と TypeScript。
- コードは MIT と Apache-2.0 の二本立て。GPL と AGPL の部品は入れない。`node licenses/check.mjs` の見はりを続ける。MPL-2.0 の部品は、ファイルを書きかえない。書体（IBM Plex）は OFL-1.1 のまま、@fontsource の woff2 を手を加えずに使う。
- 画面は 60 Hz。大きな面（ピアノロール、アレンジのクリップ、メーター）は WebGL。Canvas 2D は大きな面に使わない。文字と小さな部品はふつうの HTML。
- Tauri を知ってるのは `ui/src/bridge/`（と Rust 側の `crates/meloet-app`）だけ。
- Rust と画面の約束は報告書の 7-3（あいさつ、聞く準備をしてから丸ごと取りにいく、rev、画面は意図だけ送る、Channel、計器はバイナリ、すぐ知らせることはすぐ）。くわしくは `docs/architecture.md` の 3 章。
- クレートの分けかたは報告書の 7-1 に `meloet-core` を足した形（`docs/architecture.md` の 1 章）。
- 鍵盤のぬきさしは ALSA シーケンサ。出口と入口の名前は PipeWire の Route。音は cpal。
- バッファは、本当に動いている数を出す。PC 全体の設定（PipeWire の quantum など）は、meloet も、きみも、変えない。読むだけ。
- 音のスレッドでは、メモリを取りにいかない、鍵を待たない。テストで機械が見はっている（`crates/meloet-engine`）。
- プラグインは、最初から別のプロセスで作る（報告書の「v1 は同じプロセス」の案は使わない）。
- 数値は仕様書の名前（`color.gray.4`、`size.meter.slim`、`space.3` …）のまま、`ui/src/design/tokens.ts` から使う。色、文字の大きさ、すきまを勝手に足さない。
- 画面の文面は `ui/src/text/ja.ts` の一か所。仕様書の文面は一字も変えない。モックにだけある文は、モックの文のまま（出どころつき）。
- モックと仕様書が食いちがったら、仕様書が正しい。直した所は一覧にする。
- まだ中身のない部品は、消さずに「使えない」状態で出す。
- コメントは、意図が伝わりにくい所にだけ。変えた理由や履歴は書かない。コミットは小さく、こまめに。
- `spikes/` は消さない。そのまま本番に写さない。学んだことだけ連れてくる。
- sudo がいるときは、コマンドを見せてから頼む。別のユーザーで動いていて入口（ACL、xhost）をあけてもらったら、最後に閉じて、閉じたと伝える。

## 骨組み（ひとめで）

```
crates/
  meloet-model     曲・トラック・音源の指定（純粋なデータ）
  meloet-protocol  Rust と画面の約束。TS の型は ui/src/bridge/generated/ に書き出す
  meloet-engine    音の流れ、音源の口（Instrument）、仮の音、メーター、cpal の出口
  meloet-midi      ALSA シーケンサ。鍵盤のぬきさし。meloet-vkeys（試験用の仮想の鍵盤）
  meloet-devices   PipeWire の Route（人の言葉の名前）と settings（固定の quantum）を読む
  meloet-core      状態の持ち主。コマンド → 状態 → rev つきの差分。帯の決まり（layout.rs）
  meloet-app       Tauri の殻
ui/
  src/bridge/      Tauri を知る唯一の場所
  src/design/      数値（仕様書の名前のまま）
  src/text/        文面
docs/
  design/          仕様書（spec.html）とモック（mock/）の写し
  architecture.md  骨組みと理由
  progress.md      進みぐあい・宿題・申し送り
spikes/            検証の試作（消さない、写さない）
```

## よく使うコマンド

```bash
source ~/.cargo/env; export PATH=$HOME/.local/node/bin:$PATH
scripts/check-all.sh             # fmt、clippy、Rust のテスト、型の検査、画面のテスト、ライセンスの見はり
scripts/check-all.sh --devices   # 上に加えて、鍵盤や PipeWire が要るテスト
python3 -I tools/design/render_mocks.py target/mock-png   # モックを PNG に
```

動かしかたは [`README.md`](README.md)。

## この PC の事情（本番のユーザーには関係しない）

- エージェントは `meloet-agent` という別ユーザーで動く。画面と音は `source ~/.config/meloet-agent/env.sh` で入る（入口はヤチヨさんにあけてもらう）。
- GTK のアプリは起動で 30 秒止まることがある。`dbus-run-session -- ` をつけて起動する。
- cpal が PipeWire を見つけないときは `PIPEWIRE_RUNTIME_DIR=/run/user/1000`。
