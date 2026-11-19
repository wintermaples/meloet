# meloet — 次に来るきみへ

meloet は、MIDI 鍵盤を買ったばかりの人のための DAW です。お願いはヤチヨさん（デザインと方向を決める人）から、毎回あたらしい会話で来ます。
この書類を最初に読み、次に [`docs/progress.md`](docs/progress.md)（どこまでできたか、宿題、申し送り。いちばん下の「次の回への申し送り」から）、[`docs/architecture.md`](docs/architecture.md)（骨組みと約束と理由）、[`docs/design-notes.md`](docs/design-notes.md)（仕様書とモックの読みかた、デザインに戻した事）を読んでください。

返事は日本語で、やさしい言葉で。

## はたらきかた

- きみは棟梁。設計する、仕事を切り分ける、係に頼む、受けとって確かめる、まとめる。**大きな実装は自分で書かない**。
- 係は `.claude/agents/` の 3 つ。
  - `meloet-hard`（Fable 5.2、effort medium。2026-11-15 に Fable の使用上限で Opus 5.5 にし、2026-11-18 にヤチヨさんが Fable 5.2 に戻した）: 音のスレッド、unsafe と FFI、スレッドやプロセスの間のやりとり、WebGL、Rust と画面の約束の核、core のデータの作り、速さとメモリの測定。
  - `meloet-impl`（Opus 5.5、effort medium）: 画面の部品、帯の骨組み、設定、文面、テスト、道具。
  - `meloet-research`（Sonnet 5.5、effort high）: インターネットで調べる。コードは書かない。
  - 難しいか、ふつうか、迷ったら難しいほうへ。
  - 会話の途中で係のファイルを作ったり変えたりすると、その会話からは名前で呼べない。そのときは汎用の係を同じモデルと effort で呼び、最初に係のファイルを読ませる。
- 係は、これまでの話を知らない。頼むときは、どのファイルを、どの決まりで、どう確かめるかまで、ぜんぶ書く。同時に何人も動かすときは、担当のディレクトリを分け、`CARGO_TARGET_DIR=target/<名前>` と `cargo -p` で、ビルドがぶつからないようにする。`target/<名前>` は 1 つで 4〜9 GB になる。係の仕事を merge したら、その target は消す（2026-11-18 にディスクが満杯になり、166 GB 消した。`debug/` と `release/` は残す）。
- 同時に何人も動かすときは、係を作業ツリーに分ける（Agent の `isolation: "worktree"`）。作業ツリーは **main から**作られるので、依頼文の最初に `git reset -q --hard <foundation のいま>` を書く。`ui/node_modules` は symlink をはらせ、係にはコミットさせない。終わったら棟梁が係の作業ツリーで確かめてコミットし、foundation へ merge する。
- merge のあとは、ぶつかった所だけでなく全体を見なおす。git は enum の最後に足した腕を、となりの別の enum に入れることがある（第 3 回で 2 回あった）。`scripts/check-all.sh` は Python の道具（`tools/`）を読まないので、さわったら `python3 -c "import ast; ast.parse(open('<file>').read())"` も。
- 画面の 2 人が同時に作るときは、継ぎ目（2 人が読む状態のモジュール、片方が入れ物を作りもう片方が中身を作る部品）を、棟梁が先に小さく置いてコミットしてから頼む（第 3 回の `ui/src/components/pianoroll/view.svelte.ts`・`shown.svelte.ts`・空の `NoteListPanel.svelte` など）。
- 係から受けとった物は、自分で確かめる（ビルド、テスト、動かす）。「できました」をそのまま信じない。
- 係の依頼文には「背景で待つしかけ（`until ! pgrep -f …; do sleep …` のループ）を使わない」と書く。`pgrep -f "<文字>"` は、そのループを走らせている `bash -c` 自身の行にも合うので、いつまでも終わらない。係はそれを待ったまま止まる（2026-11-18、1 時間ほど返ってこなかった。前の回にも残っていた）。係が返ってこないときや、終わったあとは `ps -u meloet-agent -o pid,ppid,etime,args | grep -E "sleep|until"` で見て、残ったループを止める（親の `claude` は止めない）。
- 何人も同時に動かすと、同じファイルに書きかけがまざる。係ごとにさわってよいファイルを依頼文に書く（「読みなおしてから、足す形で」も）。まざったときは、スクラッチに `git worktree add --detach <dir> HEAD` を作り、その係の分のファイルだけを写して確かめ、そこでコミットしてから `git update-ref refs/heads/foundation <commit>` と `git reset -q`（ほかの係の書きかけのファイルにはさわらない）。`ui/node_modules` は symlink で足りる。
- 約束が変わる大きな仕事は 2 回に分けて頼む: 1 回目で約束の形と TS の型と偽物の場面だけ作らせてコミット → 2 回目で core と画面を別の係に同時に頼む。
- プラグインの窓をひらく試験は、ヤチヨさんの画面に窓が出るが、聞かずに進めてよい（ヤチヨさん 2026-11-19）。
- 速さを測るときは PC を静かにする（ほかの係のビルドやアプリを止める。ヤチヨさんの meloet が動いていないか `ps -u wintermaples`）。にごった数は、その条件を書いて残す。測定はヘッドホンに音が出るが、聞かずに進めてよい（ヤチヨさん 2026-11-18）。メモリの流れ（RSS）は、少しにごっていても見られる。
- 画面だけで動く物（スクロール、拡大、ドラッグ）を本物のアプリで測るときは、`MELOET_UI_SCRIPT`（測るときだけ動く、ページの中の自動の係）と `MELOET_OPEN_SONG`（曲を開いて始める）を使う（`tools/measure/ui-script.sh`）。
- 係が「止められていた」ときは、新しい係を立てる前にヤチヨさんに聞く。
- 使える量の上限で係が止まったら、上限が戻ってから `SendMessage`（係の id）で「止めた所から続けて」と呼びもどす。作業ツリーとこれまでの話がそのまま残る。**呼びもどすかもしれない間は、その係の作業ツリーを片づけない**（片づけると呼びもどせない。2026-11-20 に 1 回あった）。ヤチヨさんに頼まれたら、上限が戻る時刻に `CronCreate` の一回きりの予約を入れる（この会話の間だけ動く）。
- 係のスクラッチのファイルは、ほかの係と共用の場所になることがある。依頼文で、ファイルの名前の頭に係ごとの印（`k3core2-` など）をつけさせる（第 5 回に上書きしあった）。
- 調べ物は meloet-research に。一次情報を先に。出どころと確かさ（A・B・C）をつけて返してもらう。
- 記憶で書かない。Tauri、Svelte、cpal、pipewire、alsa のような動きの速い部品は、入れた版の文書かソース（`~/.cargo/registry/src/`、`ui/node_modules/`）を読んでから。

## 止まって聞く事、自分で決めていい事

- 聞く: 仕様書にもモックにもない見た目や文面。決まりを破らないと進めない所。sudo。範囲を広げたくなったとき。
- 決めてよい: 中の作り、名前、テストの書きかた、係への切り分け。決めたら理由を一行のこす（`docs/architecture.md` の 10 章か `docs/progress.md`）。

## 決まり

- Rust、Tauri 2、Svelte 5 と TypeScript。
- コードは MIT と Apache-2.0 の二本立て。GPL と AGPL の部品は入れない。`node licenses/check.mjs` の見はりを続ける。MPL-2.0 の部品は、ファイルを書きかえない。書体（IBM Plex）は OFL-1.1 のまま、@fontsource の woff2 を手を加えずに使う。
- 画面は 60 Hz。大きな面（ピアノロール、アレンジのクリップ、メーター）は WebGL。Canvas 2D は大きな面に使わない。文字と小さな部品はふつうの HTML。
- ドラッグ中は画面だけが先に動き、離したときに意図を 1 つ送る（ドラッグ 1 回が 1 手）。もとの位置は gray.7 の枠。Esc はキーの順のいちばん前でドラッグをやめる（`ui/src/components/pianoroll/view.svelte.ts` の `cancelDrag`）。ミキサーだけは、ドラッグ中も音を変える: 「音だけ変える」意図（`commit: false`、1 コマに 1 つまで）を送り、離して「決める」（`commit: true`、1 手）、Esc で `CancelMixPreview`（`ui/src/components/mixer/mixDrag.ts`）。
- Tauri を知ってるのは `ui/src/bridge/`（と Rust 側の `crates/meloet-app`）だけ。
- Rust と画面の約束は報告書の 7-3（あいさつ、聞く準備をしてから丸ごと取りにいく、rev、画面は意図だけ送る、Channel、計器はバイナリ、すぐ知らせることはすぐ）。くわしくは `docs/architecture.md` の 3 章。
- クレートの分けかたは報告書の 7-1 に `meloet-core` を足した形（`docs/architecture.md` の 1 章）。
- 鍵盤のぬきさしは ALSA シーケンサ。出口と入口の名前は PipeWire の Route。音は cpal。
- バッファは、本当に動いている数を出す。PC 全体の設定（PipeWire の quantum など）は、meloet も、きみも、変えない。読むだけ。
- 音のスレッドでは、メモリを取りにいかない、鍵を待たない。テストで機械が見はっている（`crates/meloet-engine`、`crates/meloet-sound`）。値は 20 ms でなめらかに、エフェクトや音源を入れかえるときは 10 ms 重ねる。NaN は和の所で止め、denormal はくり返しの所で小さな値を 0 にする（FTZ ・ DAZ は立てない）。
- 最初から入っているエフェクトは自分で書く（`crates/meloet-sound/src/effects/`）。音色の素材は CC0、CC BY、MIT だけ（CC BY-SA は使わない）。帰属表示は `NOTICE` と「meloet について」で、`node licenses/check.mjs` が素材まで見はる。素材の作りかたはリポジトリ（`tools/sounds/`）、作った包みは GitHub の Release（上げる前にヤチヨさんに聞く）。
- プラグインは別のプロセス（`meloet-plughost`、同じプラグインは 1 つのプロセスにまとめる）。標準出力 ・ 標準エラーをやりとりに使わない。止まったかは時間で見はる（`docs/architecture.md` 20 章）。子のプロセスは `meloet_plugin::launch::spawn` で 1 つずつ起こす（VS Code の LLDB のため）。形式は VST3 と LV2（CLAP は入れていない）。
- 数値は仕様書の名前（`color.gray.4`、`size.meter.slim`、`space.3` …）のまま、`ui/src/design/tokens.ts` から使う。色、文字の大きさ、すきまを勝手に足さない。
- 画面の文面は `ui/src/text/ja.ts` の一か所。仕様書の文面は一字も変えない。モックにだけある文は、モックの文のまま（出どころつき）。ヤチヨさんが決めた文は出どころ `user:<日付>`。
- モックと仕様書が食いちがったら、仕様書が正しい。直した所は一覧にする。
- まだ中身のない部品は、消さずに「使えない」状態で出す。
- 曲の中の位置はティック（4 分音符 = 480）。録った鍵は「押した瞬間に耳に届いていた位置」に置く。クオンタイズは直す操作としてだけ（`docs/architecture.md` 11・12 章）。
- 曲を変える操作は、どれも「取り消せる手」1 つ（`meloet_model::History`）。テイクは一度できたら中身を変えない。ノートを直すと新しい id のテイクを同じ場所に入れ、テイクの数は増やさない。意図はテイクの id とその中の番号で指し、古い id の意図は何もしない。選択は core が持ち、取り消しで戻す（`docs/architecture.md` 16 章）。
- 画面がためるテイクは、いま使っている物と新しい 16 個だけ（`ui/src/bridge/take.ts` の `keepOnly`）。取り消しで戻れる物は Rust の取り消しの履歴が持つ。
- 曲のファイルは版つきの JSON（いまは版 4。版 1〜3 も読める。前の版の曲のセンドは −∞。プラグインの状態は base64。見つからないプラグインや SF2 の欄は読んだまま書きもどす）。こわれない書きかたと `.prev` を守る（14 章）。形を変えたら版の扱いを決める。
- 保存していない変更は、なおすためのファイルに書く（PC が終わるときと、変えてから 2 秒）。同じこわれない書きかたで（15 章）。
- WebGL の文脈は予算 12（WebKitGTK は 16 まで、捨てた canvas も GC まで数に入る）。区画ごとに 1 枚、使いまわし（7 章）。文脈は区画の箱が来たとき（起動、帯をひらいた時）に作る（`getContext` は 1 回 50〜200 ms 止まるので、人が弾いている瞬間に作らない）。60 Hz で変わる物は小さな区画に、毎コマのレイアウトはしない。区画はいま最大 10（本物のアプリで 8）。
- この PC の WebKitGTK（NVIDIA ＋ Ubuntu）は、ページを CPU で描く。1 コマでよごれた四角が 11 個以上か、囲む四角の 25 % 以上だと、囲む四角をまるごと描きなおす（`docs/architecture.md` 7 章「CPU で描く WebKit」）。60 Hz で変わる物を足すときは、上の帯なら `use:glSlot={{ kind: "cover" }}` で `masterMeter` の canvas の下に入れ、ほかでは同じコマで変わる物と同じ canvas の下に入れるか上下の端をそろえる。足したら `tools/measure/webkit-damage.sh` で確かめる。
- コメントは、意図が伝わりにくい所にだけ。変えた理由や履歴は書かない。コミットは小さく、こまめに。
- `spikes/` は消さない。そのまま本番に写さない。学んだことだけ連れてくる。
- sudo がいるときは、コマンドを見せてから頼む。別のユーザーで動いていて入口（ACL、xhost）をあけてもらったら、最後に閉じて、閉じたと伝える。

## 骨組み（ひとめで）

```
crates/
  meloet-model     曲・トラック・クリップ・テイク・ノート（純粋なデータ）、取り消しの手（History）、録った出来事をテイクにする決まり（Recorder）、ノートを直す（notes.rs）、クオンタイズ（quantize.rs）、CC の点（controls.rs）、クリップを動かす（arrange.rs）、音色とグループ（instrument.rs）、ドラムのキット（drums.rs）、ミキサーとエフェクトのパラメータ（mixer.rs、effects.rs）
  meloet-protocol  Rust と画面の約束（版 18）。TS の型は ui/src/bridge/generated/ に書き出す。プラグインの窓とパラメータの形は params.rs、あとから入れた物は added.rs
  meloet-engine    音の流れ、音源の口（Instrument）、トラックの席、ミキサーの音の道（mixer.rs）、メーター、cpal の出口、再生まわり（transport.rs）、耳に届いている位置（clock.rs）
  meloet-sound     音の部品: サンプルを鳴らす係（sampler/）、SF2 を読む所（sf2/）、エフェクト 6 つ（effects/）、計算で作る音色（synth/）、サンプルの ▶ のフレーズ（phrases.rs）
  meloet-plugin    プラグインの鍵とまとめる鍵、種類、スキャンの記録、共有メモリと futex、プラグイン係とのやりとり（wire.rs）、子を起こす（launch.rs）
  meloet-plughost  プラグイン係のプロセス（host と scan）。VST3 ・ LV2（formats/）、窓（window.rs、X11）、音のスレッド（rt.rs）、主スレッド（control.rs）
  meloet-midi      ALSA シーケンサ。鍵盤のぬきさし。meloet-vkeys（試験用の仮想の鍵盤。--name）
  meloet-devices   PipeWire の Route（人の言葉の名前）と settings（固定の quantum）を読む
  meloet-core      状態の持ち主。プラグインのスキャンとプラグイン係のプロセス（plugins/: scanner.rs、hosts.rs、housed.rs）、棚の「あとから入れたもの」（catalog.rs）、SF2 の写し（sf2store.rs）、プラグインの読みこみ（play/plugs.rs）、窓と 15-C（play/windows.rs）、プラグインのパラメータ（play/params.rs）、コマンド → 状態 → rev つきの差分。帯の決まり（layout.rs）、再生・録音・取り消し・ピアノロールの意図（play.rs）、曲のファイル（songfile.rs）、保存の流れ（state/songs.rs）、なおすためのファイル（recovery.rs、state/recovery.rs）、つなぎなおし（service.rs）、音の素材の包みと読みこみ（sounds.rs、instruments.rs）、エンジンの席（play/seats.rs）、棚（play/shelf.rs）、ミキサーの意図とエンジンへの送り（play/mixer.rs、play/mixsync.rs、mixbook.rs）
  meloet-app       Tauri の殻。web プロセスの見はり（webprocess.rs）、PC のファイルの窓（pcwindow.rs）、logind の delay ロック（logind.rs）、開発のときだけのしかけ（devhooks.rs）
ui/
  src/bridge/      Tauri を知る唯一の場所
  src/design/      数値（仕様書の名前のまま）
  src/text/        文面
  src/gl/          WebGL の係（区画、使いまわし、予算）
  src/components/pianoroll/  ピアノロールの帯（見ている範囲 view.svelte.ts、見せているテイク shown.svelte.ts、ドラッグ drag.ts、CC の帯 cc.ts）
  src/components/mixer/      ミキサーの帯（ストリップ、フェーダー、パン、センド、インサート、ドラッグ mixDrag.ts）
  src/components/params/     右のパラメータの欄（最初から入っているエフェクト BuiltinList、プラグイン PluginList。一覧は bridge/paramTable.ts）
  src/components/menu/       上のメニューと右クリックの一覧（menus.ts、contextMenu.svelte.ts）
  src/dev/         測るときだけ動く、ページの中の自動の係（MELOET_UI_SCRIPT）
docs/
  design/          仕様書（spec.html）とモック（mock/）の写し
  architecture.md  骨組みと理由
  progress.md      進みぐあい・宿題・申し送り
  design-notes.md  仕様書とモックの読みかた、デザインに戻した事
  measurements/    測った数
  screenshots/     本物のアプリの絵とモックのならべ
  research/        調べ物の書類（出どころと確かさつき）
tools/sounds/      音の素材の包みを作る（sources.json に URL と sha256、build.py、SF2 を読む sf2.py）
licenses/          ライセンスの見はり（check.mjs）と素材のライセンスの写し
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
tools/measure/latency.sh --pattern mixed --piano-roll      # 和音と速い打鍵、ピアノロールをひらいたまま
tools/measure/record-timing.sh private                      # 録った位置のずれ（自分用の PipeWire）
tools/measure/latency.sh --mixer --song <曲> --state playing  # ミキサーをひらいたまま（起動の前に仮想の鍵盤を出す）
tools/measure/long-record.sh [--piano-roll] [--mixer --song <曲>]  # 10 分録りつづける
tools/measure/ui-script.sh                                  # 本物のアプリで、送る・拡大・ドラッグを自動で（コマの時間とメモリ）
tools/measure/app-watch.sh                                  # 本物のアプリのコマ（busyMs）とメモリを見る
tools/measure/webkit-damage.sh                              # WebKit が描きなおした四角（gdb の下で）
python3 -I tools/measure/remote.py ...                      # 本物のアプリを外から 1 手ずつ動かす（MELOET_PIPE、X の作り物の出来事、出口を録る）
tools/measure/plugin-stop.sh / open-play-save.sh            # プラグインを止める試験、保存してひらく試験
python3 -I tools/measure/window-rounds.py ...               # プラグインの窓を何十回、15-C を何百回
python3 -I tools/measure/make_song.py ... [--tracks N] [--instruments plugin,sf2,builtin --plugin 名前 --sf2 ファイル --effect-plugin 名前]  # 試験用の曲
python3 -I tools/sounds/build.py --out target/sounds/dev    # 音の素材の包みを作る（素材は ~/.cache/meloet/sounds-src、出どころは sources.json）
ln -sfn ../sounds/dev target/debug/sounds                   # 開発のビルドが包みを見つける（release も同じ）
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
- オプションなしの `cargo fmt` は `spikes/` まで書きかえる。`-p` でクレートをならべる（`scripts/check-all.sh` と同じ）。`cargo clippy --workspace` も spikes で落ちる。
- 係の作業ツリーは `.claude/worktrees/` にできる（`.git/info/exclude` で外してある）。`meloet-app` の clippy は `ui/dist` が要るので、作業ツリーでは空の `ui/dist` を一時的に作る。
- PC が終わるとき、アプリの scope は SIGTERM から 5 秒で SIGKILL（Ubuntu 26.04、GNOME 50 は Wayland だけ）。電源オフの前には logind の PrepareForShutdown が来る（delay ロックを持てば）。
- WebKitGTK 2.52 の JavaScriptCore は、ひまな時に Eden の GC しか回さず、Full の GC がめったに来ない。60 Hz で動くページでは、WebKitWebProcess のメモリがふえつづける。meloet は起動のいちばん初めに `JSC_useGenerationalGC=0` を決め、毎回 Full にしている（`crates/meloet-app/src/main.rs`、`docs/measurements/2026-11-15-round3-fix-memory.md` の g）。外から同じ変数を決めればそちらが勝つ。GC のようすは `JSC_logGC=1` で見られる（ログを書く重さで、止まりが少し長く出る）。
- この PC の WebKitGTK は、合成の層なしでページを CPU で描く（`WEBKIT_FORCE_COMPOSITING_MODE=1` などの変数でも変わらない）。WebKit の debug 情報は `~/.cache/debuginfod_client/` にある（`webkit-damage.sh` が使う）。
- 音の出口えらびは「出口をまだ決めていない ＋ 鍵盤が 1 つ以上」のときだけ出る。測る道具は、起動の前に仮想の鍵盤（`meloet-vkeys appear`）を出しておく。
- debug のビルドでも symphonia と `meloet-sound` は opt-level 3（`Cargo.toml`）。外すとグランドピアノの読みこみが 8 秒かかる。
- `env.sh` には `DISPLAY` がない。X の要る物（プラグインの窓の試験、`--ignored` の本物の試験）は `DISPLAY=:0` を足す（XWayland、窓はヤチヨさんの画面に出る）。
- この PC には Surge XT（VST3 ・ LV2）、LSP、ZamComp、Dragonfly、Calf などのプラグインがある（`/usr/lib/vst3`、`/usr/lib/lv2`）。X の画面がある所で、はじめてのスキャンは約 40 秒（LSP の VST3 が窓の型を聞かれるたびに X につなぐ）、画面がなければ 11〜19 秒。
- VS Code の「meloet（デバッグ）」は CodeLLDB。子のプロセスを何人も同時に起こすと、lldb-server が meloet を止めたまま回りつづけた（窓が出ない）。子は 1 つずつ起こし、スキャンは画面のあいさつのあとに始める形で直した（2026-11-19）。どちらかを外すと、また起きる。止まった meloet は `ps` の STAT `t` と `/proc/<pid>/status` の TracerPid で見える。
