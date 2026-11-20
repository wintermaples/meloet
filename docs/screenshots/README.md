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

## 第 6 回の状態（2026-11-21）

- 版: `5022d22`（release ビルド。製品のコードは `2b80d4a` と同じで、あとのコミットは書類だけ）の上に、作業ツリーの `tools/design/shoot_app.py` と `tools/measure/make_song.py` の変更をのせたもの。
- 撮りかた: `python3 -I tools/design/shoot_app.py --compare-small tempoApp markersApp autoRestApp autoRecApp autoEditApp mixerAutoApp mixerRestApp paramManyAutoApp historyStepsApp`（`source ~/.config/meloet-agent/env.sh`、`PIPEWIRE_RUNTIME_DIR=/run/user/1000`、`CARGO_TARGET_DIR` は決めない）。
  - 曲は `make_song.py` で 5 トラック（4 本は最初から入っている音色、5 本目は Surge XT。paramManyAutoApp は 1 本目が Surge XT、historyStepsApp は 5 本とも最初から入っている音色）。帯を 1 本目と 4 本目だけにするため、`make_song.py` に `--lane-tracks` を足した。
  - 帯の id や、いまのテイクの id のように、状態だけが知る番号が要る意図は、`MELOET_EVAL` の JavaScript で状態を読んでから、画面と同じ道（`send`）で送った（`shoot_app.py` の `core_js`）。
  - 時刻はページが読めてからの ms。
- `app/` に 9 状態 × 4 = 36 枚、`compare/` に 9 状態 × 100 % の 2 枚 = 18 枚（合計 約 9.2 MB）。
- モックは第 9 版の写し（`docs/design/mock/`）とくらべた。第 10 版で F6-AutoRec ・ F6-AutoRest ・ F6-MixerAuto ・ F6-MixerRest ・ F6-ParamMany ・ F6-Tempo が変わったが、まだ写していない。

| 状態 | どうしたか | モック |
|---|---|---|
| tempoApp | テンポの点 96 → 5 小節目 104（だんだん）→ 9 小節目 120 → 13 小節目 96、9 小節目から 3/4、13 小節目から 4/4、マーカー 3 つ。1.5 秒で 7 小節目へ `locate`、`setTimelineOpen`、5 小節目のテンポの点を `selectTimeline`。3.5 秒で撮る | F6-Tempo |
| markersApp | マーカー イントロ ・ Aメロ ・ サビ（10 小節目）。7 小節目へ `locate`、1.8 秒で `addMarker`「マーカー 4」（K と同じ名前）。名前の枠がひらいた所 | R10-Markers |
| autoRestApp | 1 トラック目に音量の帯（点 6）。5 小節目から `playStop`、5.0 ・ 5.2 秒に `setGain`（−10 を commit: false、−12 を commit: true）で手で動かし、帯が休む。9.4 秒（8 小節目）で撮る。モックも再生中 | F6-AutoRest |
| autoRecApp | 同じ曲で `setLaneWrite`（● 書く）をオンにして 5 小節目から再生、6.4 秒から 250 ms ごとに `setGain`（commit: false）で −4.8 → −12 dB へ動かしつづけ、9.4 秒で撮る（離していない） | F6-AutoRec |
| autoEditApp | 同じ曲を止めたまま、8 小節目へ `locate`、`selectAutoPoints` で点 1〜3 をえらぶ | R10-AutoEdit |
| mixerAutoApp | 1 ・ 4 トラック目に音量の帯（点 8）。7 小節目から `playStop`、そのあとミキサーをひらく（ひらいた時に「… の音量は、オートメーションで動いています」が出る）。8 秒で撮る | F6-MixerAuto |
| mixerRestApp | 同じに再生してミキサーをひらき、4.5 ・ 4.7 秒に 4 トラック目の `setGain`（−12、離す）。4 トラック目の帯が休む | F6-MixerRest |
| paramManyAutoApp | 1 トラック目の Surge XT に、音量 ・ A Filter 1 Cutoff ・ A Osc 1 Pitch の帯。`selectInstrument`、パラメータの欄をひらき、Macros をひらく | F6-ParamMany |
| historyStepsApp | 13 手（名前を変える 4、音量 4、パン、テンポの点を足す、マーカー「サビ」を足す、EQ をとばす、4 パッドの音量 −10.5 → −12.0）、ピアノロールで 3 手（クオンタイズ、長さ、動かす）、`undo` 3 回、帯をひらいて ↓ 2 回で 11 手目をえらぶ | R10-History（帯の形は F-History） |

### 目で見て気づいたモックとのちがい（第 6 回）

- 全部: 曲は `make_song.py` の物で、トラックごとにクリップが 1 つ、曲の頭から 16 小節（モックはクリップが区切られ、名前もイントロ ・ ベース A など）。トラックの名前は音色の名前（モックは「ピアノ」「ベース」…）。曲の名前は「測る曲 5 トラック 40 ノート」。帯の点の値は `--seed` からの物で、モックの線の形とはちがう。
- 全部: 入力の行は本物の Keystation 49 MK3 と仮想の鍵盤が 1 台ずつ。マスターのメーターや処理の負荷の数は、そのときの本物の値。
- tempoApp: モックとほとんど同じ（テンポの線、だんだん、3/4 ・ 4/4 の旗、状態行の文）。モックの状態行は「… だんだん速く ・ 値を変えるには 2 回押す」と続くが、アプリは「だんだん速く」で終わる。
- markersApp: R10-Markers は部品の説明の板なので、窓全体ではくらべられない。名前の枠（白い枠に「マーカー 4」）と状態行「マーカー 4 を足しました。名前を打って Enter」は板の「足した直後」と同じ形。
- autoRestApp ・ mixerRestApp: 休んだ帯は灰色の線、見出しに「もどす」「休み ・ −12.0 dB」、ミキサーのフェーダーの下に「休み」「もどす」。状態行の文はモックと同じ。
- autoRecApp: 赤い枠（書いている所）と「● 書く」「書いています」、状態行はモックと同じ。モックは枠の中の線が手の値へ下がっているが、アプリは書いている間、枠の中の線がもとの線のままで、トラックの見出しの音量も −9.0 dB のまま（`setGain` の commit: false で動かしたため。マウスでフェーダーを持ったときに同じかは見ていない）。
- 再生中の絵（autoRestApp、autoRecApp の 150 %、mixerRestApp ・ mixerAutoApp の上の「アレンジ全体」）で、再生の線が白っぽい灰色に見える物と、accent（橙）の物がある（autoRecApp の 100 % は橙）。モックの再生中の線は橙。撮ったコマのせいか、再生中の描きかたかは見ていない。
- mixerAutoApp ・ mixerRestApp: 帯の値が乱数なので、1 トラック目のフェーダーは +4 dB 前後（モックは −2.4）。1920×1080 では、ミキサーの上にアレンジが出る（窓が高いため。モックは 1280×800 だけ）。
- paramManyAutoApp: Surge XT のマクロの名前と値は Surge XT が言うまま（「M1: -」「0.000000」。モックは「Macro 1」「35.00 %」）。アプリはプラグインの帯 2 本もアレンジに見出しつきで出る（「オートメーション ・ Surge XT の…」は 1280×800 で切れる）。モックはアレンジに音量の帯だけで、欄には「オートメーションに書いている 2」が出ている。窓のボタンは「窓をひらく」（モックは窓がひらいていて「窓を前に出す」）。モックの 5 トラック目の歌のトラックはない。
- historyStepsApp: 行の形（取り消した 3 手が「やりなおせます」、13 手目が「いまここ」、11 手目に白い枠と「ここまで戻す Enter」）は R10-History と同じ。10 手目は「テンポ ・ 拍子の変更を足す」（モックは「テンポを変える」。ここではテンポの点を足したため）、12 手目は EQ 4 バンド（モックは MVerb）。ピアノロールで手を作るためにえらんだクリップの枠が、アレンジで accent になっている。
- 150 %: 1280×800 では、左上の曲の名前、処理の負荷、出口と入口の名前が引っこむ（前の回と同じ決まり）。
