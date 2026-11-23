# Glossary

Japanese words that the spec and the screen use often, and the English word to use for each when the spec is written in English. Where the code already has a name, the English follows the code. Katakana loanwords that are already English (ループ, グリッド, ベロシティ, …) are left out.

This table is for the words that explain, and for the English wording of the screen. Text shown on the screen is Japanese first (`ui/src/text/ja.ts`, the source); its English (`ui/src/text/en.ts`, round 12 D4) follows it and uses the words of this table. The names of the values (`color.gray.4`, `space.3`, …) are not translated.

Code names: `ui/src/components/…` are files under that folder; type names come from `crates/meloet-protocol` (and `ui/src/bridge/generated/`) or `crates/meloet-model`.

## The screen and its parts

| 日本語 | English | Code name | Note |
|---|---|---|---|
| 帯 | band | `Band`, `LowerBandView` | A strip of the screen that opens and folds. |
| 下の帯 | lower band | `LowerBand.svelte`, `LowerBandView` | Piano roll, mixer, undo history. |
| 帯の中の帯 | inner band | | A band inside a band (velocity, CC). Folds to one heading line. |
| 帯札 | band tab | `BandRow.svelte` | The folded heading of a lower band. |
| 帯札の行 | band tab row | `BandRow.svelte` | The row of band tabs, just above the status line. |
| 上のメニューの行 | menu bar | `menu/MenuBar.svelte` | `size.menubar`. |
| 右クリックの一覧 | context menu | `menu/ContextMenu.svelte` | |
| 再生まわり | transport | `Transport.svelte`, `TransportView` | The top band with play, record, position, tempo. |
| 入力計 | input meter | `InputMeter.svelte`, `KeyboardsView` | Shows the keyboard, held notes, pedals. |
| 細い（入力計） | slim | `MeterSize`, `size.meter.slim` | Opposite: 太い = full (`size.meter.full`). |
| アレンジ | arrange | `Arrange.svelte`, `ArrangeView` | |
| 全体図 | overview | `overview` (Arrange) | The small picture of the whole song when the arrange is low. |
| ピアノロール | piano roll | `pianoroll/PianoRoll.svelte`, `PianoRollView` | |
| ミキサー | mixer | `mixer/`, `Band::Mixer` | |
| 取り消しの履歴 | undo history | `history/HistoryList.svelte`, `HistoryView` | |
| 状態行 | status line | `StatusLine.svelte`, `StatusView` | The bottom line of the window. |
| 棚 | shelf | `Shelf.svelte`, `ShelfView` | The right side: instruments and effects to choose. |
| パラメータの欄 | params panel | `params/ParamsPanel.svelte`, `ParamsView` | |
| 欄 | panel | `NoteListPanel.svelte`, `ParamsPanel.svelte` | Left and right panels; fold to a 24-wide heading. |
| 右はし | right rail | `RightRail.svelte` | The right side of the window (shelf, params). |
| 右はし（状態行 ・ 計 ・ 帯札の行のはし） | right end | | The right end of a line or a meter. Not the right rail. |
| 見出し | heading | `heading`, `TrackHeads.svelte` | For a track, "track head". |
| トラックの見出し | track head | `TrackHeads.svelte` | |
| 一覧 | list | `menu/MenuList.svelte`, `HistoryList.svelte` | |
| 道具 | tool | `pianoroll/Toolbar.svelte` | 道具の列 = toolbar. |
| 目盛 | scale marks | `meterScale`, `mixer/scale.ts`, `pianoroll/rulerMarks.ts` | For meters and faders. Not the musical scale (スケール). |
| 窓 | window | `PluginWindowView`, `WindowsView` | A plugin window or the meloet window. |
| 窓の印 | window mark | `WindowMark.svelte` | |
| 音の設定 | sound settings | `SettingsDialog.svelte`, `SettingsDialogView` | |
| くわしい設定 | advanced settings | | Inside the sound settings. |
| 用語の説明 | term tip | `lib/Tip.svelte`, `Term` (ja.ts) | Shows "用語 — 説明" on hover. |
| 案内 | hint | `Hint`, `SoundHint.svelte`, `RecoveryHint.svelte` | |
| 文面 | wording | `ja.ts` | The exact text shown on the screen. |
| 場面 | scene | `scene` (ja.ts) | The first column of the chapter 6 table. |
| 出る場所 | where shown | | Column of the chapter 6 table. |
| スクロールの棒 | scroll bar | `lib/HScrollbar.svelte`, `mixer/VScrollbar.svelte` | |
| 目盛の行 | ruler row | `pianoroll/rulerMarks.ts` | The row of bar numbers at the top of the piano roll and arrange. |
| 名札・札 | tag | | A marker tag, the take tag, the switch tag. |
| 2 段目 | second page | | The second page of a dialog (`ダイアログの 2 段目`). 1 段目 = first page. |
| ノートの場所 | note area | | |
| パラメータの画面 | params view | | |
| パラメータの一覧 | parameter list | | |
| 見つからない物の知らせ | the notice of missing things | | Chapter 2 part "Notice of missing things". |

## Parts and states

| 日本語 | English | Code name | Note |
|---|---|---|---|
| 部品 | part | `part.*` (tokens.ts) | |
| 状態 | state | | |
| 見本 | sample | `samples.css` | The small drawn parts in chapter 2. |
| ボタン | button | `Button.svelte` | |
| 切りかえ | toggle | | M, S, ● buttons. |
| 数字の欄 | number field | `NumberField.svelte`, `ValueField` | |
| 入力の枠 | input box | `menu/textField.ts` | Where text is typed. |
| つまみ | knob / handle | `knob`, `PanKnob.svelte` | A round knob (pan), or the handle of a fader or scroll bar. |
| 溝 | groove | | The line a fader handle moves on. |
| 地 | background | `bg` | The fill under text or a part. |
| 枠 | border | | |
| 面 | face | | The fill of a fader handle or a button. |
| 線 | line | `line.*` | |
| 角 | corner | `radius.*` | |
| すきま | space | `space.*` | |
| 大きさ | size | `size.*` | |
| 文字 | text | `text.*` | |
| 色 | color | `color.*` | |
| 印 | mark | `WindowMark.svelte`, `params/AutoMark.svelte` | |
| 選ぶ印 | choice mark | | The 12×12 square mark to choose a row (chapter 2). |
| 形式の印 | format mark | | LV2, VST3, CLAP. |
| 内がわ | padding / inside | | The space inside a part. |
| そろえる | align | | 「そろえる前の位置」 = the position before aligning. |
| 一文字（キー） | one letter | | A key with no Mod, which does not work while typing (chapter 4). |
| つながりの印 | link mark | `LinkKind` | Shows how a keyboard or output is connected. |
| 読み値 | reading | `reading` | A live number (position, tempo, dB) on a dark background. |
| ふつう | normal | | |
| マウスが乗った・乗った | hover | | |
| 押した | pressed | | |
| オン | on | | Opposite: オフ = off. |
| 使えない | disabled | | A part that is shown but cannot be pressed yet (border gray.5, text gray.7). For outputs, the code uses `Unusable` (`UnusableEndpoints.svelte`). |
| キーボードでえらんでる | keyboard focus | | |
| こわれた | broken | | Border and text in `rec`. |
| 止 | stopped | `StoppedPluginView` | The mark of a plugin that stopped. |
| とばす | bypass | `bypassed` (`InsertView`) | An effect that is passed by. とばしている = bypassed. |
| 拡大率 | scale | `SCALES`, `ZoomMenu.svelte` | 100, 125 %. Different from zooming the view (拡大). |
| 拡大 | zoom | `zoom` | Of the arrange or piano roll view. |
| 引っこめる | hide | | Hide a part when the width is not enough. |
| たたむ・ひらく | fold / open | | |

## Song and editing

| 日本語 | English | Code name | Note |
|---|---|---|---|
| 曲 | song | `SongView`, `Song` | |
| トラック | track | `TrackView`, `Track` | |
| トラックの色 | track color | `TrackColor` | |
| クリップ | clip | `ClipView`, `Clip` | |
| 歌のクリップ | audio clip | `AudioClipView`, `arrange/AudioClip*.svelte` | A clip of recorded sound. The track kind is "vocal" (`VocalRecordingView`). |
| テイク | take | `Take`, `TakeId` | Once made, never changed. |
| ノート | note | `Note` | |
| CC の点 | control point | `ControlPoint` | |
| オートメーション | automation | `AutomationView`, `Lane` | |
| レーン | lane | `LaneView`, `automation/AutoLane.svelte` | |
| 取り消せる手・手 | undoable step / step | `Step`, `StepName`, `History` | One drag is one step. |
| 手の名前 | step name | `StepName`, `stepNames` (ja.ts) | |
| 取り消し | undo | `History` | やりなおし = redo. |
| 意図 | intent | `Command` | What the screen asks the core to do. The screen sends only intents. |
| 選択・選ぶ | selection / select | `NoteSelection`, `SelectedClips` | |
| えらぶ | choose | `ChooserView`, `ChoiceMenu.svelte` | Pick one from a list (output, instrument). |
| 位置 | position | `position` | |
| 小節・拍・ティック | bar / beat / tick | `BarBeatTick` | |
| テンポ | tempo | `TempoPointView`, `TempoMap` | |
| 拍子 | time signature | `MeterChangeView`, `MeterChange` | The code says "meter"; in English text, "time signature" avoids a mix-up with level meters. |
| マーカー | marker | `MarkerView`, `Marker` | |
| メトロノーム | metronome | `metronome` | |
| カウントイン | count-in | `countIn` | |
| 再生の線 | play line | | The line at the play position. |
| クオンタイズ | quantize | `QuantizeSettings` | |
| スケール | scale (musical) | `MusicalScale`, `ScaleMode` | |
| スケール固定 | scale lock | | Write only the notes of the chosen scale. |
| テンポのだんだん・だんだん | tempo ramp / ramp | | |
| のび縮み | stretch | `StretchView`, `meloet-stretch` | |
| 音の高さ | pitch | | |
| 波形 | waveform | `arrange/wave.ts` | |
| フェード | fade | `arrange/fade.ts` | |
| 音量 | gain / volume | `ClipGain` | |

## Sound and devices

| 日本語 | English | Code name | Note |
|---|---|---|---|
| 鍵盤 | keyboard | `KeyboardView`, `KeyboardsView` | The MIDI keyboard. |
| 鍵 | key | | A key of the MIDI keyboard. Different from キー. |
| キー | key (on the PC keyboard) | | Chapter 4. When both appear, say "PC key". |
| 押している音 | held notes | `LiveNotes`, `held` | |
| 音の出口・出口 | output | `OutputsView`, `EndpointView`, `EndpointRow.svelte` | |
| 入口 | input | `InputsView`, `VocalInput` | |
| 入口のチャンネル | input channel | `VocalInput` | |
| 出口えらび | output chooser | `OutputChooser.svelte`, `ChooserView` | |
| やさしい言いかえ | friendly name | `part.endpoint.friendly` | The easy name of an output or input. |
| 音の設定の奥 | deep part of the sound settings | | |
| PC の音のしくみ | PC sound system | | |
| 音の流れ | sound stream | | |
| バッファ | buffer | | |
| 遅れ | latency | `LatencyDialog.svelte`, `LatencyView` | |
| モニター | monitor | | |
| 音源 | instrument | `InstrumentKind`, `InstrumentRef` | |
| 音色 | sound | `SoundView`, `SoundRef`, `SelectedSound` | |
| グループ | group | `InstrumentGroup` | |
| ドラム・キット | drums / kit | `pianoroll/drums.ts`, `DrumPiece` | |
| ためし弾き | audition | `audition` | Play an instrument from the shelf before choosing it. |
| 弾くと鳴る先 | play target | | The track and instrument that sound when you play (input meter). |
| 作った人 | maker | `vendor` | Of a plugin. |
| 子（プラグイン） | plugin | | |
| 入れかえ | replace | `InsertReplaced` | |
| サイン波で代わり | sine stand-in | `StandIn`, `standIn` | |
| 音の素材の包み | sound pack | `pack/PackBand.svelte`, `PackFetchView` | |
| 音の素材 | sound pack | | |
| プラグイン | plugin | `PluginView` | |
| スキャン | scan | `ScanView` | |
| エフェクト | effect | `EffectKind`, `EffectRef` | |
| インサート | insert | `InsertView`, `InsertRef`, `mixer/InsertSlot.svelte` | |
| インサートの枠 | insert slot | `mixer/InsertSlot.svelte` | |
| センド | send | `SendBus`, `mixer/SendBar.svelte` | |
| リターン | return | `ReturnView` | |
| マスター | master | `MasterView` | |
| ストリップ | strip | `mixer/Strip.svelte`, `StripRef` | |
| フェーダー | fader | `mixer/Fader.svelte` | |
| パン | pan | `Pan`, `mixer/PanKnob.svelte` | |
| メーター | meter | `MeterValue` | Level meter. |
| 音割れ | clipping | | The `rec` mark at the top of a meter. |
| 処理の負荷 | CPU load | `loadPermille` | |

## Files and recording

| 日本語 | English | Code name | Note |
|---|---|---|---|
| 録音 | record / recording | `RecordingView` | |
| 録音待機 | record arm | `armed` | |
| 歌のトラック | vocal track | | |
| 実時間 | real time | | Export in real time. |
| 書き出し | export | `export/ExportDialog.svelte`, `ExportView` | |
| 保存 | save | | |
| なおすためのファイル | recovery file | `RecoveryView`, `RecoveryHint.svelte` | |
| 曲のファイル | song file | `SongFile` | |
| 持ってきている（ファイル） | dragging in | | A file dragged onto the window. |
| 取りにいく所 | the place where it fetches | | |

## About the spec itself

| 日本語 | English | Code name | Note |
|---|---|---|---|
| 仕様書 | spec | `docs/spec/` | |
| モック | mock | `docs/design/mock/` | |
| 数値の一覧 | list of values | `ui/src/design/tokens.ts` | Chapter 1. |
| 決まり | rule | | |
| 版 | version | | "第 12 版" = version 12. |
| 問い | question | | A numbered question from the developers. |
| 作る側 | the developers | `builder:` (ja.ts) | |
| オーナーさん | the owner | `user:` (ja.ts) | |
