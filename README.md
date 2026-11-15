# meloet

MIDI 鍵盤を買ったばかりの人のための DAW です。いまは第 3 回（ピアノロール）までができています。
どこまでできたかは [`docs/progress.md`](docs/progress.md)、作りは [`docs/architecture.md`](docs/architecture.md) を見てください。

ライセンス: MIT OR Apache-2.0。部品のライセンスは `licenses/check.mjs` が見はっています。

## 用意（Ubuntu Studio 26.04 で確かめた）

```bash
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev libudev-dev
# Rust（rustup、1.100）と Node.js（26）を PATH に
cargo install cargo-deny          # ライセンスの見はりに使う
(cd ui && npm ci)
```

## 動かす

```bash
(cd ui && npm run build)          # 画面を作る。meloet-app はビルドのときに ui/dist を取りこむ
cargo run --release -p meloet-app # meloet が立ちあがる
```

- 鍵盤（USB MIDI）をさすと、入力計に名前が出ます。立ちあげる前からささっていても出ます。
- 鍵盤が見つかると「音をどこから鳴らしますか」が出ます。↑ ↓ でえらび、Enter で決まります。えらんだ出口は覚えていて、次からは聞きません。
- 覚えた設定は `~/.config/meloet/settings.json`（`$XDG_CONFIG_HOME` があればその下）。出口、拡大率、メトロノーム、カウントインの小節数。
- 録る: トラックの ● が赤（録音待機）なら、R か ● で、カウントインのあと録りはじめます。Space で止めると、テイクとしてアレンジに残ります。Home で先頭へ、Space で聞きかえせます。
- ループ: 目盛（小節の数字の行）をドラッグすると範囲が決まり、L でオン・オフ。ループ中に録ると、周ごとにテイクが分かれます。クリップをえらんで T・Shift T でテイクを切りかえます。
- 保存: Ctrl+S。最初の保存は `~/ミュージック/はじめての曲/はじめての曲.meloet`（OS の「音楽」の場所）。左上の曲の名前を押すと、新しい曲・ひらく・保存の一覧。
- ピアノロール: クリップをえらんで E（か帯札）でひらく。道具は 1 選ぶ・2 書く・3 消す。ノートを押したまま動かす、右はしで長さ、空いている所から囲んでえらぶ（Shift で足す）、Alt で複製、Esc でドラッグの前に戻る。↑ ↓ 半音（Shift で 1 オクターブ）、← → グリッド 1 つ、Ctrl+A・Ctrl+D・Ctrl+X・Ctrl+C・Ctrl+V。ドラッグ 1 回が取り消しの 1 手で、テイクの数は増えない。
- 直す道具: Q でクオンタイズ（うすい枠が、そろえる前の位置。右の欄の「もとの位置へ戻す」で戻る）。下の札でサステイン・ピッチベンド・モジュレーション・そのほかの CC の帯をひらき、点を足す・動かす・消す。ノートの一覧と右の欄は、2 回押すか Enter で打ちこめる。
- アレンジのクリップ: 押したまま動かす（ほかのトラックへも）、Alt で複製、Ctrl+D ですぐうしろに複製、左右のはしで長さをつめる・のばす（つめた所の中身は消えない）。Ctrl＋ ・ Ctrl− か Ctrl ＋ ホイールで横の拡大・縮小。
- なおすためのファイル: 保存していない変更は、曲を変えて 2 秒たったときと PC が終わるときに `~/.local/state/meloet/recovery.json`（`$XDG_STATE_HOME` があればその下）に書かれ、次に起動すると戻した曲でひらく。保存すると消える。
- キー: Space 再生・停止、R 録音、Home 先頭へ、L ループ、C メトロノーム、T・Shift T テイク、Delete 消す、Ctrl+Z・Ctrl+Shift+Z 取り消す・やりなおす、Ctrl+S・Ctrl+O・Ctrl+N 保存・ひらく・新しい曲、A アレンジへ帰る、E ピアノロール、M ミキサー、H 取り消しの履歴、P 棚、U パラメータ、Esc ひとつ戻る、Ctrl+, 音の設定、Ctrl+Alt+1・2・3 拡大率 100・125・150%。

画面だけをブラウザで見るとき（Tauri なし、偽物の状態）:

```bash
(cd ui && npm run dev)            # http://localhost:1420/?scene=connect2&scale=100
```

場面の名前は `ui/src/bridge/fake.ts` にあります。

VS Code では（`.vscode/`、拡張機能 CodeLLDB が要る）:

- 実行とデバッグの「meloet（デバッグ）」: vite を立ちあげてから、Rust をデバッグ用にビルドして動かす。画面は ui/dist ではなく vite から読むので、`ui/` を書きかえるとすぐ窓に出る。
- 「画面だけ（ブラウザ、偽物の場面）」: 場面と拡大率をえらんで、Chrome で開く。
- タスク: 「meloet: release で動かす」「検査: 全部」「約束の型を書き出す（Rust → TS）」など。

## 確かめる

```bash
scripts/check-all.sh             # 書きかたの検査、clippy、Rust のテスト、型の検査、画面のテスト、ライセンス
scripts/check-all.sh --devices   # 上に加えて、鍵盤・PipeWire・音の出口が要るテスト
python3 -I tools/design/render_mocks.py target/mock-png          # モック（docs/design/mock/）を PNG に
python3 -I tools/design/shoot_ui.py target/compare connect2      # 画面（偽物の状態）とモックを左右にならべる
tools/measure/latency.sh --window 1920x1080 --count 120 --out target/measure/run   # 鍵 → 画面、鍵 → 音を測る（--state playing|recording も）
tools/measure/real-keys.sh stopped      # 本物の鍵盤で鍵 → 画面、鍵 → 音（弾いて窓を閉じる）
tools/measure/record-timing.sh private  # 録った位置のずれ（自分用の PipeWire、quantum 64〜1024。約 12 分）
tools/measure/long-record.sh            # 10 分録りつづける（xrun、メモリ）
```

## 開発用の環境変数（なければ何もしない）

| 変数 | すること |
|---|---|
| `MELOET_WINDOW=1280x800` | 窓の内側の大きさ |
| `MELOET_SNAPSHOT=out.png`、`MELOET_SNAPSHOT_AFTER_MS=1500` | 画面を PNG に書いて終わる |
| `MELOET_COMMANDS='[{"afterMs":800,"command":{"type":"chooserConfirm"}}]'` | 決まった時間に意図を送る |
| `MELOET_EXIT_AFTER_MS=10000` | その時間のあとに終わる |
| `MELOET_CONFIG_DIR=dir` | 設定ファイルの場所を変える |
| `MELOET_MEASURE_DIR=dir` | 鍵 → 音、鍵 → 画面の記録を書く |
| `MELOET_ACCEPT_VIRTUAL=1` | 試験用の仮想の鍵盤（`meloet-vkeys`）も鍵盤とみなす |
| `MELOET_ONLY_VIRTUAL=1` | 本物の鍵盤を見ず、仮想の鍵盤だけを見る |
| `MELOET_VKEYS_NAME=名前[,名前…]` | その名前（`meloet-vkeys --name`）の仮想の鍵盤だけを受ける |
| `MELOET_SONGS_DIR=dir` | 曲を置く「音楽」の場所を変える |
| `MELOET_CLOSE_AFTER_MS=10000` | その時間のあとに、閉じるボタンと同じ道で窓を閉じる |
| `MELOET_PCWINDOW_SHOT=out.png` | PC のファイルの窓が出て 1.5 秒後に、その窓を PNG に描く |
| `MELOET_MIDI_FAIL_AFTER=N` | 試験用: MIDI のスレッドを N 個の鍵のあとで止める |
| `MELOET_STATE_DIR=dir` | なおすためのファイルの場所を変える |
| `MELOET_OPEN_SONG=file.meloet` | 立ちあげたら、その曲をひらく（PC のファイルの窓のかわり） |
| `MELOET_UI_SCRIPT=pr-scroll\|pr-zoom\|pr-drag:N` | `MELOET_MEASURE_DIR` があるときだけ、ページの中で送る・拡大・ドラッグを自動でくりかえして測る |
| `JSC_useGenerationalGC` | meloet は立ちあがるときに 0 にする（ウェブのプロセスのメモリがふえつづけないように）。外から決めればそちらが勝つ |

## 検証の試作

`spikes/` には、本番の前に土台の賭けを確かめた試作があります。報告は [`docs/report.md`](docs/report.md)。消さずに残していますが、本番のコードには写していません。
