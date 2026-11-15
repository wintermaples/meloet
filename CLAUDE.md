# meloet — 次に来るきみへ

meloet は、MIDI 鍵盤を買ったばかりの人のための DAW です。お願いはヤチヨさん（デザインと方向を決める人）から、毎回あたらしい会話で来ます。
この書類を最初に読み、次に [`docs/progress.md`](docs/progress.md)（どこまでできたか、宿題、申し送り。いちばん下の「次の回への申し送り」から）、[`docs/architecture.md`](docs/architecture.md)（骨組みと約束と理由）、[`docs/design-notes.md`](docs/design-notes.md)（仕様書とモックの読みかた、デザインに戻した事）を読んでください。

返事は日本語で、やさしい言葉で。

## はたらきかた

- きみは棟梁。設計する、仕事を切り分ける、係に頼む、受けとって確かめる、まとめる。**大きな実装は自分で書かない**。
- 係は `.claude/agents/` の 3 つ。
  - `meloet-hard`（Opus 5.5、effort medium。2026-11-15 にヤチヨさんが Fable 5.2 から変えた）: 音のスレッド、unsafe と FFI、スレッドやプロセスの間のやりとり、WebGL、Rust と画面の約束の核、速さの測定。
  - `meloet-impl`（Opus 5.5、effort medium）: 画面の部品、帯の骨組み、設定、文面、テスト、道具。
  - `meloet-research`（Sonnet 5.5、effort high）: インターネットで調べる。コードは書かない。
  - 難しいか、ふつうか、迷ったら難しいほうへ。
  - 会話の途中で係のファイルを作ったり変えたりすると、その会話からは名前で呼べない。そのときは汎用の係を同じモデルと effort で呼び、最初に係のファイルを読ませる。
- 係は、これまでの話を知らない。頼むときは、どのファイルを、どの決まりで、どう確かめるかまで、ぜんぶ書く。同時に何人も動かすときは、担当のディレクトリを分け、`CARGO_TARGET_DIR=target/<名前>` と `cargo -p` で、ビルドがぶつからないようにする。
- 係から受けとった物は、自分で確かめる（ビルド、テスト、動かす）。「できました」をそのまま信じない。
- 何人も同時に動かすと、同じファイルに書きかけがまざる。係ごとにさわってよいファイルを依頼文に書く（「読みなおしてから、足す形で」も）。まざったときは、スクラッチに `git worktree add --detach <dir> HEAD` を作り、その係の分のファイルだけを写して確かめ、そこでコミットしてから `git update-ref refs/heads/foundation <commit>` と `git reset -q`（ほかの係の書きかけのファイルにはさわらない）。`ui/node_modules` は symlink で足りる。
- 約束が変わる大きな仕事は 2 回に分けて頼む: 1 回目で約束の形と TS の型と偽物の場面だけ作らせてコミット → 2 回目で core と画面を別の係に同時に頼む。
- 速さを測るときは PC を静かにする（ほかの係のビルドやアプリを止める。ヤチヨさんの meloet が動いていないか `ps -u wintermaples`）。にごった数は、その条件を書いて残す。測定はヘッドホンに音が出るので、長いものはヤチヨさんに一言。
- 係が「止められていた」ときは、新しい係を立てる前にヤチヨさんに聞く。
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
- 画面の文面は `ui/src/text/ja.ts` の一か所。仕様書の文面は一字も変えない。モックにだけある文は、モックの文のまま（出どころつき）。ヤチヨさんが決めた文は出どころ `user:<日付>`。
- モックと仕様書が食いちがったら、仕様書が正しい。直した所は一覧にする。
- まだ中身のない部品は、消さずに「使えない」状態で出す。
- 曲の中の位置はティック（4 分音符 = 480）。録った鍵は「押した瞬間に耳に届いていた位置」に置く。クオンタイズは直す操作としてだけ（`docs/architecture.md` 11・12 章）。
- 曲を変える操作は、どれも「取り消せる手」1 つ（`meloet_model::History`）。テイクは一度できたら中身を変えない（変えるなら新しい id）。
- 曲のファイルは版つきの JSON。こわれない書きかたと `.prev` を守る（14 章）。形を変えたら版の扱いを決める。
- WebGL の文脈は予算 12（WebKitGTK は 16 まで、捨てた canvas も GC まで数に入る）。区画ごとに 1 枚、使いまわし（7 章）。60 Hz で変わる物は小さな区画に、毎コマのレイアウトはしない。
- コメントは、意図が伝わりにくい所にだけ。変えた理由や履歴は書かない。コミットは小さく、こまめに。
- `spikes/` は消さない。そのまま本番に写さない。学んだことだけ連れてくる。
- sudo がいるときは、コマンドを見せてから頼む。別のユーザーで動いていて入口（ACL、xhost）をあけてもらったら、最後に閉じて、閉じたと伝える。

## 骨組み（ひとめで）

```
crates/
  meloet-model     曲・トラック・クリップ・テイク・ノート（純粋なデータ）、取り消しの手（History）、録った出来事をテイクにする決まり（Recorder）
  meloet-protocol  Rust と画面の約束。TS の型は ui/src/bridge/generated/ に書き出す
  meloet-engine    音の流れ、音源の口（Instrument）、仮の音、メーター、cpal の出口、再生まわり（transport.rs）、耳に届いている位置（clock.rs）
  meloet-midi      ALSA シーケンサ。鍵盤のぬきさし。meloet-vkeys（試験用の仮想の鍵盤。--name）
  meloet-devices   PipeWire の Route（人の言葉の名前）と settings（固定の quantum）を読む
  meloet-core      状態の持ち主。コマンド → 状態 → rev つきの差分。帯の決まり（layout.rs）、再生・録音・取り消し（play.rs）、曲のファイル（songfile.rs）、保存の流れ（state/songs.rs）、つなぎなおし（service.rs）
  meloet-app       Tauri の殻。web プロセスの見はり（webprocess.rs）、PC のファイルの窓（pcwindow.rs）
ui/
  src/bridge/      Tauri を知る唯一の場所
  src/design/      数値（仕様書の名前のまま）
  src/text/        文面
  src/gl/          WebGL の係（区画、使いまわし、予算）
docs/
  design/          仕様書（spec.html）とモック（mock/）の写し
  architecture.md  骨組みと理由
  progress.md      進みぐあい・宿題・申し送り
  design-notes.md  仕様書とモックの読みかた、デザインに戻した事
  measurements/    測った数
  screenshots/     本物のアプリの絵とモックのならべ
spikes/            検証の試作（消さない、写さない）
```

## よく使うコマンド

```bash
source ~/.cargo/env; export PATH=$HOME/.local/node/bin:$PATH
scripts/check-all.sh             # fmt、clippy、Rust のテスト、型の検査、画面のテスト、ライセンスの見はり
scripts/check-all.sh --devices   # 上に加えて、鍵盤や PipeWire が要るテスト
python3 -I tools/design/render_mocks.py target/mock-png   # モックを PNG に
python3 -I tools/design/shoot_ui.py <出力> <場面>           # 偽物の場面とモックをならべる
python3 -I tools/design/shoot_app.py <状態>                 # 本物のアプリを撮る（docs/screenshots/）
tools/measure/latency.sh --state stopped|playing|recording  # 鍵 → 画面、鍵 → 音
tools/measure/record-timing.sh private                      # 録った位置のずれ（自分用の PipeWire）
tools/measure/long-record.sh                                # 10 分録りつづける
```

動かしかたは [`README.md`](README.md)。

## この PC の事情（本番のユーザーには関係しない）

- エージェントは `meloet-agent` という別ユーザーで動く。画面と音は `source ~/.config/meloet-agent/env.sh` で入る（入口はヤチヨさんにあけてもらう）。
- GTK のアプリは起動で 30 秒止まることがある。`dbus-run-session -- ` をつけて起動する。
- cpal が PipeWire を見つけないときは `PIPEWIRE_RUNTIME_DIR=/run/user/1000`。
- ヤチヨさんは同じ作業ツリーを VS Code で開いていて、vite がポート 1420 で動いていることがある。`npm run dev` を 1420 で走らせない。`ui/node_modules/.vite/` を消さない。ヤチヨさんがデバッグ版の meloet を動かしていることもある。
- PipeWire を落とす試験は、自分用の PipeWire と WirePlumber で（`crates/meloet-engine/tests/private_pw/`）。ヤチヨさんの PipeWire は止めない。ソケットの道は 108 バイトまでなので `/run/user/1001/` の下に置く。
- 閉じるときの WebKitWebProcess の SIGSEGV（NVIDIA の EGL）は、web プロセスを先に止める形で回避した。apport は、まだ見ていない `.crash` があると同じプログラムの新しい報告を書かないので、「落ちていない」は `/var/crash` では確かめられない（`docs/progress.md` 第 2 回の区切り0）。
- 「GC のスレッドが何百もできては消える」の正体は、GTK のタイトルバーのアイコンの読みなおし（glycin）。ひびきはない（同じ所）。
- シェルで `a && b && ( … ) & c` と書くと、`&` が前の列ぜんぶを後ろに回す。変数を決める行と背景で動かす行は分ける。
