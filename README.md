# meloet

MIDI 鍵盤を買ったばかりの人のための DAW です。いまは第 1 回（土台と「つないで、光って、鳴る」まで）ができています。
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
- 覚えた設定は `~/.config/meloet/settings.json`（`$XDG_CONFIG_HOME` があればその下）。
- キー: A アレンジへ帰る、E ピアノロール、M ミキサー、H 取り消しの履歴、P 棚、U パラメータ、Esc ひとつ戻る、Ctrl+, 音の設定、Ctrl+Alt+1・2・3 拡大率 100・125・150%。

画面だけをブラウザで見るとき（Tauri なし、偽物の状態）:

```bash
(cd ui && npm run dev)            # http://localhost:1420/?scene=connect2&scale=100
```

場面の名前は `ui/src/bridge/fake.ts` にあります。

## 確かめる

```bash
scripts/check-all.sh             # 書きかたの検査、clippy、Rust のテスト、型の検査、画面のテスト、ライセンス
scripts/check-all.sh --devices   # 上に加えて、鍵盤・PipeWire・音の出口が要るテスト
python3 -I tools/design/render_mocks.py target/mock-png          # モック（docs/design/mock/）を PNG に
python3 -I tools/design/shoot_ui.py target/compare connect2      # 画面（偽物の状態）とモックを左右にならべる
tools/measure/latency.sh --window 1920x1080 --count 120 --out target/measure/run   # 鍵 → 画面、鍵 → 音を測る
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

## 検証の試作

`spikes/` には、本番の前に土台の賭けを確かめた試作があります。報告は [`docs/report.md`](docs/report.md)。消さずに残していますが、本番のコードには写していません。
