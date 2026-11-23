// All screen text lives here.
// Sources: spec:<chapter> (docs/spec/<chapter>-*.md), mock:<board> (docs/design/mock/<board>.dc.html), user:<date> (the owner's decision),
// draft:<date> (the developers' proposal for text that is in neither the spec nor the mocks; check with the owner, then make it user: or mock:).
// builder:<date> (text the developers decided for now where the spec has none; sent back to the design in B of docs/history/design-notes.ja.md).
// {name} is a hole for a value. example holds the example values written in the source; the tests fill them in and compare with the source.

import type { StepName } from "../bridge/generated/StepName";

export type Source =
  `spec:${number}` | `mock:${string}` | `user:${string}` | `draft:${string}` | `builder:${string}`;

export interface Entry {
  readonly text: string;
  readonly source: Source;
  readonly example?: Readonly<Record<string, string>>;
  /** The 「場面」 (scene) column of the spec 6 table. */
  readonly scene?: string;
  /** How it matches the wording cell. whole: the cell text as is. quoted: inside 「」 in the cell. part: a part of the cell text. */
  readonly fit?: "whole" | "quoted" | "part";
}

const e = (
  text: string,
  source: Source,
  example?: Record<string, string>,
): Entry => ({ text, source, example });
const s6 = (
  scene: string,
  text: string,
  example?: Record<string, string>,
  fit: "whole" | "quoted" | "part" = "whole",
): Entry => ({ text, source: "spec:6", scene, example, fit });

export const texts = {
  // spec 6 "Error and hint wording"
  "msg.keyboardMissing": s6(
    "Keyboard missing",
    "鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。",
  ),
  "msg.keyboardFound": s6(
    "Keyboard found",
    "{keyboard} が見つかりました。鍵は届いています。音の出口をえらぶと鳴ります。",
    { keyboard: "鍵盤 49" },
  ),
  "msg.noOutputMeter": s6(
    "No output yet",
    "音の出口がまだです。下でえらんでください",
    undefined,
    "quoted",
  ),
  "msg.noOutputArrange": s6(
    "No output yet",
    "音をどこから鳴らしますか",
    undefined,
    "quoted",
  ),
  "msg.outputNotPlugged": s6(
    "Nothing plugged into the chosen output",
    "{output}には、いま何もささっていません。ささると、ここから鳴ります。",
    { output: "（出口の名前）" },
  ),
  "msg.bufferFixedStatus": s6(
    "Buffer is fixed (status line)",
    "バッファが PC 全体の設定で {actual} に固定されています ・ 音が出るまで 約 {latencyMs} ms（{requested} なら 約 {latencyMsAtRequested} ms）",
    {
      actual: "1024",
      latencyMs: "53",
      requested: "256",
      latencyMsAtRequested: "13.5",
    },
  ),
  "msg.bufferFixedDetail": s6(
    "Buffer is fixed (deep in the sound settings)",
    "バッファが PC 全体の設定で {actual} に固定されています。meloet は {requested} を頼んでいますが、{actual} で動いています。鍵を押してから音が出るまで 約 {latencyMs} ms。{requested} で動けば 約 {latencyMsAtRequested} ms です。速いフレーズでは、音が指より遅れて聞こえます。録った音の位置は、遅れの補正で正しく置かれます。meloet は PC 全体の設定を変えません。変える場所は PipeWire の設定（quantum の固定）です。",
    {
      actual: "1024",
      latencyMs: "53",
      requested: "256",
      latencyMsAtRequested: "13.5",
    },
  ),
  "msg.soundCameOut": s6("Sound came out", "音が出ました。", undefined, "quoted"),
  // The second sentence is shown only when the record button can be used.
  "msg.soundCameOutRecord": s6(
    "Sound came out",
    "赤い丸の録音ボタンを押すと、カウントインのあとに録りはじめます。",
    undefined,
    "quoted",
  ),
  "msg.unusableSwitchedByPc": s6(
    "Unusable output: the PC switches to it",
    "さすと、PC がこちらに切りかえます",
  ),
  // When the plugged-in input is not chosen by the PC because other inputs are plugged in too (the owner's wording, now in spec 6).
  "msg.inputPluggedNotChosen": s6(
    "Input plugged in but not used",
    "ささっています ・ PC の設定でほかの入口をえらんでいます（設定 ▸ サウンド ▸ 入力 で変えられます）",
  ),
  "msg.unusableStoppedByPc": s6(
    "Unusable output: stopped by the PC settings",
    "PC の設定で、いまは使えません",
  ),
  "msg.unusableHeading": s6(
    "Heading of unusable outputs",
    "いまは使えない出口",
    undefined,
    "part",
  ),
  "msg.unusableHeadingChooser": s6(
    "Heading of unusable outputs",
    "PC が切りかえる物 {switched} ・ PC の設定で止まっている物 {stopped}",
    { switched: "1", stopped: "7" },
    "part",
  ),
  "msg.unusableHeadingSettings": s6(
    "Heading of unusable outputs",
    "meloet は、PC 全体の設定を変えません",
    undefined,
    "quoted",
  ),
  "msg.unusableInputHeading": s6(
    "Heading of unusable inputs",
    "いまは使えない入口",
    undefined,
    "part",
  ),
  "msg.countingIn": s6(
    "Counting in",
    "カウントイン中 ・ あと {beats} 拍で録音が始まります。止めるには Space",
    { beats: "2" },
  ),
  // The part shown after the status line tag 「● カウントイン中」 (status.countingIn). With the tag it makes the text above.
  "msg.countingInRest": s6(
    "Counting in",
    "あと {beats} 拍で録音が始まります。止めるには Space",
    { beats: "2" },
    "part",
  ),
  "msg.recording": s6(
    "Recording",
    "{track} へ録音 ・ 止めると テイク {take} として残ります",
    { track: "1 ピアノ", take: "2" },
  ),
  "msg.loopRecording": s6(
    "Recording in a loop",
    "{track} へ録音 ・ ループ {from}–{to} の {lap} 周目 ・ 周ごとにテイクを分けています",
    { track: "1 ピアノ", from: "5", to: "9", lap: "2" },
  ),
  "msg.takeSwitched": s6(
    "Take switched",
    "{track} ・ {clip} をテイク {take} / {takes} に切りかえました",
    { track: "1 ピアノ", clip: "Aメロ", take: "2", takes: "3" },
  ),
  "msg.noArmedTrack": s6(
    "No armed track",
    "録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。",
  ),
  "msg.keyboardLost": s6(
    "Keyboard unplugged",
    "{keyboard} とのつながりが切れました。曲はそのままです。さしなおすと、自動で戻ります。",
    { keyboard: "鍵盤 49" },
  ),
  "msg.outputLost": s6(
    "Output gone",
    "{output}が見つからなくなりました。再生を止めています。",
    { output: "ヘッドホン" },
  ),
  "msg.audioSystemStopped": s6(
    "PC sound system stopped",
    "PC の音のしくみ（PipeWire）が止まりました。曲はそのままです。動きだしたら、自動でつなぎなおします。",
  ),
  "msg.recordingStoppedAudioSystem": s6(
    "PC sound system stopped while recording",
    "PC の音のしくみ（PipeWire）が止まったので、録音を止めました。そこまでの演奏は テイク {take} に残っています。",
    { take: "2" },
  ),
  "msg.recordingStoppedOutputMissing": s6(
    "Output gone while recording",
    "{output}が見つからなくなったので、録音を止めました。そこまでの演奏は テイク {take} に残っています。",
    { output: "ヘッドホン", take: "2" },
  ),
  "msg.keyboardInputStopped": s6(
    "Input from the keyboard stopped",
    "鍵盤からの入力が止まりました。曲はそのままです。いま、つなぎなおしています。",
  ),
  "msg.micTooLoud": s6(
    "Mic too loud",
    "マイクの音が大きすぎます（0 dB に届きました）。マイクを少しはなすか、PC の入口の音量を下げてください。",
  ),
  "msg.scanning": s6(
    "Scanning",
    "スキャン中 {done} / {total} ・ 調べ終えた分から、もう使えます",
    { done: "37", total: "112" },
  ),
  "msg.scanDone": s6(
    "Scan finished",
    "スキャンが終わりました ・ {added} 個を棚に入れました。{stopped} 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。",
    { added: "667", stopped: "4" },
  ),
  "msg.windowNotYet": s6(
    "Plugin whose window cannot be opened yet",
    "この子の窓は、まだひらけません。パラメータはここで動かせます。meloet が、この型の窓にまだ対応していないためです。音は、いつもどおり鳴ります。",
  ),
  "msg.noWindow": s6(
    "Plugin that has no window",
    "この子は窓を持ちません。パラメータはここで動かせます。",
  ),
  "msg.windowsOpen": s6(
    "Windows are open",
    "プラグインの窓が {count} つひらいています",
    { count: "2" },
  ),
  // For one window (two or more: msg.windowsOpen).
  "msg.windowOpen": s6("Plugin window opened", "{name} の窓がひらいています", { name: "Surge XT" }, "part"),
  "msg.windowFailed": s6(
    "Plugin window cannot be shown",
    "{name} の窓をひらけませんでした。パラメータの欄では動かせます。",
    { name: "Surge XT" },
  ),
  "msg.windowOpening": s6(
    "Window takes time to open",
    "{name}の窓をひらいています…",
    { name: "（名前）" },
  ),
  "msg.windowTooLarge": s6(
    "Window larger than the screen",
    "{name}の窓がひらいています（{width}×{height} ・ この画面より大きい窓です）。パラメータは右でも動かせます。",
    { name: "（名前）", width: "1597", height: "995" },
  ),
  "msg.searchNoMatch": s6(
    "Search finds nothing",
    "「{query}」に合うパラメータはありません。",
    { query: "（ことば）" },
  ),
  "msg.formatSwitch": s6(
    "Before switching the format",
    "この曲では {current} 版を使っています。{next} 版に入れかえると、いまのパラメータの値は引きつがれないことがあります。",
    { current: "VST3", next: "LV2" },
  ),
  "msg.audition": s6(
    "Auditioning",
    "鍵盤を弾くと、この音色で鳴ります。まだトラックには入っていません。",
  ),
  "msg.samplePlaying": s6(
    "Sample playing",
    "サンプルを再生中 ・ {name}。行をえらぶと、鍵盤でためし弾きできます。",
    { name: "（名前）" },
  ),
  // On the status line, after the name of the auditioned sound (for a new track).
  "status.auditionNewTrack": e(
    "{instrument} ・ まだトラックには入っていません ・ 入れるには {key}",
    "mock:F4-ShelfAudition",
    {
      instrument: "シンセベース",
      key: "Enter",
    },
  ),
  "msg.auditionReplace": s6(
    "Auditioning to replace",
    "鍵盤を弾くと、この音色で鳴ります。再生すると、録ってあるノートもこの音色で鳴ります。まだ入れかえていません。",
  ),
  // Text from the 「出る場所」 (where shown) column of the spec 6 table.
  "status.auditionReplace": e(
    "{instrument} ・ {track} の音源は、まだ{current}です ・ 入れかえるには {key}",
    "spec:6",
    {
      instrument: "エレクトリックピアノ",
      track: "1 ピアノ",
      current: "グランドピアノ",
      key: "Enter",
    },
  ),
  "msg.instrumentReplaced": s6(
    "Instrument replaced",
    "{track} の音源を {instrument} にしました。取り消せます（{undoKey}）",
    {
      track: "1 ピアノ",
      instrument: "エレクトリックピアノ",
      undoKey: "Mod Z",
    },
  ),
  "msg.trackAdded": s6("Track added", "{track} を足しました。", {
    track: "5 シンセ",
  }),
  "msg.trackDeleted": s6(
    "Track deleted",
    "{track} を消しました。取り消せます（{undoKey}）",
    { track: "3 ドラム", undoKey: "Mod Z" },
  ),
  // A3: a song has 64 tracks and one more was asked for (not in the spec; in the words of 「SF2 のファイルではないので、足せません。」).
  "msg.tooManyTracks": e("トラックは 64 本までなので、これ以上足せません。", "builder:2026-11-23"),
  "msg.drumsNoScaleLock": s6(
    "Drums and 「スケール固定」",
    "ドラムには音階がないので、スケール固定は使えません。",
  ),
  "msg.auditionLoading": s6(
    "Instrument to audition is not loaded yet",
    "{name}を読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。",
    { name: "（名前）" },
  ),
  "msg.pluginMissing": s6(
    "Plugin not found",
    "前はここにありました: {path}。曲の中の設定は残しています。",
    { path: "（場所）" },
  ),
  "msg.pluginStopped": s6(
    "Plugin stopped",
    "{name}が止まりました ・ {track}。曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。",
    { name: "（名前）", track: "（トラック）" },
  ),
  "msg.pluginScanFailed": s6(
    "Plugin failed in the scan",
    "{name}は調べている途中で止まったので、棚には入れていません。ほかのプラグインは使えます。",
    { name: "（名前）" },
  ),
  "msg.undoPending": s6(
    "There are undone steps",
    "{count} 手ぶん取り消してあります。新しく何かすると、その {count} 手はやりなおせなくなります。",
    { count: "2" },
  ),
  "msg.quantized": s6(
    "Quantize applied",
    "クオンタイズ {grid}（{strength} %）を {count} ノートに。うすい枠が、そろえる前の位置。",
    { grid: "1/16", strength: "75", count: "3" },
  ),
  "msg.unsaved": s6(
    "Not saved",
    "「{song}」に、まだ保存していない変更があります。",
    { song: "はじめての曲" },
  ),
  "msg.cannotSave": s6(
    "Cannot save",
    "ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。べつの場所をえらんでください。曲は開いたままです。",
  ),
  "msg.songBroken": s6(
    "Song file is broken",
    "このファイルは読めませんでした。自動で残してある 1 つ前の状態からなら開けます。",
  ),
  // When there is no earlier state, only the first sentence of the text above.
  "msg.songBrokenFirst": s6(
    "Song file is broken",
    "このファイルは読めませんでした。",
    undefined,
    "part",
  ),
  // Recovery file (spec 3). Spec text for a file written when the PC shuts down; the owner's text for one written at other times.
  "msg.recoveredPcEnding": s6(
    "Restored from the recovery file",
    "PC が終わる前の演奏を戻しました。まだ保存していません。",
    undefined,
    "quoted",
  ),
  // The second sentence is kept as two parts, 「…です。」 and 「テイク N つ。」. With 0 takes the second part is not shown (a builder decision, sent back to the design).
  "msg.recoveredPcEndingDetail": s6(
    "Restored from the recovery file",
    "{month} 月 {day} 日 {time} に PC が終わったときの「{song}」です。",
    { month: "11", day: "15", time: "22:41", song: "新しい曲" },
    "part",
  ),
  "msg.recoveredTakes": s6(
    "Restored from the recovery file",
    "テイク {takes} つ。",
    { takes: "2" },
    "part",
  ),
  "msg.recoveredSudden": e(
    "meloet が急に終わる前の演奏を戻しました。まだ保存していません。",
    "user:2026-11-15",
  ),
  "msg.recoveredSuddenDetail": e(
    "{month} 月 {day} 日 {time} の「{song}」です。",
    "user:2026-11-15",
    {
      month: "11",
      day: "15",
      time: "22:41",
      song: "新しい曲",
    },
  ),
  // Text from the 「出る場所」 (where shown) column of the spec 6 table.
  "status.recovered": e(
    "なおすためのファイルから戻しました ・ まだ保存していません",
    "spec:6",
  ),
  "msg.recoveryDiscarded": s6(
    "「戻さない」 pressed",
    "戻した変更をすてて、最後に保存した形にしました。取り消せます（{undoKey}）",
    { undoKey: "Mod Z" },
  ),
  "msg.recoveryUnreadable": s6(
    "Recovery file cannot be read",
    "前に保存していない変更がありましたが、なおすためのファイルが読めませんでした。",
    undefined,
    "quoted",
  ),
  "msg.recoveryUnreadableSaved": s6(
    "Recovery file cannot be read",
    "最後に保存した「{song}」をひらいています。ファイルは消さずに残してあります。",
    { song: "はじめての曲" },
    "quoted",
  ),
  "msg.recoveryUnreadableNew": s6(
    "Recovery file cannot be read",
    "新しい曲をひらいています。ファイルは消さずに残してあります。",
    undefined,
    "quoted",
  ),
  // Status line. For a song never saved, the end is 「新しい曲をひらいています」 (the form of mock F-Recover3 with the spec words).
  "status.recoveryUnreadable": e(
    "なおすためのファイルが読めませんでした ・ {opened}",
    "mock:F-Recover3",
    { opened: "最後に保存した曲をひらいています" },
  ),
  "status.recoveryOpenedSaved": e(
    "最後に保存した曲をひらいています",
    "mock:F-Recover3",
  ),
  "status.recoveryOpenedNew": s6(
    "Recovery file cannot be read",
    "新しい曲をひらいています",
    undefined,
    "part",
  ),
  "msg.cpuOverload": s6(
    "Processing cannot keep up",
    "PC の処理が追いつかず、音がとぎれました。音の設定でバッファを大きくすると、とぎれにくくなります。",
  ),
  "msg.exportPeak": s6(
    "Before export",
    "いちばん大きい所 {peak} dB ・ 音割れはありません",
    { peak: "−6.2" },
  ),
  "msg.exportClipping": s6(
    "Export would clip",
    "いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。",
  ),
  "msg.exported": s6("Export finished", "書き出しました: {path}", {
    path: "~/ミュージック/はじめての曲/はじめての曲.wav",
  }),
  "msg.scaleDoesNotFit": s6("Scale that does not fit the screen", "この画面には入りません"),
  "msg.scaleKept": s6(
    "Scale that does not fit, chosen with a key",
    "{requested} % は、この画面には入りません。{kept} % のままです。",
    { requested: "150", kept: "125" },
  ),
  "msg.scaleWindowGrown": s6(
    "Scale changed and the window made larger",
    "拡大率を {scale} % にしました。窓を、いちばん小さい大きさまで広げました。",
    { scale: "125" },
  ),
  "msg.scaleLowered": s6(
    "Screen became smaller",
    "画面が小さくなったので、拡大率を {from} % から {to} % にしました。",
    { from: "150", to: "125" },
  ),
  "msg.bandFoldedWindowLow": s6(
    "Band folded automatically",
    "窓が低くなったので、{band}をたたみました。帯札を押すと、またひらきます。",
    { band: "ミキサー" },
  ),
  "msg.bandFoldedToOpen": s6(
    "Opened a band that does not fit",
    "{opened}をひらくために、{folded}をたたみました。窓を高くすると、両方ひらけます。",
    { opened: "ピアノロール", folded: "ミキサー" },
  ),
  "msg.bandFoldedThird": s6(
    "Opened a third band",
    "{opened}をひらいたので、いちばん前にひらいた{folded}をたたみました。下の帯は、2 つまでひらけます。",
    { opened: "取り消しの履歴", folded: "ミキサー" },
  ),
  "msg.bandFoldedScale": s6(
    "Band folded after raising the scale",
    "拡大率を {scale} % にしたので、{band}をたたみました。帯札を押すと、またひらきます。",
    { scale: "150", band: "ミキサー" },
  ),
  // Something that cannot be read as a number was typed in a number field. The field is value.* or note.*.
  "msg.valueRejected": s6(
    "Not readable as a number",
    "{field}は数字で打ちます。もとの {kept} のままです。",
    { field: "ベロシティ", kept: "77" },
    "part",
  ),
  // Something that cannot be read was typed in the pitch field (typed as a note name).
  "msg.keyRejected": s6(
    "Not readable in the note field",
    "{field}は {range} です。もとの {kept} のままです。",
    { field: "音", range: "C-1〜G9", kept: "C4" },
  ),
  "msg.backToArrange": s6(
    "Way back to the arrange",
    "Esc ・ ひとつ戻る。何度か押すとアレンジへ",
  ),
  // A value out of range is set to the nearest end. The fields and ranges are value.*.
  "msg.valueClamped": s6(
    "Typed a value out of range",
    "{field}は {range} なので、{value} にしました。",
    { field: "ベロシティ", range: "1〜127", value: "127" },
    "quoted",
  ),
  // Change velocities together, align lengths.
  "msg.velocityMatched": s6(
    "Set together",
    "{count} ノートのベロシティを {value} にそろえました。",
    { count: "3", value: "100" },
    "part",
  ),
  "msg.lengthMatched": s6(
    "Set together",
    "{count} ノートの長さを {value} にそろえました。",
    { count: "3", value: "0.240" },
    "part",
  ),
  // When the bar of the position was clamped. Only for the bar there is no space after 「1 から」. Beats and ticks use msg.valueClamped.
  "msg.valueClampedBar": s6(
    "Position rounded",
    "小節は 1 からなので、{value} にしました。",
    { value: "1. 1. 000" },
    "quoted",
  ),
  // Could not be placed because it overlaps another clip.
  "msg.clipsNotMoved": s6(
    "Clips overlap",
    "ほかのクリップと重なるので、動かしませんでした。",
    undefined,
    "quoted",
  ),
  "msg.clipsNotPasted": s6(
    "Clips overlap",
    "ほかのクリップと重なるので、貼りつけませんでした。",
    undefined,
    "quoted",
  ),
  "msg.clipsNotDuplicated": s6(
    "Clips overlap",
    "ほかのクリップと重なるので、複製できませんでした。",
    undefined,
    "quoted",
  ),
  // In the piano roll, writing outside the clip could not grow the clip (B-82).
  "msg.notesNotWritten": s6(
    "Cannot write outside the clip",
    "ほかのクリップと重なるので、書けませんでした。",
  ),
  // P4 「ここに置く」 where the meter is not 4/4.
  "msg.patternNeedsFourFour": e("4/4 のところにだけ置けます", "user:2026-11-23"),
  "msg.notesNotResized": s6(
    "Cannot change the length",
    "ほかのクリップと重なるので、長さを変えられませんでした。",
  ),
  "msg.notesNotMoved": s6(
    "Cannot move outside the clip",
    "ほかのクリップと重なるので、動かしませんでした。",
  ),
  "msg.notesNotDuplicated": s6(
    "Cannot copy outside the clip",
    "ほかのクリップと重なるので、複製できませんでした。",
  ),
  "msg.takeDeleted": s6(
    "Take deleted",
    "テイク {take} を消しました。テイク {active} / {total} にしました。取り消せます（{undoKey}）",
    { take: "2", active: "1", total: "2", undoKey: "Mod Z" },
  ),
  // Song dialogs (spec 3 (89-6)). The heading is the song name as is; only the status line says the name.
  "status.songDialogOpen": s6("Song dialog is open", "「{dialog}」のダイアログをひらいています", { dialog: "保存していない変更" }),
  "songDialog.unsaved": e("保存していない変更", "spec:3"),
  "songDialog.cannotSave": e("保存できませんでした", "spec:3"),
  "songDialog.broken": e("ひらけない曲", "spec:3"),
  // 「音の設定をひらいています」 is status.settingsOpen.
  "status.dialogOpen": s6(
    "Dialog is open",
    "{dialog} をひらいています",
    { dialog: "meloet について" },
    "part",
  ),
  "msg.soundsMissingSong": s6(
    "Sound pack not found (song opened)",
    "音の素材が見つからないので、{count} 本のトラックをサイン波で鳴らしています。",
    { count: "4" },
  ),
  "shelf.soundsMissing": s6(
    "Sound pack not found (shelf)",
    "音の素材が見つかりません。いまはサイン波だけ使えます。meloet を入れなおすと、もとに戻ります。",
  ),

  // [SF2] (spec 6)
  "sf2.dropHere": s6(
    "Dragging in an SF2",
    "ここに落とすと、棚に SF2 を足します。ファイルは meloet の中にうつします。",
    undefined,
    "part",
  ),
  "sf2.dropCancel": s6(
    "Dragging in an SF2",
    "やめるには、窓の外で離すか Esc",
    undefined,
    "quoted",
  ),
  // The 「出る場所」 column of spec 6 「SF2 を持ってきている」.
  "sf2.dragging": e("SF2 のファイルを持ってきています", "spec:6"),
  "sf2.notSf2": s6(
    "Dragging in a file that is not SF2",
    "SF2 のファイルではないので、足せません。",
  ),
  "sf2.notSf2Dragging": s6("Dragging in a non-SF2 file", "SF2 ではないファイルを持ってきています（足せません）"),
  // The file name part of the box in the middle of the window. With no SF2, only the first part (docs/history/design-notes.ja.md A-19).
  "sf2.manyFiles": s6("Dragging in many files", "{files} つのファイル ・ SF2 は {sf2} つ", { files: "3", sf2: "2" }),
  "sf2.manyFilesNoSf2": s6("Dragging in many files", "{files} つのファイル", { files: "3" }, "part"),
  // The two sentences of 「SF2 をいくつも足した」. When one SF2 was added, only the second sentence after sf2.added (docs/history/design-notes.ja.md A-19).
  "sf2.addedMany": s6(
    "Many SF2 files added",
    "SF2 を {count} つ足しました（{file} ほか {others} つ）。",
    { count: "2", file: "FluidR3_GM.sf2", others: "1" },
    "part",
  ),
  "sf2.skipped": s6("Many SF2 files added", "{count} つは SF2 ではないので、足していません。", { count: "1" }, "part"),
  "sf2.added": s6(
    "SF2 added",
    "{file} を足しました。音色 {count} ・ ドラムのキット {kits}",
    { file: "FluidR3_GM.sf2", count: "128", kits: "9" },
  ),
  "sf2.unreadable": s6(
    "SF2 cannot be read",
    "{file} は読めませんでした。ファイルがこわれているか、SF2 ではないかもしれません。",
    { file: "FluidR3_GM.sf2" },
  ),
  "sf2.missing": s6(
    "SF2 not found",
    "曲で使っている {file} が見つかりません。{count} 本のトラックはサイン波で鳴らしています。",
    { file: "FluidR3_GM.sf2", count: "2" },
  ),

  // [Added later] (spec 2, spec 3 (83, 85, Y12, Y14), mock F5-*)
  "shelf.builtin": e("最初から入っているもの", "mock:F5-SF2Shelf"),
  "shelf.added": e("あとから入れたもの", "mock:F5-SF2Shelf"),
  "shelf.sectionCount": e("{name} ・ {count}", "mock:F5-SF2Shelf", {
    name: "あとから入れたもの",
    count: "1",
  }),
  // The count in the heading while searching (「合う数 / 全部」).
  "shelf.matchCount": e("{shown} / {total}", "mock:F5-ShelfSearch", {
    shown: "4",
    total: "506",
  }),
  "shelf.sf2Mark": e("SF2", "spec:3"),
  "shelf.bank": e("・ バンク {bank}", "spec:3", { bank: "8" }),
  "shelf.moreFormats": e("＋{count}", "mock:F5-PluginAudition", { count: "1" }),
  "shelf.formatsTip": e("{formats} があります。押すと、使う形式をえらべます。", "mock:F5-PluginAudition", {
    formats: "LV2 ・ VST3",
  }),
  "shelf.favoriteKey": e("印 {favoriteKey}", "mock:F5-ShelfFx", { favoriteKey: "F" }),
  "shelf.loadingTitle": e("読みこみ中", "mock:F5-PluginLoading"),
  "shelf.sampleLoading": e("読みこみ中…", "spec:3"),
  "shelf.windowHint": e("音色は窓でえらべます", "mock:F5-PluginAudition"),
  "shelf.mixStart": e("ミックスは、はじめ 30 %。", "mock:F5-ShelfFx"),
  // The name of an SF2 sound (status line; mock F5-SF2Shelf 「Clean Guitar ・ FluidR3_GM.sf2」).
  "sound.sf2Name": e("{name} ・ {file}", "mock:F5-SF2Shelf", {
    name: "Clean Guitar",
    file: "FluidR3_GM.sf2",
  }),
  "button.openWindow": e("窓をひらく", "spec:3"),
  "track.standIn": e("サイン波で代わり", "spec:3"),
  "track.standInPackTip": e("音の素材が見つからないので、サイン波で鳴らしています。", "spec:3"),
  "track.standInSf2Tip": e("{file} が見つからないので、サイン波で鳴らしています。", "spec:3", { file: "FluidR3_GM.sf2" }),
  "track.standInPluginTip": e("{name} が見つからないので、サイン波で鳴らしています。", "spec:3", { name: "Surge XT" }),
  "track.standInUnreadableTip": e("{name} を読みこめなかったので、サイン波で鳴らしています。", "spec:3", { name: "Surge XT" }),
  "mixer.stoppedMark": e("止", "spec:3"),
  "shelf.missingRow": e("見つかりません", "mock:A-Mixer"),
  // The box of stopped plugins under the shelf (mock A-Mixer: the heading and the rest of the spec 6 text 「プラグインが止まった」).
  "plugin.stoppedHead": e("{name} が止まりました ・ {track}", "mock:A-Mixer", { name: "Brick EQ", track: "3 ドラム" }),
  "plugin.stoppedBody": s6(
    "Plugin stopped",
    "曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。",
    undefined,
    "part",
  ),
  "status.cancelKey": e("やめるには {key}", "mock:F5-PluginLoading", { key: "Esc" }),
  "status.shelfSearch": e("さがす「{query}」・ 合う物 {count} 個", "mock:F5-ShelfSearch", {
    query: "Dragonfly",
    count: "4",
  }),
  // Notes with the same name are grouped (「スネア ×2」).
  "note.keyCount": e("{name} ×{count}", "spec:3", { name: "スネア", count: "2" }),
  // The reading while moving notes up or down in drums (the normal piano roll uses pr.readMove).
  "pr.readMoveRows": e("{ticks} ・ {rows}", "spec:3", { ticks: "+0.120", rows: "上へ 1 行" }),
  "pr.rowsUp": e("上へ {count} 行", "spec:3", { count: "1" }),
  "pr.rowsDown": e("下へ {count} 行", "spec:3", { count: "2" }),
  // Headings of SF2 families. The part after the key prefix is GmFamily of the protocol.
  "gmFamily.piano": e("ピアノ", "spec:3"),
  "gmFamily.mallet": e("マレット", "spec:3"),
  "gmFamily.organ": e("オルガン", "spec:3"),
  "gmFamily.guitar": e("ギター", "spec:3"),
  "gmFamily.bass": e("ベース", "spec:3"),
  "gmFamily.strings": e("弦", "spec:3"),
  "gmFamily.ensemble": e("アンサンブル", "spec:3"),
  "gmFamily.brass": e("金管", "spec:3"),
  "gmFamily.reed": e("木管", "spec:3"),
  "gmFamily.pipe": e("笛", "spec:3"),
  "gmFamily.synthLead": e("シンセリード", "spec:3"),
  "gmFamily.synthPad": e("シンセパッド", "spec:3"),
  "gmFamily.synthEffects": e("シンセの効果", "spec:3"),
  "gmFamily.ethnic": e("民族楽器", "spec:3"),
  "gmFamily.percussive": e("打楽器", "spec:3"),
  "gmFamily.soundEffects": e("効果音", "spec:3"),
  "gmFamily.drumKit": e("ドラムのキット", "spec:3"),
  // Headings of plugin kinds (14-A). The part after the key prefix is PluginInstrumentCategory and PluginEffectCategory of the protocol.
  "pluginKind.instrument.synth": e("シンセ", "spec:3"),
  "pluginKind.instrument.sampler": e("サンプラー", "spec:3"),
  "pluginKind.instrument.drum": e("ドラム", "spec:3"),
  "pluginKind.instrument.keys": e("ピアノ ・ 鍵盤", "spec:3"),
  "pluginKind.instrument.other": e("そのほか", "spec:3"),
  "pluginKind.effect.eq": e("EQ", "spec:3"),
  "pluginKind.effect.dynamics": e("ダイナミクス", "spec:3"),
  "pluginKind.effect.reverb": e("リバーブ", "spec:3"),
  "pluginKind.effect.delay": e("ディレイ", "spec:3"),
  "pluginKind.effect.modulation": e("モジュレーション", "spec:3"),
  "pluginKind.effect.distortion": e("ひずみ", "spec:3"),
  "pluginKind.effect.filter": e("フィルター", "spec:3"),
  "pluginKind.effect.pitch": e("ピッチ", "spec:3"),
  "pluginKind.effect.spatial": e("空間", "spec:3"),
  "pluginKind.effect.analysis": e("分析", "spec:3"),
  "pluginKind.effect.other": e("そのほか", "spec:3"),
  // Row names of an SF2 drum kit (by key number, the GM rule). Keys not in the table use the note name.
  "gmDrum.35": e("キック 2", "spec:3"),
  "gmDrum.36": e("キック", "spec:3"),
  "gmDrum.37": e("サイドスティック", "spec:3"),
  "gmDrum.38": e("スネア", "spec:3"),
  "gmDrum.39": e("クラップ", "spec:3"),
  "gmDrum.40": e("スネア 2", "spec:3"),
  "gmDrum.41": e("フロアタム 2", "spec:3"),
  "gmDrum.42": e("クローズハイハット", "spec:3"),
  "gmDrum.43": e("フロアタム", "spec:3"),
  "gmDrum.44": e("ペダルハイハット", "spec:3"),
  "gmDrum.45": e("ロータム", "spec:3"),
  "gmDrum.46": e("オープンハイハット", "spec:3"),
  "gmDrum.47": e("ミッドタム 2", "spec:3"),
  "gmDrum.48": e("ミッドタム", "spec:3"),
  "gmDrum.49": e("クラッシュ", "spec:3"),
  "gmDrum.50": e("ハイタム", "spec:3"),
  "gmDrum.51": e("ライド", "spec:3"),
  "gmDrum.52": e("チャイナ", "spec:3"),
  "gmDrum.53": e("ライドのベル", "spec:3"),
  "gmDrum.54": e("タンバリン", "spec:3"),
  "gmDrum.55": e("スプラッシュ", "spec:3"),
  "gmDrum.56": e("カウベル", "spec:3"),
  "gmDrum.57": e("クラッシュ 2", "spec:3"),
  "gmDrum.58": e("ビブラスラップ", "spec:3"),
  "gmDrum.59": e("ライド 2", "spec:3"),
  "gmDrum.60": e("ボンゴ（高）", "spec:3"),
  "gmDrum.61": e("ボンゴ（低）", "spec:3"),
  "gmDrum.62": e("コンガ（高 ・ ミュート）", "spec:3"),
  "gmDrum.63": e("コンガ（高）", "spec:3"),
  "gmDrum.64": e("コンガ（低）", "spec:3"),
  "gmDrum.65": e("ティンバレス（高）", "spec:3"),
  "gmDrum.66": e("ティンバレス（低）", "spec:3"),
  "gmDrum.67": e("アゴゴ（高）", "spec:3"),
  "gmDrum.68": e("アゴゴ（低）", "spec:3"),
  "gmDrum.69": e("カバサ", "spec:3"),
  "gmDrum.70": e("マラカス", "spec:3"),
  "gmDrum.71": e("ホイッスル（短）", "spec:3"),
  "gmDrum.72": e("ホイッスル（長）", "spec:3"),
  "gmDrum.73": e("ギロ（短）", "spec:3"),
  "gmDrum.74": e("ギロ（長）", "spec:3"),
  "gmDrum.75": e("クラベス", "spec:3"),
  "gmDrum.76": e("ウッドブロック（高）", "spec:3"),
  "gmDrum.77": e("ウッドブロック（低）", "spec:3"),
  "gmDrum.78": e("クイーカ（ミュート）", "spec:3"),
  "gmDrum.79": e("クイーカ", "spec:3"),
  "gmDrum.80": e("トライアングル（ミュート）", "spec:3"),
  "gmDrum.81": e("トライアングル", "spec:3"),

  // [Inserts] (spec 6)
  "shelf.insertInto": s6(
    "Putting into an insert",
    "{strip} のインサート {slot} に入れます。",
    { strip: "2 ベース", slot: "2" },
  ),
  "msg.effectInserted": s6(
    "Effect put in",
    "{strip} のインサート {slot} に {effect} を入れました。",
    { strip: "2 ベース", slot: "2", effect: "リバーブ" },
  ),
  "msg.effectRemoved": s6(
    "Effect removed",
    "{strip} のインサート {slot} の {effect} を外しました。取り消せます（{undoKey}）",
    { strip: "2 ベース", slot: "1", effect: "コンプレッサー", undoKey: "Mod Z" },
  ),
  // When bands disappear together with remove, replace, or replacing the instrument, added before the final 「。」 of the status line text (spec 3 (92-19)).
  "msg.lanesAlsoGone": s6("Bands went away too", "（そのパラメータの帯 {count} 本も消えました）", { count: "2" }, "part"),
  "msg.insertsReordered": s6(
    "Inserts reordered",
    "{strip} のインサートを並べかえました（{order}）。",
    { strip: "2 ベース", order: "EQ 4 バンド → コンプレッサー" },
  ),
  "shelf.insertReplace": s6(
    "Replacing an insert",
    "{strip} のインサート {slot} の {current} を、{effect} に入れかえます。いまの設定は残りません（取り消せます）。",
    { strip: "2 ベース", slot: "1", current: "コンプレッサー", effect: "EQ 4 バンド" },
  ),
  "msg.insertReplaced": s6(
    "Insert replaced",
    "{strip} のインサート {slot} を {effect} に入れかえました。取り消せます（{undoKey}）",
    { strip: "2 ベース", slot: "1", effect: "EQ 4 バンド", undoKey: "Mod Z" },
  ),
  // {target} is 「{ストリップ} の {エフェクト} 」 (with a space after). The spec example for undoing a bypass is 「…をもどしました。」.
  "msg.effectBypassed": s6(
    "Effect bypassed or brought back",
    "{target}をとばしました。",
    { target: "2 ベース の コンプレッサー " },
    "part",
  ),
  "msg.effectRestored": s6(
    "Effect bypassed or brought back",
    "{target}をもどしました。",
    { target: "…" },
    "quoted",
  ),
  "status.paramSelectedInsert": s6(
    "Parameter chosen (track, master)",
    "{strip} ・ {effect}の「{param}」をえらんでいます",
    { strip: "2 ベース", effect: "コンプレッサー", param: "しきい値" },
  ),
  "params.valueClamped": s6(
    "Typed value goes over the range",
    "{param}は {range} なので、{value} にしました。",
    { param: "レシオ", range: "1〜20", value: "20 : 1" },
  ),
  "params.nothingSelected": s6(
    "Params panel with nothing chosen",
    "ミキサーでインサートの枠をえらぶと、ここにパラメータが出ます。",
  ),
  "mixer.noOutput": s6("No output", "出口なし ▸", undefined, "part"),
  "mixer.noOutputTip": s6(
    "No output",
    "音の出口がありません。押すと音の設定をひらきます。",
    undefined,
    "quoted",
  ),

  // [Windows and parameters] Text found only in the mocks.
  // 「（名前）」 in the spec 6 window text is the place and the plugin name (with a space after; docs/history/design-notes.ja.md A-17).
  "status.windowPlace": e("{strip} ・ {name}", "mock:F-ParamMany", {
    strip: "1 ピアノ",
    name: "Surge XT",
  }),
  "status.windowNotYet": e(
    "{name} の窓はまだひらけません。パラメータは右で動かせます",
    "mock:F-ParamNoWindow",
    { name: "Mossy Verb" },
  ),
  "status.paramMoving": e(
    "{name} の {param} を動かしています",
    "mock:F-ParamFew",
    { name: "Dragonfly Reverb", param: "Decay" },
  ),
  "status.paramSearching": e(
    "パラメータをさがしています ・「{query}」・ {count} 個",
    "mock:F-ParamSearch",
    { query: "cutoff", count: "24" },
  ),
  "status.insertBypassedNow": s6(
    "Mixer opened (with bypassed or stopped slots)",
    "{strip} の {effect} をとばしています",
    { strip: "4 パッド", effect: "MVerb" },
    "part",
  ),
  "status.pluginStoppedNow": s6(
    "Mixer opened (with bypassed or stopped slots)",
    "{strip} の {name} は止まっています（音は、その枠をとばして流しています）",
    { strip: "3 ドラム", name: "Brick EQ" },
    "part",
  ),
  "windows.heading": e("ひらいている窓 ・ 押すと前に出ます", "mock:F-WindowOpen"),
  "windows.raiseAll": e("ぜんぶ前に出す", "mock:F-WindowOpen"),
  "params.whereInstrument": e("{strip} ・ 音源", "mock:F-ParamMany", {
    strip: "1 ピアノ",
  }),
  "button.raiseWindow": e("窓を前に出す", "mock:F-ParamMany"),
  "windows.button": e("窓 {count}", "spec:6", { count: "2" }),
  // 「（ストリップ）の（エフェクト）」 on the status line, with a space after ({target} of msg.effectBypassed).
  "status.insertTarget": e("{strip} の {effect} ", "spec:6", { strip: "2 ベース", effect: "コンプレッサー" }),
  "windows.shelfAudition": e("棚 ・ ためし弾き", "user:2026-11-19"),
  "params.foldedHint": e("40 個をこえる子は、グループをたたんで始めます", "mock:F-ParamMany"),
  "params.searchHint": e("さがす枠を空にすると、グループの一覧に戻ります", "mock:F-ParamSearch"),
  "params.more": e("… ほか {count}", "spec:3", { count: "6" }),
  "params.groupOther": e("そのほか", "mock:F-ParamMany"),
  // Parameters that only receive the keyboard's knobs (spec 3 (91-3)). Shown in this group only while searching.
  "params.keyboardKnobs": e("鍵盤のつまみを受ける所", "spec:3"),
  // Under the list. first and last are the first and last names (last without the start it shares with first; 「MIDI CC 0|0」〜「15|129」).
  "params.keyboardKnobsHidden": e(
    "ほかに、鍵盤のつまみを受けるだけの {count} 個（{first} 〜 {last}）は出していません。さがすと出ます。",
    "spec:3",
    { count: "2080", first: "MIDI CC 0|0", last: "15|129" },
  ),
  "params.automated": e("オートメーションに書いている", "spec:3"),
  "params.unknownValue": e("—", "user:2026-11-19"),
  "params.nothingInstrument": e("音源の枠をえらぶと、音源のパラメータが出ます。", "spec:3"),
  "params.bypass": e("とばす", "spec:3"),
  "params.bypassing": e("とばしている", "spec:3"),
  "key.bypass": e("B", "spec:4"),
  // Added after the name on hover over an insert slot (spec 2 "On mark of an insert slot").
  "mixer.bypassedTip": e("（とばしている）", "spec:2"),
  "insert.replace": e("入れかえる…", "spec:3"),
  "insert.moveUp": e("上へ動かす", "spec:3"),
  "insert.moveDown": e("下へ動かす", "spec:3"),
  "track.replaceSound": e("音源を入れかえる…", "user:2026-11-19"),
  "shelf.titleReplaceEffect": e("{strip} のインサート {slot} を入れかえる", "mock:R9-Inserts", { strip: "2 ベース", slot: "1" }),
  "shelf.currentEffect": e("いまのエフェクト", "spec:3"),

  // [Plugins] (spec 6)
  "plugin.newFound": s6(
    "New plugins found",
    "新しいプラグインが {count} 個見つかりました。棚の「あとから入れたもの」に入れました。",
    { count: "3" },
  ),
  "plugin.stoppedList": s6(
    "List of plugins that stopped in the scan",
    "この {count} 個は、調べている途中で止まったので、棚に入れていません。meloet と曲は無事です。次のスキャンからは、とばします。",
    { count: "4" },
  ),
  "plugin.retryOk": s6(
    "Tried again and it got in",
    "{name} を、もう一度ためしました。今度は入りました。",
    { name: "Granular Fog" },
  ),
  "plugin.retryStopped": s6(
    "Tried again and it stopped again",
    "{name} は、また止まりました。棚には入れません。",
    { name: "Granular Fog" },
  ),
  "plugin.windowStep": s6(
    "Changed in a plugin window (history)",
    "{name} の窓で変えた所",
    { name: "Surge XT" },
  ),
  "plugin.loading": s6(
    "Loading a plugin",
    "{name} を読みこんでいます…",
    { name: "Surge XT" },
  ),
  "plugin.loadFailed": s6(
    "Plugin cannot be loaded",
    "{name} を読みこめませんでした。",
    { name: "Surge XT" },
  ),
  "plugin.loadFailedShelf": s6(
    "Plugin cannot be loaded (shelf)",
    "{name} を読みこめませんでした。こわれているか、この PC では動かないのかもしれません。",
    { name: "Surge XT" },
  ),
  "plugin.missingSong": s6(
    "Plugin not found (song opened)",
    "曲で使っている {name} が見つかりません。{count} 本のトラックはサイン波で鳴らしています。",
    { name: "Surge XT", count: "1" },
  ),
  "plugin.unreadableSong": s6(
    "Plugin cannot be loaded (song opened)",
    "曲で使っている {name} を読みこめませんでした。{count} 本のトラックはサイン波で鳴らしています。",
    { name: "Surge XT", count: "1" },
  ),
  "plugin.instrumentRemoved": s6(
    "Instrument plugin removed",
    "{track} の音源を外しました。サイン波で鳴らします。取り消せます（{undoKey}）",
    { track: "5 Surge XT", undoKey: "Mod Z" },
  ),
  // With 0 stopped plugins, only the first half of the spec 6 text (docs/history/design-notes.ja.md A-16).
  "msg.scanDoneNone": s6(
    "Scan finished",
    "スキャンが終わりました ・ {added} 個を棚に入れました。",
    { added: "667" },
    "part",
  ),
  // Dialogs 「スキャンで止まったプラグイン」 and 「プラグインの場所」 (spec 2, spec 3 (Y13), mock F5-ScanStopped, R9-Scan).
  "plugin.stoppedTitle": e("スキャンで止まったプラグイン", "spec:3"),
  "plugin.reason.crashed": e("読みこむと止まりました", "spec:3"),
  "plugin.reason.timedOut": e("30 秒たっても終わりませんでした", "spec:3"),
  "plugin.reason.broken": e("作りがこわれています", "spec:3"),
  "plugin.retrying": e("ためしています…", "spec:2"),
  "plugin.format.vst3": e("VST3", "mock:F5-ScanStopped"),
  "plugin.format.lv2": e("LV2", "mock:F5-ScanStopped"),
  "plugin.places": e("プラグインの場所", "spec:3"),
  "plugin.usualPlace": e("ふつうの場所", "spec:2"),
  "button.addPlace": e("場所を足す…", "mock:R9-Scan"),

  // Names and ranges of number fields (spec 6 「範囲の外の値を打った」, the tempo and time signature boxes in spec 3).
  "value.tempo": e("テンポ", "spec:6"),
  "value.tempoRange": s6(
    "Typed a value out of range",
    "30.00〜300.00",
    undefined,
    "part",
  ),
  "value.meterNum": e("拍子の上", "spec:6"),
  "value.meterNumRange": s6("Typed a value out of range", "1〜12", undefined, "part"),
  // The spec 6 text says 「下は」. It is 「拍子の下」 so that the field name reads on its own (a builder decision).
  "value.meterDen": e("拍子の下", "user:2026-11-18"),
  "value.meterDenRange": s6(
    "Typed a value out of range",
    "2 ・ 4 ・ 8 ・ 16",
    undefined,
    "part",
  ),
  "value.velocity": e("ベロシティ", "spec:6"),
  "value.velocityRange": s6(
    "Typed a value out of range",
    "1〜127",
    undefined,
    "part",
  ),
  // Round 12 P3: 「割合」 and 「ばらつき」 of 「ベロシティをまとめて変える」 (the ranges of the round's spec).
  "value.percent": e("割合", "user:2026-11-23"),
  "value.percentRange": e("10〜300", "user:2026-11-23"),
  "value.amount": e("ばらつき", "user:2026-11-23"),
  "value.amountRange": e("1〜32", "user:2026-11-23"),
  // Bar, beat and tick of the position (design-notes item 68, docs/history/design-notes.ja.md). The beat and tick ranges change with the time signature, so they come with the message.
  "value.bar": e("小節", "user:2026-11-18"),
  "value.beat": e("拍", "user:2026-11-18"),
  "value.tick": e("ティック", "user:2026-11-18"),

  // Buttons of spec 6 (text inside 「（…）」 is an explanation, so it is left out)
  "button.playHere": e("ここから鳴らす", "spec:6"),
  "button.openAudioSettings": e("音の設定をひらく", "spec:6"),
  "button.details": e("くわしく", "spec:6"),
  "button.showStopped": e("止まった {count} 個を見る", "spec:6", {
    count: "4",
  }),
  "button.windows": e("窓", "spec:6"),
  "button.clearSearch": e("さがす枠を空にする", "spec:6"),
  "button.switchFormat": e("入れかえる", "spec:6"),
  "button.cancel": e("やめる", "spec:6"),
  // spec 6 says 「トラック（次の番号）を作る」. The form with the number is from the mock.
  "button.makeTrack": e("トラック {number} を作る", "mock:F4-ShelfAudition", {
    number: "5",
  }),
  "button.replaceInstrument": e("入れかえる", "spec:6"),
  "button.insertEffect": e("入れる", "user:2026-11-19"),
  "button.locate": e("場所をさがす", "spec:6"),
  "button.rescan": e("もう一度スキャン", "spec:6"),
  "button.removeFromSong": e("曲から外す", "spec:6"),
  "button.reload": e("もう一度読みこむ", "spec:6"),
  "button.remove": e("外す", "spec:6"),
  "button.retry": e("もう一度ためす", "spec:6"),
  "button.keepRemoved": e("このまま外しておく", "spec:6"),
  "button.restorePosition": e("もとの位置へ戻す", "spec:6"),
  "button.save": e("保存する", "spec:6"),
  "button.dontSave": e("保存しない", "spec:6"),
  "button.chooseLocation": e("場所をえらぶ", "spec:6"),
  "button.openPrevious": e("1 つ前を開く", "spec:6"),
  "button.exportAnyway": e("このまま書き出す", "spec:6"),
  "button.openFolder": e("フォルダをひらく", "spec:6"),
  "button.dontRestore": e("戻さない", "spec:6"),
  "button.openFileLocation": e("ファイルの場所をひらく", "spec:6"),
  "button.close": e("閉じる", "spec:6"),
  "button.addSf2": e("SF2 を足す…", "spec:6"),
  "button.pluginFolders": e("プラグインの場所…", "spec:6"),

  // Key tags (the key name written small at the right end of a band tab or button; the table in spec 4)
  "key.enter": e("Enter", "spec:4"),
  "key.esc": e("Esc", "spec:4"),
  "key.arrange": e("A", "spec:4"),
  "key.pianoRoll": e("E", "spec:4"),
  "key.mixer": e("M", "spec:4"),
  "key.history": e("H", "spec:4"),
  "key.shelf": e("P", "spec:4"),
  "key.params": e("U", "spec:4"),
  "key.record": e("R", "spec:4"),
  "key.playStop": e("Space", "spec:4"),
  "key.metronome": e("C", "spec:4"),
  "key.toStart": e("Home", "spec:4"),
  "key.loop": e("L", "spec:4"),
  "key.quantize": e("Q", "spec:4"),
  "key.toolSelect": e("1", "spec:4"),
  "key.toolDraw": e("2", "spec:4"),
  "key.toolErase": e("3", "spec:4"),
  // Keys of the song name list (Mod is read the same way as in the scale list).
  "key.newSong": e("N", "spec:4"),
  "key.openSong": e("O", "spec:4"),
  "key.saveSong": e("S", "spec:4"),
  "key.undo": e("Z", "spec:4"),
  // How scale keys are written (docs/history/design-notes.ja.md A-2: 「Ctrl Alt 1」 on Linux and Windows, 「Cmd Option 1」 on Mac)
  "key.ctrl": e("Ctrl", "spec:4"),
  "key.cmd": e("Cmd", "spec:4"),
  "key.alt": e("Alt", "spec:4"),
  "key.option": e("Option", "spec:4"),

  // Transport
  "transport.songNew": e("新しい曲", "mock:F-Connect1"),
  "transport.songNewUndo": e(
    "新しい曲 ・ 取り消し {undo} / {total}",
    "mock:F-PREmpty",
    { undo: "0", total: "0" },
  ),
  "transport.songSaved": e(
    "保存済み ・ 取り消し {undo} / {total}",
    "mock:F-WideArrange",
    { undo: "14", total: "14" },
  ),
  "transport.songChanged": e(
    "未保存の変更 ・ 取り消し {undo} / {total}",
    "mock:F-History",
    { undo: "12", total: "14" },
  ),
  // Rows of the list that opens when the song name at the top left is pressed.
  "song.new": s6("Song name list", "新しい曲", undefined, "quoted"),
  "song.open": s6("Song name list", "ひらく…", undefined, "quoted"),
  "song.save": s6("Song name list", "保存", undefined, "quoted"),
  // The ▾ where the song name was hidden (spec 2 "Song ▾"). The aria-label is in the mock's form.
  "transport.songMenu": e("曲（{name}）", "mock:F-Menus", {
    name: "はじめての曲",
  }),
  "transport.songMenuTip": e("曲 ・ {name}", "spec:2", {
    name: "（曲の名前）",
  }),
  "transport.toStart": e("先頭へ戻る", "mock:Main"),
  "transport.play": e("再生", "mock:Main"),
  "transport.stop": e("停止", "mock:Main"),
  "transport.record": e("録音", "mock:Main"),
  "transport.loop": e("ループ", "mock:Main"),
  // The overdub switch (mock P10-Overdub, option A; the owner kept the mock's words).
  "transport.overdub": e("重ねる", "user:2026-11-23"),
  // R1 (round 12): what was played while not recording becomes a clip.
  "transport.keepPlayed": e("さっきのを残す", "user:2026-11-23"),
  "msg.nothingToKeep": e("残せる演奏がありません", "user:2026-11-23"),
  "transport.position": e("小節 . 拍 . ティック", "mock:Main"),
  "transport.time": e("時間", "mock:Main"),
  "transport.tempo": e("テンポ", "mock:Main"),
  "transport.timeSignature": e("拍子", "mock:Main"),
  "transport.metronome": e("メトロノーム", "mock:Main"),
  "transport.countIn": e("カウントイン", "mock:Main"),
  "transport.countInBars": s6(
    "Number of count-in bars",
    "{bars} 小節",
    { bars: "1" },
    "quoted",
  ),
  // The button value and the list row when the count-in is 「なし」.
  "transport.countInNone": s6(
    "Number of count-in bars",
    "なし",
    undefined,
    "quoted",
  ),
  // The metronome ▾ when the count-in is hidden (spec 2 "Metronome ▾").
  "transport.metronomeMenu": e("メトロノームとカウントイン", "mock:F-Menus"),
  // The small window of the metronome ▾ (round 12 R3).
  "metronome.on": e("オン", "user:2026-11-23"),
  "metronome.off": e("オフ", "user:2026-11-23"),
  "metronome.volume": e("音量", "user:2026-11-23"),
  "metronome.sound": e("音", "user:2026-11-23"),
  "metronome.beep": e("ピッ", "user:2026-11-23"),
  "metronome.tick": e("カチ", "user:2026-11-23"),
  "metronome.cowbell": e("カウベル", "user:2026-11-23"),
  "metronome.when": e("鳴らすとき", "user:2026-11-23"),
  "metronome.always": e("いつも", "user:2026-11-23"),
  "metronome.recordingOnly": e("録音のときだけ", "user:2026-11-23"),
  "metronome.tapRow": e("テンポを手拍子で", "user:2026-11-23"),
  "metronome.tap": e("タップ", "user:2026-11-23"),
  "metronome.tapNone": e("—", "user:2026-11-23"),
  "metronome.useTempo": e("この速さにする", "user:2026-11-23"),
  "transport.cpu": e("処理の負荷", "mock:Main"),
  "transport.cpuValue": e("{percent} %", "mock:Main", { percent: "9" }),
  "transport.master": e("マスター", "mock:Main"),
  "common.db": e("{value} dB", "mock:Main", { value: "−4.8" }),
  "common.dbSilent": e("−∞ dB", "mock:Main"),
  "common.dbBare": e("{value}", "spec:3", { value: "−6.2" }),
  "common.dbSilentBare": e("−∞", "spec:3"),
  "transport.masterTip": e("マスター ・ {value}", "spec:3", {
    value: "−6.2 dB",
  }),

  // Input meter
  "meter.input": e("入力", "mock:Main"),
  "meter.noKeyboard": e("鍵盤なし", "mock:F-Connect1"),
  "meter.held": e("いま押している音", "mock:Main"),
  "meter.heldNone": e("— — —", "mock:Main"),
  "meter.velocity": e("ベロシティ", "mock:Main"),
  "meter.sustain": e("サステイン", "mock:Main"),
  "meter.on": e("ON", "mock:F-Connect3"),
  "meter.off": e("OFF", "mock:Main"),
  "meter.pitchBend": e("ピッチベンド", "mock:Main"),
  "meter.modulation": e("モジュレーション", "mock:Main"),
  "meter.playTarget": e("弾くと鳴る先", "mock:Main"),
  "meter.otherKeyboards": e("+{count}", "spec:3", { count: "1" }),
  "meter.playTargetWithInstrument": e(
    "{track} ・ {instrument}",
    "mock:F-Connect3",
    { track: "1 ピアノ", instrument: "グランドピアノ" },
  ),
  // During count-in and recording. 「● テイク {take}」 is rec.
  "meter.playTargetRecording": e("{track} ● テイク {take}", "mock:F-Connect4", {
    track: "1 ピアノ",
    take: "1",
  }),

  // The 「弾くと鳴る先」 (play target) during audition.
  "meter.auditionTarget": e(
    "ためし弾き ・ {instrument}",
    "mock:F4-ShelfAudition",
    { instrument: "シンセベース" },
  ),

  // Tracks and instruments
  "track.label": e("{number} {name}", "mock:F-Connect3", {
    number: "1",
    name: "ピアノ",
  }),
  // Built-in sounds (spec 3 (84)). The part after the key prefix is InstrumentKind of the protocol. In shelf order.
  "instrument.grandPiano": e("グランドピアノ", "spec:3"),
  "instrument.electricPiano": e("エレクトリックピアノ", "spec:3"),
  "instrument.organDrawbar": e("ドローバーオルガン", "spec:3"),
  "instrument.organPercussive": e("パーカッシブオルガン", "spec:3"),
  "instrument.organRock": e("ロックオルガン", "spec:3"),
  "instrument.organ": e("パイプオルガン", "spec:3"),
  "instrument.organQuiet": e("パイプオルガン（やわらかい）", "spec:3"),
  "instrument.organChurch": e("チャーチオルガン", "spec:3"),
  "instrument.organReed": e("リードオルガン", "spec:3"),
  "instrument.vibraphone": e("ビブラフォン", "spec:3"),
  "instrument.marimba": e("マリンバ", "spec:3"),
  "instrument.guitarNylon": e("ナイロンギター", "spec:3"),
  "instrument.guitarSteel": e("スチールギター", "spec:3"),
  "instrument.guitarElectric": e("エレキギター", "spec:3"),
  "instrument.guitarDistortion": e("ひずんだギター", "spec:3"),
  "instrument.fingerBass": e("フィンガーベース", "spec:3"),
  "instrument.pickBass": e("ピックベース", "spec:3"),
  "instrument.woodBass": e("ウッドベース", "spec:3"),
  "instrument.synthBass": e("シンセベース", "spec:3"),
  "instrument.studioKit": e("スタジオキット", "spec:3"),
  "instrument.electroKit": e("エレクトロキット", "spec:3"),
  "instrument.brushKit": e("ブラシキット", "spec:3"),
  "instrument.strings": e("ストリングス", "spec:3"),
  "instrument.pizzicato": e("ピチカート", "spec:3"),
  "instrument.warmPad": e("あたたかいパッド", "spec:3"),
  "instrument.trumpet": e("トランペット", "spec:3"),
  "instrument.brassSection": e("ブラスセクション", "spec:3"),
  "instrument.altoSax": e("アルトサックス", "spec:3"),
  "instrument.flute": e("フルート", "spec:3"),
  "instrument.synthLead": e("シンセリード", "spec:3"),
  "instrument.sine": e("サイン波", "spec:3"),
  // Row names of the drum piano roll (names of the drum pieces; the part after the key prefix is the DRUM_ROWS id).
  "drum.crash": e("クラッシュ", "spec:3"),
  "drum.ride": e("ライド", "spec:3"),
  "drum.openHiHat": e("オープンハイハット", "spec:3"),
  "drum.pedalHiHat": e("ペダルハイハット", "spec:3"),
  "drum.closedHiHat": e("クローズハイハット", "spec:3"),
  "drum.highTom": e("ハイタム", "spec:3"),
  "drum.midTom": e("ミッドタム", "spec:3"),
  "drum.lowTom": e("ロータム", "spec:3"),
  "drum.clap": e("クラップ", "spec:3"),
  "drum.snare": e("スネア", "spec:3"),
  "drum.sideStick": e("サイドスティック", "spec:3"),
  "drum.kick": e("キック", "spec:3"),
  // 「スケール」 in the right panel of a drum track (scale 「—」, lock 「使えない」). 「スケール固定 —」 in the toolbar uses the same 「—」.
  "inspector.scaleNone": e("—", "spec:3"),
  "inspector.lockUnusable": e("使えない", "spec:3"),
  // Shelf headings. The part after the key prefix is InstrumentGroup of the protocol.
  "instrumentGroup.pianoKeys": e("ピアノ ・ 鍵盤", "spec:3"),
  "instrumentGroup.guitar": e("ギター", "spec:3"),
  "instrumentGroup.bass": e("ベース", "spec:3"),
  "instrumentGroup.drums": e("ドラム", "spec:3"),
  "instrumentGroup.padStrings": e("パッド ・ 弦", "spec:3"),
  "instrumentGroup.wind": e("管楽器", "spec:3"),
  "instrumentGroup.synth": e("シンセ", "spec:3"),

  // Shelf (choosing an instrument)
  "shelf.titleNewTrack": e(
    "新しいトラックの音源をえらぶ",
    "mock:F4-ShelfSample",
  ),
  "shelf.titleInsert": e("{strip} にエフェクトを入れる", "user:2026-11-18", {
    strip: "2 ベース",
  }),
  "shelf.titleReplace": e(
    "{track} の音源を入れかえる",
    "mock:F4-ShelfReplace",
    { track: "1 ピアノ" },
  ),
  "shelf.search": e("さがす", "mock:F4-ShelfSample"),
  "shelf.searchLabel": e("名前 ・ 種類", "mock:F4-ShelfSample"),
  "shelf.all": e("すべて", "mock:F4-ShelfSample"),
  "shelf.favorites": e("お気に入り", "mock:F4-ShelfSample"),
  "shelf.keys": e(
    "サンプル {sampleKey} ・ 印 {favoriteKey}",
    "mock:F4-ShelfSample",
    { sampleKey: "Space", favoriteKey: "F" },
  ),
  "shelf.favorite": e("お気に入り", "mock:F4-ShelfSample"),
  "shelf.playSample": e("サンプルを聞く", "mock:F4-ShelfSample"),
  "shelf.sampleTime": e("{at} / {length}", "mock:F4-ShelfSample", {
    at: "0:02",
    length: "0:04",
  }),
  "shelf.current": e("いまの音源", "mock:F4-ShelfReplace"),
  "shelf.auditioning": e("ためし弾き中", "mock:F4-ShelfAudition"),
  "shelf.instrumentCount": e("音源 {count}", "mock:F4-ShelfSample", {
    count: "11",
  }),
  "shelf.effectCount": e("エフェクト {count}", "mock:F4-ShelfSample", {
    count: "6",
  }),

  // P4 (round 12): the shelf's 「パターン」. The part after "pattern." is the id of the pattern (tools/patterns/drums.json).
  "shelf.patterns": e("パターン", "user:2026-11-23"),
  "shelf.playPattern": e("パターンを聞く", "builder:2026-11-23"),
  "shelf.patternBars": e("{bars} 小節", "user:2026-11-23", { bars: "1" }),
  "shelf.placeOn": e("{track} ・ {bar} 小節めに置きます", "user:2026-11-23", { track: "3 スタジオキット", bar: "9" }),
  "shelf.placeOnNewTrack": e("トラック {number}（{instrument}）を作って、{bar} 小節めに置きます", "builder:2026-11-23", {
    number: "4",
    instrument: "スタジオキット",
    bar: "9",
  }),
  "button.placePattern": e("ここに置く", "user:2026-11-23"),
  "pattern.group.basic": e("基本", "user:2026-11-23"),
  "pattern.group.fill": e("フィルイン", "user:2026-11-23"),
  "pattern.eightBeat": e("8 ビート", "user:2026-11-23"),
  "pattern.eightBeatOpen": e("8 ビート（ハイハット開き）", "user:2026-11-23"),
  "pattern.sixteenBeat": e("16 ビート", "user:2026-11-23"),
  "pattern.fourOnTheFloor": e("4 つ打ち", "user:2026-11-23"),
  "pattern.shuffle": e("シャッフル", "user:2026-11-23"),
  "pattern.halfTime": e("ハーフタイム", "user:2026-11-23"),
  "pattern.rideEightBeat": e("ライドで 8 ビート", "user:2026-11-23"),
  "pattern.ballad": e("バラード", "user:2026-11-23"),
  "pattern.snareRoll": e("スネアの連打", "user:2026-11-23"),
  "pattern.tomFill": e("タムまわし", "user:2026-11-23"),
  "pattern.crashStop": e("クラッシュで止める", "user:2026-11-23"),
  "pattern.smallFill": e("小さなフィル", "user:2026-11-23"),

  // Built-in effects (spec 3 (Y8)). The part after the key prefix is EffectKind of the protocol.
  "effect.eq4": e("EQ 4 バンド", "spec:3"),
  "effect.compressor": e("コンプレッサー", "spec:3"),
  "effect.reverb": e("リバーブ", "spec:3"),
  "effect.delay": e("ディレイ", "spec:3"),
  "effect.chorus": e("コーラス", "spec:3"),
  "effect.limiter": e("リミッター", "spec:3"),
  // Round 12 M6 (the table of the round's spec).
  "effect.gate": e("ゲート", "user:2026-11-23"),
  "effect.deEsser": e("ディエッサー", "user:2026-11-23"),
  "effect.saturation": e("サチュレーション", "user:2026-11-23"),
  // Parameter names. The key is param.<EffectKind>.<EFFECT_PARAMS id>. Reverb and delay are from spec 3,
  // the other 4 from the table in docs/architecture.md 19-3 (the owner's decision).
  "param.reverb.decay": e("響きの長さ", "spec:3"),
  "param.reverb.size": e("部屋の大きさ", "spec:3"),
  "param.reverb.preDelay": e("プリディレイ", "spec:3"),
  "param.reverb.highCut": e("高い音を落とす", "spec:3"),
  "param.reverb.lowCut": e("低い音を落とす", "spec:3"),
  "param.reverb.width": e("広がり", "spec:3"),
  "param.reverb.mix": e("ミックス", "spec:3"),
  // 「間隔」 is one row. With 「テンポに合わせる」 on, it is a step (intervalStep); off, milliseconds (intervalMs).
  "param.delay.intervalStep": e("間隔", "spec:3"),
  "param.delay.intervalMs": e("間隔", "spec:3"),
  "param.delay.sync": e("テンポに合わせる", "spec:3"),
  "param.delay.feedback": e("くり返し", "spec:3"),
  "param.delay.highCut": e("高い音を落とす", "spec:3"),
  "param.delay.lowCut": e("低い音を落とす", "spec:3"),
  "param.delay.pingPong": e("左右にゆれる", "spec:3"),
  "param.delay.mix": e("ミックス", "spec:3"),
  // Steps of 「間隔」. The part after the key prefix is steps of EFFECT_PARAMS.
  "param.delay.step.quarter": e("1/4", "spec:3"),
  "param.delay.step.eighthDotted": e("1/8 付点", "spec:3"),
  "param.delay.step.eighth": e("1/8", "spec:3"),
  "param.delay.step.eighthTriplet": e("1/8 三連符", "spec:3"),
  "param.delay.step.sixteenth": e("1/16", "spec:3"),
  // EQ headings (group of EFFECT_PARAMS) and the rows under them.
  "param.eq4.group.low": e("低い音", "user:2026-11-18"),
  "param.eq4.group.lowMid": e("中の低い音", "user:2026-11-18"),
  "param.eq4.group.highMid": e("中の高い音", "user:2026-11-18"),
  "param.eq4.group.high": e("高い音", "user:2026-11-18"),
  "param.eq4.lowFreq": e("高さ", "user:2026-11-18"),
  "param.eq4.lowGain": e("上げ下げ", "user:2026-11-18"),
  "param.eq4.lowMidFreq": e("高さ", "user:2026-11-18"),
  "param.eq4.lowMidGain": e("上げ下げ", "user:2026-11-18"),
  "param.eq4.lowMidWidth": e("幅", "user:2026-11-18"),
  "param.eq4.highMidFreq": e("高さ", "user:2026-11-18"),
  "param.eq4.highMidGain": e("上げ下げ", "user:2026-11-18"),
  "param.eq4.highMidWidth": e("幅", "user:2026-11-18"),
  "param.eq4.highFreq": e("高さ", "user:2026-11-18"),
  "param.eq4.highGain": e("上げ下げ", "user:2026-11-18"),
  "param.compressor.threshold": e("おさえはじめ", "user:2026-11-18"),
  "param.compressor.ratio": e("おさえる強さ", "user:2026-11-18"),
  "param.compressor.attack": e("おさえるまで", "user:2026-11-18"),
  "param.compressor.release": e("もどるまで", "user:2026-11-18"),
  "param.compressor.makeup": e("持ち上げ", "user:2026-11-18"),
  "param.chorus.rate": e("ゆれの速さ", "user:2026-11-18"),
  "param.chorus.depth": e("ゆれの深さ", "user:2026-11-18"),
  "param.chorus.width": e("広がり", "user:2026-11-18"),
  "param.chorus.mix": e("ミックス", "user:2026-11-18"),
  "param.limiter.ceiling": e("上限", "user:2026-11-18"),
  "param.limiter.gain": e("持ち上げ", "user:2026-11-18"),
  "param.limiter.release": e("もどるまで", "user:2026-11-18"),
  "param.gate.threshold": e("しきい値", "user:2026-11-23"),
  "param.gate.range": e("下げる量", "user:2026-11-23"),
  "param.gate.release": e("戻る速さ", "user:2026-11-23"),
  "param.deEsser.frequency": e("周波数", "user:2026-11-23"),
  "param.deEsser.threshold": e("しきい値", "user:2026-11-23"),
  "param.deEsser.reduction": e("下げる量", "user:2026-11-23"),
  "param.saturation.drive": e("ひずみ", "user:2026-11-23"),
  "param.saturation.tone": e("明るさ", "user:2026-11-23"),
  "param.saturation.mix": e("まぜる量", "user:2026-11-23"),
  "param.saturation.output": e("出る大きさ", "user:2026-11-23"),
  // 「プリセット ▾」 at the head of the parameter list of a built-in effect (round 12 M1). preset.named: the name of the preset whose values the insert has.
  "preset.button": e("プリセット ▾", "user:2026-11-23"),
  "preset.named": e("{name} ▾", "builder:2026-11-24", { name: "歌をそろえる" }),
  "preset.factory": e("はじめから入っているもの", "user:2026-11-23"),
  "preset.user": e("自分で残したもの", "user:2026-11-23"),
  "preset.save": e("いまの設定を残す…", "user:2026-11-23"),
  "preset.delete": e("消す", "user:2026-11-23"),
  "preset.eq4.vocalClear": e("歌をくっきり", "user:2026-11-23"),
  "preset.eq4.unmuddy": e("こもりを取る", "user:2026-11-23"),
  "preset.eq4.cleanLows": e("低い音をすっきり", "user:2026-11-23"),
  "preset.eq4.fatBass": e("ベースを太く", "user:2026-11-23"),
  "preset.compressor.vocalEven": e("歌をそろえる", "user:2026-11-23"),
  "preset.compressor.drumPunch": e("ドラムをはっきり", "user:2026-11-23"),
  "preset.compressor.bassSolid": e("ベースをどっしり", "user:2026-11-23"),
  "preset.compressor.gentleGlue": e("やさしくまとめる", "user:2026-11-23"),
  "preset.reverb.smallRoom": e("小さな部屋", "user:2026-11-23"),
  "preset.reverb.hall": e("ホール", "user:2026-11-23"),
  "preset.reverb.behindVocal": e("歌のうしろに", "user:2026-11-23"),
  "preset.delay.echo": e("やまびこ", "user:2026-11-23"),
  "preset.delay.slapback": e("短いはね返り", "user:2026-11-23"),
  "preset.delay.pingPong": e("左右にとぶ", "user:2026-11-23"),
  "preset.chorus.subtleWide": e("うすく広げる", "user:2026-11-23"),
  "preset.chorus.deepSwirl": e("深くゆらす", "user:2026-11-23"),
  "preset.limiter.finish": e("仕上げ", "user:2026-11-23"),
  "preset.limiter.gentle": e("やさしく止める", "user:2026-11-23"),
  "preset.gate.noiseOff": e("雑音を消す", "builder:2026-11-24"),
  "preset.gate.shortTail": e("余韻を短く切る", "builder:2026-11-24"),
  "preset.deEsser.softS": e("サ行をやわらかく", "builder:2026-11-24"),
  "preset.deEsser.firmS": e("サ行をしっかりおさえる", "builder:2026-11-24"),
  "preset.saturation.warm": e("あたたかく", "builder:2026-11-24"),
  "preset.saturation.rough": e("荒くひずませる", "builder:2026-11-24"),
  "msg.presetOverwritten": e("「{name}」を上書きしました", "builder:2026-11-24", { name: "いつものピアノ" }),
  "msg.presetLanesKept": e("レーンが動かしている {count} つのパラメータは、そのままにしました", "builder:2026-11-24", { count: "2" }),
  "msg.tooManyPresets": e("{effect}のプリセットは {max} 個までなので、これ以上残せません。", "builder:2026-11-24", { effect: "コンプレッサー", max: "100" }),
  // Round 12 E3: the knobs of a built-in sound (the round's spec).
  "knob.brightness": e("明るさ", "user:2026-11-23"),
  "knob.attack": e("立ち上がり", "user:2026-11-23"),
  "knob.release": e("のび", "user:2026-11-23"),
  "knob.vibrato": e("ゆれ", "user:2026-11-23"),
  // How values are written (number and unit; the examples in the spec 3 rule on parameter names). The part after the key prefix is unit of EFFECT_PARAMS.
  "param.value.s": e("{value} s", "spec:3", { value: "2.40" }),
  "param.value.ms": e("{value} ms", "spec:3", { value: "20" }),
  "param.value.db": e("{value} dB", "spec:3", { value: "−6.0" }),
  "param.value.percent": e("{value} %", "spec:3", { value: "35" }),
  "param.value.hz": e("{value} Hz", "spec:3", { value: "6000" }),
  "param.value.ratio": e("{value} : 1", "user:2026-11-18", { value: "4.0" }),
  "param.value.oct": e("{value} oct", "user:2026-11-18", { value: "1.0" }),
  "param.on": e("オン", "mock:R6-Delay"),
  "param.off": e("オフ", "mock:R6-Delay"),

  // Params panel (built-in effects)
  "params.builtin": e("最初から入っている", "mock:F4-Mixer"),
  // When a built-in sound or an SF2 sound is pressed on the left (spec 3 (92-1, 19-A)).
  "params.noSoundParams": e("この音色には、動かせるパラメータがありません。", "spec:3"),
  "params.noSoundParamsHint": e("音量とパンはミキサーで、音色を変えるには入れかえます。", "spec:3"),
  "msg.noSoundParams": s6("No parameters to move", "{name}には、動かせるパラメータがありません", { name: "グランドピアノ" }),
  "button.replaceSound": e("音源を入れかえる…", "spec:6"),
  "params.where": e("{strip} ・ インサート {slot}", "mock:F4-Mixer", {
    strip: "A リバーブ",
    slot: "1",
  }),
  "params.search": e("さがす", "mock:F4-Mixer"),
  "params.searchLabel": e("名前", "mock:F4-Mixer"),
  "params.count": e("{shown} / {total}", "mock:F4-Mixer", {
    shown: "7",
    total: "7",
  }),
  "params.automation": e("オートメーションに書く", "mock:F4-Mixer"),
  "params.tabCount": e("{name} {count}", "mock:F-WindowOpen", {
    name: "Surge XT",
    count: "2855",
  }),
  "status.paramSelected": e(
    "{strip}の「{param}」をえらんでいます",
    "mock:F4-Mixer",
    { strip: "A リバーブ", param: "響きの長さ" },
  ),

  // Mixer
  "mixer.flow": e(
    "音は上から下へ流れます: 音源 → インサート → センド → パン → フェーダー → マスター",
    "mock:F4-Mixer",
  ),
  "mixer.meterUnit": e("メーターの単位", "mock:F4-Mixer"),
  "mixer.instrument": e("音源 ・ {name}", "mock:F4-Mixer", {
    name: "グランドピアノ",
  }),
  "mixer.inserts": e("インサート", "mock:F4-Mixer"),
  "mixer.emptySlot": e("空き", "mock:F4-Mixer"),
  "mixer.emptySlotAdd": e("＋", "mock:F4-Mixer"),
  "mixer.sends": e("センド", "mock:F4-Mixer"),
  "mixer.sendA": e("A", "mock:F4-Mixer"),
  "mixer.sendB": e("B", "mock:F4-Mixer"),
  "mixer.pan": e("パン", "mock:F4-Mixer"),
  "mixer.panCenter": e("C", "mock:F4-Mixer"),
  "mixer.panLeft": e("L {value}", "mock:F4-Mixer", { value: "20" }),
  "mixer.panRight": e("R {value}", "mock:F4-Mixer", { value: "20" }),
  "mixer.removeEffect": e("外す", "user:2026-11-18"),
  "mixer.returnName": e("{bus} {effect}", "user:2026-11-18", {
    bus: "A",
    effect: "リバーブ",
  }),
  "mixer.returnOf": e("センド {bus} の戻り", "mock:F4-Mixer", { bus: "A" }),
  "mixer.master": e("マスター", "mock:F4-Mixer"),
  "mixer.output": e("出口 ・ {name}", "mock:F4-Mixer", { name: "ヘッドホン" }),
  "mixer.db": e("dB", "mock:F4-Mixer"),

  // Arrange
  "arrange.title": e("アレンジ", "mock:F-Empty"),
  "arrange.grid": e("グリッド", "mock:F-Empty"),
  "arrange.marker": e("マーカー", "mock:F-Empty"),
  "arrange.tempoChanges": e("テンポ ・ 拍子の変更", "mock:F-Empty"),
  "arrange.changeCount": e("{count} 件", "mock:F-Empty", { count: "0" }),
  "arrange.mute": e("ミュート", "mock:F-Empty"),
  "arrange.solo": e("ソロ", "mock:F-Empty"),
  "arrange.arm": e("録音待機", "mock:F-Empty"),
  "arrange.muteShort": e("M", "mock:F-Empty"),
  "arrange.soloShort": e("S", "mock:F-Empty"),
  "arrange.armShort": e("●", "mock:F-Empty"),
  // The 「帯」 button in the heading (spec 2) and the status line while its list is open (mock F8-FirstLane).
  "arrange.laneShort": e("帯", "spec:2"),
  "status.laneAddOpen": e("{track} にオートメーションの帯を足します ・ えらぶと、帯がひらきます", "mock:F8-FirstLane", { track: "2 ベース" }),
  "arrange.addTrackKinds": e("音源 ・ 歌", "mock:F-Empty"),
  // The instrument name in the track head while auditioning a replacement.
  "arrange.instrumentAuditioned": e(
    "{instrument}（ためし弾き）",
    "mock:F4-ShelfReplace",
    { instrument: "エレクトリックピアノ" },
  ),
  "arrange.tempoAndSignature": e("{tempo} ・ {signature}", "mock:F-Empty", {
    tempo: "96.00",
    signature: "4/4",
  }),
  "arrange.emptyTitle": e("ここに、録った音がならびます。", "mock:F-Empty"),
  "arrange.emptyBody": e(
    "鍵盤を弾いて音を確かめたら、赤い丸の録音ボタンへ。",
    "mock:F-Empty",
  ),
  "arrange.emptyAddTrack": e(
    "トラックを増やすには左下の「＋ トラックを追加」",
    "mock:F-Empty",
  ),
  "arrange.overview": e("アレンジ 全体", "mock:A-PianoRoll"),
  "arrange.markerLoop": e("マーカー ・ ループ {range}", "mock:Main", {
    range: "5–9",
  }),
  "arrange.recordingClip": e("● 録音中 テイク {take}", "mock:F-ArrangeRec", {
    take: "2",
  }),
  // While overdubbing (mock P10-Overdub). The status line is the mock's sentence cut at 「 ・ 」 into 3; without a loop the 2nd is left out.
  "arrange.overdubClip": e("● 重ねて録音中 {clip}", "user:2026-11-23", { clip: "イントロ" }),
  "msg.overdubRecording": e("{track} の {clip} に重ねて録音", "user:2026-11-23", { track: "1 ピアノ", clip: "イントロ" }),
  "msg.overdubLap": e("ループ {from}–{to} の {lap} 周目", "user:2026-11-23", { from: "1", to: "4", lap: "2" }),
  "msg.overdubKept": e("止めても {clip} に足したまま（取り消しで戻せます）", "user:2026-11-23", { clip: "イントロ" }),
  "arrange.takeTag": e("{take}/{takes}", "mock:F-Takes", {
    take: "2",
    takes: "3",
  }),
  "arrange.takeSwitch": e("テイク {take} / {takes}", "mock:F-Takes", {
    take: "2",
    takes: "3",
  }),
  "arrange.takePrev": e("前のテイク", "mock:F-Takes"),
  "arrange.takeNext": e("次のテイク", "mock:F-Takes"),
  "arrange.takeKey": e("T", "mock:F-Takes"),
  // The live reading while dragging a clip (spec 2).
  "arrange.readMove": e("{position}（{delta} 小節）", "mock:R3-Arrange", {
    position: "6.1.000",
    delta: "+1",
  }),
  "arrange.readCopy": e("複製 ・ {position}", "mock:R3-Arrange", {
    position: "9.1.000",
  }),
  "arrange.readTrim": e("長さ {length}", "mock:R3-Arrange", {
    length: "2.2.000",
  }),
  "arrange.soundCameOutKeys": e(
    "録音 {recordKey} ・ 再生と停止 {playKey}",
    "mock:F-Connect3",
    { recordKey: "R", playKey: "Space" },
  ),

  // Output chooser (flow 2) and the output and input rows
  "connect.changeLater": e(
    "あとから「音の設定」で変えられます",
    "mock:F-Connect2",
  ),
  "connect.help": e(
    "↑ ↓ でえらぶ ・ ▶ でテスト音。何かささっている出口を、上にならべています",
    "mock:F-Connect2",
  ),
  "endpoint.testTone": e("テスト音を鳴らす", "mock:F-Connect2"),
  "endpoint.plugged": s6(
    "Text on the output and input rows",
    "ささっています",
    undefined,
    "quoted",
  ),
  "endpoint.connected": s6(
    "Text on the output and input rows",
    "つながっています",
    undefined,
    "quoted",
  ),
  "endpoint.nothingPlugged": s6(
    "Text on the output and input rows",
    "何もささっていません",
    undefined,
    "quoted",
  ),
  "friendly.headphones": s6(
    "Text on the output and input rows",
    "ヘッドホンの穴",
    undefined,
    "quoted",
  ),
  "friendly.hdmi": s6(
    "Text on the output and input rows",
    "画面のスピーカー",
    undefined,
    "quoted",
  ),
  "friendly.lineOut": s6(
    "Text on the output and input rows",
    "うしろのスピーカーの穴",
    undefined,
    "quoted",
  ),
  "friendly.rearMic": s6(
    "Text on the output and input rows",
    "うしろのマイクの穴",
    undefined,
    "quoted",
  ),
  "friendly.keyboard": e("この鍵盤", "mock:F-Settings"),

  // Band tab row and vertical headings
  "band.arrange": e("アレンジ", "mock:Main"),
  "band.trackCount": e("トラック {count}", "mock:Main", { count: "5" }),
  "band.pianoRoll": e("ピアノロール", "mock:Main"),
  "band.noteCount": e("ノート {count}", "mock:Main", { count: "23" }),
  "band.mixer": e("ミキサー", "mock:Main"),
  "band.masterDb": e("マスター {db}", "mock:Main", { db: "0.0 dB" }),
  "band.history": e("取り消しの履歴", "mock:Main"),
  // The 「12 / 14」 form from the band table in spec 3 is used whether or not steps are undone (not 「14 手」 of mock Main; the owner's decision).
  "band.stepCount": e("{undo} / {total}", "spec:3", {
    undo: "12",
    total: "14",
  }),
  "band.shelf": e("棚", "mock:F-ParamFew"),
  "band.shelfCount": e(
    "音源 {instruments} ・ エフェクト {effects}",
    "mock:F-ParamFew",
    { instruments: "9", effects: "12" },
  ),
  "band.params": e("パラメータ", "mock:F-ShelfArrange"),

  // Status line
  "status.stopped": s6("Status line labels", "■ 停止中", undefined, "quoted"),
  "status.playing": s6("Status line labels", "▶ 再生中", undefined, "quoted"),
  "status.recording": s6("Status line labels", "● 録音中", undefined, "quoted"),
  "status.countingIn": s6(
    "Status line labels",
    "● カウントイン中",
    undefined,
    "quoted",
  ),
  "status.auditioning": e("♪ ためし弾き中", "mock:F4-ShelfAudition"),
  // When another track is chosen with ●.
  "status.armMoved": e(
    "{recordKey} を押すと {track} に録ります",
    "mock:R7-Arrange6",
    { recordKey: "R", track: "2 ベース" },
  ),
  // The left text when stopped. Only the parts present, joined with 「 ・ 」. With both, status.loopNoSelection.
  "status.loopNoSelection": s6(
    "Left of the status line while stopped",
    "ループ {from}–{to} ・ 選択なし",
    { from: "5", to: "9" },
    "quoted",
  ),
  "status.loop": e("ループ {from}–{to}", "mock:Main", { from: "5", to: "9" }),
  "status.noSelection": e("選択なし", "mock:Main"),
  // Muted tracks. With two or more, the names are listed with 「 ・ 」.
  "status.muted": s6(
    "Muted",
    "{tracks} はミュート",
    { tracks: "4 パッド" },
    "part",
  ),
  "status.mutedMany": s6(
    "Muted",
    "{tracks} はミュート",
    { tracks: "4 パッド ・ 5 ベース" },
    "quoted",
  ),
  // When the shelf opens and when a heading opens or folds (spec 6 「棚をひらいている（見出し）」). groups are the open headings.
  // With 2, 3 (or 4) it is status.shelfGroupsLast (the earlier ones joined with LIST_COMMA); with 5 or more, status.shelfGroupsMore.
  "status.shelfOpen": s6("Shelf open (headings)", "音色 {count} ・ {groups} をひらいています", { count: "31", groups: "ピアノ ・ 鍵盤 と ギター" }, "part"),
  "status.shelfCount": s6("Shelf open (headings)", "音色 {count}", { count: "31" }, "part"),
  "status.shelfGroupsLast": s6("Shelf open (headings)", "{first} と {last}", { first: "ピアノ ・ 鍵盤、ギター", last: "ベース" }, "part"),
  "status.shelfGroupsMore": s6("Shelf open (headings)", "{first} ほか {count} つ", { first: "ピアノ ・ 鍵盤", count: "4" }, "part"),
  // The 「出る場所」 column of spec 6 「棚をひらいている（見出し）」 (the effect list).
  "status.shelfEffectsOpen": e("エフェクト {count} ・ {groups} をひらいています", "spec:6", { count: "512", groups: "リバーブ" }),
  "status.output": e("出口: {output}", "mock:Main", { output: "ヘッドホン" }),
  "status.outputNone": e("出口: まだ", "spec:3"),
  "status.inputs": e("入口: {inputs}", "mock:Main", {
    inputs: "鍵盤 49 ・ マイク",
  }),
  "status.inputsNone": e("入口: まだ", "spec:3"),
  "status.buffer": e("バッファ {actual}", "mock:Main", { actual: "256" }),
  "status.bufferMismatch": e(
    "バッファ {actual} ・ 頼んだ数 {requested}",
    "spec:3",
    { actual: "1024", requested: "256" },
  ),
  "status.windows": e("窓 {count}", "spec:6", { count: "2" }),
  "status.scale": e("{scale} %", "mock:F-Zoom", { scale: "125" }),
  "zoom.heading": e("拡大率", "mock:F-Zoom"),
  "status.searchingKeyboard": e("鍵盤をさがしています…", "mock:F-Connect1"),
  "status.outputChosen": e(
    "出口を{output}にしました。{track} が録音待機になっています。",
    "mock:F-Connect3",
    { output: "アナログヘッドフォン", track: "1 ピアノ" },
  ),
  "status.newSong": e(
    "新しい曲 ・ テンポ {tempo} ・ {signature} ・ {track} が録音待機",
    "mock:F-Empty",
    { tempo: "96.00", signature: "4/4", track: "1 ピアノ" },
  ),
  // With no armed track (track is null), only the first parts of the two texts above.
  "status.outputChosenOnly": e(
    "出口を{output}にしました。",
    "mock:F-Connect3",
    { output: "アナログヘッドフォン" },
  ),
  "status.newSongOnly": e(
    "新しい曲 ・ テンポ {tempo} ・ {signature}",
    "mock:F-Empty",
    { tempo: "96.00", signature: "4/4" },
  ),
  "status.settingsOpen": e("音の設定をひらいています", "mock:F-Settings"),

  // Sound settings
  "settings.title": e("音の出口と入口", "mock:F-Settings"),
  "settings.close": e("閉じる", "mock:F-Settings"),
  "settings.outputs": e("音の出口", "mock:F-Settings"),
  "settings.voiceInput": e("歌の入口", "mock:F-Settings"),
  "settings.notUsed": e("使わない", "mock:F-Settings"),
  "settings.level": e("いまの大きさ", "mock:F-Settings"),
  "settings.keyboard": e("鍵盤", "mock:F-Settings"),
  "settings.keysLight": e("鍵を押すと光ります", "mock:F-Settings"),
  "settings.latency": e("遅れ", "mock:F-Settings"),
  "settings.keyToSound": e("鍵を押してから鳴るまで", "mock:F-Settings"),
  "settings.aboutMs": e("約 {ms} ms", "mock:F-Settings", { ms: "13.5" }),
  "settings.latencyHint": e("小さいほど、弾いた感じが自然", "mock:F-Settings"),
  "settings.recordedPosition": e("録った音の位置", "mock:F-Settings"),
  "settings.autoFixOn": e("自動で直す ・ オン", "mock:F-Settings"),
  "settings.autoFixHint": e("遅れのぶんだけ前へ戻して置く", "mock:F-Settings"),
  "settings.advanced": e("くわしい設定", "mock:F-Settings"),
  "settings.advancedSummary": e(
    "{system} ・ {rate} Hz ・ バッファ {buffer}",
    "mock:F-Settings",
    { system: "PipeWire", rate: "48000", buffer: "256" },
  ),
  "settings.advancedHint": e("ふだんは開かなくて大丈夫", "mock:F-Settings"),
  "settings.ok": e("これで決まり", "mock:F-Settings"),
  "settings.sampleRate": e("サンプルレート", "mock:F-SettingsAdvFixed"),

  // Around the piano roll: horizontal zoom of the arrange, note list, selected notes, status line
  "zoom.out": e("横に縮小", "mock:R3-Arrange"),
  "zoom.in": e("横に拡大", "mock:R3-Arrange"),
  "zoom.outMark": e("−", "mock:R3-Arrange"),
  "zoom.inMark": e("＋", "mock:R3-Arrange"),
  "grid.bar": e("1 小節", "mock:R3-Arrange"),
  "grid.division": e("1/{division}", "mock:A-PianoRoll", { division: "16" }),
  "grid.triplet": e("三連符", "mock:A-PianoRoll"),
  "note.position": e("位置", "mock:A-PianoRoll"),
  "note.key": e("音", "mock:A-PianoRoll"),
  "note.length": e("長さ", "mock:A-PianoRoll"),
  "noteList.velocity": e("Vel", "mock:A-PianoRoll"),
  "inspector.from": s6(
    "Reading of the selected notes",
    "{position} 〜",
    { position: "3.1.000" },
    "quoted",
  ),
  "inspector.mixed": s6(
    "Reading of the selected notes",
    "ばらばら",
    undefined,
    "quoted",
  ),
  "inspector.range": s6(
    "Reading of the selected notes",
    "{low} – {high}",
    { low: "77", high: "83" },
    "quoted",
  ),
  "inspector.quantize": e("クオンタイズ", "mock:A-PianoRoll"),
  "inspector.quantized": e("かけてある", "mock:A-PianoRoll"),
  "inspector.strength": e("強さ", "mock:A-PianoRoll"),
  "inspector.swing": e("スウィング", "mock:A-PianoRoll"),
  "inspector.percent": e("{value} %", "mock:A-PianoRoll", { value: "75" }),
  "inspector.align": e("そろえる所", "mock:A-PianoRoll"),
  "inspector.alignStart": e("はじまり", "mock:R5-Lists"),
  "inspector.alignStartAndEnd": e("はじまりと終わり", "user:2026-11-15"),
  "inspector.quantizeApply": e("かける", "spec:3"),
  "inspector.requantize": e("かけなおす", "spec:3"),
  "inspector.unquantize": e("もとの位置へ戻す", "mock:A-PianoRoll"),
  "inspector.scale": e("スケール", "mock:A-PianoRoll"),
  "inspector.musicalScale": e("音階", "mock:A-PianoRoll"),
  "inspector.lock": e("固定", "mock:A-PianoRoll"),
  "inspector.highlight": e("色をつける", "user:2026-11-23"),
  "inspector.on": e("オン", "mock:A-PianoRoll"),
  "inspector.off": s6("Note outside the scale (reading)", "オフ", undefined, "quoted"),
  "inspector.sameLength": e("長さをそろえる", "mock:A-PianoRoll"),
  "inspector.velocityAll": e("ベロシティをまとめて変える", "mock:A-PianoRoll"),
  "scale.name": e("{root} {mode}", "mock:A-PianoRoll", {
    root: "C",
    mode: "メジャー",
  }),
  "scale.major": e("メジャー", "mock:A-PianoRoll"),
  "scale.minor": e("マイナー", "user:2026-11-15"),
  "scale.harmonicMinor": e("ハーモニックマイナー", "user:2026-11-23"),
  "scale.melodicMinor": e("メロディックマイナー", "user:2026-11-23"),
  "scale.majorPentatonic": e("メジャーペンタトニック", "user:2026-11-23"),
  "scale.minorPentatonic": e("マイナーペンタトニック", "user:2026-11-23"),
  "scale.blues": e("ブルース", "user:2026-11-23"),
  "scale.dorian": e("ドリアン", "user:2026-11-23"),
  "scale.mixolydian": e("ミクソリディアン", "user:2026-11-23"),
  "scale.kind": e("種類", "user:2026-11-15"),
  "scale.root": e("主音", "user:2026-11-15"),
  // Status line. Selected notes: the count and the first 4 note names (「…」 added for 5 or more). For a quantized take, 「 ・ クオンタイズ済み…」 after it.
  "status.notesSelected": s6(
    "Notes selected",
    "選択 {count} ノート ・ {keys}",
    { count: "3", keys: "D4 E4 F4" },
    "part",
  ),
  "status.notesSelectedMore": s6(
    "Notes selected",
    "選択 {count} ノート ・ {keys} …",
    { count: "7", keys: "C3 E3 G3 C4" },
    "quoted",
  ),
  "status.notesQuantized": e(
    "クオンタイズ済み（もとの位置へ戻せます）",
    "mock:A-PianoRoll",
  ),
  // While playing with the piano roll open. The take is arrange.takeSwitch. With no sounding notes, that part is not shown.
  "status.pianoRollClip": s6(
    "Playing, piano roll",
    "{track} ・ {clip}",
    { track: "1 ピアノ", clip: "Aメロ" },
    "part",
  ),
  "status.sounding": s6(
    "Playing, piano roll",
    "鳴っている音 {keys}",
    { keys: "C3 E3 G3 C4" },
    "part",
  ),
  // While recording with the piano roll open. With no held keys, 「 ・ 押している音 …」 is not shown.
  "msg.recordingPianoRoll": s6(
    "Recording, piano roll",
    "{track} へ録音 ・ テイク {take}",
    { track: "1 ピアノ", take: "3" },
    "part",
  ),
  "status.held": s6(
    "Recording, piano roll",
    "押している音 {keys}",
    { keys: "C3 E3 G3 C4" },
    "part",
  ),
  // When the piano roll is open where there is no clip (the left side when stopped).
  "status.noClip": e("{track} ・ クリップなし", "mock:F-PREmpty", {
    track: "1 ピアノ",
  }),
  "msg.outOfScale": s6(
    "Tried to write outside the scale",
    "{key} は {scale}の外の音です（スケール固定 ・ オン）",
    { key: "D#", scale: "C メジャー" },
  ),
  // The piano roll band: toolbar, headings of the left and right panels, velocity and CC tags, the live reading.
  "pr.clip": e(
    "{number} {track} ・ {clip} ・ 小節 {bars} ・ ノート {count}",
    "mock:F-PRPlay",
    { number: "1", track: "ピアノ", clip: "Aメロ", bars: "1–4", count: "23" },
  ),
  "pr.clipSelected": e(
    "{number} {track} ・ {clip} ・ 小節 {bars} ・ ノート {count}（選択 {selected}）",
    "mock:A-PianoRoll",
    {
      number: "1",
      track: "ピアノ",
      clip: "イントロ",
      bars: "1–4",
      count: "23",
      selected: "3",
    },
  ),
  "pr.clipRecording": e(
    "{number} {track} ・ {clip} ・ 録音中 ・ ノート {count}",
    "mock:F-PRRec",
    { number: "1", track: "ピアノ", clip: "Aメロ", count: "22" },
  ),
  "pr.recordingDot": e("●", "mock:F-PRRec"),
  "pr.noClip": e("{number} {track} ・ クリップなし", "spec:3", {
    number: "1",
    track: "ピアノ",
  }),
  // The hint where there is no clip (top left of the note area). {key} is the key of the draw tool (key.toolDraw).
  "pr.emptyTitle": s6(
    "Piano roll where there is no clip",
    "ここにはクリップがありません。",
    undefined,
    "quoted",
  ),
  "pr.emptyBody": s6(
    "Piano roll where there is no clip",
    "書く道具（{key}）で押すと、そこから 4 小節のクリップを作って書きます。鍵盤で弾いて録っても作られます。",
    { key: "2" },
    "quoted",
  ),
  "pr.toolSelect": e("選ぶ", "mock:A-PianoRoll"),
  "pr.toolDraw": e("書く", "mock:A-PianoRoll"),
  "pr.toolErase": e("消す", "mock:A-PianoRoll"),
  "pr.grid": e("グリッド", "mock:A-PianoRoll"),
  "pr.scaleLock": e("スケール固定", "mock:A-PianoRoll"),
  "pr.scaleHighlight": e("色", "user:2026-11-23"),
  // Chord help: the words of docs/design/proposals/P10-ChordStamp and P10-ChordTable (option A), as the mocks have them.
  "pr.chordShapes": e("和音の形", "user:2026-11-23"),
  "pr.chordRootC": e("根音が C のとき", "user:2026-11-23"),
  "pr.chordSingle": e("1 音（ふだん）", "user:2026-11-23"),
  "pr.chordMajor": e("メジャー", "user:2026-11-23"),
  "pr.chordMinor": e("マイナー", "user:2026-11-23"),
  "pr.chordSeventh": e("7", "user:2026-11-23"),
  "pr.chordMajorSeventh": e("M7", "user:2026-11-23"),
  "pr.chordMinorSeventh": e("m7", "user:2026-11-23"),
  "pr.chordSus4": e("sus4", "user:2026-11-23"),
  "pr.chordDim": e("dim", "user:2026-11-23"),
  "pr.chordAug": e("aug", "user:2026-11-23"),
  "pr.chordOnCSingle": e("C", "user:2026-11-23"),
  "pr.chordOnCMajor": e("C E G", "user:2026-11-23"),
  "pr.chordOnCMinor": e("C E♭ G", "user:2026-11-23"),
  "pr.chordOnCSeventh": e("C E G B♭", "user:2026-11-23"),
  "pr.chordOnCMajorSeventh": e("C E G B", "user:2026-11-23"),
  "pr.chordOnCMinorSeventh": e("C E♭ G B♭", "user:2026-11-23"),
  "pr.chordOnCSus4": e("C F G", "user:2026-11-23"),
  "pr.chordOnCDim": e("C E♭ G♭", "user:2026-11-23"),
  "pr.chordOnCAug": e("C E G♯", "user:2026-11-23"),
  "pr.chordStatus": e("書く ・ {shape} ・ 根音 {root}（{notes}）", "user:2026-11-23", {
    shape: "メジャー",
    root: "C4",
    notes: "C4 E4 G4",
  }),
  "pr.keyChords": e("調の和音", "user:2026-11-23"),
  "pr.keyChordsOff": e("スケール固定をオンにすると、その調の和音がここにならびます。", "spec:3"),
  "pr.keyChordStatus": e("書く ・ {chord}（{notes}）", "user:2026-11-23", { chord: "Dm", notes: "D4 F4 A4" }),
  "pr.keyChordMinor": e("{root}m", "user:2026-11-23", { root: "D" }),
  "pr.keyChordDim": e("{root}dim", "user:2026-11-23", { root: "B" }),
  "pr.keyChordAug": e("{root}aug", "user:2026-11-23", { root: "C" }),
  // P6 (round 12): 「進行から並べる ▾」 at the right end of the chord row. The separator and the bottom line are those of the spec's picture.
  "pr.progressionOpen": e("進行から並べる ▾", "user:2026-11-23"),
  "pr.progressionPlace": e("並べる", "user:2026-11-23"),
  "pr.progressionListen": e("聞く", "user:2026-11-23"),
  "pr.progressionHint": e("▶ で聞く ・ Enter で 1 小節に 1 つずつ並べる", "user:2026-11-23"),
  "pr.progSeparator": e(" – ", "user:2026-11-23"),
  "pr.progRoyal": e("王道", "user:2026-11-23"),
  "pr.progCanon": e("カノン", "user:2026-11-23"),
  "pr.progKomuro": e("小室", "user:2026-11-23"),
  "pr.progCircle": e("循環", "user:2026-11-23"),
  "pr.progPop": e("ポップ", "user:2026-11-23"),
  "pr.progSad": e("切ない", "user:2026-11-23"),
  // P2 (round 12): 「ほかのトラックも見る ▾」 in the tool row; the ▾ is a button of its own (as the scale ▾). The other two are made up (docs/round12/P2.md).
  "pr.ghost": e("ほかのトラックも見る", "user:2026-11-23"),
  "pr.ghostTracks": e("見せるトラック", "builder:2026-11-23"),
  "pr.ghostDrums": e("ドラムのトラックでは、ほかのトラックは見られません。", "builder:2026-11-23"),
  "pr.noteList": e("ノートの一覧", "mock:A-PianoRoll"),
  "pr.selectedNotes": e("選んだノート", "mock:A-PianoRoll"),
  "pr.ccList": e("CC の一覧", "mock:A-PianoRoll"),
  "pr.velocity": e("ベロシティ", "mock:A-PianoRoll"),
  "pr.velocitySelected": e("選択 {values}", "mock:A-PianoRoll", {
    values: "80 83 77",
  }),
  "pr.velocityScale": e("目盛 0–127", "mock:A-PianoRoll"),
  "pr.sustain": e("サステイン", "mock:A-PianoRoll"),
  "pr.sustainDetail": e("CC 64 ・ 踏む {count}", "mock:A-PianoRoll", {
    count: "5",
  }),
  "pr.pitchBend": e("ピッチベンド", "mock:A-PianoRoll"),
  "pr.points": e("点 {count}", "mock:A-PianoRoll", { count: "6" }),
  "pr.modulation": e("モジュレーション", "mock:A-PianoRoll"),
  "pr.modulationDetail": e("CC 1 ・ 点 {count}", "mock:A-PianoRoll", {
    count: "0",
  }),
  "pr.otherCc": e("そのほかの CC", "mock:A-PianoRoll"),
  "pr.readDraw": e("{key} ・ {position} ・ 長さ {length}", "mock:R3-PRMouse", {
    key: "D3",
    position: "1.2.000",
    length: "0.360",
  }),
  "pr.readResize": e("長さ {from} → {to}", "mock:R3-PRMouse", {
    from: "1.000",
    to: "2.120",
  }),
  "pr.readMove": e("{ticks} ・ {keys} 半音", "mock:R3-PRMouse", {
    ticks: "+0.240",
    keys: "+2",
  }),
  "pr.readBox": e("{count} ノート", "mock:R3-PRMouse", { count: "3" }),
  "pr.readVelocity": e("{delta}（{count} ノート）", "mock:R3-Lanes", {
    delta: "+12",
    count: "3",
  }),
  "pr.readControl": e("{delta} ・ {position}", "mock:R3-Lanes", {
    delta: "+0512",
    position: "1.3.000",
  }),
  "pr.readSwitch": s6(
    "Reading of an on/off CC",
    "{state}（{value}）",
    { state: "踏む", value: "127" },
    "quoted",
  ),
  "pr.switchOn": s6("Reading of an on/off CC", "踏む", undefined, "part"),
  // Rows of the 「そのほかの CC」 list, and the band name for the chosen number.
  "pr.ccNumber": e("CC {number}", "user:2026-11-15", { number: "7" }),
  // Menu bar and context menus (the owner's decision; docs/history/design-notes.ja.md B-59, 60). Context menus use the same row names.
  "menu.file": e("ファイル", "user:2026-11-18"),
  "menu.edit": e("編集", "user:2026-11-18"),
  "menu.view": e("表示", "user:2026-11-18"),
  "menu.play": e("再生", "user:2026-11-18"),
  "menu.notes": e("ノート", "user:2026-11-18"),
  "menu.track": e("トラック", "user:2026-11-18"),
  "menu.newSong": e("新しい曲", "user:2026-11-18"),
  "menu.openSong": e("ひらく…", "user:2026-11-18"),
  // A5: the ▸ child list under 「ひらく…」 (the song's name, its place in thin text).
  "menu.recentSongs": e("最近ひらいた曲", "user:2026-11-23"),
  "menu.clearRecent": e("一覧を消す", "user:2026-11-23"),
  "menu.noRecent": e("まだありません", "user:2026-11-23"),
  "menu.save": e("保存", "user:2026-11-18"),
  "menu.saveAs": e("名前をつけて保存…", "user:2026-11-18"),
  "menu.export": e("書き出し…", "user:2026-11-18"),
  "menu.audioSettings": e("音の設定…", "user:2026-11-18"),
  "menu.quit": e("meloet を終わる", "user:2026-11-18"),
  "menu.undo": e("取り消す", "user:2026-11-18"),
  "menu.redo": e("やりなおす", "user:2026-11-18"),
  "menu.cut": e("切り取り", "user:2026-11-18"),
  "menu.copy": e("コピー", "user:2026-11-18"),
  "menu.paste": e("貼りつけ", "user:2026-11-18"),
  "menu.duplicate": e("複製", "user:2026-11-18"),
  "menu.delete": e("消す", "user:2026-11-18"),
  "menu.selectAll": e("ぜんぶ選ぶ", "user:2026-11-18"),
  "menu.goArrange": e("アレンジへ", "user:2026-11-18"),
  "menu.pianoRoll": e("ピアノロール", "user:2026-11-18"),
  "menu.mixer": e("ミキサー", "user:2026-11-18"),
  "menu.history": e("取り消しの履歴", "user:2026-11-18"),
  "menu.shelf": e("棚", "user:2026-11-18"),
  "menu.params": e("パラメータ", "user:2026-11-18"),
  "menu.zoomIn": e("横に拡大", "user:2026-11-18"),
  "menu.zoomOut": e("横に縮小", "user:2026-11-18"),
  "menu.zoomRowsIn": e("縦に拡大", "user:2026-11-18"),
  "menu.zoomRowsOut": e("縦に縮小", "user:2026-11-18"),
  "menu.scale": e("拡大率", "user:2026-11-18"),
  "menu.theme": e("テーマ", "user:2026-11-23"),
  "menu.spectrum": e("音の動きを見せる", "user:2026-11-23"),
  "theme.dark": e("ダーク", "user:2026-11-23"),
  "theme.light": e("ライト", "user:2026-11-23"),
  "theme.pinkDark": e("ピンク ・ ダーク", "user:2026-11-23"),
  "theme.pinkLight": e("ピンク ・ ライト", "user:2026-11-23"),
  "theme.mintDark": e("ミント ・ ダーク", "user:2026-11-23"),
  "theme.mintLight": e("ミント ・ ライト", "user:2026-11-23"),
  "menu.noteGlow": e("鳴っている音を光らせる", "user:2026-11-23"),
  "menu.playStart": e("再生", "user:2026-11-18"),
  "menu.stop": e("停止", "user:2026-11-18"),
  "menu.record": e("録音", "user:2026-11-18"),
  "menu.keepPlayed": e("さっきの演奏を残す", "user:2026-11-23"),
  "menu.toStart": e("先頭へ", "user:2026-11-18"),
  "menu.loop": e("ループ", "user:2026-11-18"),
  "menu.metronome": e("メトロノーム", "user:2026-11-18"),
  "menu.countIn": e("カウントイン", "user:2026-11-18"),
  "menu.tools": e("道具", "user:2026-11-18"),
  "menu.quantize": e("クオンタイズ", "user:2026-11-18"),
  "menu.unquantize": e("もとの位置へ戻す", "user:2026-11-18"),
  "menu.grid": e("グリッド", "user:2026-11-18"),
  "menu.triplet": e("三連符", "user:2026-11-18"),
  "menu.scaleLock": e("スケール固定", "user:2026-11-18"),
  "menu.mute": e("ミュート", "user:2026-11-18"),
  "menu.solo": e("ソロ", "user:2026-11-18"),
  "menu.arm": e("録音待機", "user:2026-11-18"),
  "menu.nextTake": e("次のテイク", "user:2026-11-18"),
  "menu.prevTake": e("前のテイク", "user:2026-11-18"),
  "menu.deleteTake": e("テイクを消す", "user:2026-11-18"),
  "menu.openInPianoRoll": e("ピアノロールでひらく", "user:2026-11-18"),
  // Round 10 rows of the context menus and of 「編集」 (spec 3, "Rules added in round 10").
  "menu.splitHere": e("ここで切る", "spec:3"),
  "menu.join": e("つなげる", "spec:3"),
  "menu.renameClip": e("名前を変える", "spec:3"),
  "menu.createClipHere": e("ここにクリップを作る", "spec:3"),
  "menu.addMarkerHere": e("ここにマーカーを足す", "spec:3"),
  "menu.splitClip": e("クリップを切る", "spec:3"),
  "menu.joinClips": e("クリップをつなげる", "spec:3"),
  // Round 12 A1: repeating clips (the handle at the right end of the name strip, and the menus' row with its box).
  "menu.repeatClips": e("くり返す…", "user:2026-11-23"),
  "arrange.repeatMark": e("⟳", "user:2026-11-23"),
  "arrange.readRepeat": e("{count} 回くり返します", "user:2026-11-23", { count: "3" }),
  "arrange.repeatCount": e("くり返す回数", "builder:2026-11-23"),
  // Velocity lock (B-66) and changing velocities together (B-67).
  "input.velocityFixed": e("固定", "user:2026-11-18"),
  "inspector.matchVelocity": e("そろえる", "user:2026-11-18"),
  // P3 (round 12): more rows of 「ベロシティをまとめて変える」 and 「長さをそろえる」. The marks % → ± are those of the spec's picture.
  "inspector.scaleVelocity": e("割合", "user:2026-11-23"),
  "inspector.rampVelocity": e("だんだん", "user:2026-11-23"),
  "inspector.randomVelocity": e("ばらつき", "user:2026-11-23"),
  "inspector.applyTool": e("かける", "user:2026-11-23"),
  "inspector.percentMark": e("%", "user:2026-11-23"),
  "inspector.rampMark": e("→", "user:2026-11-23"),
  "inspector.spreadMark": e("±", "user:2026-11-23"),
  "inspector.legato": e("次の音までのばす", "user:2026-11-23"),
  "inspector.legatoApply": e("のばす", "user:2026-11-23"),
  "msg.velocityScaled": e("{count} ノートのベロシティに {percent} % をかけました。", "builder:2026-11-23", { count: "3", percent: "120" }),
  "msg.velocityRamped": e("{count} ノートのベロシティを {from} から {to} へだんだん変えました。", "builder:2026-11-23", {
    count: "3",
    from: "40",
    to: "110",
  }),
  "msg.velocityRandomized": e("{count} ノートのベロシティを ± {amount} でばらつかせました。", "builder:2026-11-23", { count: "3", amount: "8" }),
  "msg.notesLegato": e("{count} ノートを次の音までのばしました。", "builder:2026-11-23", { count: "3" }),

  // [Piano roll]
  // Column headings of the opened 「CC の一覧」 (the position is note.position).
  "ccList.value": e("値", "mock:R5-Lists"),
  // [Arrange, transport, input meter]
  // Something unreadable was typed in the play position field (design-notes item 61). The play position has no upper end, so the range text form is not used.
  "msg.playPositionRejected": s6(
    "Not readable as a position",
    "位置は「5」「5.2」「5.2.240」の形で打ちます。もとの {kept} のままです。",
    { kept: "1. 1. 000" },
  ),

  // [Tempo and time signature changes, markers] (spec 3 (Y17, Y18), spec 6. Provisional text: docs/history/design-notes.ja.md B-92)
  "msg.tempoRampSelected": s6(
    "Tempo ramp chosen",
    "{place}の変更をえらんでいます ・ {tempo} から {toPlace}の {toTempo} へ、だんだん{direction}",
    { place: "5 小節目", tempo: "104.00", toPlace: "9 小節目", toTempo: "120.00", direction: "速く" },
  ),
  "msg.markerAdded": s6("Marker added", "{name} を足しました。名前を打って Enter", { name: "マーカー 4" }),
  "timeline.rampFaster": s6("Tempo ramp chosen", "速く", undefined, "part"),
  "timeline.rampSlower": e("遅く", "builder:2026-11-20"),
  "timeline.placeBar": s6("Tempo ramp chosen", "{bar} 小節目", { bar: "5" }, "part"),
  "timeline.placeBeat": e("{bar} 小節目の {beat} 拍目", "builder:2026-11-20", { bar: "5", beat: "3" }),
  "timeline.placeHead": e("曲の頭", "builder:2026-11-20"),
  "msg.tempoPointSelected": e("{place}の変更をえらんでいます ・ {tempo}", "builder:2026-11-20", { place: "5 小節目", tempo: "120.00" }),
  "msg.meterChangeSelected": e("{place}の変更をえらんでいます ・ {signature}", "builder:2026-11-20", { place: "9 小節目", signature: "3/4" }),
  // The point and flag at the start of the song are not called a 「変更」 (spec 3 (92-2)).
  "msg.headTempoSelected": s6("Tempo at the song start chosen", "曲の頭のテンポをえらんでいます ・ {tempo}", { tempo: "96.00" }, "part"),
  "msg.headMeterSelected": s6("Tempo at the song start chosen", "曲の頭の拍子をえらんでいます ・ {signature}", { signature: "4/4" }, "quoted"),
  "msg.tempoPointSet": e("{place}のテンポを {tempo} にしました。", "builder:2026-11-20", { place: "5 小節目", tempo: "120.00" }),
  "msg.meterChangeSet": e("{place}の拍子を {signature} にしました。", "builder:2026-11-20", { place: "9 小節目", signature: "3/4" }),
  "marker.newName": e("マーカー {count}", "spec:3", { count: "4" }),
  // Band heading (when open; with 0 changes, no 「ほか」), the text above a ramp line, context menu rows.
  "timeline.tempoSummary": e("テンポ {tempo} ほか {count}", "spec:3", { tempo: "96.00", count: "3" }),
  "timeline.tempoOnly": e("テンポ {tempo}", "spec:3", { tempo: "96.00" }),
  "timeline.meterSummary": e("拍子 {signature} ほか {count}", "spec:3", { signature: "4/4", count: "2" }),
  "timeline.meterOnly": e("拍子 {signature}", "spec:3", { signature: "4/4" }),
  "timeline.ramp": e("だんだん", "spec:2"),
  "menu.tempoRamp": e("次の点まで、だんだん", "spec:3"),
  "menu.loopFromMarker": e("ここから次のマーカーまでをループに", "spec:3"),
  "menu.renameMarker": e("名前を変える", "spec:3"),
  // Round 12 A2: whole sections (a marker to the next one), in the marker's context menu.
  "menu.duplicateSection": e("この区間をすぐ後ろに複製", "user:2026-11-23"),
  "menu.deleteSection": e("この区間を消して詰める", "user:2026-11-23"),
  "menu.insertSpace": e("この前に空きを入れる…", "user:2026-11-23"),
  "timeline.sectionNotOnBars": e("区間の両端が小節の頭のときに使えます", "user:2026-11-23"),
  "timeline.sectionEmpty": e("このマーカーより後ろにクリップがないので、区間がありません", "builder:2026-11-24"),
  "timeline.insertSpaceBars": e("入れる小節の数", "builder:2026-11-24"),
  "timeline.readInsertSpace": e("{count} 小節の空きを入れます", "builder:2026-11-24", { count: "4" }),
  "msg.songTooLong": e("曲が長くなりすぎるので、できませんでした。", "builder:2026-11-24"),
  // Context menu of a time signature flag (spec 3 (92-17)).
  "menu.typeMeter": e("拍子を打ちこむ…", "spec:3"),

  // [Automation, undo history] (spec 3 (Y16, Y19), spec 6. Provisional text: docs/history/design-notes.ja.md B-92)
  // Status line. For a step with no 「（何を）」, the brackets are left out too (Bare).
  "msg.undone": s6("Undone", "「{name}」を取り消しました（{what}）。やりなおすには {redoKey}", {
    name: "ノートを動かす",
    what: "3 ノート",
    redoKey: "Mod Shift Z",
  }),
  "msg.undoneBare": e("「{name}」を取り消しました。やりなおすには {redoKey}", "builder:2026-11-20", { name: "ミュート", redoKey: "Mod Shift Z" }),
  "msg.redone": s6("Redone", "「{name}」をやりなおしました（{what}）。", { name: "ノートを動かす", what: "3 ノート" }),
  "msg.redoneBare": e("「{name}」をやりなおしました。", "builder:2026-11-20", { name: "ミュート" }),
  "msg.historyJumped": s6("Went back in the history", "{index} 手目「{name}」まで戻しました（{steps} 手）。", { index: "11", name: "マーカーを足す", steps: "5" }),
  // When jumping to a row above the current one (spec 3 (Y19) 「そこまでやりなおす」; spec 6 has no text for it).
  "msg.historyJumpedForward": e("{index} 手目「{name}」までやりなおしました（{steps} 手）。", "builder:2026-11-20", {
    index: "14",
    name: "クオンタイズ",
    steps: "2",
  }),
  "msg.nothingToUndo": s6("No step to undo", "取り消せる手がありません。"),
  "msg.nothingToRedo": s6("No step to redo", "やりなおせる手がありません。"),
  "msg.automationWriting": s6("Writing automation", "{track} ・ {target}を書いています（● 書く ・ 止めるとオフに戻ります）", {
    track: "1 ピアノ",
    target: "音量",
  }),
  "msg.automationRested": s6(
    "Automation put to rest",
    "{track} の{target}を手で動かしたので、オートメーションを休ませました。「もどす」で線のとおりに戻ります",
    { track: "4 パッド", target: "音量" },
  ),
  "msg.automationRestored": s6("Automation brought back", "{track} の{target}を、オートメーションのとおりに戻しました。", {
    track: "4 パッド",
    target: "音量",
  }),
  "msg.automationMoving": e("{tracks} の音量は、オートメーションで動いています", "mock:F6-MixerAuto", { tracks: "1 ピアノ と 4 パッド" }),
  // Names of what a band moves (band heading 「オートメーション ・ （何を）」, status line, history rows). Pan is mixer.pan.
  "lane.volume": s6("Writing automation", "音量", undefined, "part"),
  "lane.mute": e("ミュート", "spec:3"),
  "lane.send": e("センド {bus}", "builder:2026-11-20", { bus: "A" }),
  "lane.param": e("{effect}の「{param}」", "builder:2026-11-20", { effect: "コンプレッサー", param: "しきい値" }),
  // When the plugin does not give a parameter name.
  "lane.pluginParam": e("{plugin}のパラメータ", "builder:2026-11-20", { plugin: "Surge XT " }),
  // Arrange band heading (2 lines) and the reading (spec 2, spec 3 (Y16)).
  "lane.title": e("オートメーション ・ {target}", "mock:F6-AutoRec", { target: "音量" }),
  "lane.write": e("● 書く", "spec:2"),
  "lane.writing": e("書いています", "mock:F6-AutoRec"),
  "lane.restore": e("もどす", "spec:2"),
  "lane.resting": e("休み ・ {value}", "mock:F6-AutoRest", { value: "−12.0 dB" }),
  "lane.restingBare": e("休み", "spec:2"),
  "lane.now": e("いま {value} ・ 点 {count}", "mock:F6-ParamMany", { value: "523.25 Hz", count: "4" }),
  "lane.count": e("点 {count}", "mock:F6-ParamMany", { count: "4" }),
  // The row under the heading and the last row of the list it opens (spec 3 (92-5, 20-A)).
  "lane.add": e("＋ 帯を足す ▾", "spec:3"),
  "lane.addEffectParams": e("エフェクトのパラメータ…", "spec:3"),
  "lane.delete": e("帯を消す", "spec:3"),
  // Top right and bottom right of the band (spec 3 (92-6, 21-B)).
  "lane.rangeTop": e("上 {value}", "spec:3", { value: "−3.0 dB" }),
  "lane.rangeBottom": e("下 {value}", "spec:3", { value: "−9.0 dB" }),
  "lane.readAdd": e("{position} ・ {value}", "mock:R10-AutoEdit", { position: "5. 3. 000", value: "−8.4 dB" }),
  "lane.readMove": e("{delta} ・ {from} → {to}", "mock:R10-AutoEdit", { delta: "+0. 1. 000", from: "−2.1", to: "−0.6 dB" }),
  // Under the mixer fader value (spec 2 "Handle moved by automation"). Resting is lane.restingBare and lane.restore.
  "mixer.automation": e("オートメーション", "spec:2"),
  // The history row 「「（手の名前）」・ （何を）」 and the forms of 「（何を）」. Step names are in stepNames below.
  "history.row": e("「{name}」・ {what}", "mock:R10-History", { name: "ノートを動かす", what: "3 ノート" }),
  "history.rowBare": e("「{name}」", "builder:2026-11-20", { name: "ミュート" }),
  "history.notes": s6("Undone", "{count} ノート", { count: "3" }, "part"),
  "history.points": e("{count} 点", "builder:2026-11-20", { count: "3" }),
  "history.clips": e("{count} クリップ", "spec:3", { count: "3" }),
  "history.take": e("テイク {number}", "mock:F-History", { number: "1" }),
  "history.takeSwitch": e("テイク {from} → {to}", "builder:2026-11-20", { from: "1", to: "2" }),
  "history.change": e("{from} → {to}", "spec:3", { from: "ベース", to: "ベース 2" }),
  "history.changeAt": e("{place} {from} → {to}", "spec:3", { place: "5 小節目", from: "96.00", to: "120.00" }),
  "history.valueAt": e("{place} {value}", "builder:2026-11-20", { place: "5 小節目", value: "120" }),
  "history.bars": e("{from}〜{to} 小節", "spec:3", { from: "5", to: "9" }),
  "history.gain": e("{strip} ・ {from} → {to} dB", "spec:3", { strip: "4 パッド", from: "−10.5", to: "−12.0" }),
  "history.pan": e("{strip} ・ {from} → {to}", "spec:3", { strip: "4 パッド", from: "C", to: "L 20" }),
  "history.send": e("{strip} ・ センド {bus} {from} → {to} dB", "spec:3", { strip: "4 パッド", bus: "A", from: "−15.0", to: "−12.0" }),
  "history.lanePoints": e("{target} ・ {count} 点", "spec:3", { target: "音量", count: "3" }),
  "history.muteOn": e("{strip} ・ オン", "spec:3", { strip: "4 パッド" }),
  "history.missing": e("トラック {tracks} ・ インサート {inserts}", "builder:2026-11-20", { tracks: "2", inserts: "1" }),
  // 「（何を）」 that protocol 23 made possible (spec 3 (92-3)). One note is the note name only (「C4」).
  "history.quantized": e("{grid} ・ {count} ノート", "spec:3", { grid: "1/16", count: "12" }),
  "history.velocity": e("{count} ノート ・ {from} → {to}", "spec:3", { count: "3", from: "96", to: "110" }),
  "history.controlPoints": e("{control} ・ {count} 点", "spec:3", { control: "サステイン", count: "4" }),
  "history.color": e("{track} ・ 色 {color}", "spec:3", { track: "4 パッド", color: "3" }),
  "history.section": e("{name} ・ {bars} 小節", "builder:2026-11-24", { name: "A メロ", bars: "8" }),
  "history.soundReplaced": e("{track} ・ {from} → {to}", "spec:3", { track: "1 ピアノ", from: "グランドピアノ", to: "オルガン" }),
  // Input list of a vocal track (spec 3, the vocal track head).
  "vocal.input.mono1": e("入口 1 ・ モノ", "spec:3"),
  "vocal.input.mono2": e("入口 2 ・ モノ", "spec:3"),
  "vocal.input.stereo12": e("入口 1 ・ 2 ・ ステレオ", "spec:3"),
  // Step name when the plugin did not give a parameter name.
  "history.pluginParam": e("{plugin}のパラメータを変える", "builder:2026-11-20", { plugin: "Surge XT " }),
  // 「どこで」 on the right of the row. Others are band.arrange, band.pianoRoll, band.mixer and track.label.
  // 「（何を）」 of a window step (spec 3 (94-6)). The right column stays history.placePluginWindow (docs/history/design-notes.ja.md B-95-3).
  "history.pluginWindowWhat": e("プラグインの窓", "spec:3"),
  "history.placePluginWindow": e("{strip} ・ プラグインの窓", "spec:6", { strip: "5 Surge XT" }),
  // Band heading row (spec 3 (Y19, 92-9), spec 6).
  "history.head": e("{total} 手 ・ いまは {current} 手目。上の {redo} 手は取り消してあり、やりなおせます", "mock:F-History", {
    total: "14",
    current: "12",
    redo: "2",
  }),
  "history.headLatest": s6("History heading: no undone step", "{total} 手 ・ いまは {current} 手目（いちばん新しい手）", {
    total: "14",
    current: "14",
  }),
  "history.headAllUndone": s6("History heading: everything undone", "{total} 手 ・ ぜんぶ取り消してあります ・ やりなおすには {redoKey}", {
    total: "14",
    redoKey: "Mod Shift Z",
  }),
  "history.headEmpty": s6("History heading: no steps", "まだ手がありません ・ 何かすると、ここに 1 手ずつならびます"),
  "history.undo": e("取り消す", "mock:F-History"),
  "history.redo": e("やりなおす", "mock:F-History"),
  // The right of the row (spec 2 "History row: hover, chosen", spec 3 (Y19)). key.enter to the right of the button.
  "history.current": e("いまここ", "mock:F-History"),
  "history.redoable": e("やりなおせます", "mock:F-History"),
  "history.jumpHere": e("ここまで戻す", "spec:3"),
  // The bottom row of the band, when there are steps before the visible rows.
  "history.more": e("… {index} 手目より前は、下へスクロール", "mock:F-History", { index: "6" }),

  // [Vocal tracks, audio files, latency, export] (spec 3 (93), spec 6).
  "msg.vocalTrackAdded": s6(
    "Vocal track added",
    "{track} を足しました ・ {input} から録ります ・ 歌のトラックの ● は、選んでいなくても自分でつけます",
    { track: "6 歌", input: "入口 1" },
  ),
  "msg.noAudioInput": s6("No input", "音の入口がありません。マイクをつなぐか、音の設定で入口をえらんでください。"),
  "msg.monitorFirstOn": s6(
    "Monitor turned on for the first time",
    "自分の声が少し遅れて聞こえます（いまは {ms} ms）。スピーカーだと音がまわって「キーン」となるので、ヘッドホンで聞いてください。",
    { ms: "13" },
  ),
  "msg.vocalNoPianoRoll": s6("Vocal tracks have no piano roll", "歌のトラックにはピアノロールがありません。波形はアレンジで直します。"),
  "msg.recordingVocal": s6("Recording vocals", "● 録音中 ・ {track}（{input}）・ モニター オン ・ 止めるには {stopKey}", {
    track: "6 歌",
    input: "入口 1",
    stopKey: "Space",
  }),
  "msg.recordingKeysAndVocal": s6(
    "Recording keyboard and vocals",
    "● 録音中 ・ {keys}（鍵盤）と {vocal}（{input}）に録っています ・ 止めるには {stopKey}",
    { keys: "1 ピアノ", vocal: "6 歌", input: "入口 1", stopKey: "Space" },
  ),
  "recovery.vocalKept": s6("Vocals being recorded were kept", "録っていた歌 {count} つを、止まる直前（{position} ごろ）まで残しました。", {
    count: "1",
    position: "9. 3. 0000",
  }),
  "msg.audioFilesMissing": s6("Audio files not found", "曲で使っている音のファイルが {count} つ見つかりません。そのクリップは鳴りません。", {
    count: "2",
  }),
  "params.audioFileMissing": s6("Audio file not found (panel)", "{file} が見つかりません。前はここにありました: {path}", {
    file: "歌 テイク 1.wav",
    path: "はじめての曲/音/歌 テイク 1.wav",
  }),
  "button.findFile": e("さがす…", "spec:6"),
  "msg.clipGainDragging": s6(
    "Moving the clip gain",
    "{clip} のクリップの音量 {value}（ドラッグ中）・ 離すと決まります ・ やめるには {cancelKey}",
    { clip: "歌 テイク 1", value: "−3.0 dB", cancelKey: "Esc" },
  ),
  "msg.clipSplit": s6("Cut", "{clip} を {position} で切りました。", { clip: "歌 テイク 1", position: "7. 1. 0000" }),
  "msg.audioRendering": s6("Making the sound", "{clip} の音を作っています {percent} % ・ できるまでは、もとの音で鳴らします", {
    clip: "歌 テイク 1",
    percent: "34",
  }),
  "msg.audioImported": s6("Audio file brought in", "{file} を入れました（{track} ・ {position}）。", {
    file: "ドラム.wav",
    track: "6 歌",
    position: "5. 1. 0000",
  }),
  "clip.loading": s6("Loading an audio file", "読みこんでいます {percent} %", { percent: "40" }),
  "msg.audioUnreadable": s6("File that cannot be read", "「{file}」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。", { file: "声.aac" }),
  "msg.audioDragging": s6(
    "Dragging in audio files",
    "音のファイルを {count} つ持ってきています ・ 左に落とすと別々のトラック、右に落とすと 1 本にならべる",
    { count: "8" },
  ),
  // While one audio file is being dragged in (the status line text in the 「出る所」 column of spec 6 「音のファイルを 1 つ持ってきている」).
  "msg.audioDraggingOne": e("音のファイルを持ってきています ・ 落とすと、枠の所に入れます", "spec:6"),
  // The answer of 「さがす」 (「この音のテンポ」). The found text 「…くらいです」 is joined with an ending for put in or the same.
  "msg.sourceTempoFound": s6("Searched for the tempo of this sound", "この音のテンポは {bpm} くらいです", { bpm: "120.00" }, "part"),
  "msg.sourceTempoSet": s6("Searched for the tempo of this sound", "（さがした数を入れました）。", undefined, "part"),
  "msg.sourceTempoSame": s6("Searched for the tempo of this sound", "（いまの数と同じです）。", undefined, "part"),
  "msg.sourceTempoNotFound": s6("Searched for the tempo of this sound", "この音のテンポは見つかりませんでした。数を打ちこんでください。", undefined, "quoted"),
  "drop.mixed": s6("Dragging in SF2 and audio files together", "SF2 は {sf2} つ棚へ ・ 音のファイル {audio} つはアレンジへ", {
    sf2: "1",
    audio: "3",
  }),
  // The split box while dragging in (spec 2, spec 3 (30-C), mock F7-AudioDrop) and the names of the drop frames.
  "drop.separate": e("別々のトラックに", "spec:3"),
  "drop.separateSay": e("{files} つのファイルを {tracks} 本の歌のトラックにして、落とした位置から入れます。", "mock:F7-AudioDrop", {
    files: "8",
    tracks: "8",
  }),
  "drop.inRow": e("1 本にならべる", "spec:3"),
  "drop.inRowSay": e("落とした所のトラックに、順にうしろへ。", "mock:F7-AudioDrop"),
  "drop.frameMore": e("{file} ほか {more}", "mock:F7-AudioDrop", { file: "ドラム キック.wav", more: "7" }),
  // Text inside an audio clip (spec 3, when the audio file is not found).
  "clip.missing": e("音のファイルが見つかりません", "spec:3"),
  // The selected clip panel (spec 3 (93-4, Y22), mock F7-AudioEdit, F7-AudioMissing).
  "clipParams.selected": e("選んだクリップ", "spec:3"),
  "clipParams.rate": e("{rate} Hz", "spec:3", { rate: "48000" }),
  "clipParams.rateResampled": e("{rate} Hz（曲に合わせて鳴らします）", "spec:3", { rate: "44100" }),
  "clipParams.bits": e("{bits} ビット", "spec:3", { bits: "24" }),
  "clipParams.groupClip": e("クリップ", "spec:3"),
  "clipParams.position": e("位置", "spec:3"),
  "clipParams.length": e("長さ", "spec:3"),
  "clipParams.gain": e("音量", "spec:3"),
  "clipParams.fade": e("フェード", "spec:3"),
  "clipParams.fadeValue": e("前 {fadeIn} ・ 後 {fadeOut}", "mock:F7-AudioEdit", { fadeIn: "0.120", fadeOut: "0.000" }),
  "clipParams.groupStretch": e("のび縮み ・ 音の高さ", "spec:3"),
  "clipParams.follow": e("曲のテンポについていく", "spec:3"),
  "clipParams.sourceTempo": e("この音のテンポ", "spec:3"),
  "clipParams.pitch": e("音の高さ", "spec:3"),
  "clipParams.pitchValue": e("{semitones} 半音 {cents} セント", "mock:F7-AudioEdit", { semitones: "+0", cents: "0" }),
  "clipParams.keepFormant": e("声の感じを保つ", "spec:3"),
  "clipParams.findTempo": e("さがす", "spec:3"),
  // Round 12, M7: the strength row and the scale row under it (the strength is shown with param.value.percent, the scale with scale.name).
  "clipParams.tune": e("音程を寄せる", "user:2026-11-23"),
  "clipParams.tuneToScale": e("いまのスケールにする", "user:2026-11-23"),
  "clipParams.tuneTip": e("ひとりの声や、音が1つずつの楽器に使えます", "user:2026-11-23"),
  // The progress bar row above the panel while the sound is being made (spec 2 "Selected clip", mock F7-Stretch).
  "clipParams.rendering": e("音を作っています", "mock:F7-Stretch"),
  "clipParams.renderingPercent": e("{percent} %", "mock:F7-Stretch", { percent: "34" }),
  // The mark right of the audio clip name, and the progress below (spec 2 "Audio clip").
  "clip.ratioMark": e("×{ratio}", "spec:2", { ratio: "1.25" }),
  "clip.pitchMarkCents": e("{semitones} ・ {cents}", "spec:3", { semitones: "−1", cents: "+30" }),
  "clip.rendering": e("音を作っています {percent} %", "spec:2", { percent: "34" }),
  // Places for later rounds under the audio clip context menu (spec 3, while the sound is being made).
  "menu.reverse": e("逆にする", "spec:3"),
  "menu.normalize": e("ノーマライズ", "spec:3"),
  // What 「ノーマライズ」 and 「逆にする」 say (round 12, A9). The spec has no text for them: made in the words of the lines around.
  "msg.normalizeLimited": e(
    "{count} つのクリップは、いちばん大きい音を −1.0 dB にできませんでした（クリップの音量は −60 〜 +12 dB まで）。",
    "builder:2026-11-23",
    { count: "1" },
  ),
  "msg.normalizeSilent": e("{count} つのクリップは音がないので、そのままにしました。", "builder:2026-11-23", { count: "1" }),
  "msg.reverseFollowOff": e(
    "{count} つのクリップは、もとの音のテンポが途中で変わるので、「曲のテンポについていく」をオフにしました。",
    "builder:2026-11-23",
    { count: "1" },
  ),
  "latency.measuring": s6("Measuring the latency", "測っています {done} / {total}", { done: "3", total: "5" }),
  "latency.failed": s6("Latency cannot be measured", "音が小さくて測れませんでした。ヘッドホンの音量を上げて、もう一度測ってください。"),
  "button.useThisValue": e("この数を使う", "spec:6"),
  "button.again": e("もう一度", "spec:6"),
  "button.audioSettingsEllipsis": e("音の設定…", "spec:6"),
  "settings.notAsRequested": s6(
    "Not running at the asked size",
    "頼んだ {requested} ではなく、{actual} で動いています（PC の設定で決まっています）。meloet は PC 全体の設定は変えません。",
    { requested: "128", actual: "256" },
  ),
  "msg.exporting": s6("Exporting", "書き出しています {percent} % ・ {file} ・ 残り 約 {left} ・ 弾けます（書き出しの音は聞こえません）", {
    percent: "46",
    file: "はじめての曲.wav",
    left: "40 秒",
  }),
  "msg.exportingRealtime": s6("Exporting in real time", "速く書き出せないプラグインがあるので、実時間で書き出します（{length}）。", {
    length: "3:02",
  }),
  "msg.exportedFile": s6("Exported", "{file} を書き出しました（{length} ・ {rate} Hz ・ {bits} ビット）。", {
    file: "はじめての曲.wav",
    length: "3:02",
    rate: "48000",
    bits: "24",
  }),
  "msg.exportCanceled": s6("Export stopped", "書き出しをやめました。途中までのファイルは消しました。"),

  // [Export] (spec 3 (34-B), mock F7-Export, F7-Exporting, F-Export)
  "export.title": e("書き出し", "mock:F7-Export"),
  "export.range": e("範囲", "mock:F7-Export"),
  "export.rangeSong": e("曲ぜんぶ", "mock:F7-Export"),
  "export.rangeLoop": e("ループ {from}〜{to}", "mock:F7-Export", { from: "5", to: "9" }),
  "export.rangeTracks": e("トラックごと", "mock:F7-Export"),
  "export.spanSong": e("曲ぜんぶ ・ {start} 〜 {end} と、音が消えるまで", "mock:F7-Export", {
    start: "1. 1. 0000",
    end: "17. 1. 0000",
  }),
  "export.format": e("形", "mock:F7-Export"),
  "export.formatWav": e("wav", "mock:F7-Export"),
  "export.formatFlac": e("flac", "mock:F7-Export"),
  "export.formatMp3": e("mp3", "user:2026-11-23"),
  "export.place": e("保存する所", "mock:F7-Export"),
  "export.choosePlace": e("えらぶ…", "mock:F7-Export"),
  "export.check": e("書き出す前のたしかめ", "mock:F7-Export"),
  "export.peak": e("いちばん大きい所", "mock:F7-Export"),
  "export.peakDb": e("{peak} dB", "mock:F7-Export", { peak: "−0.6" }),
  "export.noClipping": e("・ 音割れなし", "mock:F7-Export"),
  "export.noStoppedPlugins": e("止まっているプラグインはありません", "mock:F7-Export"),
  "export.detailsSummary": e("{rate} Hz ・ {bits} ビット ・ しっぽは音が消えるまで（最大 {seconds} 秒）", "mock:F7-Export", {
    rate: "48000",
    bits: "24",
    seconds: "10",
  }),
  "export.start": e("書き出す", "mock:F7-Export"),
  // When there are stopped plugins (spec 3, "keep as in F-Export").
  "export.stoppedPlugins": e("止まっているプラグイン", "mock:F-Export"),
  "export.stoppedPlugin": e("{plugin} ・ {strip}", "mock:F-Export", { plugin: "Brick EQ", strip: "3 ドラム" }),
  "export.stoppedSkipped": e("この枠をとばした音で書き出します", "mock:F-Export"),
  // Progress on the left of the status line (spec 2 "Export progress").
  "export.running": e("書き出しています", "mock:F7-Exporting"),
  "export.runningPercent": e("{percent} %", "mock:F7-Exporting", { percent: "46" }),
  "export.runningRest": e("{file} ・ 残り 約 {left} ・ 弾けます（書き出しの音は聞こえません）", "mock:F7-Exporting", {
    file: "はじめての曲.wav",
    left: "40 秒",
  }),
  // Provisional text where the spec and mocks have none (B-94-69 and on).
  "export.checkFailedPlugin": e("{plugin} を書き出し用に読みこめませんでした", "builder:2026-11-21", { plugin: "Surge XT" }),
  "export.checkFailedWrite": e("作業の場所に書けませんでした", "builder:2026-11-21"),
  "export.spanSongNoTail": e("曲ぜんぶ ・ {start} 〜 {end}", "builder:2026-11-21", { start: "1. 1. 0000", end: "17. 1. 0000" }),
  "export.spanLoop": e("ループ ・ {start} 〜 {end} と、音が消えるまで", "builder:2026-11-21", { start: "5. 1. 0000", end: "9. 1. 0000" }),
  "export.spanLoopNoTail": e("ループ ・ {start} 〜 {end}", "builder:2026-11-21", { start: "5. 1. 0000", end: "9. 1. 0000" }),
  "export.spanTracks": e("トラックごと ・ {count} つのファイル", "builder:2026-11-21", { count: "4" }),
  "export.sampleRate": e("サンプルレート", "builder:2026-11-21"),
  "export.bits": e("ビット", "builder:2026-11-21"),
  "export.tail": e("しっぽ", "builder:2026-11-21"),
  "export.realtime": e("実時間で書き出す", "builder:2026-11-21"),
  "export.tailUntilSilent": e("音が消えるまで（最大 {seconds} 秒）", "builder:2026-11-21", { seconds: "10" }),
  "export.tailOff": e("なし", "builder:2026-11-21"),
  "export.detailsSummaryNoTail": e("{rate} Hz ・ {bits} ビット ・ しっぽなし", "builder:2026-11-21", { rate: "48000", bits: "24" }),
  "export.detailsRealtime": e("・ 実時間", "builder:2026-11-21"),
  "export.leftSeconds": e("{seconds} 秒", "builder:2026-11-21", { seconds: "40" }),
  "export.leftMinutes": e("{minutes} 分", "builder:2026-11-21", { minutes: "2" }),
  // While the time left is not known yet (the first few blocks), 「残り 約 …」 is left out (B-94-81).
  "export.runningRestUnknown": e("{file} ・ 弾けます（書き出しの音は聞こえません）", "builder:2026-11-21", { file: "はじめての曲.wav" }),
  "msg.exportFailedPlugin": e("書き出せませんでした。{plugin} が止まりました。途中までのファイルは消しました。", "builder:2026-11-21", {
    plugin: "Surge XT",
  }),
  "msg.exportFailedWrite": e("書き出せませんでした。保存する所に書けません。", "builder:2026-11-21"),
  "msg.exportingRealtimeChosen": e("実時間で書き出しています（{length}）。", "builder:2026-11-21", { length: "3:02" }),

  // [Vocal track screens] (spec 2, spec 3 (23-A, 24-C, 31-C, 33-A), mock F7-*)
  // Under the track list, and 「トラック」 in the menu bar.
  "arrange.addKeysTrack": e("＋ 鍵盤のトラック", "spec:3"),
  "arrange.addVocalTrack": e("＋ 歌のトラック", "spec:3"),
  "menu.addKeysTrack": e("鍵盤のトラックを足す…", "spec:3"),
  "menu.addVocalTrack": e("歌のトラックを足す", "spec:3"),
  "menu.importAudio": e("音のファイルを入れる…", "spec:3"),
  // The second line of a vocal track head. Input names are vocal.input.*.
  "vocal.inputChoose": e("{input} ▾", "mock:F7-VocalRec", { input: "入口 1 ・ モノ" }),
  "vocal.noInput": e("入口なし ▾", "spec:3"),
  "vocal.monitor": e("モニター", "spec:3"),
  // The input in 「6 歌（入口 1）」 on the status line (input 2 is spec 6 「歌 2 本以上を録っている」).
  "vocal.inputShort.mono1": e("入口 1", "spec:6"),
  "vocal.inputShort.mono2": e("入口 2", "spec:6"),
  "vocal.inputShort.stereo12": e("入口 1 ・ 2", "spec:5"),
  // The name of an audio clip being recorded (「歌 テイク 1」 of mock F7-VocalRec).
  "arrange.recordingVocalClip": e("{track} テイク {take}", "mock:F7-VocalRec", { track: "歌", take: "1" }),
  // Right of the input meter (the microphone meter). The scale marks use the mock's text.
  "meter.monitorOn": e("モニター オン", "mock:F7-VocalRec"),
  "meter.monitorOff": e("モニター オフ", "mock:F7-VocalTrack"),
  "meter.micScaleLow": e("−60", "mock:F7-VocalTrack"),
  "meter.micScaleMid": e("−12", "mock:F7-VocalTrack"),
  "meter.micScaleTop": e("0", "mock:F7-VocalTrack"),
  // There is no play target (not a single keyboard track; B-94-1).
  "meter.playTargetNone": e("—", "spec:3"),
  // Sound settings ▸ advanced settings (mock F7-SettingsAdv).
  "settings.advTitle": e("音の設定", "mock:F7-SettingsAdv"),
  "settings.advBack": e("◂ 音の設定", "mock:F7-SettingsAdv"),
  "settings.input": e("入口", "mock:F7-SettingsAdv"),
  "settings.buffer": e("バッファ", "mock:F7-SettingsAdv"),
  "settings.runningLatency": e("動いている {frames} ・ {ms} ms", "mock:F7-SettingsAdv", { frames: "256", ms: "5.3" }),
  "settings.running": e("動いている {value}", "mock:F7-SettingsAdv", { value: "48000" }),
  "settings.rateHz": e("{rate} Hz", "mock:F7-SettingsAdv", { rate: "48000" }),
  "settings.recordFormat": e("録る形", "mock:F7-SettingsAdv"),
  "settings.bits": e("{bits} ビット", "mock:F7-SettingsAdv", { bits: "24" }),
  "settings.latencyAlign": e("遅れの合わせ", "mock:F7-SettingsAdv"),
  "settings.latencyReported": e("PC が言う数 ・ {ms} ms", "mock:F7-SettingsAdv", { ms: "8.0" }),
  // When a measured number is used (the form of the number the PC gives, following 「測った数」 in spec 3 (33-A)).
  "settings.latencyMeasured": e("測った数 ・ {ms} ms", "builder:2026-11-21", { ms: "12.4" }),
  "settings.inputChoice": e("{channels}（{name}）", "mock:F7-SettingsAdv", { channels: "入口 1 ・ 2", name: "USB マイク" }),
  "button.measureLatency": e("遅れを測る…", "mock:F7-SettingsAdv"),
  "button.manualLatency": e("手で決める…", "mock:F7-SettingsAdv"),
  "button.openPlaces": e("ひらく…", "mock:F7-SettingsAdv"),
  // Measuring latency (mock F7-Latency).
  "latency.title": e("遅れを測る", "mock:F7-Latency"),
  "latency.how": e(
    "ヘッドホンをマイクに近づけるか、出口と入口をケーブルでつなぎます。「測る」を押すと、カチッという音を {count} 回鳴らして、もどってくるまでの時間を測ります。",
    "mock:F7-Latency",
    { count: "5" },
  ),
  "latency.reported": e("PC が言う数", "mock:F7-Latency"),
  "latency.measured": e("測った数", "mock:F7-Latency"),
  "latency.ms": e("{ms} ms", "mock:F7-Latency", { ms: "12.4" }),
  "latency.partialMean": e("（まだ {count} 回の平均）", "mock:F7-Latency", { count: "3" }),
  "latency.quietHint": e("音が小さくて測れないときは、ヘッドホンの音量を上げてください。", "mock:F7-Latency"),
  "button.measure": e("測る", "mock:F7-Latency"),

  // [Messages from the core, and the sound pack] (spec 6 version 12, mock F8-*)
  "latency.failedUnsteady": s6(
    "Latency cannot be measured: spread",
    "測るたびに数が大きくちがったので、決められませんでした。まわりの音を小さくして、もう一度測ってください。",
  ),
  "latency.failedNoInput": s6("Latency cannot be measured: no input", "音の入口がないので測れません。マイクをつなぐか、入口をえらんでください。"),
  "msg.inputLost": s6("Input unplugged", "{input}（{device}）が抜けました。録音を止めました。録れた所までは残しています。", {
    input: "入口 1",
    device: "USB マイク",
  }),
  "msg.recordingGaps": s6(
    "Sound broke up while recording",
    "録っている間に音が {count} 回とぎれました（PC がいそがしかったため）。とぎれた所は無音で残しています。バッファを大きくすると、とぎれにくくなります。",
    { count: "3" },
  ),
  "msg.audioFound": s6("Found by searching", "{file} を見つけました。{clips} つのクリップがまた鳴ります。", {
    file: "歌 テイク 1.wav",
    clips: "2",
  }),
  "msg.audioFoundMismatch": s6("Searched file does not match", "えらんだファイルは使えません（サンプルレートかチャンネルの数がちがいます）。"),
  "msg.audioBroken": s6("Broken file, 3 or more channels", "「{file}」はこわれているので読めません。", { file: "声.wav" }, "part"),
  "msg.audioTooManyChannels": s6(
    "Broken file, 3 or more channels",
    "「{file}」は 3 チャンネル以上なので読めません（読めるのはモノとステレオ）。",
    { file: "8ch.wav" },
    "part",
  ),
  "msg.savingAudio": s6("Copying the audio while saving", "保存しています ・ 音のファイルを写しています {done} / {total}（{doneMb} / {totalMb} MB）", {
    done: "3",
    total: "12",
    doneMb: "80",
    totalMb: "240",
  }),
  "msg.diskTooSlow": s6("Disk too slow", "音のファイルを読むのが間に合わず、歌が少し切れました（ディスクがいそがしかったため）。"),
  // Sound pack (spec 2, spec 3 (43-B, 44-A, 45-A), spec 6, mock F8-Pack*). The two lines of the hint band split the spec 6 text.
  "pack.fetching": s6("Fetching the sound pack", "音の素材を取りにいっています", undefined, "part"),
  "pack.fetchingBytes": s6("Fetching the sound pack", "{percent} % ・ {done} / {total} MB", { percent: "38", done: "92", total: "240" }, "part"),
  "pack.fetchingLeft": s6("Fetching the sound pack", "のこり 約 {minutes} 分", { minutes: "2" }, "part"),
  "pack.fetched": s6("Sound pack fetched", "音の素材をぜんぶ取りました。{count} の音色が鳴ります。", { count: "31" }),
  "pack.offline": s6(
    "Sound pack: not connected",
    "音の素材を取りにいけませんでした（インターネットにつながっていません）。つながったら「もう一度」を押してください。",
  ),
  "pack.cutOff": s6("Sound pack: cut off on the way", "とちゅうで切れました。{done} / {total} MB まで取ってあります。続きから取ります。", {
    done: "92",
    total: "240",
  }),
  "pack.diskFull": s6("Sound pack: disk full", "ディスクがいっぱいで、音の素材を置けません（あと {mb} MB 要ります）。", { mb: "150" }),
  "pack.corrupt": s6("Sound pack: broken", "取った音の素材がこわれていました。取りなおしても直りませんでした。"),
  "pack.newPack": s6("Sound pack: new pack", "音の素材の新しい包みがあります（{mb} MB）。", { mb: "12" }),
  "button.fetchPack": e("取りにいく", "spec:6"),
  "button.resumeFetch": e("続きから", "spec:6"),
  "button.packLater": e("あとで", "spec:3"),
  "pack.ask": e("音の素材（約 {mb} MB）を取りにいきます", "spec:3", { mb: "240" }),
  "pack.askDetail": e("グランドピアノなど {count} の音色 ・ 取っている間も弾けます", "spec:3", { count: "31" }),
  // When the small text of the asking band does not fit in a window 853 wide.
  "pack.askDetailShort": e("取っている間も弾けます", "mock:R12-Fit"),
  "pack.soundReady": e("{instrument} が鳴るようになりました ・ のこりの音色を取っています", "spec:3", { instrument: "グランドピアノ" }),
  "pack.askingStatus": e("音の素材がまだありません ・ いまはサイン波で鳴ります", "mock:F8-PackAsk"),
  "pack.failedTitle": e("音の素材を取りにいけませんでした", "mock:F8-PackFail"),
  "pack.failedOfflineDetail": e("インターネットにつながっていません ・ {done} / {total} MB まで取ってあります", "mock:F8-PackFail", {
    done: "92",
    total: "240",
  }),
  "shelf.packFetching": e("取ってきています", "spec:2"),
  "shelf.packNotYet": e("まだ", "spec:2"),
  "settings.packRow": e("音の素材", "spec:3"),
  "settings.packAll": e("ぜんぶあります ・ {mb} MB", "spec:3", { mb: "240" }),
  "settings.packNotYet": e("まだ {have} / {total}", "spec:3", { have: "12", total: "31" }),
  // The row 「AI とつなぐ」 of the advanced settings (mock P10-AiLink, option B).
  "settings.aiLink": e("AI とつなぐ", "user:2026-11-23"),
  "settings.aiLinkOn": e("オン", "user:2026-11-23"),
  "settings.aiLinkOff": e("オフ", "user:2026-11-23"),
  "settings.aiClosed": e("閉じています", "user:2026-11-23"),
  "settings.aiWaiting": e("待っています", "user:2026-11-23"),
  "settings.aiConnected": e("つながっています", "user:2026-11-23"),
  "settings.aiLinkNote": e("オンの間だけ、この PC の中に口をひらきます。同じユーザーのアプリだけがつなげます。", "user:2026-11-23"),
  "settings.aiLinkCan": e("できること: 曲を読んで助言する ・ 頼まれたら書く（1 手ずつ取り消せます） ・ 書き出した音を調べる", "user:2026-11-23"),
  // 「新しい版をたしかめる」 (round 12, D1): the status line, 「meloet について」 and the row of the advanced settings.
  "update.newVersion": e("新しい版 {version} があります", "user:2026-11-23", { version: "0.3.0" }),
  "button.openReleasePage": e("ページをひらく", "user:2026-11-23"),
  "button.newVersionLater": e("あとで", "user:2026-11-23"),
  "settings.checkUpdates": e("新しい版をたしかめる", "user:2026-11-23"),
  "settings.checkUpdatesOn": e("オン", "user:2026-11-23"),
  "settings.checkUpdatesOff": e("オフ", "user:2026-11-23"),
  // 「音が大きすぎます」 (round 12, M2): the status line while it plays and after; {db} is the measured amount to a tenth, the
  // button's {db} the whole dB it lowers by. 「閉じる」 is button.close.
  "msg.tooLoudLimiter": e("音が大きすぎます。リミッターがずっと {db} dB おさえています", "user:2026-11-23", { db: "6" }),
  "msg.tooLoudOver": e("音が大きすぎます。マスターが {db} dB 越えています", "user:2026-11-23", { db: "2.1" }),
  "button.lowerAllTracks": e("全部のトラックを {db} dB 下げる", "user:2026-11-23", { db: "6" }),

  // [Menus, keys]
  // Key names written at the right of menu rows (the table in spec 4). Mod becomes key.ctrl or key.cmd.
  "key.shift": e("Shift", "spec:4"),
  "key.delete": e("Delete", "spec:4"),
  "key.cut": e("X", "spec:4"),
  "key.copy": e("C", "spec:4"),
  "key.paste": e("V", "spec:4"),
  "key.duplicate": e("D", "spec:4"),
  "key.selectAll": e("A", "spec:4"),
  "key.export": e("E", "spec:4"),
  "key.audioSettings": e(",", "spec:4"),
  "key.zoomIn": e("+", "spec:4"),
  "key.zoomOut": e("−", "spec:4"),
  "key.take": e("T", "spec:4"),
  "key.split": e("S", "spec:4"),
  "key.join": e("J", "spec:4"),
  "key.mute": e("M", "spec:4"),
  "key.solo": e("S", "spec:4"),
  // Mod Q quits meloet (not in the spec 4 table; docs/history/design-notes.ja.md B-59).
  "key.quit": e("Q", "user:2026-11-18"),

  // [Track heads and shelf screens]
  // The 「トラック」 and 「ヘルプ」 rows of the menu bar (the menu bar of spec 3). Context menus use the same names.
  "menu.renameTrack": e("名前を変える", "spec:3"),
  "menu.duplicateTrack": e("トラックを複製", "user:2026-11-23"),
  "menu.trackColor": e("色", "spec:3"),
  "menu.moveTrackUp": e("上へ動かす", "spec:3"),
  "menu.moveTrackDown": e("下へ動かす", "spec:3"),
  "menu.deleteTrack": e("トラックを消す", "spec:3"),
  "menu.help": e("ヘルプ", "spec:3"),
  "menu.keyList": e("キーの一覧", "user:2026-11-23"),
  "menu.userGuide": e("使い方をひらく", "user:2026-11-23"),
  "menu.about": e("meloet について", "spec:3"),
  // Rows of the color list (spec 2 「「色 1」〜「色 5」」).
  "menu.colorNumber": e("色 {number}", "spec:2", { number: "1" }),
  "key.renameTrack": e("F2", "spec:4"),
  "key.favorite": e("F", "spec:4"),
  // Key names of the key list (spec 4, round 12 G1).
  "key.up": e("↑", "spec:4"),
  "key.down": e("↓", "spec:4"),
  "key.left": e("←", "spec:4"),
  "key.right": e("→", "spec:4"),
  "key.f10": e("F10", "spec:4"),
  "key.tab": e("Tab", "spec:4"),
  "key.marker": e("K", "spec:4"),
  "key.find": e("F", "spec:4"),
  "key.plus": e("＋", "spec:4"),
  "key.wheel": e("ホイール", "spec:4"),
  "key.drag": e("ドラッグ", "spec:4"),
  // The keys of round 12 wave 2 (R1, R2, A1).
  "key.keepLast": e("R", "user:2026-11-23"),
  "key.playByKeys": e("K", "user:2026-11-23"),
  "key.repeatClips": e("R", "user:2026-11-23"),

  // [About meloet] (spec 3, mock R7-About, R7-AboutLicense)
  "about.title": e("meloet について", "mock:R7-About"),
  "about.close": e("閉じる", "mock:R7-About"),
  "about.back": e("もどる", "mock:R7-AboutLicense"),
  "about.name": e("meloet", "mock:R7-About"),
  "about.tagline": e(
    "MIDI 鍵盤ではじめる、音楽づくりのアプリ",
    "mock:R7-About",
  ),
  "about.version": e("バージョン", "mock:R7-About"),
  "about.license": e("meloet のライセンス", "mock:R7-About"),
  "about.licenseOr": e("{a} または {b}", "mock:R7-About", {
    a: "MIT",
    b: "Apache-2.0",
  }),
  "about.openSource": e("オープンソースのライセンス", "mock:R7-About"),
  "about.openSourceSummary": e(
    "音の素材 ・ 書体 ・ 部品 約 {count}",
    "mock:R7-About",
    { count: "500" },
  ),
  "about.backTo": e("◂ {name}", "mock:R7-AboutLicense", {
    name: "meloet について",
  }),
  "about.search": e("さがす", "mock:R7-AboutLicense"),
  "about.searchLabel": e("名前 ・ ライセンス", "mock:R7-AboutLicense"),
  "about.sounds": e("音の素材", "mock:R7-AboutLicense"),
  "about.fonts": e("書体", "mock:R7-AboutLicense"),
  "about.parts": e("部品（ライセンスごと）", "mock:R7-AboutLicense"),
  "about.soundName": e("名前", "mock:R7-AboutLicense"),
  "about.soundCreator": e("作った人", "spec:3"),
  "about.soundLicense": e("ライセンス", "mock:R7-AboutLicense"),
  "about.soundSource": e("出どころ", "spec:3"),
  "about.soundChanges": e("手を加えたこと", "spec:3"),
  // What was changed (for each sound pack source). Written like 「音の大きさをそろえ、ファイルの形を変えた」 in the mock,
  // with the same content as CHANGES in tools/sounds/build.py (the English text of NOTICE).
  "about.changes.salamander-grand-piano": e(
    "16 段の強さから 4 段と、離した音 ・ 共鳴 ・ ペダルの音をえらび、音の前の無音を切り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.gregsullivan-wurlitzer-ep200": e(
    "音の前の無音を切り、音の終わりにくり返す所を作り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.naked-drums": e(
    "直のマイクと上のマイクを作者の GM の組みのとおりにステレオにまぜ、11 の音とテイク 4 つをえらび、音の前の無音を切り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.vcsl-claps": e(
    "クラップのテイク 6 つを使い、音の前の無音を切り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.vcsl-pipe-organ": e(
    "くり返す所を作ってその終わりで切り、音の前の無音を切り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.black-and-blue-basses": e(
    "指弾きのテイク 4 つから 2 つをえらび、鍵と鳴る高さを 1 オクターブずらしてそろえ、終わりの 100 ms を小さくして消し、音の前の無音を切り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.vsco-2-ce-strings": e(
    "ヴァイオリン ・ ヴィオラ ・ チェロ ・ コントラバスを 1 つの鍵の範囲にまとめ、くり返す所を作り、音の前の無音を切り、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  "about.changes.fluidr3-gm": e(
    "オルガン 5 つ、ビブラフォン、マリンバ、ギター 4 つ、ベース 2 つ、ピチカート、管楽器 4 つと、ブラシキットの 12 の音をとり出し、左右の組と重なって鳴る層を 1 つのファイルにまとめ、同じ音を使う強さの段を 1 つにし、くり返す所の終わりと音の前の無音を切り、音量の減りを（4 つはフィルタも）ファイルに焼きこみ、ずれていたドラムの左右の高さをそろえ、ほかのフィルタ・ゆれ・響きの送りを外し、ファイルの形を変えた",
    "user:2026-11-19",
  ),
  // [Key list] (round 12 G1). Row labels that are not menu rows are builder text.
  "keyList.title": e("キーの一覧", "user:2026-11-23"),
  "keyList.search": e("さがす…", "user:2026-11-23"),
  "keyList.openGuide": e("使い方のページをひらく", "user:2026-11-23"),
  "keyList.or": e(" ／ ", "user:2026-11-23"),
  "keyList.group.file": e("曲とファイル", "user:2026-11-23"),
  "keyList.group.play": e("再生と録音", "user:2026-11-23"),
  "keyList.group.bands": e("帯", "user:2026-11-23"),
  "keyList.group.arrange": e("アレンジ", "user:2026-11-23"),
  "keyList.group.pianoRoll": e("ピアノロール", "user:2026-11-23"),
  "keyList.group.mixer": e("ミキサー", "user:2026-11-23"),
  "keyList.group.view": e("表示", "user:2026-11-23"),
  "keyList.playStop": e("再生 ／ 止める", "user:2026-11-23"),
  "keyList.quantize": e("クオンタイズをかけなおす", "user:2026-11-23"),
  "keyList.keepLast": e("さっきのを残す", "user:2026-11-23"),
  "keyList.playByKeys": e("キーで弾く", "user:2026-11-23"),
  "keyList.repeatClips": e("クリップをくり返す", "user:2026-11-23"),
  "keyList.back": e("ひとつ戻る ／ 閉じる", "builder:2026-11-23"),
  "keyList.menuBar": e("メニューバーへ", "builder:2026-11-23"),
  "keyList.focus": e("次のボタンや欄へ ／ 前へ", "builder:2026-11-23"),
  "keyList.renameTrack": e("トラック ／ クリップの名前を変える", "builder:2026-11-23"),
  "keyList.tools": e("道具：選ぶ ／ 書く ／ 消す", "builder:2026-11-23"),
  "keyList.nudgeKeys": e("ノートを半音 ／ 1 オクターブ動かす", "builder:2026-11-23"),
  "keyList.nudgeSteps": e("ノートをグリッド 1 つ動かす", "builder:2026-11-23"),
  "keyList.noteList": e("ノートの一覧：欄に打ちこむ ／ 右の欄へ", "builder:2026-11-23"),
  "keyList.dragCopy": e("複製しながら動かす", "builder:2026-11-23"),
  "keyList.dragFree": e("グリッドに合わせずに動かす", "builder:2026-11-23"),
  "keyList.dragCancel": e("ドラッグをやめて元に戻す", "builder:2026-11-23"),
  "keyList.addMarker": e("マーカーを足す", "builder:2026-11-23"),
  "keyList.stepMarker": e("次のマーカーへ ／ 前のマーカーへ", "builder:2026-11-23"),
  "keyList.renameMarker": e("マーカーの名前を変える", "builder:2026-11-23"),
  "keyList.deletePoints": e("選んだ点 ／ マーカーを消す", "builder:2026-11-23"),
  "keyList.tempoBox": e("テンポ ／ 拍子の箱で 1 ／ 10 ずつ変える", "builder:2026-11-23"),
  "keyList.shelfStep": e("棚：上 ／ 下の行を選ぶ", "builder:2026-11-23"),
  "keyList.shelfConfirm": e("棚：選んだ音を入れる", "builder:2026-11-23"),
  "keyList.shelfSample": e("棚：サンプルを鳴らす ／ 止める", "builder:2026-11-23"),
  "keyList.shelfFavorite": e("棚：お気に入りの印", "builder:2026-11-23"),
  "keyList.historyStep": e("履歴：上 ／ 下の行を選ぶ", "builder:2026-11-23"),
  "keyList.historyHome": e("履歴：いまのステップへ", "builder:2026-11-23"),
  "keyList.historyConfirm": e("履歴：ここまで戻す", "builder:2026-11-23"),
  "keyList.bypass": e("選んだインサートをとばす ／ 戻す", "builder:2026-11-23"),
  "keyList.removeInsert": e("選んだインサートを外す", "builder:2026-11-23"),
  "keyList.fader": e("フェーダー ／ センドを動かす（細かく）", "builder:2026-11-23"),
  "keyList.pan": e("パンを動かす（大きく）", "builder:2026-11-23"),
  "keyList.mixHome": e("フェーダー ／ パン ／ センドを最初の値に戻す", "builder:2026-11-23"),
  "keyList.mixType": e("フェーダー ／ パン ／ センドに値を打ちこむ", "builder:2026-11-23"),
  "keyList.paramStep": e("パラメータ：上 ／ 下の行を選ぶ", "builder:2026-11-23"),
  "keyList.paramMove": e("パラメータ：値を動かす（細かく）", "builder:2026-11-23"),
  "keyList.paramType": e("パラメータ：値を打ちこむ", "builder:2026-11-23"),
  "keyList.paramFind": e("パラメータ：さがす箱へ", "builder:2026-11-23"),
  // [Text made by the screen alone] (spec 3, spec 6 version 12, mock F8-*)
  // Status line progress while the sound pack is fetched and a flow hint is shown (「44-B の形」 of spec 3 (44-A), mock Opt8-PackShow).
  "pack.fetchingPercent": e("{percent} %", "mock:Opt8-PackShow", { percent: "38" }),
  // The params panel when two or more audio clips are chosen (spec 3 (40-A)), and the status line then (mock F8-MultiClip).
  "clips.selected": e("{count} クリップを選んでいます", "spec:3", { count: "3" }),
  "clips.groupAll": e("まとめて変える", "spec:3"),
  "clips.onlyOne": e("位置 ・ 長さ ・ フェード ・ この音のテンポは、1 つだけ選ぶと出ます。", "spec:3"),
  "status.clipsSelected": e("選択 {count} クリップ ・ 欄の「音量」に打つと、{count} つともその音量になります", "mock:F8-MultiClip", { count: "3" }),
  // Check before export (spec 3 (41-A, 42-C)). {peak} is in the form 「−3.5 dB」.
  "export.checkPreparing": e("用意をしています", "spec:3"),
  "export.checkChecking": e("たしかめています", "spec:3"),
  "export.checkPeakSoFar": e("ここまでのいちばん大きい所 {peak}", "spec:3", { peak: "−3.5 dB" }),
  "export.checkOver": e("0 dB をこえる所が {count} か所あります（いちばん大きい所 {peak} dB）。音が割れます。", "spec:3", { count: "3", peak: "+1.2" }),
  "export.checkOverHint": e("マスターのフェーダーを下げるか、リミッターの上限を見てください。", "spec:3"),
  "export.solo": e("いま {track} をソロにしています。書き出しには入れません（曲ぜんぶを書き出します）。", "spec:3", { track: "2 ベース" }),
  "export.soloMore": e("{track} ほか {count} 本", "spec:3", { track: "2 ベース", count: "1" }),
  // 「音の大きさ」 of the export and its lines in the check (round 12 F1). {lufs}, {peak} and {db} are numbers like 「−18.6」.
  "export.loudness": e("音の大きさ", "user:2026-11-23"),
  "export.loudnessAsIs": e("そのまま", "user:2026-11-23"),
  "export.loudnessMatch": e("公開先に合わせる", "user:2026-11-23"),
  "export.loudnessTarget": e("{lufs} LUFS ・ {peak} dBTP", "user:2026-11-23", { lufs: "−14", peak: "−1" }),
  "export.checkLoudness": e("曲ぜんたいの大きさ {lufs} LUFS ・ トゥルーピーク {peak} dBTP", "user:2026-11-23", { lufs: "−18.6", peak: "−3.2" }),
  "export.checkLoudnessSoFar": e("ここまでの大きさ {lufs} LUFS ・ トゥルーピーク {peak} dBTP", "builder:2026-11-23", { lufs: "−18.6", peak: "−3.2" }),
  "export.loudnessLouder": e("合わせると {db} dB 大きくなります", "user:2026-11-23", { db: "4.6" }),
  "export.loudnessQuieter": e("合わせると {db} dB 小さくなります", "builder:2026-11-23", { db: "2.4" }),
  "export.loudnessLimited": e("−1 dBTP を越えないよう、{lufs} LUFS になります", "user:2026-11-23", { lufs: "−15.2" }),
  // 「曲の情報」 in 「くわしく」 of the export (round 12 F2).
  "export.songInfo": e("曲の情報", "user:2026-11-23"),
  "export.songTitle": e("題名", "user:2026-11-23"),
  "export.artist": e("作者", "user:2026-11-23"),
  "export.picture": e("絵", "user:2026-11-23"),
  "export.choosePicture": e("えらぶ…", "user:2026-11-23"),
  "export.removePicture": e("はずす", "builder:2026-11-24"),
  "export.noPicture": e("なし", "builder:2026-11-24"),
  "msg.exportPictureTooLarge": e("「{file}」は大きすぎるので使えません（絵は {mb} MB まで）。", "builder:2026-11-24", {
    file: "表紙.png",
    mb: "5",
  }),
  "msg.exportPictureUnreadable": e("「{file}」は絵として読めません（使えるのは png ・ jpg）。", "builder:2026-11-24", { file: "表紙.bmp" }),
  // The dialog 「書き出しました」 when an export ends (round 12 E9). 「フォルダをひらく」 and 「閉じる」 are button.openFolder and button.close.
  "exported.title": e("書き出しました", "user:2026-11-23"),
  "exported.files": e("{count} つのファイル", "user:2026-11-23", { count: "5" }),
  "exported.length": e("{minutes} 分 {seconds} 秒", "user:2026-11-23", { minutes: "3", seconds: "12" }),
  "exported.lengthSeconds": e("{seconds} 秒", "builder:2026-11-23", { seconds: "45" }),
  "exported.details": e("{length} ・ {rate} Hz ・ {bits} ビット", "user:2026-11-23", { length: "3 分 12 秒", rate: "48000", bits: "24" }),
  "exported.loudness": e("大きさ {lufs} LUFS ・ トゥルーピーク {peak} dBTP", "user:2026-11-23", { lufs: "−14.0", peak: "−1.0" }),
  "exported.play": e("PC のプレーヤーで聞く", "user:2026-11-23"),
  // The rest of the status line while the export dialog is open (after 「「書き出し」のダイアログをひらいています ・ 」).
  "status.exportChecking": e("たしかめています {percent} %", "mock:F8-ExportCheck", { percent: "38" }),
  "status.exportOver": e("0 dB をこえる所があります", "mock:F8-ExportClip"),
  // When the tempo point at the start of the song begins a ramp (spec 3 (94-10)).
  "msg.headTempoRampSelected": s6(
    "Tempo at the song start (start of a ramp) chosen",
    "曲の頭のテンポをえらんでいます ・ {tempo} から {toPlace}の {toTempo} へ、だんだん{direction}",
    { tempo: "96.00", toPlace: "5 小節目", toTempo: "120.00", direction: "速く" },
  ),
  // Recording two or more vocals. With the monitor off, 「モニター オン ・ 」 is left out (the rest of the same scene text).
  "msg.recordingVocals": s6(
    "Recording 2 or more vocal tracks",
    "● 録音中 ・ {vocals}・ モニター オン ・ 止めるには {stopKey}",
    { vocals: "6 歌（入口 1）、7 歌 2（入口 2）", stopKey: "Space" },
    "part",
  ),
  // Text under the name in the middle box while one audio file is being dragged in.
  "drop.audioOneSay": s6("Dragging in one audio file", "ここに落とすと、そのトラックの落とした位置に入れます。", undefined, "part"),
  "drop.audioOneNewTrack": s6("Dragging in one audio file", "新しい歌のトラックに入れます。", undefined, "quoted"),
  // While dragging a fade handle (status line; the live reading is only {value}). {edge} is clip.fadeEdgeIn or clip.fadeEdgeOut.
  "msg.fadeDragging": s6(
    "Moving a fade",
    "{clip} の{edge}のフェード {value}（ドラッグ中）・ 離すと決まります ・ やめるには {cancelKey}",
    { clip: "歌 テイク 1", edge: "前", value: "0. 1. 0480", cancelKey: "Esc" },
  ),
  "clip.fadeEdgeIn": e("前", "spec:6"),
  "clip.fadeEdgeOut": e("後", "mock:F7-AudioEdit"),
  // Status line while dragging the pitch (semitones, cents). The live reading has the form of clip.pitchMarkCents (「+2 ・ +30」).
  "msg.pitchDragging": s6("Moving the pitch", "音の高さ {semitones} 半音 {cents} セント（ドラッグ中）", { semitones: "+2", cents: "+30" }),
  // Status line while dragging the strength of 「音程を寄せる」 (round 12, M7; in the form of msg.pitchDragging).
  "msg.tuneDragging": e("音程を寄せる {percent} %（ドラッグ中）", "builder:2026-11-23", { percent: "60" }),
  // The unit after the 「手で決める…」 box (spec 3 (94-19) 「「8.0」と「ms」」).
  "settings.latencyUnit": e("ms", "spec:3"),
  // Short, at the end of the status line left while recording during export (spec 3 (94-84)).
  "status.exportShort": s6("Recording while exporting", "書き出し {percent} %", { percent: "46" }, "part"),
} as const satisfies Record<string, Entry>;

export type TextKey = keyof typeof texts;

/**
 * The 「手の名前」 (step names) of the undo history (spec 3 (Y19)). One for each kind of step name in the protocol (kind of StepName).
 * spec:3 is how the step name list in spec 3 says it (「CC の点を書く ・ 動かす ・ 消す」 is opened up as 「CC の点を動かす」 and so on.
 * 「録音（テイク）」 in the list is 「録音」. 「クリップを … 切る」 is S (cut at the play line; spec 3 "a fix to the mock"), and
 * cut (Mod X) is 「クリップを切り取る」 for now (docs/history/design-notes.ja.md B-94-7)).
 * Provisional names added to the list in version 11 (92-7) are also spec:3. Names not in the list are builder: (docs/history/design-notes.ja.md B-92).
 */
export const stepNames: Readonly<Record<StepName["kind"], Entry>> = {
  writeNote: e("ノートを書く", "spec:3"),
  deleteNotes: e("ノートを消す", "spec:3"),
  cutNotes: e("ノートを切り取る", "builder:2026-11-20"),
  moveNotes: e("ノートを動かす", "spec:3"),
  duplicateNotes: e("ノートを複製する", "spec:3"),
  resizeNotes: e("ノートの長さを変える", "spec:3"),
  changeVelocity: e("ベロシティを変える", "spec:3"),
  pasteNotes: e("ノートを貼りつける", "spec:3"),
  quantize: e("クオンタイズ", "spec:3"),
  unquantize: e("もとの位置へ戻す", "spec:3"),
  addControl: e("CC の点を書く", "spec:3"),
  moveControls: e("CC の点を動かす", "spec:3"),
  deleteControls: e("CC の点を消す", "spec:3"),
  record: e("録音", "spec:3"),
  switchTake: e("テイクを切りかえる", "spec:3"),
  deleteTake: e("テイクを消す", "spec:3"),
  moveClips: e("クリップを動かす", "spec:3"),
  duplicateClips: e("クリップを複製する", "spec:3"),
  cutClips: e("クリップを切り取る", "builder:2026-11-21"),
  pasteClips: e("クリップを貼りつける", "spec:3"),
  trimClip: e("クリップの長さを変える", "spec:3"),
  deleteClips: e("クリップを消す", "spec:3"),
  addTrack: e("トラックを足す", "spec:3"),
  deleteTrack: e("トラックを消す", "spec:3"),
  renameTrack: e("トラックの名前を変える", "spec:3"),
  moveTrack: e("トラックを並べかえる", "spec:3"),
  setTrackColor: e("トラックの色を変える", "spec:3"),
  setInstrument: e("音源を入れかえる", "spec:3"),
  removeInstrument: e("音源を外す", "spec:3"),
  removeMissing: e("見つからない物を曲から外す", "spec:3"),
  addEffect: e("エフェクトを入れる", "spec:3"),
  removeEffect: e("エフェクトを外す", "spec:3"),
  moveInsert: e("エフェクトを並べかえる", "spec:3"),
  replaceEffect: e("エフェクトを入れかえる", "spec:3"),
  bypassEffect: e("エフェクトをとばす", "spec:3"),
  restoreEffect: e("エフェクトをもどす", "spec:3"),
  setParam: e("{effect}の「{param}」を変える", "spec:3", { effect: "コンプレッサー", param: "しきい値" }),
  setPluginParam: e("{effect}の「{param}」を変える", "spec:3", { effect: "コンプレッサー", param: "しきい値" }),
  setGain: e("音量を変える", "spec:3"),
  setPan: e("パンを変える", "spec:3"),
  setSend: e("センドを変える", "spec:3"),
  mute: e("ミュート", "spec:3"),
  unmute: e("ミュートを外す", "builder:2026-11-20"),
  pluginWindow: texts["plugin.windowStep"],
  addLane: e("帯を足す", "spec:3"),
  deleteLane: e("帯を消す", "spec:3"),
  addAutoPoint: e("オートメーションの点を書く", "spec:3"),
  moveAutoPoints: e("オートメーションの点を動かす", "spec:3"),
  deleteAutoPoints: e("オートメーションの点を消す", "spec:3"),
  writeAutomation: e("オートメーションを書く", "spec:3"),
  restLane: e("オートメーションを休ませる", "spec:3"),
  restoreLane: e("オートメーションをもどす", "spec:3"),
  setTempo: e("テンポを変える", "spec:3"),
  setMeter: e("拍子を変える", "spec:3"),
  addTimelineChange: e("テンポ ・ 拍子の変更を足す", "spec:3"),
  moveTimelineChange: e("テンポ ・ 拍子の変更を動かす", "spec:3"),
  deleteTimelineChange: e("テンポ ・ 拍子の変更を消す", "spec:3"),
  setTempoRamp: e("だんだんにする", "spec:3"),
  clearTempoRamp: e("だんだんをやめる", "spec:3"),
  addMarker: e("マーカーを足す", "spec:3"),
  renameMarker: e("マーカーの名前を変える", "spec:3"),
  moveMarker: e("マーカーを動かす", "spec:3"),
  deleteMarker: e("マーカーを消す", "spec:3"),
  setLoop: e("ループの範囲を変える", "spec:3"),
  recovered: e("なおすためのファイルから戻す", "mock:F-Recover2"),
  discardRecovered: e("戻した変更をすてる（最後に保存した形にする）", "spec:6"),
  setVocalInput: e("歌の入口を変える", "builder:2026-11-21"),
  splitClips: e("クリップを切る", "spec:3"),
  importAudio: e("音のファイルを入れる", "builder:2026-11-21"),
  setClipGain: e("クリップの音量を変える", "builder:2026-11-21"),
  setClipFade: e("フェードを変える", "builder:2026-11-21"),
  setClipFollow: e("のび縮みを変える", "builder:2026-11-21"),
  setSourceTempo: e("この音のテンポを決める", "builder:2026-11-21"),
  setClipPitch: e("音の高さを変える", "builder:2026-11-21"),
  renameClip: e("クリップの名前を変える", "user:2026-11-23"),
  joinClips: e("クリップをつなげる", "user:2026-11-23"),
  writeClip: e("クリップを書く", "spec:3"),
  scaleVelocity: e("ベロシティを割合で変える", "user:2026-11-23"),
  rampVelocity: e("ベロシティをだんだん変える", "user:2026-11-23"),
  randomVelocity: e("ベロシティをばらつかせる", "user:2026-11-23"),
  legato: e("次の音までのばす", "user:2026-11-23"),
  duplicateTrack: e("トラックを複製", "user:2026-11-23"),
  normalizeClips: e("ノーマライズ", "user:2026-11-23"),
  reverseClips: e("逆にする", "user:2026-11-23"),
  keepPlayed: e("さっきの演奏を残す", "user:2026-11-23"),
  placePattern: e("パターンを置く", "user:2026-11-23"),
  repeatClips: e("クリップをくり返す", "user:2026-11-23"),
  applyPreset: e("プリセットを使う", "user:2026-11-23"),
  lowerAll: e("全部のトラックを下げる", "user:2026-11-23"),
  setArp: e("アルペジオを変える", "user:2026-11-23"),
  setSoundKnob: e("音のつまみを変える", "user:2026-11-23"),
  duplicateSection: e("区間を複製", "user:2026-11-23"),
  deleteSection: e("区間を消して詰める", "user:2026-11-23"),
  insertSpace: e("空きを入れる", "user:2026-11-23"),
  bounce: e("音のクリップにする", "user:2026-11-23"),
  addFadeOut: e("フェードアウトをつける", "user:2026-11-23"),
  setComp: e("いいとこ取り", "user:2026-11-23"),
  // The spec names only the step of 「いいとこ取り」; this one undoes it with the verb of 「もとに戻す」 (「外し」).
  clearComp: e("いいとこ取りを外す", "builder:2026-11-24"),
  setClipTune: e("音程を寄せる", "user:2026-11-23"),
};

export interface Term {
  readonly term: string;
  readonly description: string;
  readonly source: Source;
}

const st = (term: string, description: string): Term => ({
  term,
  description,
  source: "spec:5",
});
const ut = (term: string, description: string): Term => ({ term, description, source: "user:2026-11-23" });

/** spec 5 "Term tips". The form is 「用語 — 説明」. */
export const terms: readonly Term[] = [
  st(
    "ソロ（書き出し）",
    "ソロは聞くための物なので、書き出しには入らない。ソロのとおりに書き出したいときは、ミュートか「トラックごと」で。",
  ),
  st(
    "音の素材の包み",
    "グランドピアノなど 31 の音色の音。meloet には入っておらず、はじめて起動したときに取りにいく（約 240 MB）。",
  ),
  st("モニター", "入ってくる自分の声を、meloet を通してヘッドホンで聞くこと。少し遅れて聞こえる。"),
  st("波形", "音の大きさを、時間にそってならべた形。"),
  st("入口のチャンネル", "音の入口の番号。マイクが 1 本ならモノ（入口 1）、2 本ならステレオ（入口 1 ・ 2）。"),
  st("のび縮み", "音の高さを変えずに、音を長く ・ 短くすること。タイムストレッチとも呼ぶ。"),
  st("セント", "半音を 100 に分けた、音の高さの細かい目盛り。"),
  st("声の感じを保つ", "音の高さを変えても、声が子どものように ・ 低い声のようにならないようにする。フォルマントとも呼ぶ。"),
  st("この音のテンポ", "その音のファイルを作ったときのテンポ。曲のテンポとくらべて、のび縮みの量を決める。"),
  st("しっぽ", "音を止めたあとに残る響き。書き出しでは、これが消えるまで書く。"),
  st(
    "休み（オートメーション）",
    "手で動かしたので、オートメーションがそのパラメータを動かすのを休んでいること。「もどす」で線のとおりに戻る。",
  ),
  st("だんだん", "テンポを、ある点から次の点まで、少しずつ変えていくこと。"),
  st("手", "取り消し ・ やりなおしの 1 回分。ドラッグ 1 回、ボタン 1 回が 1 手。"),
  st("テンポの変更", "曲のとちゅうで、速さを変える所。"),
  st("拍子の変更", "曲のとちゅうで、1 小節の拍の数を変える所。小節の頭でだけ変えられる。"),
  st(
    "とばす",
    "エフェクトを外さずに、音がそこを通らないようにする。かける前と後をくらべるのに使う。ほかでは「バイパス」とも呼ぶ。",
  ),
  st("作った人", "プラグインを作った人や会社。プラグインが自分で言ってくる。"),
  st("SF2", "音色がたくさん入った 1 つのファイル。サウンドフォントとも呼ぶ。"),
  st(
    "GM",
    "音色の番号の、世界共通の決まり。1 番がピアノ、ドラムは決まった鍵に決まった楽器。",
  ),
  st(
    "サイン波",
    "いちばん単純な、まるい音。音の高さを確かめるのにむいている。",
  ),
  st("キック", "いちばん低い、足で鳴らす大きな太鼓。バスドラムとも呼ぶ。"),
  st("スネア", "「タン」と鳴る、手前の小さな太鼓。"),
  st("サイドスティック", "スネアのふちを、ばちで「コッ」と鳴らす音。"),
  st("クラップ", "手をたたく音。"),
  st(
    "タム",
    "音の高さのちがう、まるい太鼓。ハイ ・ ミッド ・ ロー ・ フロアがある。",
  ),
  st(
    "ハイハット",
    "2 枚のシンバルを合わせた物。とじて「チッ」（クローズ）、ひらいて「シャー」（オープン）、ペダルで「チャッ」（ペダル）。",
  ),
  st("クラッシュ", "曲の区切りで「ジャーン」と鳴らすシンバル。"),
  st("ライド", "「チーン」と刻む、大きなシンバル。"),
  st("リバーブ", "部屋の響きを足すエフェクト。"),
  st("ディレイ", "やまびこのように、音をくり返すエフェクト。"),
  st(
    "EQ",
    "音の高さごとに、大きさを上げ下げするエフェクト。イコライザーとも呼ぶ。",
  ),
  st("コンプレッサー", "大きい音をおさえて、音量をそろえるエフェクト。"),
  st(
    "リミッター",
    "決めた大きさをこえないようにして、音割れを防ぐエフェクト。",
  ),
  st("コーラス", "音を少しずらして重ね、厚みを出すエフェクト。"),
  st(
    "響きの長さ",
    "響きが消えるまでの時間。ほかのリバーブでは「ディケイ」「リバーブタイム」とも呼ぶ。",
  ),
  st(
    "部屋の大きさ",
    "響く部屋の広さ。ほかのリバーブでは「サイズ」「ルームサイズ」とも呼ぶ。",
  ),
  st("プリディレイ", "元の音から、響きが始まるまでの時間。"),
  st(
    "高い音を落とす",
    "この高さより上の音を弱くする。ほかでは「ハイカット」とも呼ぶ。",
  ),
  st(
    "低い音を落とす",
    "この高さより下の音を弱くする。ほかでは「ローカット」とも呼ぶ。",
  ),
  st("広がり", "響きの左右の広さ。ほかでは「ワイズ」「ステレオ幅」とも呼ぶ。"),
  st(
    "ミックス",
    "元の音と、エフェクトの音の混ぜかた。トラックとマスターでは、はじめ 30 %、戻りでは 100 %。ほかでは「ドライ ・ ウェット」とも呼ぶ。",
  ),
  st("間隔", "くり返すまでの時間。"),
  st("テンポに合わせる", "オンなら、間隔を曲のテンポの音符の長さで決める。"),
  st(
    "くり返し",
    "くり返しの回数の多さ。ほかのディレイでは「フィードバック」とも呼ぶ。",
  ),
  st(
    "左右にゆれる",
    "くり返す音が、左右に交互に鳴る。ほかでは「ピンポン」とも呼ぶ。",
  ),
  st(
    "取り消しの履歴",
    "これまでにした手の一覧。どこまで取り消したかが見える。",
  ),
  st(
    "ベロシティ固定",
    "鍵盤をどの強さで押しても、決めた強さで鳴らして録る。書く道具のノートも同じ強さになる。",
  ),
  st(
    "なおすためのファイル",
    "PC が急に終わったときのために、保存していない変更を急いで書いておくファイル。次に起動したとき、ここから戻せる。",
  ),
  st("トラック", "楽器ひとつぶんの列。音源をひとつ持つ。"),
  st("クリップ", "トラックの上に置く、ひとかたまりの演奏や録音。"),
  st("アレンジ", "トラックとクリップを時間にそってならべる、曲の全体図。"),
  st(
    "ピアノロール",
    "縦が音の高さ、横が時間の方眼。ノートを一つずつ直す場所。",
  ),
  st("ミキサー", "各トラックの音量や響きをまとめて整える卓。"),
  st(
    "ノート",
    "鍵を押してから離すまでの 1 音。位置、長さ、高さ、ベロシティを持つ。",
  ),
  st("ベロシティ", "鍵盤を押した強さ。1 がいちばん弱く、127 がいちばん強い。"),
  st(
    "MIDI",
    "音そのものではなく、「どの鍵を、いつ、どの強さで」という演奏の記録。あとから直せる。",
  ),
  st(
    "CC",
    "コントロールチェンジ。ペダルやホイールなど、鍵盤いがいの動きの記録。番号で種類が決まっている。",
  ),
  st("サステイン", "足のペダル（CC 64）。踏んでいる間、音がのびる。"),
  st("ピッチベンド", "音の高さをなめらかに上下させるホイール。"),
  st("モジュレーション", "音にゆれを足すホイール（CC 1）。"),
  st(
    "テンポ",
    "曲の速さ。4 分音符が 1 分間に何回か（BPM）。拍子が 6/8 でも 4 分音符で数えるので、拍子を変えてもテンポの数は変わらない。6/8 の 1 拍（付点 4 分音符）は、テンポ 120 なら 1 分間に 80 回。",
  ),
  st("拍子", "1 小節に拍がいくつ入るか。4/4 は 4 拍。"),
  st(
    "小節 . 拍 . ティック",
    "曲の中の位置。ティックは、4 分音符を 1920 に分けた細かい目盛り（4 けた）。",
  ),
  st("メトロノーム", "拍に合わせて鳴るクリック音。"),
  st("カウントイン", "録音が始まる前に鳴る合図の拍。"),
  st("ループ", "決めた範囲をくり返し再生する。"),
  {
    term: "重ねる",
    description: "録るとき、いまのクリップのノートを残したまま、弾いたノートを足していく。ループで録ると、1 周ごとに足す。テイクはふえない。戻すときは取り消しで。",
    source: "user:2026-11-23",
  },
  st("マーカー", "曲の中の場所につける名札。"),
  st("テイク", "同じ場所を録りなおした 1 回ぶん。前のテイクも消えずに残る。"),
  st("録音待機", "録音ボタンを押したとき、このトラックに録る、という印。"),
  st("グリッド", "ノートやクリップが吸いつく時間の目盛り。1/16 は 16 分音符。"),
  st("三連符", "1 拍を 3 つに分けるグリッド。"),
  st(
    "クオンタイズ",
    "弾いたタイミングをグリッドにそろえる。強さで、どれだけ寄せるかを決める。",
  ),
  st("スウィング", "裏拍を少し後ろへずらして、はねたノリにする。"),
  st("スケール固定", "えらんだ音階の音だけに書きこめるようにする。"),
  st("ミュート", "このトラックの音を消す。"),
  st("ソロ", "このトラックだけを聞く。"),
  st("パン", "音を左右のどこに置くか。"),
  st("フェーダー", "音量を上げ下げする、縦に動かすつまみ。"),
  st("dB", "音の大きさの単位。0 が基準で、マイナスが大きいほど小さい。"),
  st("メーター", "いま鳴っている音の大きさ。いちばん上まで届くと音が割れる。"),
  st("マスター", "すべてのトラックが最後に集まる出口。"),
  st("インサート", "エフェクトをさしこむ枠。上から順に音が通る。"),
  st("センド", "音の一部を、みんなで使うエフェクトへ送る道。"),
  st(
    "戻り（リターン）",
    "センドで送った音が、エフェクトを通って帰ってくるチャンネル。",
  ),
  st("オートメーション", "つまみの動きを時間にそって書いておく機能。"),
  st(
    "プラグイン",
    "あとから足せる音源やエフェクト。中の画面は、作った人ごとに見た目がちがう。",
  ),
  st("音源", "鍵盤を弾いたときに鳴る楽器の役。"),
  st(
    "サンプル",
    "その音色で、決まった短いフレーズを鳴らす見本。棚の ▶ で聞ける。",
  ),
  st(
    "ためし弾き",
    "棚で音源をえらんでいる間だけ、鍵盤がその音色で鳴ること。トラックには入らない。",
  ),
  st("お気に入り", "星の印をつけた音源。棚の「お気に入り」でまとめて出せる。"),
  st("エフェクト", "音に響きや変化を足す役。"),
  st(
    "パラメータ",
    "プラグインが持っている、動かせる値のひとつひとつ。つまみやスイッチにあたる。",
  ),
  st(
    "プラグインの窓",
    "プラグインが自分で持っている画面。meloet の外に、別の窓としてひらく。",
  ),
  st(
    "形式の印が 2 つ",
    "同じプラグインが 2 つの形式で入っている。明るいほうを使う。もう一方の印を押すと入れかわる。",
  ),
  st(
    "形式の印",
    "プラグインの作られかたの種類（LV2 ・ VST3 ・ CLAP）。使いかたはどれも同じ。",
  ),
  st("スキャン", "PC の中のプラグインをさがして、棚にならべる作業。"),
  st("フェード", "音の出だしや終わりを、なめらかに大きく、または小さくする。"),
  st(
    "バッファ",
    "音をまとめて処理するひとかたまりの大きさ。小さいほど遅れが減り、PC は忙しくなる。",
  ),
  st("遅れ（レイテンシ）", "弾いてから音が出るまでの時間。"),
  st("遅れの補正", "録った音を、遅れのぶんだけ前へ戻して置くこと。"),
  st(
    "サンプルレート",
    "1 秒を何こまに分けて音を記録するか。48000 Hz がふつう。",
  ),
  st("書き出し", "曲を 1 つの音のファイルにすること。"),
  // Chord help: the term wording of the mocks P10-ChordStamp and P10-ChordTable, not yet in spec 5.
  ut("和音のスタンプ", "書く道具で、押した音を根音にして、えらんだ形の和音をまとめて書く。"),
  ut("根音", "和音のいちばん元になる音。C メジャーなら C。"),
  ut("調の和音", "スケール固定でえらんだ調の音だけでできる和音。C メジャーなら C ・ Dm ・ Em ・ F ・ G ・ Am ・ Bdim。"),
  // The term wording of the mock P10-AiLink.
  ut("AI とつなぐ", "AI のアプリ（MCP）が、動いている meloet の曲を読んだり、頼まれて書いたりできるようにする。"),
];

/** The mark that joins a term and its tip (spec 5 「用語 — 説明」). */
export const TERM_SEPARATOR = " — ";

export function findTerm(term: string): Term | undefined {
  return terms.find((t) => t.term === term);
}

/** Fills the {name} holes with values. Holes with no value are left as they are. */
export function fill(
  text: string,
  values: Readonly<Record<string, string | number>> = {},
): string {
  return text.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}

export function t(
  key: TextKey,
  values?: Readonly<Record<string, string | number>>,
): string {
  return fill(texts[key].text, values);
}

/** The joint between parts of the status line left text (joined with 「 ・ 」 as in spec 6 「止まっているときの状態行の左」). */
export const STATUS_JOIN = " ・ ";

/** The joint before the last 「と」 when listing three or more names (spec 6 「棚をひらいている（見出し）」, as in 「ピアノ ・ 鍵盤、ギター と ベース」). */
export const LIST_COMMA = "、";
