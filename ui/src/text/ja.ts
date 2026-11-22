// 画面の文面はすべてここに置く。
// 出どころ: spec:<章>（docs/design/spec.html）、mock:<ボード>（docs/design/mock/<ボード>.dc.html）、user:<日付>（ヤチヨさんの決め）、
// draft:<日付>（仕様書にもモックにもない文の、作る側の案。ヤチヨさんに確かめて user: か mock: にする）。
// builder:<日付>（仕様書にない所を、作る側が仮に決めた文。docs/design-notes.md の B に戻してある）。
// {name} はうめこむ値の穴。example は出どころに書いてある例の値で、テストがこれを入れて出どころと照らし合わせる。

import type { StepName } from "../bridge/generated/StepName";

export type Source =
  `spec:${number}` | `mock:${string}` | `user:${string}` | `draft:${string}` | `builder:${string}`;

export interface Entry {
  readonly text: string;
  readonly source: Source;
  readonly example?: Readonly<Record<string, string>>;
  /** spec:6 の表の「場面」。 */
  readonly scene?: string;
  /** 文面のセルとの合わせかた。whole: セルの文そのまま。quoted: セルの「」の中。part: セルの文の一部。 */
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
  // 仕様書 6 章 「エラーと案内の文面」
  "msg.keyboardMissing": s6(
    "鍵盤がない",
    "鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。",
  ),
  "msg.keyboardFound": s6(
    "鍵盤が見つかった",
    "{keyboard} が見つかりました。鍵は届いています。音の出口をえらぶと鳴ります。",
    { keyboard: "鍵盤 49" },
  ),
  "msg.noOutputMeter": s6(
    "音の出口がまだ",
    "音の出口がまだです。下でえらんでください",
    undefined,
    "quoted",
  ),
  "msg.noOutputArrange": s6(
    "音の出口がまだ",
    "音をどこから鳴らしますか",
    undefined,
    "quoted",
  ),
  "msg.outputNotPlugged": s6(
    "えらんだ出口に何もささっていない",
    "{output}には、いま何もささっていません。ささると、ここから鳴ります。",
    { output: "（出口の名前）" },
  ),
  "msg.bufferFixedStatus": s6(
    "バッファが固定されている（状態行）",
    "バッファが PC 全体の設定で {actual} に固定されています ・ 音が出るまで 約 {latencyMs} ms（{requested} なら 約 {latencyMsAtRequested} ms）",
    {
      actual: "1024",
      latencyMs: "53",
      requested: "256",
      latencyMsAtRequested: "13.5",
    },
  ),
  "msg.bufferFixedDetail": s6(
    "バッファが固定されている（音の設定の奥）",
    "バッファが PC 全体の設定で {actual} に固定されています。meloet は {requested} を頼んでいますが、{actual} で動いています。鍵を押してから音が出るまで 約 {latencyMs} ms。{requested} で動けば 約 {latencyMsAtRequested} ms です。速いフレーズでは、音が指より遅れて聞こえます。録った音の位置は、遅れの補正で正しく置かれます。meloet は PC 全体の設定を変えません。変える場所は PipeWire の設定（quantum の固定）です。",
    {
      actual: "1024",
      latencyMs: "53",
      requested: "256",
      latencyMsAtRequested: "13.5",
    },
  ),
  "msg.soundCameOut": s6("音が出た", "音が出ました。", undefined, "quoted"),
  // 2 文目は、録音ボタンが使えるときだけ出す。
  "msg.soundCameOutRecord": s6(
    "音が出た",
    "赤い丸の録音ボタンを押すと、カウントインのあとに録りはじめます。",
    undefined,
    "quoted",
  ),
  "msg.unusableSwitchedByPc": s6(
    "えらべない出口 ・ PC が切りかえる",
    "さすと、PC がこちらに切りかえます",
  ),
  // ささっている入口を、ほかの入口もささっているので PC がえらんでいないとき（オーナーさん 2026-11-22）。
  "msg.inputPluggedNotChosen": e(
    "ささっています ・ PC の設定でほかの入口をえらんでいます（設定 ▸ サウンド ▸ 入力 で変えられます）",
    "user:2026-11-22",
  ),
  "msg.unusableStoppedByPc": s6(
    "えらべない出口 ・ PC の設定で止まっている",
    "PC の設定で、いまは使えません",
  ),
  "msg.unusableHeading": s6(
    "えらべない出口の見出し",
    "いまは使えない出口",
    undefined,
    "part",
  ),
  "msg.unusableHeadingChooser": s6(
    "えらべない出口の見出し",
    "PC が切りかえる物 {switched} ・ PC の設定で止まっている物 {stopped}",
    { switched: "1", stopped: "7" },
    "part",
  ),
  "msg.unusableHeadingSettings": s6(
    "えらべない出口の見出し",
    "meloet は、PC 全体の設定を変えません",
    undefined,
    "quoted",
  ),
  "msg.unusableInputHeading": s6(
    "えらべない入口の見出し",
    "いまは使えない入口",
    undefined,
    "part",
  ),
  "msg.countingIn": s6(
    "カウントイン中",
    "カウントイン中 ・ あと {beats} 拍で録音が始まります。止めるには Space",
    { beats: "2" },
  ),
  // 状態行の左の札「● カウントイン中」（status.countingIn）のうしろに出す部分。札と合わせて上の文になる。
  "msg.countingInRest": s6(
    "カウントイン中",
    "あと {beats} 拍で録音が始まります。止めるには Space",
    { beats: "2" },
    "part",
  ),
  "msg.recording": s6(
    "録音中",
    "{track} へ録音 ・ 止めると テイク {take} として残ります",
    { track: "1 ピアノ", take: "2" },
  ),
  "msg.loopRecording": s6(
    "ループ中の録音",
    "{track} へ録音 ・ ループ {from}–{to} の {lap} 周目 ・ 周ごとにテイクを分けています",
    { track: "1 ピアノ", from: "5", to: "9", lap: "2" },
  ),
  "msg.takeSwitched": s6(
    "テイクを切りかえた",
    "{track} ・ {clip} をテイク {take} / {takes} に切りかえました",
    { track: "1 ピアノ", clip: "Aメロ", take: "2", takes: "3" },
  ),
  "msg.noArmedTrack": s6(
    "録音待機のトラックがない",
    "録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。",
  ),
  "msg.keyboardLost": s6(
    "鍵盤が抜けた",
    "{keyboard} とのつながりが切れました。曲はそのままです。さしなおすと、自動で戻ります。",
    { keyboard: "鍵盤 49" },
  ),
  "msg.outputLost": s6(
    "音の出口が消えた",
    "{output}が見つからなくなりました。再生を止めています。",
    { output: "ヘッドホン" },
  ),
  "msg.audioSystemStopped": s6(
    "PC の音のしくみが止まった",
    "PC の音のしくみ（PipeWire）が止まりました。曲はそのままです。動きだしたら、自動でつなぎなおします。",
  ),
  "msg.recordingStoppedAudioSystem": s6(
    "録音中に PC の音のしくみが止まった",
    "PC の音のしくみ（PipeWire）が止まったので、録音を止めました。そこまでの演奏は テイク {take} に残っています。",
    { take: "2" },
  ),
  "msg.recordingStoppedOutputMissing": s6(
    "録音中に出口が消えた",
    "{output}が見つからなくなったので、録音を止めました。そこまでの演奏は テイク {take} に残っています。",
    { output: "ヘッドホン", take: "2" },
  ),
  "msg.keyboardInputStopped": s6(
    "鍵盤からの入力が止まった",
    "鍵盤からの入力が止まりました。曲はそのままです。いま、つなぎなおしています。",
  ),
  "msg.micTooLoud": s6(
    "マイクの音が大きすぎる",
    "マイクの音が大きすぎます（0 dB に届きました）。マイクを少しはなすか、PC の入口の音量を下げてください。",
  ),
  "msg.scanning": s6(
    "スキャン中",
    "スキャン中 {done} / {total} ・ 調べ終えた分から、もう使えます",
    { done: "37", total: "112" },
  ),
  "msg.scanDone": s6(
    "スキャンが終わった",
    "スキャンが終わりました ・ {added} 個を棚に入れました。{stopped} 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。",
    { added: "667", stopped: "4" },
  ),
  "msg.windowNotYet": s6(
    "まだ窓をひらけない子",
    "この子の窓は、まだひらけません。パラメータはここで動かせます。meloet が、この型の窓にまだ対応していないためです。音は、いつもどおり鳴ります。",
  ),
  "msg.noWindow": s6(
    "窓を持たない子",
    "この子は窓を持ちません。パラメータはここで動かせます。",
  ),
  "msg.windowsOpen": s6(
    "窓がひらいている",
    "プラグインの窓が {count} つひらいています",
    { count: "2" },
  ),
  // 窓が 1 つのとき（2 つ以上は msg.windowsOpen）。
  "msg.windowOpen": s6("プラグインの窓がひらいた", "{name} の窓がひらいています", { name: "Surge XT" }, "part"),
  "msg.windowFailed": s6(
    "プラグインの窓を出せない",
    "{name} の窓をひらけませんでした。パラメータの欄では動かせます。",
    { name: "Surge XT" },
  ),
  "msg.windowOpening": s6(
    "窓をひらくのに時間がかかる",
    "{name}の窓をひらいています…",
    { name: "（名前）" },
  ),
  "msg.windowTooLarge": s6(
    "窓が画面より大きい",
    "{name}の窓がひらいています（{width}×{height} ・ この画面より大きい窓です）。パラメータは右でも動かせます。",
    { name: "（名前）", width: "1597", height: "995" },
  ),
  "msg.searchNoMatch": s6(
    "さがしても見つからない",
    "「{query}」に合うパラメータはありません。",
    { query: "（ことば）" },
  ),
  "msg.formatSwitch": s6(
    "形式を入れかえる前",
    "この曲では {current} 版を使っています。{next} 版に入れかえると、いまのパラメータの値は引きつがれないことがあります。",
    { current: "VST3", next: "LV2" },
  ),
  "msg.audition": s6(
    "ためし弾き中",
    "鍵盤を弾くと、この音色で鳴ります。まだトラックには入っていません。",
  ),
  "msg.samplePlaying": s6(
    "サンプル再生中",
    "サンプルを再生中 ・ {name}。行をえらぶと、鍵盤でためし弾きできます。",
    { name: "（名前）" },
  ),
  // 状態行で、ためし弾きの音色の名前のうしろに出す（新しいトラックのとき）。
  "status.auditionNewTrack": e(
    "{instrument} ・ まだトラックには入っていません ・ 入れるには {key}",
    "mock:F4-ShelfAudition",
    {
      instrument: "シンセベース",
      key: "Enter",
    },
  ),
  "msg.auditionReplace": s6(
    "ためし弾き中 ・ 入れかえ",
    "鍵盤を弾くと、この音色で鳴ります。再生すると、録ってあるノートもこの音色で鳴ります。まだ入れかえていません。",
  ),
  // 仕様書 6 章の表の「出る場所」の列にある文。
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
    "音源を入れかえた",
    "{track} の音源を {instrument} にしました。取り消せます（{undoKey}）",
    {
      track: "1 ピアノ",
      instrument: "エレクトリックピアノ",
      undoKey: "Mod Z",
    },
  ),
  "msg.trackAdded": s6("トラックを足した", "{track} を足しました。", {
    track: "5 シンセ",
  }),
  "msg.trackDeleted": s6(
    "トラックを消した",
    "{track} を消しました。取り消せます（{undoKey}）",
    { track: "3 ドラム", undoKey: "Mod Z" },
  ),
  "msg.drumsNoScaleLock": s6(
    "ドラムとスケール固定",
    "ドラムには音階がないので、スケール固定は使えません。",
  ),
  "msg.auditionLoading": s6(
    "ためし弾きしたい音源がまだ読めない",
    "{name}を読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。",
    { name: "（名前）" },
  ),
  "msg.pluginMissing": s6(
    "プラグインが見つからない",
    "前はここにありました: {path}。曲の中の設定は残しています。",
    { path: "（場所）" },
  ),
  "msg.pluginStopped": s6(
    "プラグインが止まった",
    "{name}が止まりました ・ {track}。曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。",
    { name: "（名前）", track: "（トラック）" },
  ),
  "msg.pluginScanFailed": s6(
    "プラグインがスキャンでこけた",
    "{name}は調べている途中で止まったので、棚には入れていません。ほかのプラグインは使えます。",
    { name: "（名前）" },
  ),
  "msg.undoPending": s6(
    "取り消してある手がある",
    "{count} 手ぶん取り消してあります。新しく何かすると、その {count} 手はやりなおせなくなります。",
    { count: "2" },
  ),
  "msg.quantized": s6(
    "クオンタイズをかけた",
    "クオンタイズ {grid}（{strength} %）を {count} ノートに。うすい枠が、そろえる前の位置。",
    { grid: "1/16", strength: "75", count: "3" },
  ),
  "msg.unsaved": s6(
    "保存していない",
    "「{song}」に、まだ保存していない変更があります。",
    { song: "はじめての曲" },
  ),
  "msg.cannotSave": s6(
    "保存できない",
    "ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。べつの場所をえらんでください。曲は開いたままです。",
  ),
  "msg.songBroken": s6(
    "曲のファイルがこわれている",
    "このファイルは読めませんでした。自動で残してある 1 つ前の状態からなら開けます。",
  ),
  // 1 つ前の状態がないときは、上の文の 1 文目だけ。
  "msg.songBrokenFirst": s6(
    "曲のファイルがこわれている",
    "このファイルは読めませんでした。",
    undefined,
    "part",
  ),
  // なおすためのファイル（仕様書 3 章）。PC が終わるときに書いた物は仕様書の文、ふだん書いた物はヤチヨさんの文。
  "msg.recoveredPcEnding": s6(
    "なおすためのファイルから戻した",
    "PC が終わる前の演奏を戻しました。まだ保存していません。",
    undefined,
    "quoted",
  ),
  // 2 文目は「…です。」と「テイク N つ。」に分けて持つ。テイクが 0 のときはうしろを出さない（棟梁の決め 2026-11-15、デザインに戻す）。
  "msg.recoveredPcEndingDetail": s6(
    "なおすためのファイルから戻した",
    "{month} 月 {day} 日 {time} に PC が終わったときの「{song}」です。",
    { month: "11", day: "15", time: "22:41", song: "新しい曲" },
    "part",
  ),
  "msg.recoveredTakes": s6(
    "なおすためのファイルから戻した",
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
  // 仕様書 6 章の表の「出る場所」の列にある文。
  "status.recovered": e(
    "なおすためのファイルから戻しました ・ まだ保存していません",
    "spec:6",
  ),
  "msg.recoveryDiscarded": s6(
    "戻さないを押した",
    "戻した変更をすてて、最後に保存した形にしました。取り消せます（{undoKey}）",
    { undoKey: "Mod Z" },
  ),
  "msg.recoveryUnreadable": s6(
    "なおすためのファイルが読めない",
    "前に保存していない変更がありましたが、なおすためのファイルが読めませんでした。",
    undefined,
    "quoted",
  ),
  "msg.recoveryUnreadableSaved": s6(
    "なおすためのファイルが読めない",
    "最後に保存した「{song}」をひらいています。ファイルは消さずに残してあります。",
    { song: "はじめての曲" },
    "quoted",
  ),
  "msg.recoveryUnreadableNew": s6(
    "なおすためのファイルが読めない",
    "新しい曲をひらいています。ファイルは消さずに残してあります。",
    undefined,
    "quoted",
  ),
  // 状態行。保存したことのない曲のときは、うしろを「新しい曲をひらいています」にする（モック F-Recover3 の形に、仕様書の言葉を入れた）。
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
    "なおすためのファイルが読めない",
    "新しい曲をひらいています",
    undefined,
    "part",
  ),
  "msg.cpuOverload": s6(
    "処理が追いつかない",
    "PC の処理が追いつかず、音がとぎれました。音の設定でバッファを大きくすると、とぎれにくくなります。",
  ),
  "msg.exportPeak": s6(
    "書き出しの前",
    "いちばん大きい所 {peak} dB ・ 音割れはありません",
    { peak: "−6.2" },
  ),
  "msg.exportClipping": s6(
    "書き出しで音が割れる",
    "いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。",
  ),
  "msg.exported": s6("書き出しが終わった", "書き出しました: {path}", {
    path: "~/ミュージック/はじめての曲/はじめての曲.wav",
  }),
  "msg.scaleDoesNotFit": s6("画面に入らない拡大率", "この画面には入りません"),
  "msg.scaleKept": s6(
    "入らない拡大率をキーでえらんだ",
    "{requested} % は、この画面には入りません。{kept} % のままです。",
    { requested: "150", kept: "125" },
  ),
  "msg.scaleWindowGrown": s6(
    "拡大率を変えて、窓が大きくなった",
    "拡大率を {scale} % にしました。窓を、いちばん小さい大きさまで広げました。",
    { scale: "125" },
  ),
  "msg.scaleLowered": s6(
    "画面が小さくなった",
    "画面が小さくなったので、拡大率を {from} % から {to} % にしました。",
    { from: "150", to: "125" },
  ),
  "msg.bandFoldedWindowLow": s6(
    "帯が自動でたたまれた",
    "窓が低くなったので、{band}をたたみました。帯札を押すと、またひらきます。",
    { band: "ミキサー" },
  ),
  "msg.bandFoldedToOpen": s6(
    "入りきらない帯をひらいた",
    "{opened}をひらくために、{folded}をたたみました。窓を高くすると、両方ひらけます。",
    { opened: "ピアノロール", folded: "ミキサー" },
  ),
  "msg.bandFoldedThird": s6(
    "3 つめの帯をひらいた",
    "{opened}をひらいたので、いちばん前にひらいた{folded}をたたみました。下の帯は、2 つまでひらけます。",
    { opened: "取り消しの履歴", folded: "ミキサー" },
  ),
  "msg.bandFoldedScale": s6(
    "拡大率を上げて帯がたたまれた",
    "拡大率を {scale} % にしたので、{band}をたたみました。帯札を押すと、またひらきます。",
    { scale: "150", band: "ミキサー" },
  ),
  // 数字の欄に、数字として読めない物を打った。欄は value.* か note.*。
  "msg.valueRejected": s6(
    "数字として読めない",
    "{field}は数字で打ちます。もとの {kept} のままです。",
    { field: "ベロシティ", kept: "77" },
    "part",
  ),
  // 音の欄（音名で打つ）に読めない物を打った。
  "msg.keyRejected": s6(
    "音の欄で読めない",
    "{field}は {range} です。もとの {kept} のままです。",
    { field: "音", range: "C-1〜G9", kept: "C4" },
  ),
  "msg.backToArrange": s6(
    "アレンジへ帰る道",
    "Esc ・ ひとつ戻る。何度か押すとアレンジへ",
  ),
  // 範囲の外の値は近いはしにして決める。欄と範囲は value.*。
  "msg.valueClamped": s6(
    "範囲の外の値を打った",
    "{field}は {range} なので、{value} にしました。",
    { field: "ベロシティ", range: "1〜127", value: "127" },
    "quoted",
  ),
  // ベロシティをまとめて変える、長さをそろえる。
  "msg.velocityMatched": s6(
    "まとめてそろえた",
    "{count} ノートのベロシティを {value} にそろえました。",
    { count: "3", value: "100" },
    "part",
  ),
  "msg.lengthMatched": s6(
    "まとめてそろえた",
    "{count} ノートの長さを {value} にそろえました。",
    { count: "3", value: "0.240" },
    "part",
  ),
  // 位置の小節を丸めたとき。小節だけ「1 から」のあとに空白がない。拍とティックは msg.valueClamped。
  "msg.valueClampedBar": s6(
    "位置を丸めた",
    "小節は 1 からなので、{value} にしました。",
    { value: "1. 1. 000" },
    "quoted",
  ),
  // ほかのクリップと重なって置けなかった。
  "msg.clipsNotMoved": s6(
    "クリップが重なる",
    "ほかのクリップと重なるので、動かしませんでした。",
    undefined,
    "quoted",
  ),
  "msg.clipsNotPasted": s6(
    "クリップが重なる",
    "ほかのクリップと重なるので、貼りつけませんでした。",
    undefined,
    "quoted",
  ),
  "msg.clipsNotDuplicated": s6(
    "クリップが重なる",
    "ほかのクリップと重なるので、複製できませんでした。",
    undefined,
    "quoted",
  ),
  // ピアノロールでクリップの外に書こうとして、クリップをのばせなかった（B-82）。
  "msg.notesNotWritten": s6(
    "クリップの外に書けない",
    "ほかのクリップと重なるので、書けませんでした。",
  ),
  "msg.notesNotResized": s6(
    "長さを変えられない",
    "ほかのクリップと重なるので、長さを変えられませんでした。",
  ),
  "msg.notesNotMoved": s6(
    "クリップの外へ動かせない",
    "ほかのクリップと重なるので、動かしませんでした。",
  ),
  "msg.notesNotDuplicated": s6(
    "クリップの外へ写せない",
    "ほかのクリップと重なるので、複製できませんでした。",
  ),
  "msg.takeDeleted": s6(
    "テイクを消した",
    "テイク {take} を消しました。テイク {active} / {total} にしました。取り消せます（{undoKey}）",
    { take: "2", active: "1", total: "2", undoKey: "Mod Z" },
  ),
  // 曲のダイアログ（仕様書 3 章 89-6）。見出しは曲の名前のまま、状態行だけ名前で言う。
  "status.songDialogOpen": s6("曲のダイアログをひらいている", "「{dialog}」のダイアログをひらいています", { dialog: "保存していない変更" }),
  "songDialog.unsaved": e("保存していない変更", "spec:3"),
  "songDialog.cannotSave": e("保存できませんでした", "spec:3"),
  "songDialog.broken": e("ひらけない曲", "spec:3"),
  // 「音の設定をひらいています」は status.settingsOpen。
  "status.dialogOpen": s6(
    "ダイアログをひらいている",
    "{dialog} をひらいています",
    { dialog: "meloet について" },
    "part",
  ),
  "msg.soundsMissingSong": s6(
    "音の素材が見つからない（曲をひらいた）",
    "音の素材が見つからないので、{count} 本のトラックをサイン波で鳴らしています。",
    { count: "4" },
  ),
  "shelf.soundsMissing": s6(
    "音の素材が見つからない（棚）",
    "音の素材が見つかりません。いまはサイン波だけ使えます。meloet を入れなおすと、もとに戻ります。",
  ),

  // [第 5 回 ・ SF2]（仕様書 6 章）
  "sf2.dropHere": s6(
    "SF2 を持ってきている",
    "ここに落とすと、棚に SF2 を足します。ファイルは meloet の中にうつします。",
    undefined,
    "part",
  ),
  "sf2.dropCancel": s6(
    "SF2 を持ってきている",
    "やめるには、窓の外で離すか Esc",
    undefined,
    "quoted",
  ),
  // 6 章「SF2 を持ってきている」の出る場所の列。
  "sf2.dragging": e("SF2 のファイルを持ってきています", "spec:6"),
  "sf2.notSf2": s6(
    "SF2 ではないファイルを持ってきている",
    "SF2 のファイルではないので、足せません。",
  ),
  "sf2.notSf2Dragging": s6("SF2 でないファイルを持ってきている", "SF2 ではないファイルを持ってきています（足せません）"),
  // 窓のまん中の箱の、ファイルの名前の所。SF2 が 1 つもないときは前だけ（design-notes A-19）。
  "sf2.manyFiles": s6("ファイルをいくつも持ってきている", "{files} つのファイル ・ SF2 は {sf2} つ", { files: "3", sf2: "2" }),
  "sf2.manyFilesNoSf2": s6("ファイルをいくつも持ってきている", "{files} つのファイル", { files: "3" }, "part"),
  // 「SF2 をいくつも足した」の 2 文。足した SF2 が 1 つなら、sf2.added のうしろに 2 文目だけ（design-notes A-19）。
  "sf2.addedMany": s6(
    "SF2 をいくつも足した",
    "SF2 を {count} つ足しました（{file} ほか {others} つ）。",
    { count: "2", file: "FluidR3_GM.sf2", others: "1" },
    "part",
  ),
  "sf2.skipped": s6("SF2 をいくつも足した", "{count} つは SF2 ではないので、足していません。", { count: "1" }, "part"),
  "sf2.added": s6(
    "SF2 を足した",
    "{file} を足しました。音色 {count} ・ ドラムのキット {kits}",
    { file: "FluidR3_GM.sf2", count: "128", kits: "9" },
  ),
  "sf2.unreadable": s6(
    "SF2 が読めない",
    "{file} は読めませんでした。ファイルがこわれているか、SF2 ではないかもしれません。",
    { file: "FluidR3_GM.sf2" },
  ),
  "sf2.missing": s6(
    "SF2 が見つからない",
    "曲で使っている {file} が見つかりません。{count} 本のトラックはサイン波で鳴らしています。",
    { file: "FluidR3_GM.sf2", count: "2" },
  ),

  // [第 5 回 ・ あとから入れたもの]（仕様書 2 章・3 章 83 ・ 85 ・ Y12 ・ Y14、モック F5-*）
  "shelf.builtin": e("最初から入っているもの", "mock:F5-SF2Shelf"),
  "shelf.added": e("あとから入れたもの", "mock:F5-SF2Shelf"),
  "shelf.sectionCount": e("{name} ・ {count}", "mock:F5-SF2Shelf", {
    name: "あとから入れたもの",
    count: "1",
  }),
  // さがしているときの見出しの数（「合う数 / 全部」）。
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
  // SF2 の音色の名前（状態行。モック F5-SF2Shelf「Clean Guitar ・ FluidR3_GM.sf2」）。
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
  // 棚の下の、止まったプラグインの箱（モック A-Mixer: 見出しと、6 章「プラグインが止まった」の文の残り）。
  "plugin.stoppedHead": e("{name} が止まりました ・ {track}", "mock:A-Mixer", { name: "Brick EQ", track: "3 ドラム" }),
  "plugin.stoppedBody": s6(
    "プラグインが止まった",
    "曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。",
    undefined,
    "part",
  ),
  "status.cancelKey": e("やめるには {key}", "mock:F5-PluginLoading", { key: "Esc" }),
  "status.shelfSearch": e("さがす「{query}」・ 合う物 {count} 個", "mock:F5-ShelfSearch", {
    query: "Dragonfly",
    count: "4",
  }),
  // 同じ名前のノートをまとめる（「スネア ×2」）。
  "note.keyCount": e("{name} ×{count}", "spec:3", { name: "スネア", count: "2" }),
  // ドラムでノートを上下に動かすときの読み値（ふつうのピアノロールは pr.readMove）。
  "pr.readMoveRows": e("{ticks} ・ {rows}", "spec:3", { ticks: "+0.120", rows: "上へ 1 行" }),
  "pr.rowsUp": e("上へ {count} 行", "spec:3", { count: "1" }),
  "pr.rowsDown": e("下へ {count} 行", "spec:3", { count: "2" }),
  // SF2 の仲間の見出し。キーの後ろは約束の GmFamily。
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
  // プラグインの種類の見出し（14-A）。キーの後ろは約束の PluginInstrumentCategory ・ PluginEffectCategory。
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
  // SF2 のドラムのキットの行の名前（鍵の番号、GM の決まり）。表にない鍵は音名。
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

  // [第 5 回 ・ インサート]（仕様書 6 章）
  "shelf.insertInto": s6(
    "インサートに入れる",
    "{strip} のインサート {slot} に入れます。",
    { strip: "2 ベース", slot: "2" },
  ),
  "msg.effectInserted": s6(
    "エフェクトを入れた",
    "{strip} のインサート {slot} に {effect} を入れました。",
    { strip: "2 ベース", slot: "2", effect: "リバーブ" },
  ),
  "msg.effectRemoved": s6(
    "エフェクトを外した",
    "{strip} のインサート {slot} の {effect} を外しました。取り消せます（{undoKey}）",
    { strip: "2 ベース", slot: "1", effect: "コンプレッサー", undoKey: "Mod Z" },
  ),
  // 外す ・ 入れかえる ・ 音源を入れかえるのといっしょに帯が消えたとき、状態行の文の終わりの「。」の前に足す（仕様書 3 章 92-19）。
  "msg.lanesAlsoGone": s6("帯もいっしょに消えた", "（そのパラメータの帯 {count} 本も消えました）", { count: "2" }, "part"),
  "msg.insertsReordered": s6(
    "インサートを並べかえた",
    "{strip} のインサートを並べかえました（{order}）。",
    { strip: "2 ベース", order: "EQ 4 バンド → コンプレッサー" },
  ),
  "shelf.insertReplace": s6(
    "インサートを入れかえる",
    "{strip} のインサート {slot} の {current} を、{effect} に入れかえます。いまの設定は残りません（取り消せます）。",
    { strip: "2 ベース", slot: "1", current: "コンプレッサー", effect: "EQ 4 バンド" },
  ),
  "msg.insertReplaced": s6(
    "インサートを入れかえた",
    "{strip} のインサート {slot} を {effect} に入れかえました。取り消せます（{undoKey}）",
    { strip: "2 ベース", slot: "1", effect: "EQ 4 バンド", undoKey: "Mod Z" },
  ),
  // {target} は「{ストリップ} の {エフェクト} 」（うしろに空白）。仕様書のもどしたときの例は「…をもどしました。」。
  "msg.effectBypassed": s6(
    "エフェクトをとばした ・ もどした",
    "{target}をとばしました。",
    { target: "2 ベース の コンプレッサー " },
    "part",
  ),
  "msg.effectRestored": s6(
    "エフェクトをとばした ・ もどした",
    "{target}をもどしました。",
    { target: "…" },
    "quoted",
  ),
  "status.paramSelectedInsert": s6(
    "パラメータをえらんでいる（トラック ・ マスター）",
    "{strip} ・ {effect}の「{param}」をえらんでいます",
    { strip: "2 ベース", effect: "コンプレッサー", param: "しきい値" },
  ),
  "params.valueClamped": s6(
    "打ちこんだ値がはみ出した",
    "{param}は {range} なので、{value} にしました。",
    { param: "レシオ", range: "1〜20", value: "20 : 1" },
  ),
  "params.nothingSelected": s6(
    "何もえらんでいないパラメータの欄",
    "ミキサーでインサートの枠をえらぶと、ここにパラメータが出ます。",
  ),
  "mixer.noOutput": s6("出口がない", "出口なし ▸", undefined, "part"),
  "mixer.noOutputTip": s6(
    "出口がない",
    "音の出口がありません。押すと音の設定をひらきます。",
    undefined,
    "quoted",
  ),

  // [第 5 回 区切り3 ・ 窓とパラメータ] モックにだけある文。
  // 6 章の窓の文の「（名前）」は、居場所とプラグインの名前（うしろに空白。design-notes A-17）。
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
    "ミキサーをひらいた（とばした ・ 止まった枠がある）",
    "{strip} の {effect} をとばしています",
    { strip: "4 パッド", effect: "MVerb" },
    "part",
  ),
  "status.pluginStoppedNow": s6(
    "ミキサーをひらいた（とばした ・ 止まった枠がある）",
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
  // 状態行の「（ストリップ）の（エフェクト）」。うしろに空白（msg.effectBypassed の {target}）。
  "status.insertTarget": e("{strip} の {effect} ", "spec:6", { strip: "2 ベース", effect: "コンプレッサー" }),
  "windows.shelfAudition": e("棚 ・ ためし弾き", "user:2026-11-19"),
  "params.foldedHint": e("40 個をこえる子は、グループをたたんで始めます", "mock:F-ParamMany"),
  "params.searchHint": e("さがす枠を空にすると、グループの一覧に戻ります", "mock:F-ParamSearch"),
  "params.more": e("… ほか {count}", "spec:3", { count: "6" }),
  "params.groupOther": e("そのほか", "mock:F-ParamMany"),
  // 鍵盤のつまみを受けるだけのパラメータ（仕様書 3 章 91-3）。さがしているときだけ、このグループに出す。
  "params.keyboardKnobs": e("鍵盤のつまみを受ける所", "spec:3"),
  // 一覧の下。first と last は、最初と最後の名前（last は first と同じ頭を外した所。「MIDI CC 0|0」〜「15|129」）。
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
  // インサートの枠に乗ったとき、名前のうしろにそえる（仕様書 2 章「インサートの枠のオンの印」）。
  "mixer.bypassedTip": e("（とばしている）", "spec:2"),
  "insert.replace": e("入れかえる…", "spec:3"),
  "insert.moveUp": e("上へ動かす", "spec:3"),
  "insert.moveDown": e("下へ動かす", "spec:3"),
  "track.replaceSound": e("音源を入れかえる…", "user:2026-11-19"),
  "shelf.titleReplaceEffect": e("{strip} のインサート {slot} を入れかえる", "mock:R9-Inserts", { strip: "2 ベース", slot: "1" }),
  "shelf.currentEffect": e("いまのエフェクト", "spec:3"),

  // [第 5 回 ・ プラグイン]（仕様書 6 章）
  "plugin.newFound": s6(
    "新しいプラグインがあった",
    "新しいプラグインが {count} 個見つかりました。棚の「あとから入れたもの」に入れました。",
    { count: "3" },
  ),
  "plugin.stoppedList": s6(
    "スキャンで止まった物の一覧",
    "この {count} 個は、調べている途中で止まったので、棚に入れていません。meloet と曲は無事です。次のスキャンからは、とばします。",
    { count: "4" },
  ),
  "plugin.retryOk": s6(
    "もう一度ためして入った",
    "{name} を、もう一度ためしました。今度は入りました。",
    { name: "Granular Fog" },
  ),
  "plugin.retryStopped": s6(
    "もう一度ためしても止まった",
    "{name} は、また止まりました。棚には入れません。",
    { name: "Granular Fog" },
  ),
  "plugin.windowStep": s6(
    "プラグインの窓で変えた（履歴）",
    "{name} の窓で変えた所",
    { name: "Surge XT" },
  ),
  "plugin.loading": s6(
    "プラグインを読みこんでいる",
    "{name} を読みこんでいます…",
    { name: "Surge XT" },
  ),
  "plugin.loadFailed": s6(
    "プラグインを読みこめない",
    "{name} を読みこめませんでした。",
    { name: "Surge XT" },
  ),
  "plugin.loadFailedShelf": s6(
    "プラグインを読みこめない（棚）",
    "{name} を読みこめませんでした。こわれているか、この PC では動かないのかもしれません。",
    { name: "Surge XT" },
  ),
  "plugin.missingSong": s6(
    "プラグインが見つからない（曲をひらいた）",
    "曲で使っている {name} が見つかりません。{count} 本のトラックはサイン波で鳴らしています。",
    { name: "Surge XT", count: "1" },
  ),
  "plugin.unreadableSong": s6(
    "プラグインを読みこめない（曲をひらいた）",
    "曲で使っている {name} を読みこめませんでした。{count} 本のトラックはサイン波で鳴らしています。",
    { name: "Surge XT", count: "1" },
  ),
  "plugin.instrumentRemoved": s6(
    "音源のプラグインを外した",
    "{track} の音源を外しました。サイン波で鳴らします。取り消せます（{undoKey}）",
    { track: "5 Surge XT", undoKey: "Mod Z" },
  ),
  // 止まった物が 0 のときは、6 章の文の前半だけ（design-notes A-16）。
  "msg.scanDoneNone": s6(
    "スキャンが終わった",
    "スキャンが終わりました ・ {added} 個を棚に入れました。",
    { added: "667" },
    "part",
  ),
  // ダイアログ「スキャンで止まったプラグイン」と「プラグインの場所」（仕様書 2 章・3 章 Y13、モック F5-ScanStopped・R9-Scan）。
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

  // 数字の欄の名前と範囲（仕様書 6 章「範囲の外の値を打った」、3 章のテンポと拍子の枠）。
  "value.tempo": e("テンポ", "spec:6"),
  "value.tempoRange": s6(
    "範囲の外の値を打った",
    "30.00〜300.00",
    undefined,
    "part",
  ),
  "value.meterNum": e("拍子の上", "spec:6"),
  "value.meterNumRange": s6("範囲の外の値を打った", "1〜12", undefined, "part"),
  // 6 章の文は「下は」。欄の名前だけで読めるように「拍子の下」にした（棟梁の決め 2026-11-15）。
  "value.meterDen": e("拍子の下", "user:2026-11-18"),
  "value.meterDenRange": s6(
    "範囲の外の値を打った",
    "2 ・ 4 ・ 8 ・ 16",
    undefined,
    "part",
  ),
  "value.velocity": e("ベロシティ", "spec:6"),
  "value.velocityRange": s6(
    "範囲の外の値を打った",
    "1〜127",
    undefined,
    "part",
  ),
  // 位置の小節・拍・ティック（デザインに戻した事 68）。拍とティックの範囲は拍子で変わるので、知らせについてくる。
  "value.bar": e("小節", "user:2026-11-18"),
  "value.beat": e("拍", "user:2026-11-18"),
  "value.tick": e("ティック", "user:2026-11-18"),

  // 仕様書 6 章のボタン（「（…）」の中は説明なので入れない）
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
  // 仕様書 6 章は「トラック（次の番号）を作る」。番号を入れた形はモックから。
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

  // キーの札（帯札やボタンの右はしに小さく書くキーの名前。仕様書 4 章の表）
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
  // 曲の名前の一覧のキー（Mod の読みかえは拡大率の一覧と同じ）。
  "key.newSong": e("N", "spec:4"),
  "key.openSong": e("O", "spec:4"),
  "key.saveSong": e("S", "spec:4"),
  "key.undo": e("Z", "spec:4"),
  // 拡大率のキーの書きかた（docs/design-notes.md A-2: Linux と Windows では「Ctrl Alt 1」、Mac では「Cmd Option 1」）
  "key.ctrl": e("Ctrl", "spec:4"),
  "key.cmd": e("Cmd", "spec:4"),
  "key.alt": e("Alt", "spec:4"),
  "key.option": e("Option", "spec:4"),

  // 再生まわり
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
  // 左上の曲の名前を押すとひらく一覧の行。
  "song.new": s6("曲の名前の一覧", "新しい曲", undefined, "quoted"),
  "song.open": s6("曲の名前の一覧", "ひらく…", undefined, "quoted"),
  "song.save": s6("曲の名前の一覧", "保存", undefined, "quoted"),
  // 曲の名前が引っこんだ所の ▾（仕様書 2 章「曲の ▾」）。aria-label はモックの形。
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
  "transport.position": e("小節 . 拍 . ティック", "mock:Main"),
  "transport.time": e("時間", "mock:Main"),
  "transport.tempo": e("テンポ", "mock:Main"),
  "transport.timeSignature": e("拍子", "mock:Main"),
  "transport.metronome": e("メトロノーム", "mock:Main"),
  "transport.countIn": e("カウントイン", "mock:Main"),
  "transport.countInBars": s6(
    "カウントインの小節の数",
    "{bars} 小節",
    { bars: "1" },
    "quoted",
  ),
  // カウントインが「なし」のときのボタンの値と、一覧の行。
  "transport.countInNone": s6(
    "カウントインの小節の数",
    "なし",
    undefined,
    "quoted",
  ),
  // カウントインが引っこんだときの、メトロノームの ▾（仕様書 2 章「メトロノームの ▾」）。
  "transport.metronomeMenu": e("メトロノームとカウントイン", "mock:F-Menus"),
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

  // 入力計
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
  // カウントイン中と録音中。「● テイク {take}」は rec。
  "meter.playTargetRecording": e("{track} ● テイク {take}", "mock:F-Connect4", {
    track: "1 ピアノ",
    take: "1",
  }),

  // ためし弾きの間の「弾くと鳴る先」。
  "meter.auditionTarget": e(
    "ためし弾き ・ {instrument}",
    "mock:F4-ShelfAudition",
    { instrument: "シンセベース" },
  ),

  // トラックと音源
  "track.label": e("{number} {name}", "mock:F-Connect3", {
    number: "1",
    name: "ピアノ",
  }),
  // 最初から入っている音色（仕様書 3 章 84）。キーの後ろは約束の InstrumentKind。棚の並び。
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
  // ドラムのピアノロールの行の名前（楽器の名前。キーの後ろは DRUM_ROWS の id）。
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
  // ドラムのトラックの右の欄の「スケール」（音階「—」、固定「使えない」）。道具の列の「スケール固定 —」も同じ「—」。
  "inspector.scaleNone": e("—", "spec:3"),
  "inspector.lockUnusable": e("使えない", "spec:3"),
  // 棚の見出し。キーの後ろは約束の InstrumentGroup。
  "instrumentGroup.pianoKeys": e("ピアノ ・ 鍵盤", "spec:3"),
  "instrumentGroup.guitar": e("ギター", "spec:3"),
  "instrumentGroup.bass": e("ベース", "spec:3"),
  "instrumentGroup.drums": e("ドラム", "spec:3"),
  "instrumentGroup.padStrings": e("パッド ・ 弦", "spec:3"),
  "instrumentGroup.wind": e("管楽器", "spec:3"),
  "instrumentGroup.synth": e("シンセ", "spec:3"),

  // 棚（音源をえらぶ）
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

  // 最初から入っているエフェクト（仕様書 3 章 Y8）。キーの後ろは約束の EffectKind。
  "effect.eq4": e("EQ 4 バンド", "spec:3"),
  "effect.compressor": e("コンプレッサー", "spec:3"),
  "effect.reverb": e("リバーブ", "spec:3"),
  "effect.delay": e("ディレイ", "spec:3"),
  "effect.chorus": e("コーラス", "spec:3"),
  "effect.limiter": e("リミッター", "spec:3"),
  // パラメータの名前。キーは param.<EffectKind>.<EFFECT_PARAMS の id>。リバーブとディレイは仕様書 3 章、
  // ほかの 4 つは docs/architecture.md 19-3 の表（ヤチヨさんの決め）。
  "param.reverb.decay": e("響きの長さ", "spec:3"),
  "param.reverb.size": e("部屋の大きさ", "spec:3"),
  "param.reverb.preDelay": e("プリディレイ", "spec:3"),
  "param.reverb.highCut": e("高い音を落とす", "spec:3"),
  "param.reverb.lowCut": e("低い音を落とす", "spec:3"),
  "param.reverb.width": e("広がり", "spec:3"),
  "param.reverb.mix": e("ミックス", "spec:3"),
  // 「間隔」は 1 行。「テンポに合わせる」がオンなら段階（intervalStep）、オフならミリ秒（intervalMs）。
  "param.delay.intervalStep": e("間隔", "spec:3"),
  "param.delay.intervalMs": e("間隔", "spec:3"),
  "param.delay.sync": e("テンポに合わせる", "spec:3"),
  "param.delay.feedback": e("くり返し", "spec:3"),
  "param.delay.highCut": e("高い音を落とす", "spec:3"),
  "param.delay.lowCut": e("低い音を落とす", "spec:3"),
  "param.delay.pingPong": e("左右にゆれる", "spec:3"),
  "param.delay.mix": e("ミックス", "spec:3"),
  // 「間隔」の段階。キーの後ろは EFFECT_PARAMS の steps。
  "param.delay.step.quarter": e("1/4", "spec:3"),
  "param.delay.step.eighthDotted": e("1/8 付点", "spec:3"),
  "param.delay.step.eighth": e("1/8", "spec:3"),
  "param.delay.step.eighthTriplet": e("1/8 三連符", "spec:3"),
  "param.delay.step.sixteenth": e("1/16", "spec:3"),
  // EQ の見出し（EFFECT_PARAMS の group）と、その下の行。
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
  // 値の書きかた（数と単位。仕様書 3 章「パラメータの名前の決まり」の例）。キーの後ろは EFFECT_PARAMS の unit。
  "param.value.s": e("{value} s", "spec:3", { value: "2.40" }),
  "param.value.ms": e("{value} ms", "spec:3", { value: "20" }),
  "param.value.db": e("{value} dB", "spec:3", { value: "−6.0" }),
  "param.value.percent": e("{value} %", "spec:3", { value: "35" }),
  "param.value.hz": e("{value} Hz", "spec:3", { value: "6000" }),
  "param.value.ratio": e("{value} : 1", "user:2026-11-18", { value: "4.0" }),
  "param.value.oct": e("{value} oct", "user:2026-11-18", { value: "1.0" }),
  "param.on": e("オン", "mock:R6-Delay"),
  "param.off": e("オフ", "mock:R6-Delay"),

  // パラメータの欄（最初から入っているエフェクト）
  "params.builtin": e("最初から入っている", "mock:F4-Mixer"),
  // 最初から入っている音色と SF2 の音色を左で押したとき（仕様書 3 章 92-1 ・ 19-A）。
  "params.noSoundParams": e("この音色には、動かせるパラメータがありません。", "spec:3"),
  "params.noSoundParamsHint": e("音量とパンはミキサーで、音色を変えるには入れかえます。", "spec:3"),
  "msg.noSoundParams": s6("動かせるパラメータがない", "{name}には、動かせるパラメータがありません", { name: "グランドピアノ" }),
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

  // ミキサー
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

  // アレンジ
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
  "arrange.addTrackKinds": e("音源 ・ 歌", "mock:F-Empty"),
  // 入れかえのためし弾きの間、トラックの見出しの音源の名前。
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
  // クリップのドラッグの「その場の読み値」（仕様書 2 章）。
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

  // 出口えらび（流れ 2）と、出口・入口の行
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
    "出口 ・ 入口の行の文",
    "ささっています",
    undefined,
    "quoted",
  ),
  "endpoint.connected": s6(
    "出口 ・ 入口の行の文",
    "つながっています",
    undefined,
    "quoted",
  ),
  "endpoint.nothingPlugged": s6(
    "出口 ・ 入口の行の文",
    "何もささっていません",
    undefined,
    "quoted",
  ),
  "friendly.headphones": s6(
    "出口 ・ 入口の行の文",
    "ヘッドホンの穴",
    undefined,
    "quoted",
  ),
  "friendly.hdmi": s6(
    "出口 ・ 入口の行の文",
    "画面のスピーカー",
    undefined,
    "quoted",
  ),
  "friendly.lineOut": s6(
    "出口 ・ 入口の行の文",
    "うしろのスピーカーの穴",
    undefined,
    "quoted",
  ),
  "friendly.rearMic": s6(
    "出口 ・ 入口の行の文",
    "うしろのマイクの穴",
    undefined,
    "quoted",
  ),
  "friendly.keyboard": e("この鍵盤", "mock:F-Settings"),

  // 帯札の行と、縦の見出し
  "band.arrange": e("アレンジ", "mock:Main"),
  "band.trackCount": e("トラック {count}", "mock:Main", { count: "5" }),
  "band.pianoRoll": e("ピアノロール", "mock:Main"),
  "band.noteCount": e("ノート {count}", "mock:Main", { count: "23" }),
  "band.mixer": e("ミキサー", "mock:Main"),
  "band.masterDb": e("マスター {db}", "mock:Main", { db: "0.0 dB" }),
  "band.history": e("取り消しの履歴", "mock:Main"),
  // 仕様書 3 章の帯の表の「12 / 14」の形を、取り消してあってもなくても使う（モック Main の「14 手」は使わない。ヤチヨさんの決め 2026-11-15）。
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

  // 状態行
  "status.stopped": s6("状態行の札", "■ 停止中", undefined, "quoted"),
  "status.playing": s6("状態行の札", "▶ 再生中", undefined, "quoted"),
  "status.recording": s6("状態行の札", "● 録音中", undefined, "quoted"),
  "status.countingIn": s6(
    "状態行の札",
    "● カウントイン中",
    undefined,
    "quoted",
  ),
  "status.auditioning": e("♪ ためし弾き中", "mock:F4-ShelfAudition"),
  // ● から別のトラックを選んだとき。
  "status.armMoved": e(
    "{recordKey} を押すと {track} に録ります",
    "mock:R7-Arrange6",
    { recordKey: "R", track: "2 ベース" },
  ),
  // 止まっているときの左の文。ある物だけを「 ・ 」でつなぐ。両方あるときは status.loopNoSelection。
  "status.loopNoSelection": s6(
    "止まっているときの状態行の左",
    "ループ {from}–{to} ・ 選択なし",
    { from: "5", to: "9" },
    "quoted",
  ),
  "status.loop": e("ループ {from}–{to}", "mock:Main", { from: "5", to: "9" }),
  "status.noSelection": e("選択なし", "mock:Main"),
  // ミュートしたトラック。2 本以上は名前を「 ・ 」でならべる。
  "status.muted": s6(
    "ミュート",
    "{tracks} はミュート",
    { tracks: "4 パッド" },
    "part",
  ),
  "status.mutedMany": s6(
    "ミュート",
    "{tracks} はミュート",
    { tracks: "4 パッド ・ 5 ベース" },
    "quoted",
  ),
  // 棚をひらいた時と、見出しをひらく ・ たたむ時（仕様書 6 章「棚をひらいている（見出し）」）。groups はひらいている見出し。
  // 2 つ ・ 3 つ（・ 4 つ）は status.shelfGroupsLast（前は LIST_COMMA でならべる）、5 つ以上は status.shelfGroupsMore。
  "status.shelfOpen": s6("棚をひらいている（見出し）", "音色 {count} ・ {groups} をひらいています", { count: "31", groups: "ピアノ ・ 鍵盤 と ギター" }, "part"),
  "status.shelfCount": s6("棚をひらいている（見出し）", "音色 {count}", { count: "31" }, "part"),
  "status.shelfGroupsLast": s6("棚をひらいている（見出し）", "{first} と {last}", { first: "ピアノ ・ 鍵盤、ギター", last: "ベース" }, "part"),
  "status.shelfGroupsMore": s6("棚をひらいている（見出し）", "{first} ほか {count} つ", { first: "ピアノ ・ 鍵盤", count: "4" }, "part"),
  // 6 章「棚をひらいている（見出し）」の出る場所の列（エフェクトの一覧）。
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
  // 録音待機のトラックがないとき（track が null）は、上の 2 つの文の前の部分だけ。
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

  // 音の設定
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

  // ピアノロールのまわり（第 3 回）: アレンジの横の拡大、ノートの一覧、選んだノート、状態行
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
    "選んだノートの読み値",
    "{position} 〜",
    { position: "3.1.000" },
    "quoted",
  ),
  "inspector.mixed": s6(
    "選んだノートの読み値",
    "ばらばら",
    undefined,
    "quoted",
  ),
  "inspector.range": s6(
    "選んだノートの読み値",
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
  "inspector.on": e("オン", "mock:A-PianoRoll"),
  "inspector.off": s6("音階の外の音（読み値）", "オフ", undefined, "quoted"),
  "inspector.sameLength": e("長さをそろえる", "mock:A-PianoRoll"),
  "inspector.velocityAll": e("ベロシティをまとめて変える", "mock:A-PianoRoll"),
  "scale.name": e("{root} {mode}", "mock:A-PianoRoll", {
    root: "C",
    mode: "メジャー",
  }),
  "scale.major": e("メジャー", "mock:A-PianoRoll"),
  "scale.minor": e("マイナー", "user:2026-11-15"),
  "scale.kind": e("種類", "user:2026-11-15"),
  "scale.root": e("主音", "user:2026-11-15"),
  // 状態行。選んだノートは、数と最初の 4 つの音名（5 つ以上は「…」をそえる）。クオンタイズしたテイクなら、うしろに「 ・ クオンタイズ済み…」。
  "status.notesSelected": s6(
    "ノートを選んでいる",
    "選択 {count} ノート ・ {keys}",
    { count: "3", keys: "D4 E4 F4" },
    "part",
  ),
  "status.notesSelectedMore": s6(
    "ノートを選んでいる",
    "選択 {count} ノート ・ {keys} …",
    { count: "7", keys: "C3 E3 G3 C4" },
    "quoted",
  ),
  "status.notesQuantized": e(
    "クオンタイズ済み（もとの位置へ戻せます）",
    "mock:A-PianoRoll",
  ),
  // ピアノロールをひらいて再生しているとき。テイクは arrange.takeSwitch。鳴っている音がなければ、その部分を出さない。
  "status.pianoRollClip": s6(
    "再生中 ・ ピアノロール",
    "{track} ・ {clip}",
    { track: "1 ピアノ", clip: "Aメロ" },
    "part",
  ),
  "status.sounding": s6(
    "再生中 ・ ピアノロール",
    "鳴っている音 {keys}",
    { keys: "C3 E3 G3 C4" },
    "part",
  ),
  // ピアノロールをひらいて録音しているとき。押している鍵がなければ「 ・ 押している音 …」を出さない。
  "msg.recordingPianoRoll": s6(
    "録音中 ・ ピアノロール",
    "{track} へ録音 ・ テイク {take}",
    { track: "1 ピアノ", take: "3" },
    "part",
  ),
  "status.held": s6(
    "録音中 ・ ピアノロール",
    "押している音 {keys}",
    { keys: "C3 E3 G3 C4" },
    "part",
  ),
  // クリップがない所でピアノロールをひらいているとき（止まっているときの左）。
  "status.noClip": e("{track} ・ クリップなし", "mock:F-PREmpty", {
    track: "1 ピアノ",
  }),
  "msg.outOfScale": s6(
    "音階の外に書こうとした",
    "{key} は {scale}の外の音です（スケール固定 ・ オン）",
    { key: "D#", scale: "C メジャー" },
  ),
  // ピアノロールの帯（第 3 回）: 道具の列、左右の欄の見出し、ベロシティと CC の札、その場の読み値。
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
  // クリップがない所の案内（ノートの場所の左上）。{key} は書く道具のキー（key.toolDraw）。
  "pr.emptyTitle": s6(
    "クリップがない所のピアノロール",
    "ここにはクリップがありません。",
    undefined,
    "quoted",
  ),
  "pr.emptyBody": s6(
    "クリップがない所のピアノロール",
    "書く道具（{key}）で押すと、そこから 4 小節のクリップを作って書きます。鍵盤で弾いて録っても作られます。",
    { key: "2" },
    "quoted",
  ),
  "pr.toolSelect": e("選ぶ", "mock:A-PianoRoll"),
  "pr.toolDraw": e("書く", "mock:A-PianoRoll"),
  "pr.toolErase": e("消す", "mock:A-PianoRoll"),
  "pr.grid": e("グリッド", "mock:A-PianoRoll"),
  "pr.scaleLock": e("スケール固定", "mock:A-PianoRoll"),
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
    "オン ・ オフの CC の読み値",
    "{state}（{value}）",
    { state: "踏む", value: "127" },
    "quoted",
  ),
  "pr.switchOn": s6("オン ・ オフの CC の読み値", "踏む", undefined, "part"),
  // 「そのほかの CC」の一覧の行と、えらんだ番号の帯の名前。
  "pr.ccNumber": e("CC {number}", "user:2026-11-15", { number: "7" }),
  // 上のメニューの行と右クリックの一覧（ヤチヨさんの決め 2026-11-18。design-notes B-59 ・ 60）。右クリックの一覧も同じ行の名前を使う。
  "menu.file": e("ファイル", "user:2026-11-18"),
  "menu.edit": e("編集", "user:2026-11-18"),
  "menu.view": e("表示", "user:2026-11-18"),
  "menu.play": e("再生", "user:2026-11-18"),
  "menu.notes": e("ノート", "user:2026-11-18"),
  "menu.track": e("トラック", "user:2026-11-18"),
  "menu.newSong": e("新しい曲", "user:2026-11-18"),
  "menu.openSong": e("ひらく…", "user:2026-11-18"),
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
  "menu.playStart": e("再生", "user:2026-11-18"),
  "menu.stop": e("停止", "user:2026-11-18"),
  "menu.record": e("録音", "user:2026-11-18"),
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
  // ベロシティ固定（B-66）とベロシティをまとめて変える（B-67）。
  "input.velocityFixed": e("固定", "user:2026-11-18"),
  "inspector.matchVelocity": e("そろえる", "user:2026-11-18"),

  // ---- 区切り1 で画面の係が足す文は、ここから下に（ピアノロールの係はこの段、アレンジと再生まわりの係は次の段）。
  // [ピアノロール]
  // ひらいた「CC の一覧」の列の見出し（位置は note.position）。
  "ccList.value": e("値", "mock:R5-Lists"),
  // [アレンジ ・ 再生まわり ・ 入力計]
  // 再生の位置の欄に読めない物を打った（デザインに戻した事 61）。再生の位置には上のはしがないので、範囲の文の形を使わない。
  "msg.playPositionRejected": s6(
    "位置として読めない",
    "位置は「5」「5.2」「5.2.240」の形で打ちます。もとの {kept} のままです。",
    { kept: "1. 1. 000" },
  ),

  // [第 6 回 ・ テンポと拍子の変更、マーカー]（仕様書 3 章 Y17 ・ Y18、6 章。仮の文は design-notes B-92）
  "msg.tempoRampSelected": s6(
    "テンポのだんだんをえらんでいる",
    "{place}の変更をえらんでいます ・ {tempo} から {toPlace}の {toTempo} へ、だんだん{direction}",
    { place: "5 小節目", tempo: "104.00", toPlace: "9 小節目", toTempo: "120.00", direction: "速く" },
  ),
  "msg.markerAdded": s6("マーカーを足した", "{name} を足しました。名前を打って Enter", { name: "マーカー 4" }),
  "timeline.rampFaster": s6("テンポのだんだんをえらんでいる", "速く", undefined, "part"),
  "timeline.rampSlower": e("遅く", "builder:2026-11-20"),
  "timeline.placeBar": s6("テンポのだんだんをえらんでいる", "{bar} 小節目", { bar: "5" }, "part"),
  "timeline.placeBeat": e("{bar} 小節目の {beat} 拍目", "builder:2026-11-20", { bar: "5", beat: "3" }),
  "timeline.placeHead": e("曲の頭", "builder:2026-11-20"),
  "msg.tempoPointSelected": e("{place}の変更をえらんでいます ・ {tempo}", "builder:2026-11-20", { place: "5 小節目", tempo: "120.00" }),
  "msg.meterChangeSelected": e("{place}の変更をえらんでいます ・ {signature}", "builder:2026-11-20", { place: "9 小節目", signature: "3/4" }),
  // 曲の頭の点と旗は「変更」と呼ばない（仕様書 3 章 92-2）。
  "msg.headTempoSelected": s6("曲の頭のテンポをえらんでいる", "曲の頭のテンポをえらんでいます ・ {tempo}", { tempo: "96.00" }, "part"),
  "msg.headMeterSelected": s6("曲の頭のテンポをえらんでいる", "曲の頭の拍子をえらんでいます ・ {signature}", { signature: "4/4" }, "quoted"),
  "msg.tempoPointSet": e("{place}のテンポを {tempo} にしました。", "builder:2026-11-20", { place: "5 小節目", tempo: "120.00" }),
  "msg.meterChangeSet": e("{place}の拍子を {signature} にしました。", "builder:2026-11-20", { place: "9 小節目", signature: "3/4" }),
  "marker.newName": e("マーカー {count}", "spec:3", { count: "4" }),
  // 帯の見出し（ひらいたとき。変更が 0 件なら「ほか」をつけない）、だんだんの線の上の文字、右クリックの行。
  "timeline.tempoSummary": e("テンポ {tempo} ほか {count}", "spec:3", { tempo: "96.00", count: "3" }),
  "timeline.tempoOnly": e("テンポ {tempo}", "spec:3", { tempo: "96.00" }),
  "timeline.meterSummary": e("拍子 {signature} ほか {count}", "spec:3", { signature: "4/4", count: "2" }),
  "timeline.meterOnly": e("拍子 {signature}", "spec:3", { signature: "4/4" }),
  "timeline.ramp": e("だんだん", "spec:2"),
  "menu.tempoRamp": e("次の点まで、だんだん", "spec:3"),
  "menu.loopFromMarker": e("ここから次のマーカーまでをループに", "spec:3"),
  "menu.renameMarker": e("名前を変える", "spec:3"),
  // 拍子の旗の右クリック（仕様書 3 章 92-17）。
  "menu.typeMeter": e("拍子を打ちこむ…", "spec:3"),

  // [第 6 回 ・ オートメーション、取り消しの履歴]（仕様書 3 章 Y16 ・ Y19、6 章。仮の文は design-notes B-92）
  // 状態行。（何を）がない手は、かっこごと出さない（Bare）。
  "msg.undone": s6("取り消した", "「{name}」を取り消しました（{what}）。やりなおすには {redoKey}", {
    name: "ノートを動かす",
    what: "3 ノート",
    redoKey: "Mod Shift Z",
  }),
  "msg.undoneBare": e("「{name}」を取り消しました。やりなおすには {redoKey}", "builder:2026-11-20", { name: "ミュート", redoKey: "Mod Shift Z" }),
  "msg.redone": s6("やりなおした", "「{name}」をやりなおしました（{what}）。", { name: "ノートを動かす", what: "3 ノート" }),
  "msg.redoneBare": e("「{name}」をやりなおしました。", "builder:2026-11-20", { name: "ミュート" }),
  "msg.historyJumped": s6("履歴で戻した", "{index} 手目「{name}」まで戻しました（{steps} 手）。", { index: "11", name: "マーカーを足す", steps: "5" }),
  // いまより上の行へとんだとき（仕様書 3 章 Y19「そこまでやりなおす」。文は 6 章にない）。
  "msg.historyJumpedForward": e("{index} 手目「{name}」までやりなおしました（{steps} 手）。", "builder:2026-11-20", {
    index: "14",
    name: "クオンタイズ",
    steps: "2",
  }),
  "msg.nothingToUndo": s6("取り消せる手がない", "取り消せる手がありません。"),
  "msg.nothingToRedo": s6("やりなおせる手がない", "やりなおせる手がありません。"),
  "msg.automationWriting": s6("オートメーションを書いている", "{track} ・ {target}を書いています（● 書く ・ 止めるとオフに戻ります）", {
    track: "1 ピアノ",
    target: "音量",
  }),
  "msg.automationRested": s6(
    "オートメーションを休ませた",
    "{track} の{target}を手で動かしたので、オートメーションを休ませました。「もどす」で線のとおりに戻ります",
    { track: "4 パッド", target: "音量" },
  ),
  "msg.automationRestored": s6("オートメーションをもどした", "{track} の{target}を、オートメーションのとおりに戻しました。", {
    track: "4 パッド",
    target: "音量",
  }),
  "msg.automationMoving": e("{tracks} の音量は、オートメーションで動いています", "mock:F6-MixerAuto", { tracks: "1 ピアノ と 4 パッド" }),
  // 帯が動かす物の名前（帯の見出し「オートメーション ・ （何を）」、状態行、履歴の行）。パンは mixer.pan。
  "lane.volume": s6("オートメーションを書いている", "音量", undefined, "part"),
  "lane.mute": e("ミュート", "spec:3"),
  "lane.send": e("センド {bus}", "builder:2026-11-20", { bus: "A" }),
  "lane.param": e("{effect}の「{param}」", "builder:2026-11-20", { effect: "コンプレッサー", param: "しきい値" }),
  // プラグインがパラメータの名前を言ってこないとき。
  "lane.pluginParam": e("{plugin}のパラメータ", "builder:2026-11-20", { plugin: "Surge XT " }),
  // アレンジの帯の見出し（2 段）と、読み値（仕様書 2 章 ・ 3 章 Y16）。
  "lane.title": e("オートメーション ・ {target}", "mock:F6-AutoRec", { target: "音量" }),
  "lane.write": e("● 書く", "spec:2"),
  "lane.writing": e("書いています", "mock:F6-AutoRec"),
  "lane.restore": e("もどす", "spec:2"),
  "lane.resting": e("休み ・ {value}", "mock:F6-AutoRest", { value: "−12.0 dB" }),
  "lane.restingBare": e("休み", "spec:2"),
  "lane.now": e("いま {value} ・ 点 {count}", "mock:F6-ParamMany", { value: "523.25 Hz", count: "4" }),
  "lane.count": e("点 {count}", "mock:F6-ParamMany", { count: "4" }),
  // 見出しの下の行と、押すとひらく一覧の最後の行（仕様書 3 章 92-5 ・ 20-A）。
  "lane.add": e("＋ 帯を足す ▾", "spec:3"),
  "lane.addEffectParams": e("エフェクトのパラメータ…", "spec:3"),
  "lane.delete": e("帯を消す", "spec:3"),
  // 帯の右上と右下（仕様書 3 章 92-6 ・ 21-B）。
  "lane.rangeTop": e("上 {value}", "spec:3", { value: "−3.0 dB" }),
  "lane.rangeBottom": e("下 {value}", "spec:3", { value: "−9.0 dB" }),
  "lane.readAdd": e("{position} ・ {value}", "mock:R10-AutoEdit", { position: "5. 3. 000", value: "−8.4 dB" }),
  "lane.readMove": e("{delta} ・ {from} → {to}", "mock:R10-AutoEdit", { delta: "+0. 1. 000", from: "−2.1", to: "−0.6 dB" }),
  // ミキサーのフェーダーの値の下（仕様書 2 章「オートメーションで動くつまみ」）。休みは lane.restingBare と lane.restore。
  "mixer.automation": e("オートメーション", "spec:2"),
  // 履歴の行「「（手の名前）」・ （何を）」と、（何を）の形。手の名前は下の stepNames。
  "history.row": e("「{name}」・ {what}", "mock:R10-History", { name: "ノートを動かす", what: "3 ノート" }),
  "history.rowBare": e("「{name}」", "builder:2026-11-20", { name: "ミュート" }),
  "history.notes": s6("取り消した", "{count} ノート", { count: "3" }, "part"),
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
  // 約束 23 で出せるようになった（何を）（仕様書 3 章 92-3）。1 ノートは音名だけ（「C4」）。
  "history.quantized": e("{grid} ・ {count} ノート", "spec:3", { grid: "1/16", count: "12" }),
  "history.velocity": e("{count} ノート ・ {from} → {to}", "spec:3", { count: "3", from: "96", to: "110" }),
  "history.controlPoints": e("{control} ・ {count} 点", "spec:3", { control: "サステイン", count: "4" }),
  "history.color": e("{track} ・ 色 {color}", "spec:3", { track: "4 パッド", color: "3" }),
  "history.soundReplaced": e("{track} ・ {from} → {to}", "spec:3", { track: "1 ピアノ", from: "グランドピアノ", to: "オルガン" }),
  // 歌のトラックの入口の一覧（仕様書 3 章「歌のトラックの見出し」）。
  "vocal.input.mono1": e("入口 1 ・ モノ", "spec:3"),
  "vocal.input.mono2": e("入口 2 ・ モノ", "spec:3"),
  "vocal.input.stereo12": e("入口 1 ・ 2 ・ ステレオ", "spec:3"),
  // プラグインがパラメータの名前を言ってこなかった手の名前。
  "history.pluginParam": e("{plugin}のパラメータを変える", "builder:2026-11-20", { plugin: "Surge XT " }),
  // 行の右の「どこで」。ほかは band.arrange ・ band.pianoRoll ・ band.mixer と track.label。
  "history.placePluginWindow": e("{strip} ・ プラグインの窓", "spec:6", { strip: "5 Surge XT" }),
  // 帯の見出しの行（仕様書 3 章 Y19 ・ 92-9、6 章）。
  "history.head": e("{total} 手 ・ いまは {current} 手目。上の {redo} 手は取り消してあり、やりなおせます", "mock:F-History", {
    total: "14",
    current: "12",
    redo: "2",
  }),
  "history.headLatest": s6("履歴の見出し ・ 取り消した手がない", "{total} 手 ・ いまは {current} 手目（いちばん新しい手）", {
    total: "14",
    current: "14",
  }),
  "history.headAllUndone": s6("履歴の見出し ・ ぜんぶ取り消した", "{total} 手 ・ ぜんぶ取り消してあります ・ やりなおすには {redoKey}", {
    total: "14",
    redoKey: "Mod Shift Z",
  }),
  "history.headEmpty": s6("履歴の見出し ・ 手がない", "まだ手がありません ・ 何かすると、ここに 1 手ずつならびます"),
  "history.undo": e("取り消す", "mock:F-History"),
  "history.redo": e("やりなおす", "mock:F-History"),
  // 行の右（仕様書 2 章「履歴の行 ・ 乗った ・ えらんだ」、3 章 Y19）。ボタンの右に key.enter。
  "history.current": e("いまここ", "mock:F-History"),
  "history.redoable": e("やりなおせます", "mock:F-History"),
  "history.jumpHere": e("ここまで戻す", "spec:3"),
  // 帯のいちばん下の行。見えている行より前の手があるとき。
  "history.more": e("… {index} 手目より前は、下へスクロール", "mock:F-History", { index: "6" }),

  // [第 7 回 ・ 歌のトラック、音のファイル、遅れ、書き出し]（仕様書 3 章 93 ・ 6 章）。区切り2〜5 で使う。
  "msg.vocalTrackAdded": s6(
    "歌のトラックを足した",
    "{track} を足しました ・ {input} から録ります ・ 歌のトラックの ● は、選んでいなくても自分でつけます",
    { track: "6 歌", input: "入口 1" },
  ),
  "msg.noAudioInput": s6("入口がない", "音の入口がありません。マイクをつなぐか、音の設定で入口をえらんでください。"),
  "msg.monitorFirstOn": s6(
    "はじめてモニターをオンにした",
    "自分の声が少し遅れて聞こえます（いまは {ms} ms）。スピーカーだと音がまわって「キーン」となるので、ヘッドホンで聞いてください。",
    { ms: "13" },
  ),
  "msg.vocalNoPianoRoll": s6("歌のトラックにピアノロールはない", "歌のトラックにはピアノロールがありません。波形はアレンジで直します。"),
  "msg.recordingVocal": s6("歌を録っている", "● 録音中 ・ {track}（{input}）・ モニター オン ・ 止めるには {stopKey}", {
    track: "6 歌",
    input: "入口 1",
    stopKey: "Space",
  }),
  "msg.recordingKeysAndVocal": s6(
    "鍵盤と歌を録っている",
    "● 録音中 ・ {keys}（鍵盤）と {vocal}（{input}）に録っています ・ 止めるには {stopKey}",
    { keys: "1 ピアノ", vocal: "6 歌", input: "入口 1", stopKey: "Space" },
  ),
  "recovery.vocalKept": s6("録っていた歌を残した", "録っていた歌 {count} つを、止まる直前（{position} ごろ）まで残しました。", {
    count: "1",
    position: "9. 3. 0000",
  }),
  "msg.audioFilesMissing": s6("音のファイルが見つからない", "曲で使っている音のファイルが {count} つ見つかりません。そのクリップは鳴りません。", {
    count: "2",
  }),
  "params.audioFileMissing": s6("音のファイルが見つからない（欄）", "{file} が見つかりません。前はここにありました: {path}", {
    file: "歌 テイク 1.wav",
    path: "はじめての曲/音/歌 テイク 1.wav",
  }),
  "button.findFile": e("さがす…", "spec:6"),
  "msg.clipGainDragging": s6(
    "クリップの音量を動かしている",
    "{clip} のクリップの音量 {value}（ドラッグ中）・ 離すと決まります ・ やめるには {cancelKey}",
    { clip: "歌 テイク 1", value: "−3.0 dB", cancelKey: "Esc" },
  ),
  "msg.clipSplit": s6("切った", "{clip} を {position} で切りました。", { clip: "歌 テイク 1", position: "7. 1. 0000" }),
  "msg.audioRendering": s6("音を作っている", "{clip} の音を作っています {percent} % ・ できるまでは、もとの音で鳴らします", {
    clip: "歌 テイク 1",
    percent: "34",
  }),
  "msg.audioImported": s6("音のファイルを入れた", "{file} を入れました（{track} ・ {position}）。", {
    file: "ドラム.wav",
    track: "6 歌",
    position: "5. 1. 0000",
  }),
  "clip.loading": s6("音のファイルを読みこんでいる", "読みこんでいます {percent} %", { percent: "40" }),
  "msg.audioUnreadable": s6("読めないファイル", "「{file}」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。", { file: "声.aac" }),
  "msg.audioDragging": s6(
    "音のファイルを持ってきている",
    "音のファイルを {count} つ持ってきています ・ 左に落とすと別々のトラック、右に落とすと 1 本にならべる",
    { count: "8" },
  ),
  // 音のファイルを 1 つだけ持ってきている間（仕様書の文は 2 つ以上のときの形。「SF2 のファイルを持ってきています」にならった）。
  "msg.audioDraggingOne": e("音のファイルを持ってきています", "builder:2026-11-21"),
  // 「さがす」（「この音のテンポ」）の答え。見つけた文は「…くらいです」に、入れたか同じかの終わりをつなぐ。
  "msg.sourceTempoFound": s6("この音のテンポをさがした", "この音のテンポは {bpm} くらいです", { bpm: "120.00" }, "part"),
  "msg.sourceTempoSet": s6("この音のテンポをさがした", "（さがした数を入れました）。", undefined, "part"),
  "msg.sourceTempoSame": s6("この音のテンポをさがした", "（いまの数と同じです）。", undefined, "part"),
  "msg.sourceTempoNotFound": s6("この音のテンポをさがした", "この音のテンポは見つかりませんでした。数を打ちこんでください。", undefined, "quoted"),
  "drop.mixed": s6("SF2 と音のファイルをまぜて持ってきた", "SF2 は {sf2} つ棚へ ・ 音のファイル {audio} つはアレンジへ", {
    sf2: "1",
    audio: "3",
  }),
  // 持ってきている間の、分かれた箱（仕様書 2 章、3 章 30-C、モック F7-AudioDrop）と、入る所の枠の名前。
  "drop.separate": e("別々のトラックに", "spec:3"),
  "drop.separateSay": e("{files} つのファイルを {tracks} 本の歌のトラックにして、落とした位置から入れます。", "mock:F7-AudioDrop", {
    files: "8",
    tracks: "8",
  }),
  "drop.inRow": e("1 本にならべる", "spec:3"),
  "drop.inRowSay": e("落とした所のトラックに、順にうしろへ。", "mock:F7-AudioDrop"),
  "drop.frameMore": e("{file} ほか {more}", "mock:F7-AudioDrop", { file: "ドラム キック.wav", more: "7" }),
  // 歌のクリップの中の文（仕様書 3 章「音のファイルが見つからないとき」）。
  "clip.missing": e("音のファイルが見つかりません", "spec:3"),
  // 選んだクリップの欄（仕様書 3 章 93-4 ・ Y22、モック F7-AudioEdit ・ F7-AudioMissing）。
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
  // 音を作っている間の、欄の上の進みの棒の行（仕様書 2 章「選んだクリップ」、モック F7-Stretch）。
  "clipParams.rendering": e("音を作っています", "mock:F7-Stretch"),
  "clipParams.renderingPercent": e("{percent} %", "mock:F7-Stretch", { percent: "34" }),
  // 歌のクリップの、名前の右の印と、下の進み（仕様書 2 章「歌のクリップ」）。
  "clip.ratioMark": e("×{ratio}", "spec:2", { ratio: "1.25" }),
  "clip.pitchMarkCents": e("{semitones} ・ {cents}", "spec:3", { semitones: "−1", cents: "30" }),
  "clip.rendering": e("音を作っています {percent} %", "spec:2", { percent: "34" }),
  // 歌のクリップの右クリックの一覧の下の、あとの回の席（仕様書 3 章「音を作っている間」）。
  "menu.reverse": e("逆にする", "spec:3"),
  "menu.normalize": e("ノーマライズ", "spec:3"),
  "latency.measuring": s6("遅れを測っている", "測っています {done} / {total}", { done: "3", total: "5" }),
  "latency.failed": s6("遅れを測れない", "音が小さくて測れませんでした。ヘッドホンの音量を上げて、もう一度測ってください。"),
  "button.useThisValue": e("この数を使う", "spec:6"),
  "button.again": e("もう一度", "spec:6"),
  "button.audioSettingsEllipsis": e("音の設定…", "spec:6"),
  "settings.notAsRequested": s6(
    "頼んだ数で動いていない",
    "頼んだ {requested} ではなく、{actual} で動いています（PC の設定で決まっています）。meloet は PC 全体の設定は変えません。",
    { requested: "128", actual: "256" },
  ),
  "msg.exporting": s6("書き出している", "書き出しています {percent} % ・ {file} ・ 残り 約 {left} ・ 弾けます（書き出しの音は聞こえません）", {
    percent: "46",
    file: "はじめての曲.wav",
    left: "40 秒",
  }),
  "msg.exportingRealtime": s6("実時間で書き出している", "速く書き出せないプラグインがあるので、実時間で書き出します（{length}）。", {
    length: "3:02",
  }),
  "msg.exportedFile": s6("書き出した", "{file} を書き出しました（{length} ・ {rate} Hz ・ {bits} ビット）。", {
    file: "はじめての曲.wav",
    length: "3:02",
    rate: "48000",
    bits: "24",
  }),
  "msg.exportCanceled": s6("書き出しをやめた", "書き出しをやめました。途中までのファイルは消しました。"),

  // [第 7 回 区切り5 ・ 書き出し]（仕様書 3 章 34-B、モック F7-Export ・ F7-Exporting ・ F-Export）
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
  // 止まっているプラグインがあるとき（仕様書 3 章「F-Export のまま残す」）。
  "export.stoppedPlugins": e("止まっているプラグイン", "mock:F-Export"),
  "export.stoppedPlugin": e("{plugin} ・ {strip}", "mock:F-Export", { plugin: "Brick EQ", strip: "3 ドラム" }),
  "export.stoppedSkipped": e("この枠をとばした音で書き出します", "mock:F-Export"),
  // 状態行の左の進み（仕様書 2 章「書き出しの進み」）。
  "export.running": e("書き出しています", "mock:F7-Exporting"),
  "export.runningPercent": e("{percent} %", "mock:F7-Exporting", { percent: "46" }),
  "export.runningRest": e("{file} ・ 残り 約 {left} ・ 弾けます（書き出しの音は聞こえません）", "mock:F7-Exporting", {
    file: "はじめての曲.wav",
    left: "40 秒",
  }),
  // 仕様書 ・ モックにない所の仮の文（B-94-69〜）。
  "export.checkWaiting": e("たしかめの用意をしています", "builder:2026-11-21"),
  "export.checkCounting": e("数えています {percent} % ・ ここまでのいちばん大きい所 {peak} dB", "builder:2026-11-21", {
    percent: "38",
    peak: "−3.5",
  }),
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
  // 残りの時間がまだ分からない間（はじめの数ブロック）は、「残り 約 …」を外す（B-94-81）。
  "export.runningRestUnknown": e("{file} ・ 弾けます（書き出しの音は聞こえません）", "builder:2026-11-21", { file: "はじめての曲.wav" }),
  "msg.exportFailedPlugin": e("書き出せませんでした。{plugin} が止まりました。途中までのファイルは消しました。", "builder:2026-11-21", {
    plugin: "Surge XT",
  }),
  "msg.exportFailedWrite": e("書き出せませんでした。保存する所に書けません。", "builder:2026-11-21"),
  "msg.exportingRealtimeChosen": e("実時間で書き出しています（{length}）。", "builder:2026-11-21", { length: "3:02" }),

  // [第 7 回 区切り2 ・ 歌のトラックの画面]（仕様書 2 章 ・ 3 章 23-A ・ 24-C ・ 31-C ・ 33-A、モック F7-*）
  // トラックの一覧の下と、上のメニュー「トラック」。
  "arrange.addKeysTrack": e("＋ 鍵盤のトラック", "spec:3"),
  "arrange.addVocalTrack": e("＋ 歌のトラック", "spec:3"),
  "menu.addKeysTrack": e("鍵盤のトラックを足す…", "spec:3"),
  "menu.addVocalTrack": e("歌のトラックを足す", "spec:3"),
  "menu.importAudio": e("音のファイルを入れる…", "spec:3"),
  // 歌のトラックの見出しの 2 段目。入口の名前は vocal.input.*。
  "vocal.inputChoose": e("{input} ▾", "mock:F7-VocalRec", { input: "入口 1 ・ モノ" }),
  "vocal.noInput": e("入口なし ▾", "spec:3"),
  "vocal.monitor": e("モニター", "spec:3"),
  // 状態行の「6 歌（入口 1）」の入口。入口 2 は仕様書にない（入口 1 ・ 入口 1 ・ 2 にならった）。
  "vocal.inputShort.mono1": e("入口 1", "spec:6"),
  "vocal.inputShort.mono2": e("入口 2", "builder:2026-11-21"),
  "vocal.inputShort.stereo12": e("入口 1 ・ 2", "spec:5"),
  // 録っている歌のクリップの名前（モック F7-VocalRec の「歌 テイク 1」）。
  "arrange.recordingVocalClip": e("{track} テイク {take}", "mock:F7-VocalRec", { track: "歌", take: "1" }),
  // 入力計の右（マイクの計）。目盛はモックの字。
  "meter.monitorOn": e("モニター オン", "mock:F7-VocalRec"),
  "meter.monitorOff": e("モニター オフ", "mock:F7-VocalTrack"),
  "meter.micScaleLow": e("−60", "mock:F7-VocalTrack"),
  "meter.micScaleMid": e("−12", "mock:F7-VocalTrack"),
  "meter.micScaleTop": e("0", "mock:F7-VocalTrack"),
  // 弾くと鳴る先がない（鍵盤のトラックが 1 本もない。B-94-1）。
  "meter.playTargetNone": e("—", "spec:3"),
  // 音の設定 ▸ くわしい設定（モック F7-SettingsAdv）。
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
  // 測った数を使っているとき（仕様書 3 章 33-A の「測った数」に、PC が言う数の形をならった）。
  "settings.latencyMeasured": e("測った数 ・ {ms} ms", "builder:2026-11-21", { ms: "12.4" }),
  "settings.inputChoice": e("{channels}（{name}）", "mock:F7-SettingsAdv", { channels: "入口 1 ・ 2", name: "USB マイク" }),
  "button.measureLatency": e("遅れを測る…", "mock:F7-SettingsAdv"),
  "button.manualLatency": e("手で決める…", "mock:F7-SettingsAdv"),
  "button.openPlaces": e("ひらく…", "mock:F7-SettingsAdv"),
  // 遅れを測る（モック F7-Latency）。
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

  // [第 8 回 ・ core から出す知らせと音の素材]（仕様書 6 章 第 12 版、モック F8-*）。約束の係（k8p）がこの下に足す。
  "latency.failedUnsteady": s6(
    "遅れを測れない ・ ばらつき",
    "測るたびに数が大きくちがったので、決められませんでした。まわりの音を小さくして、もう一度測ってください。",
  ),
  "latency.failedNoInput": s6("遅れを測れない ・ 入口がない", "音の入口がないので測れません。マイクをつなぐか、入口をえらんでください。"),
  "msg.inputLost": s6("入口が抜けた", "{input}（{device}）が抜けました。録音を止めました。録れた所までは残しています。", {
    input: "入口 1",
    device: "USB マイク",
  }),
  "msg.recordingGaps": s6(
    "録っている間に音がとぎれた",
    "録っている間に音が {count} 回とぎれました（PC がいそがしかったため）。とぎれた所は無音で残しています。バッファを大きくすると、とぎれにくくなります。",
    { count: "3" },
  ),
  "msg.audioFound": s6("さがして見つけた", "{file} を見つけました。{clips} つのクリップがまた鳴ります。", {
    file: "歌 テイク 1.wav",
    clips: "2",
  }),
  "msg.audioFoundMismatch": s6("さがしたファイルが合わない", "えらんだファイルは使えません（サンプルレートかチャンネルの数がちがいます）。"),
  "msg.audioBroken": s6("こわれたファイル ・ 3 チャンネル以上", "「{file}」はこわれているので読めません。", { file: "声.wav" }, "part"),
  "msg.audioTooManyChannels": s6(
    "こわれたファイル ・ 3 チャンネル以上",
    "「{file}」は 3 チャンネル以上なので読めません（読めるのはモノとステレオ）。",
    { file: "8ch.wav" },
    "part",
  ),
  "msg.savingAudio": s6("保存で音を写している", "保存しています ・ 音のファイルを写しています {done} / {total}（{doneMb} / {totalMb} MB）", {
    done: "3",
    total: "12",
    doneMb: "80",
    totalMb: "240",
  }),
  "msg.diskTooSlow": s6("ディスクが間に合わない", "音のファイルを読むのが間に合わず、歌が少し切れました（ディスクがいそがしかったため）。"),
  // 音の素材（仕様書 2 章 ・ 3 章 43-B ・ 44-A ・ 45-A ・ 6 章、モック F8-Pack*）。案内の帯の 2 行は 6 章の文を分けた物。
  "pack.fetching": s6("音の素材を取りにいっている", "音の素材を取りにいっています", undefined, "part"),
  "pack.fetchingBytes": s6("音の素材を取りにいっている", "{percent} % ・ {done} / {total} MB", { percent: "38", done: "92", total: "240" }, "part"),
  "pack.fetchingLeft": s6("音の素材を取りにいっている", "のこり 約 {minutes} 分", { minutes: "2" }, "part"),
  "pack.fetched": s6("音の素材が取れた", "音の素材をぜんぶ取りました。{count} の音色が鳴ります。", { count: "31" }),
  "pack.offline": s6(
    "音の素材 ・ つながっていない",
    "音の素材を取りにいけませんでした（インターネットにつながっていません）。つながったら「もう一度」を押してください。",
  ),
  "pack.cutOff": s6("音の素材 ・ とちゅうで切れた", "とちゅうで切れました。{done} / {total} MB まで取ってあります。続きから取ります。", {
    done: "92",
    total: "240",
  }),
  "pack.diskFull": s6("音の素材 ・ ディスクがいっぱい", "ディスクがいっぱいで、音の素材を置けません（あと {mb} MB 要ります）。", { mb: "150" }),
  "pack.corrupt": s6("音の素材 ・ こわれていた", "取った音の素材がこわれていました。取りなおしても直りませんでした。"),
  "pack.newPack": s6("音の素材 ・ 新しい包み", "音の素材の新しい包みがあります（{mb} MB）。", { mb: "12" }),
  "button.fetchPack": e("取りにいく", "spec:6"),
  "button.resumeFetch": e("続きから", "spec:6"),
  "button.packLater": e("あとで", "spec:3"),
  "pack.ask": e("音の素材（約 {mb} MB）を取りにいきます", "spec:3", { mb: "240" }),
  "pack.askDetail": e("グランドピアノなど {count} の音色 ・ 取っている間も弾けます", "spec:3", { count: "31" }),
  // 幅 853 の窓で、聞く帯の小さい字が入りきらないとき。
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

  // [メニュー ・ キー]
  // メニューの行の右に書くキーの名前（仕様書 4 章の表）。Mod は key.ctrl か key.cmd に読みかえる。
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
  "key.mute": e("M", "spec:4"),
  "key.solo": e("S", "spec:4"),
  // meloet を終わる Mod Q（4 章の表にはない。design-notes B-59）。
  "key.quit": e("Q", "user:2026-11-18"),

  // [第 4 回 区切り1 ・ トラックの見出しと棚の画面]
  // 上のメニューの「トラック」と「ヘルプ」の行（仕様書 3 章の上のメニューの行）。右クリックの一覧も同じ名前。
  "menu.renameTrack": e("名前を変える", "spec:3"),
  "menu.trackColor": e("色", "spec:3"),
  "menu.moveTrackUp": e("上へ動かす", "spec:3"),
  "menu.moveTrackDown": e("下へ動かす", "spec:3"),
  "menu.deleteTrack": e("トラックを消す", "spec:3"),
  "menu.help": e("ヘルプ", "spec:3"),
  "menu.about": e("meloet について", "spec:3"),
  // 色の一覧の行（仕様書 2 章「「色 1」〜「色 5」」）。
  "menu.colorNumber": e("色 {number}", "spec:2", { number: "1" }),
  "key.renameTrack": e("F2", "spec:4"),
  "key.favorite": e("F", "spec:4"),

  // [第 4 回 区切り2 ・ meloet について]（仕様書 3 章、モック R7-About ・ R7-AboutLicense）
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
  // 手を加えたこと（音の素材ごと）。モックの「音の大きさをそろえ、ファイルの形を変えた」の書きかたで、
  // tools/sounds/build.py の CHANGES（NOTICE の英語の文）と同じ中身を言う。
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
  // [第 8 回 ・ 画面だけで出す文]（仕様書 3 章 ・ 6 章 第 12 版、モック F8-*）。画面の係（k8ua ・ k8ub）がこの下に足す。
} as const satisfies Record<string, Entry>;

export type TextKey = keyof typeof texts;

/**
 * 取り消しの履歴の「手の名前」（仕様書 3 章 Y19）。約束の手の名前の種類（StepName の kind）ごとに 1 つ。
 * spec:3 は 3 章「手の名前」の一覧の言いかた（「CC の点を書く ・ 動かす ・ 消す」は「CC の点を動かす」のように開く。
 * 一覧の「録音（テイク）」は「録音」。「クリップを … 切る」は S（再生の線で切る。3 章「モックの直し」）で、
 * 切り取り（Mod X）は仮に「クリップを切り取る」（design-notes B-94-7））。
 * 第 11 版で一覧に入った仮の名前（92-7）も spec:3。一覧にない名前は builder:（design-notes B-92）。
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

/** 仕様書 5 章「用語の説明」。形は「用語 — 説明」。 */
export const terms: readonly Term[] = [
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
];

/** 用語と用語の説明をつなぐ記号（仕様書 5 章「用語 — 説明」）。 */
export const TERM_SEPARATOR = " — ";

export function findTerm(term: string): Term | undefined {
  return terms.find((t) => t.term === term);
}

/** {name} の穴に値を入れる。値のない穴はそのままのこす。 */
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

/** 状態行の左の文の、部分どうしのつなぎ（仕様書 6 章「止まっているときの状態行の左」の「 ・ 」でつなぐ）。 */
export const STATUS_JOIN = " ・ ";

/** 3 つ以上の名前をならべるときの、最後の「と」より前のつなぎ（仕様書 6 章「棚をひらいている（見出し）」の「ピアノ ・ 鍵盤、ギター と ベース」）。 */
export const LIST_COMMA = "、";
