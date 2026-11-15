# 本物のアプリの画面

- 撮った日: 2026-11-15
- 版: `ab00a06`（release ビルド。2026-11-15 のデザインの改訂を入れた後）
- 撮りかた: `python3 -I tools/design/shoot_app.py`（`source ~/.config/meloet-agent/env.sh`、`PIPEWIRE_RUNTIME_DIR=/run/user/1000`、`CARGO_TARGET_DIR=target/agent`）
  - 1 回の起動ごとに新しい設定ディレクトリ（`MELOET_CONFIG_DIR`）。
  - 「出口を覚えた状態」は、最初に 1 回 `chooserConfirm` で決めさせた settings.json を写して使う。
  - 拡大率は `setScale` のコマンド。窓は `MELOET_WINDOW`。PNG は `MELOET_SNAPSHOT`。
  - 鍵盤: Keystation 49 MK3（本物）と `meloet-vkeys`（仮想。ベロシティ 25）。和音は vkeys を音ごとに 1 つずつ同時に動かして押したままにする（`play` は次の音の前に前の音を離すため）。
- `app/<状態>-<幅>x<高さ>-<率>.png`: アプリの絵だけ（13 状態 × 4 = 52 枚）。
- `compare/<状態>-<幅>x<高さ>-<率>.png`: 左にアプリ、右にモック（100 % のモックを左上から切り出し。ならべかたは `shoot_ui.py` と同じ）。くらべるモックのない plugIn・plugOut はなし（11 状態 × 4 = 44 枚）。
- 合計 約 15 MB なので、compare は全部の大きさと率で作った。

## 状態

| 状態 | どうしたか | モック |
|---|---|---|
| flow1 | `MELOET_ONLY_VIRTUAL=1`、vkeys なし、出口なし | F-Connect1 |
| flow2 | `MELOET_ACCEPT_VIRTUAL=1`、出口なし、vkeys で C4・E4・G4 を押したまま | F-Connect2 |
| flow3 | flow2 と同じ始まりで、1.5 秒で `chooserConfirm`、そのあと和音を押したまま | F-Connect3 |
| empty | 出口を覚えた状態、何も押していない | F-Empty |
| settings | 出口を覚えた状態で `openSettings`（advanced: false） | F-Settings |
| settingsAdv | `openSettings`（advanced: true） | F-SettingsAdvFixed |
| pianoRoll・mixer・history | `toggleBand` でそれぞれの帯をひらく | A-PianoRoll・A-Mixer・F-History |
| shelf・params | `toggleBand`（shelf・params）で P と U の欄をひらく | F-ShelfArrange・F-ParamFew |
| plugIn | 出口を覚えた状態、`MELOET_ONLY_VIRTUAL=1`、`vkeys appear` が出ている間 | なし |
| plugOut | 同じで、`appear` が消えたあと | なし |

## 目で見て気づいたモックとのちがい

- 全部: モックは曲の中身がある（トラック 5 本、グランドピアノ）。アプリは 1 トラック「仮の音」。
- 全部: 再生・停止・録音・ループ・メトロノーム・カウントインがアプリでは使えない状態。モックは押せる色（録音は赤、トラックの ● は録音待機のオン）。
- flow2・flow3: 入力計の名前が「meloet-vkeys +3」。撮る道具が和音のために仮想の鍵盤を音ごとに 1 台ずつ出すので、本物の Keystation と合わせて 4 台になるため（最後に弾いた鍵盤の名前に、残りの台数をそえる決まりのとおり）。
- flow2・settings: この日は GB206 の「HDMI / DisplayPort 2」に音の行き先があり、えらべる出口に入っている。モックは「USB オーディオ」（ささっているかわからない出口の例）。えらべない出口は 7（モックは 8）。
- settings: えらべない出口の装置の名前は本物の名前（ヤチヨさんの決め。モックは「内蔵オーディオ」「グラフィック 1」）。モックはまとめをひらいた絵、アプリはたたんで始まる。
- settings: 歌の入口は 4 行とも「使えない」の見た目（歌の回まで）。モックはリアマイクロフォンをえらんだ形。
- flow3: 状態行の「出口をアナログヘッドフォンにしました。1 ピアノ が録音待機になっています。」はアプリにない（今回は本当でない文は出さない）。「音が出ました。」も 1 文だけ。
- empty: 「ここに、録った音がならびます…」のヒントがアプリに出ていない（同じ理由）。
- settingsAdv: 歌の入口が「使わない」（使えない色）。遅れの補正の 3 つのボタンが使えない色。バッファ固定の囲みは出ない（この PC は 256 で動いていて固定されていないため。モックは 1024 固定の場面）。
- pianoRoll・history・shelf・params: 帯と欄の中身は空。
- 150 %: 太い入力計は、モジュレーション、ピッチベンド、細い鍵、「弾くと鳴る先」のラベル、サステインの順に引っこむ。1280×800 では、「弾くと鳴る先」の値に音源の名前（「仮の音」）があるぶん、モックより先にサステインまで引っこむ（`design-notes.md` B-28）。
- 150 %: 文字とすきまは 1.5 倍になり、グリッドの線（line.hair）は 1px のまま。

## 第 2 回の状態（2026-11-15 に撮りなおし・撮り足し）

- 版: `0aa65bc` の上に、作業ツリーの `tools/design/shoot_app.py` の変更をのせたもの（release ビルド、`CARGO_TARGET_DIR=target/hard-h4`）。撮りかたは上と同じで、`python3 -I tools/design/shoot_app.py --no-build flow3 flow4 empty recording main takes`。
- 仮想の鍵盤は、1 回ごとに `meloet-vkeys-shoot-<pid>-<番号>` という名前で出し、アプリには `MELOET_VKEYS_NAME` でその名前だけを受けさせた（ほかの係の仮想の鍵盤がまざらないように）。
- 撮りなおし: flow3、empty（1280×800・1920×1080 × 100・150 %、各 4 枚）。撮り足し: flow4、recording、main、takes（各 4 枚）。

| 状態 | どうしたか | モック |
|---|---|---|
| flow3 | 上と同じ（出口を決めた直後、和音を押したまま） | F-Connect3 |
| flow4 | 出口を覚えた状態で、`toggleMetronome` のあと `record`。カウントインの途中（ページが読めてから 1.8 秒）で撮る | F-Connect4 |
| empty | 上と同じ（からっぽのアレンジ） | F-Empty |
| recording | `MELOET_ONLY_VIRTUAL=1`。`record`、カウントインのあと vkeys で C4・E4・G4・C5 を 250 ms ごとに弾いている最中に撮る | F-ArrangeRec |
| main | recording と同じに 10 音録って、`record` で止めたあと（録ったクリップがえらばれている） | Main |
| takes | 曲の頭から 8 音録って止め、`goToStart`、もう 1 回 8 音録って止めたあと（テイク 2/2 がえらばれている） | F-Takes |

### 目で見て気づいたモックとのちがい（第 2 回）

- flow3: 状態行に「出口をアナログヘッドフォンにしました。1 ピアノ が録音待機になっています。」と「音が出ました。…カウントインのあとに録りはじめます。」が出て、モックと同じになった（第 1 回のちがいは消えた）。
- empty: 「ここに、録った音がならびます…」のヒントが出て、モックと同じになった。
- flow4: カウントインの拍の四角（3 / 4）と状態行の「あと 2 拍で録音が始まります。止めるには Space」はモックと同じ。モックでは「カウントイン」の説明のふきだしが出ている（マウスをのせた絵）が、アプリは出していない。1280×800 では、左上の曲の名前（「はじめての曲 / 新しい曲」）がアプリに出ていない（モックは出ている）。
- recording: 「録音中 テイク 1」の赤いクリップが、録った長さだけ伸びていて、状態行は「1 ピアノ へ録音 ・ 止めると テイク 1 として残ります」。モックは曲の中身があり、ループ 5–9 の 1 周目の場面なので、状態行の文はモックとちがう。
- main・takes: 撮るときに vkeys が先に終わるので、入力計と状態行に「meloet-vkeys-shoot-… とのつながりが切れました。…」が出ている。撮る道具のせいで、アプリの不具合ではない。モックの Main の状態行（えらんだクリップのこと）とはくらべられない。
- takes: クリップの左上に「2/2」の札、上に「◂ テイク 2 / 2 ▸ T」の帯が出る。モックの F-Takes は部品の説明の板（いろいろな幅のクリップを横にならべた物）なので、窓全体ではくらべられない。
- recording・main・takes: 1 トラックで、曲の中身がない（モックはトラック 5 本）。
