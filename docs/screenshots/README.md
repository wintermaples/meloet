# 本物のアプリの画面

- 撮った日: 2026-11-15
- 版: `64da326`（release ビルド）
- 撮りかた: `python3 -I tools/design/shoot_app.py`（`source ~/.config/meloet-agent/env.sh`、`PIPEWIRE_RUNTIME_DIR=/run/user/1000`）
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

- 全部: モックは曲の中身がある（トラック 5 本、テンポの変更 2 件、グランドピアノ）。アプリは 1 トラック「仮の音」、0 件。
- 全部: 再生・停止・録音・ループ・メトロノーム・カウントインがアプリでは使えない状態。モックは押せる色（録音は赤）。
- 全部: 状態行の入口が、モックは「鍵盤 49 ・ マイク」、アプリは鍵盤の名前だけ。
- 全部: ミキサーの帯見出しが、モックは「マスター −∞ dB」、アプリは「マスター 0.0 dB」。
- flow1: アプリの状態行に「バッファ」がない（モックは「バッファ 256」）。
- flow2・flow3: 鍵盤の名前がアプリでは「meloet-vkeys」。本物の Keystation もささっているが、押した仮想の鍵盤の名前が出る。
- flow2: 出口のえらびが、アプリは 9 行（HDMI / DisplayPort が同じ名前で 2 回ずつ、何もささっていない出口も全部）。モックは 3 行。
- flow3: ヒントの「録音 R ・ 再生と停止 Space」がアプリにない。状態行の「出口をヘッドホンにしました…」もアプリにない。
- flow3: 出口の名前が、モックは「ヘッドホン」、アプリは「アナログヘッドフォン」。
- empty: 「ここに、録った音がならびます…」のヒントがアプリに出ていない。
- settings: 出口の一覧が長く（9 行）、遅れの欄と「くわしい設定」の行が見えない所までスクロールしている。見出し「音の出口と入口」の字がモックより大きい。
- settingsAdv: 歌の入口が「使わない」（使えない色）。遅れの補正の 3 つのボタンが使えない色。バッファ固定の囲みは出ない（この PC は 256 で動いていて固定されていないため。モックは 1024 固定の場面）。
- pianoRoll: 帯の中身は空。見出しの位置（アレンジ全体の下）はモックと同じ。
- mixer: モックではミキサーの帯がアレンジの場所に入り、上は「アレンジ全体」の細い行だけ。アプリはアレンジのトラック欄が残り、ミキサーの帯はその下（1280×800 で y≈335 から）。
- history: 帯の見出しの高さ（1280×800 で y≈515）はモックと同じ。中身は空。
- shelf・params: 欄の幅はモックとほぼ同じ。中身（音源 9・エフェクト 12 のタブ、一覧）は空。
- 150 %（1280×800）: 上の段に曲の名前が出ない（仕様書 1 章の引っこめる順で、いちばん先に引っこむ）。
- flow2・150 %（1280×800）: 出口えらびが窓に入りきらず、下の行と、見出しの右の「あとから「音の設定」で変えられます」の終わりが切れる（`design-notes.md` B-24）。
- 150 %: 文字とすきまは 1.5 倍になり、グリッドの線（line.hair）は 1px のまま。
