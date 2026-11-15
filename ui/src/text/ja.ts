// 画面の文面はすべてここに置く。
// 出どころ: spec:<章>（docs/design/spec.html）、mock:<ボード>（docs/design/mock/<ボード>.dc.html）、user:<日付>（ヤチヨさんの決め）。
// {name} はうめこむ値の穴。example は出どころに書いてある例の値で、テストがこれを入れて出どころと照らし合わせる。

export type Source = `spec:${number}` | `mock:${string}` | `user:${string}`;

export interface Entry {
  readonly text: string;
  readonly source: Source;
  readonly example?: Readonly<Record<string, string>>;
  /** spec:6 の表の「場面」。 */
  readonly scene?: string;
  /** 文面のセルとの合わせかた。whole: セルの文そのまま。quoted: セルの「」の中。part: セルの文の一部。 */
  readonly fit?: "whole" | "quoted" | "part";
}

const e = (text: string, source: Source, example?: Record<string, string>): Entry => ({ text, source, example });
const s6 = (
  scene: string,
  text: string,
  example?: Record<string, string>,
  fit: "whole" | "quoted" | "part" = "whole",
): Entry => ({ text, source: "spec:6", scene, example, fit });

export const texts = {
  // 仕様書 6 章 「エラーと案内の文面」
  "msg.keyboardMissing": s6("鍵盤がない", "鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。"),
  "msg.keyboardFound": s6("鍵盤が見つかった", "{keyboard} が見つかりました。鍵は届いています。音の出口をえらぶと鳴ります。", { keyboard: "鍵盤 49" }),
  "msg.noOutputMeter": s6("音の出口がまだ", "音の出口がまだです。下でえらんでください", undefined, "quoted"),
  "msg.noOutputArrange": s6("音の出口がまだ", "音をどこから鳴らしますか", undefined, "quoted"),
  "msg.outputNotPlugged": s6("えらんだ出口に何もささっていない", "{output}には、いま何もささっていません。ささると、ここから鳴ります。", { output: "（出口の名前）" }),
  "msg.bufferFixedStatus": s6(
    "バッファが固定されている（状態行）",
    "バッファが PC 全体の設定で {actual} に固定されています ・ 音が出るまで 約 {latencyMs} ms（{requested} なら 約 {latencyMsAtRequested} ms）",
    { actual: "1024", latencyMs: "53", requested: "256", latencyMsAtRequested: "13.5" },
  ),
  "msg.bufferFixedDetail": s6(
    "バッファが固定されている（音の設定の奥）",
    "バッファが PC 全体の設定で {actual} に固定されています。meloet は {requested} を頼んでいますが、{actual} で動いています。鍵を押してから音が出るまで 約 {latencyMs} ms。{requested} で動けば 約 {latencyMsAtRequested} ms です。速いフレーズでは、音が指より遅れて聞こえます。録った音の位置は、遅れの補正で正しく置かれます。meloet は PC 全体の設定を変えません。変える場所は PipeWire の設定（quantum の固定）です。",
    { actual: "1024", latencyMs: "53", requested: "256", latencyMsAtRequested: "13.5" },
  ),
  "msg.soundCameOut": s6("音が出た", "音が出ました。", undefined, "quoted"),
  // 2 文目は、録音ボタンが使えるときだけ出す。
  "msg.soundCameOutRecord": s6("音が出た", "赤い丸の録音ボタンを押すと、カウントインのあとに録りはじめます。", undefined, "quoted"),
  "msg.unusableSwitchedByPc": s6("えらべない出口 ・ PC が切りかえる", "さすと、PC がこちらに切りかえます"),
  "msg.unusableStoppedByPc": s6("えらべない出口 ・ PC の設定で止まっている", "PC の設定で、いまは使えません"),
  "msg.unusableHeading": s6("えらべない出口の見出し", "いまは使えない出口", undefined, "part"),
  "msg.unusableHeadingChooser": s6(
    "えらべない出口の見出し",
    "PC が切りかえる物 {switched} ・ PC の設定で止まっている物 {stopped}",
    { switched: "1", stopped: "7" },
    "part",
  ),
  "msg.unusableHeadingSettings": s6("えらべない出口の見出し", "meloet は、PC 全体の設定を変えません", undefined, "quoted"),
  "msg.unusableInputHeading": s6("えらべない入口の見出し", "いまは使えない入口", undefined, "part"),
  "msg.countingIn": s6("カウントイン中", "カウントイン中 ・ あと {beats} 拍で録音が始まります。止めるには Space", { beats: "2" }),
  // 状態行の左の札「● カウントイン中」（status.countingIn）のうしろに出す部分。札と合わせて上の文になる。
  "msg.countingInRest": s6("カウントイン中", "あと {beats} 拍で録音が始まります。止めるには Space", { beats: "2" }, "part"),
  "msg.recording": s6("録音中", "{track} へ録音 ・ 止めると テイク {take} として残ります", { track: "1 ピアノ", take: "2" }),
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
  "msg.noArmedTrack": s6("録音待機のトラックがない", "録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。"),
  "msg.keyboardLost": s6("鍵盤が抜けた", "{keyboard} とのつながりが切れました。曲はそのままです。さしなおすと、自動で戻ります。", { keyboard: "鍵盤 49" }),
  "msg.outputLost": s6("音の出口が消えた", "{output}が見つからなくなりました。再生を止めています。", { output: "ヘッドホン" }),
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
  "msg.keyboardInputStopped": s6("鍵盤からの入力が止まった", "鍵盤からの入力が止まりました。曲はそのままです。いま、つなぎなおしています。"),
  "msg.micTooLoud": s6("マイクの音が大きすぎる", "入ってくる音が大きすぎて割れています。マイクから少し離れるか、マイクがわの音量を下げてください。"),
  "msg.scanning": s6("スキャン中", "スキャン中 {done} / {total} ・ 調べ終えた分から、もう使えます", { done: "37", total: "112" }),
  "msg.scanDone": s6(
    "スキャンが終わった",
    "スキャンが終わりました ・ {added} 個を棚に入れました。{stopped} 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。",
    { added: "667", stopped: "4" },
  ),
  "msg.windowNotYet": s6("まだ窓をひらけない子", "この子の窓は、まだひらけません。パラメータはここで動かせます。meloet が、この型の窓にまだ対応していないためです。音は、いつもどおり鳴ります。"),
  "msg.noWindow": s6("窓を持たない子", "この子は窓を持ちません。パラメータはここで動かせます。"),
  "msg.windowsOpen": s6("窓がひらいている", "プラグインの窓が {count} つひらいています", { count: "2" }),
  "msg.windowOpening": s6("窓をひらくのに時間がかかる", "{name}の窓をひらいています…", { name: "（名前）" }),
  "msg.windowTooLarge": s6(
    "窓が画面より大きい",
    "{name}の窓がひらいています（{width}×{height} ・ この画面より大きい窓です）。パラメータは右でも動かせます。",
    { name: "（名前）", width: "1597", height: "995" },
  ),
  "msg.searchNoMatch": s6("さがしても見つからない", "「{query}」に合うパラメータはありません。", { query: "（ことば）" }),
  "msg.formatSwitch": s6(
    "形式を入れかえる前",
    "この曲では {current} 版を使っています。{next} 版に入れかえると、いまのパラメータの値は引きつがれないことがあります。",
    { current: "VST3", next: "LV2" },
  ),
  "msg.audition": s6("ためし弾き中", "鍵盤を弾くと、この音色で鳴ります。まだトラックには入っていません。"),
  "msg.samplePlaying": s6("サンプル再生中", "サンプルを再生中 ・ {name}。行をえらぶと、鍵盤でためし弾きできます。", { name: "（名前）" }),
  "msg.auditionLoading": s6("ためし弾きしたい音源がまだ読めない", "{name}を読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。", { name: "（名前）" }),
  "msg.pluginMissing": s6("プラグインが見つからない", "前はここにありました: {path}。曲の中の設定は残しています。", { path: "（場所）" }),
  "msg.pluginStopped": s6(
    "プラグインが止まった",
    "{name}が止まりました ・ {track}。曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。",
    { name: "（名前）", track: "（トラック）" },
  ),
  "msg.pluginScanFailed": s6("プラグインがスキャンでこけた", "{name}は調べている途中で止まったので、棚には入れていません。ほかのプラグインは使えます。", { name: "（名前）" }),
  "msg.undoPending": s6("取り消してある手がある", "{count} 手ぶん取り消してあります。新しく何かすると、その {count} 手はやりなおせなくなります。", { count: "2" }),
  "msg.quantized": s6("クオンタイズをかけた", "クオンタイズ {grid}（{strength} %）を {count} ノートに。うすい枠が、そろえる前の位置。", { grid: "1/16", strength: "75", count: "3" }),
  "msg.unsaved": s6("保存していない", "「{song}」に、まだ保存していない変更があります。", { song: "はじめての曲" }),
  "msg.cannotSave": s6("保存できない", "ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。べつの場所をえらんでください。曲は開いたままです。"),
  "msg.songBroken": s6("曲のファイルがこわれている", "このファイルは読めませんでした。自動で残してある 1 つ前の状態からなら開けます。"),
  // 1 つ前の状態がないときは、上の文の 1 文目だけ。
  "msg.songBrokenFirst": s6("曲のファイルがこわれている", "このファイルは読めませんでした。", undefined, "part"),
  // なおすためのファイル（仕様書 3 章）。PC が終わるときに書いた物は仕様書の文、ふだん書いた物はヤチヨさんの文。
  "msg.recoveredPcEnding": s6("なおすためのファイルから戻した", "PC が終わる前の演奏を戻しました。まだ保存していません。", undefined, "quoted"),
  // 2 文目は「…です。」と「テイク N つ。」に分けて持つ。テイクが 0 のときはうしろを出さない（棟梁の決め 2026-11-15、デザインに戻す）。
  "msg.recoveredPcEndingDetail": s6(
    "なおすためのファイルから戻した",
    "{month} 月 {day} 日 {time} に PC が終わったときの「{song}」です。",
    { month: "11", day: "15", time: "22:41", song: "新しい曲" },
    "part",
  ),
  "msg.recoveredTakes": s6("なおすためのファイルから戻した", "テイク {takes} つ。", { takes: "2" }, "part"),
  "msg.recoveredSudden": e("meloet が急に終わる前の演奏を戻しました。まだ保存していません。", "user:2026-11-15"),
  "msg.recoveredSuddenDetail": e("{month} 月 {day} 日 {time} の「{song}」です。", "user:2026-11-15", {
    month: "11",
    day: "15",
    time: "22:41",
    song: "新しい曲",
  }),
  // 仕様書 6 章の表の「出る場所」の列にある文。
  "status.recovered": e("なおすためのファイルから戻しました ・ まだ保存していません", "spec:6"),
  "msg.recoveryDiscarded": s6("戻さないを押した", "戻した変更をすてて、最後に保存した形にしました。取り消せます（{undoKey}）", { undoKey: "Mod Z" }),
  "msg.recoveryUnreadable": s6("なおすためのファイルが読めない", "前に保存していない変更がありましたが、なおすためのファイルが読めませんでした。", undefined, "quoted"),
  "msg.recoveryUnreadableSaved": s6(
    "なおすためのファイルが読めない",
    "最後に保存した「{song}」をひらいています。ファイルは消さずに残してあります。",
    { song: "はじめての曲" },
    "quoted",
  ),
  "msg.recoveryUnreadableNew": s6("なおすためのファイルが読めない", "新しい曲をひらいています。ファイルは消さずに残してあります。", undefined, "quoted"),
  // 状態行。保存したことのない曲のときは、うしろを「新しい曲をひらいています」にする（モック F-Recover3 の形に、仕様書の言葉を入れた）。
  "status.recoveryUnreadable": e("なおすためのファイルが読めませんでした ・ {opened}", "mock:F-Recover3", { opened: "最後に保存した曲をひらいています" }),
  "status.recoveryOpenedSaved": e("最後に保存した曲をひらいています", "mock:F-Recover3"),
  "status.recoveryOpenedNew": s6("なおすためのファイルが読めない", "新しい曲をひらいています", undefined, "part"),
  "msg.cpuOverload": s6("処理が追いつかない", "PC の処理が追いつかず、音がとぎれました。音の設定でバッファを大きくすると、とぎれにくくなります。"),
  "msg.exportPeak": s6("書き出しの前", "いちばん大きい所 {peak} dB ・ 音割れはありません", { peak: "−6.2" }),
  "msg.exportClipping": s6("書き出しで音が割れる", "いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。"),
  "msg.exported": s6("書き出しが終わった", "書き出しました: {path}", { path: "~/ミュージック/はじめての曲/はじめての曲.wav" }),
  "msg.scaleDoesNotFit": s6("画面に入らない拡大率", "この画面には入りません"),
  "msg.scaleKept": s6("入らない拡大率をキーでえらんだ", "{requested} % は、この画面には入りません。{kept} % のままです。", { requested: "150", kept: "125" }),
  "msg.scaleWindowGrown": s6("拡大率を変えて、窓が大きくなった", "拡大率を {scale} % にしました。窓を、いちばん小さい大きさまで広げました。", { scale: "125" }),
  "msg.scaleLowered": s6("画面が小さくなった", "画面が小さくなったので、拡大率を {from} % から {to} % にしました。", { from: "150", to: "125" }),
  "msg.bandFoldedWindowLow": s6("帯が自動でたたまれた", "窓が低くなったので、{band}をたたみました。帯札を押すと、またひらきます。", { band: "ミキサー" }),
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
  // 形は棟梁の決め（2026-11-15）:「{欄}は {範囲} です。もとの {値} のままです。」。欄と範囲は value.*。
  "msg.valueRejected": s6(
    "使えない値を打った",
    "{field}は {range} です。もとの {kept} のままです。",
    { field: "ベロシティ", range: "1〜127", kept: "77" },
    "part",
  ),
  "msg.backToArrange": s6("アレンジへ帰る道", "Esc ・ ひとつ戻る。何度か押すとアレンジへ"),

  // 使えない値を打ったときの欄の名前と範囲（仕様書 6 章「使えない値を打った」、3 章のテンポと拍子の枠）。
  "value.tempo": e("テンポ", "spec:6"),
  "value.tempoRange": s6("使えない値を打った", "30.00〜300.00", undefined, "part"),
  "value.meterNum": e("拍子の上", "spec:6"),
  "value.meterNumRange": s6("使えない値を打った", "1〜12", undefined, "part"),
  // 6 章の文は「下は」。欄の名前だけで読めるように「拍子の下」にした（棟梁の決め 2026-11-15）。
  "value.meterDen": e("拍子の下", "user:2026-11-15"),
  "value.meterDenRange": s6("使えない値を打った", "2 ・ 4 ・ 8 ・ 16", undefined, "part"),
  "value.velocity": e("ベロシティ", "spec:6"),
  "value.velocityRange": s6("使えない値を打った", "1〜127", undefined, "part"),

  // 仕様書 6 章のボタン（「（…）」の中は説明なので入れない）
  "button.playHere": e("ここから鳴らす", "spec:6"),
  "button.openAudioSettings": e("音の設定をひらく", "spec:6"),
  "button.details": e("くわしく", "spec:6"),
  "button.showStopped": e("止まった {count} 個を見る", "spec:6", { count: "4" }),
  "button.windows": e("窓", "spec:6"),
  "button.clearSearch": e("さがす枠を空にする", "spec:6"),
  "button.switchFormat": e("入れかえる", "spec:6"),
  "button.cancel": e("やめる", "spec:6"),
  "button.makeTrack": e("トラック {number} を作る", "spec:6", { number: "6" }),
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
  "transport.songSaved": e("保存済み ・ 取り消し {count}", "mock:Main", { count: "14" }),
  "transport.songChanged": e("取り消し {count}", "mock:F-ArrangeRec", { count: "12" }),
  // 左上の曲の名前を押すとひらく一覧の行。
  "song.new": s6("曲の名前の一覧", "新しい曲", undefined, "quoted"),
  "song.open": s6("曲の名前の一覧", "ひらく…", undefined, "quoted"),
  "song.save": s6("曲の名前の一覧", "保存", undefined, "quoted"),
  // 曲の名前が引っこんだ所の ▾（仕様書 2 章「曲の ▾」）。aria-label はモックの形。
  "transport.songMenu": e("曲（{name}）", "mock:F-Menus", { name: "はじめての曲" }),
  "transport.songMenuTip": e("曲 ・ {name}", "spec:2", { name: "（曲の名前）" }),
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
  "transport.countInBars": s6("カウントインの小節の数", "{bars} 小節", { bars: "1" }, "quoted"),
  // カウントインが「なし」のときのボタンの値と、一覧の行。
  "transport.countInNone": s6("カウントインの小節の数", "なし", undefined, "quoted"),
  // カウントインが引っこんだときの、メトロノームの ▾（仕様書 2 章「メトロノームの ▾」）。
  "transport.metronomeMenu": e("メトロノームとカウントイン", "mock:F-Menus"),
  "transport.cpu": e("処理の負荷", "mock:Main"),
  "transport.cpuValue": e("{percent} %", "mock:Main", { percent: "9" }),
  "transport.master": e("マスター", "mock:Main"),
  "common.db": e("{value} dB", "mock:Main", { value: "−4.8" }),
  "common.dbSilent": e("−∞ dB", "mock:Main"),

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
  "meter.playTargetWithInstrument": e("{track} ・ {instrument}", "mock:F-Connect3", { track: "1 ピアノ", instrument: "グランドピアノ" }),
  // カウントイン中と録音中。「● テイク {take}」は rec。
  "meter.playTargetRecording": e("{track} ● テイク {take}", "mock:F-Connect4", { track: "1 ピアノ", take: "1" }),

  // トラックと音源
  "track.label": e("{number} {name}", "mock:F-Connect3", { number: "1", name: "ピアノ" }),
  "instrument.placeholder": e("仮の音", "user:2026-11-14"),

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
  "arrange.addTrack": e("＋ トラックを追加", "mock:F-Empty"),
  "arrange.addTrackKinds": e("音源 ・ 歌", "mock:F-Empty"),
  "arrange.tempoAndSignature": e("{tempo} ・ {signature}", "mock:F-Empty", { tempo: "96.00", signature: "4/4" }),
  "arrange.emptyTitle": e("ここに、録った音がならびます。", "mock:F-Empty"),
  "arrange.emptyBody": e("鍵盤を弾いて音を確かめたら、赤い丸の録音ボタンへ。", "mock:F-Empty"),
  "arrange.emptyAddTrack": e("トラックを増やすには左下の「＋ トラックを追加」", "mock:F-Empty"),
  "arrange.overview": e("アレンジ 全体", "mock:A-PianoRoll"),
  "arrange.markerLoop": e("マーカー ・ ループ {range}", "mock:Main", { range: "5–9" }),
  "arrange.recordingClip": e("● 録音中 テイク {take}", "mock:F-ArrangeRec", { take: "2" }),
  "arrange.takeTag": e("{take}/{takes}", "mock:F-Takes", { take: "2", takes: "3" }),
  "arrange.takeSwitch": e("テイク {take} / {takes}", "mock:F-Takes", { take: "2", takes: "3" }),
  "arrange.takePrev": e("前のテイク", "mock:F-Takes"),
  "arrange.takeNext": e("次のテイク", "mock:F-Takes"),
  "arrange.takeKey": e("T", "mock:F-Takes"),
  // クリップのドラッグの「その場の読み値」（仕様書 2 章）。
  "arrange.readMove": e("{position}（{delta} 小節）", "mock:R3-Arrange", { position: "6.1.000", delta: "+1" }),
  "arrange.readCopy": e("複製 ・ {position}", "mock:R3-Arrange", { position: "9.1.000" }),
  "arrange.readTrim": e("長さ {length}", "mock:R3-Arrange", { length: "2.2.000" }),
  "arrange.soundCameOutKeys": e("録音 {recordKey} ・ 再生と停止 {playKey}", "mock:F-Connect3", { recordKey: "R", playKey: "Space" }),

  // 出口えらび（流れ 2）と、出口・入口の行
  "connect.changeLater": e("あとから「音の設定」で変えられます", "mock:F-Connect2"),
  "connect.help": e("↑ ↓ でえらぶ ・ ▶ でテスト音。何かささっている出口を、上にならべています", "mock:F-Connect2"),
  "endpoint.testTone": e("テスト音を鳴らす", "mock:F-Connect2"),
  "endpoint.plugged": s6("出口 ・ 入口の行の文", "ささっています", undefined, "quoted"),
  "endpoint.connected": s6("出口 ・ 入口の行の文", "つながっています", undefined, "quoted"),
  "endpoint.nothingPlugged": s6("出口 ・ 入口の行の文", "何もささっていません", undefined, "quoted"),
  "friendly.headphones": s6("出口 ・ 入口の行の文", "ヘッドホンの穴", undefined, "quoted"),
  "friendly.hdmi": s6("出口 ・ 入口の行の文", "画面のスピーカー", undefined, "quoted"),
  "friendly.lineOut": s6("出口 ・ 入口の行の文", "うしろのスピーカーの穴", undefined, "quoted"),
  "friendly.rearMic": s6("出口 ・ 入口の行の文", "うしろのマイクの穴", undefined, "quoted"),
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
  "band.stepCount": e("{undo} / {total}", "spec:3", { undo: "12", total: "14" }),
  "band.shelf": e("棚", "mock:F-ParamFew"),
  "band.shelfCount": e("音源 {instruments} ・ エフェクト {effects}", "mock:F-ParamFew", { instruments: "9", effects: "12" }),
  "band.params": e("パラメータ", "mock:F-ShelfArrange"),

  // 状態行
  "status.stopped": s6("状態行の札", "■ 停止中", undefined, "quoted"),
  "status.playing": s6("状態行の札", "▶ 再生中", undefined, "quoted"),
  "status.recording": s6("状態行の札", "● 録音中", undefined, "quoted"),
  "status.countingIn": s6("状態行の札", "● カウントイン中", undefined, "quoted"),
  // 止まっているときの左の文。ある物だけを「 ・ 」でつなぐ。両方あるときは status.loopNoSelection。
  "status.loopNoSelection": s6("止まっているときの状態行の左", "ループ {from}–{to} ・ 選択なし", { from: "5", to: "9" }, "quoted"),
  "status.loop": e("ループ {from}–{to}", "mock:Main", { from: "5", to: "9" }),
  "status.noSelection": e("選択なし", "mock:Main"),
  "status.output": e("出口: {output}", "mock:Main", { output: "ヘッドホン" }),
  "status.outputNone": e("出口: まだ", "spec:3"),
  "status.inputs": e("入口: {inputs}", "mock:Main", { inputs: "鍵盤 49 ・ マイク" }),
  "status.inputsNone": e("入口: まだ", "spec:3"),
  "status.buffer": e("バッファ {actual}", "mock:Main", { actual: "256" }),
  "status.bufferMismatch": e("バッファ {actual} ・ 頼んだ数 {requested}", "spec:3", { actual: "1024", requested: "256" }),
  "status.windows": e("窓 {count}", "spec:6", { count: "2" }),
  "status.scale": e("{scale} %", "mock:F-Zoom", { scale: "125" }),
  "zoom.heading": e("拡大率", "mock:F-Zoom"),
  "status.searchingKeyboard": e("鍵盤をさがしています…", "mock:F-Connect1"),
  "status.outputChosen": e("出口を{output}にしました。{track} が録音待機になっています。", "mock:F-Connect3", { output: "アナログヘッドフォン", track: "1 ピアノ" }),
  "status.newSong": e("新しい曲 ・ テンポ {tempo} ・ {signature} ・ {track} が録音待機", "mock:F-Empty", { tempo: "96.00", signature: "4/4", track: "1 ピアノ" }),
  // 録音待機のトラックがないとき（track が null）は、上の 2 つの文の前の部分だけ。
  "status.outputChosenOnly": e("出口を{output}にしました。", "mock:F-Connect3", { output: "アナログヘッドフォン" }),
  "status.newSongOnly": e("新しい曲 ・ テンポ {tempo} ・ {signature}", "mock:F-Empty", { tempo: "96.00", signature: "4/4" }),
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
  "settings.advancedSummary": e("{system} ・ {rate} Hz ・ バッファ {buffer}", "mock:F-Settings", { system: "PipeWire", rate: "48000", buffer: "256" }),
  "settings.advancedHint": e("ふだんは開かなくて大丈夫", "mock:F-Settings"),
  "settings.ok": e("これで決まり", "mock:F-Settings"),
  "settings.advancedOpenHint": e("ここから下は、機材にくわしい人むけ", "mock:F-SettingsAdvFixed"),
  "settings.system": e("音のしくみ", "mock:F-SettingsAdvFixed"),
  "settings.systemPipeWire": e("PipeWire", "mock:F-SettingsAdvFixed"),
  "settings.systemJack": e("JACK", "mock:F-SettingsAdvFixed"),
  "settings.systemAlsa": e("ALSA", "mock:F-SettingsAdvFixed"),
  "settings.outputDevice": e("出口のデバイス", "mock:F-SettingsAdvFixed"),
  "settings.midiInput": e("MIDI の入口", "mock:F-SettingsAdvFixed"),
  "settings.sampleRate": e("サンプルレート", "mock:F-SettingsAdvFixed"),
  "settings.hz": e("Hz", "mock:F-SettingsAdvFixed"),
  "settings.bufferRequested": e("バッファ ・ 頼む数", "mock:F-SettingsAdvFixed"),
  "settings.samples": e("サンプル", "mock:F-SettingsAdvFixed"),
  "settings.bufferActual": e("バッファ ・ 動いている数", "mock:F-SettingsAdvFixed"),
  "settings.bufferDiffers": e("サンプル ・ 頼んだ数とちがいます", "mock:F-SettingsAdvFixed"),
  "settings.bufferFixedTitle": e("バッファが PC 全体の設定で {actual} に固定されています", "mock:F-SettingsAdvFixed", { actual: "1024" }),
  "settings.latencyComp": e("遅れの補正", "mock:F-SettingsAdvFixed"),
  "settings.auto": e("自動", "mock:F-SettingsAdvFixed"),
  "settings.manual": e("手で決める", "mock:F-SettingsAdvFixed"),
  "settings.remeasure": e("測りなおす", "mock:F-SettingsAdvFixed"),

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
  "inspector.from": e("{position} 〜", "mock:A-PianoRoll", { position: "3.1.000" }),
  "inspector.mixed": e("ばらばら", "mock:A-PianoRoll"),
  "inspector.range": e("{low} – {high}", "mock:A-PianoRoll", { low: "77", high: "83" }),
  "inspector.quantize": e("クオンタイズ", "mock:A-PianoRoll"),
  "inspector.quantized": e("かけてある", "mock:A-PianoRoll"),
  "inspector.strength": e("強さ", "mock:A-PianoRoll"),
  "inspector.swing": e("スウィング", "mock:A-PianoRoll"),
  "inspector.percent": e("{value} %", "mock:A-PianoRoll", { value: "75" }),
  "inspector.align": e("そろえる所", "mock:A-PianoRoll"),
  "inspector.alignStart": e("はじまり", "mock:A-PianoRoll"),
  "inspector.alignStartAndEnd": e("はじまりと終わり", "user:2026-11-15"),
  "inspector.requantize": e("かけなおす", "mock:A-PianoRoll"),
  "inspector.unquantize": e("もとの位置へ戻す", "mock:A-PianoRoll"),
  "inspector.scale": e("スケール", "mock:A-PianoRoll"),
  "inspector.musicalScale": e("音階", "mock:A-PianoRoll"),
  "inspector.lock": e("固定", "mock:A-PianoRoll"),
  "inspector.on": e("オン", "mock:A-PianoRoll"),
  "inspector.off": e("オフ", "mock:Opt3-Metro"),
  "inspector.sameLength": e("長さをそろえる", "mock:A-PianoRoll"),
  "inspector.velocityAll": e("ベロシティをまとめて変える", "mock:A-PianoRoll"),
  "scale.name": e("{root} {mode}", "mock:A-PianoRoll", { root: "C", mode: "メジャー" }),
  "scale.major": e("メジャー", "mock:A-PianoRoll"),
  "scale.minor": e("マイナー", "user:2026-11-15"),
  "scale.kind": e("種類", "user:2026-11-15"),
  "scale.root": e("主音", "user:2026-11-15"),
  // 状態行。選んだノートは、数と最初の 4 つの音名。クオンタイズしたテイクなら、うしろに「 ・ クオンタイズ済み…」。
  "status.notesSelected": e("選択 {count} ノート ・ {keys}", "mock:A-PianoRoll", { count: "3", keys: "D4 E4 F4" }),
  "status.notesQuantized": e("クオンタイズ済み（もとの位置へ戻せます）", "mock:A-PianoRoll"),
  // ピアノロールをひらいて再生しているとき。テイクは arrange.takeSwitch。鳴っている音がなければ、その部分を出さない。
  "status.pianoRollClip": e("{track} ・ {clip}", "mock:F-PRPlay", { track: "1 ピアノ", clip: "Aメロ" }),
  "status.sounding": e("鳴っている音 {keys}", "mock:F-PRPlay", { keys: "C3 E3 G3 C4" }),
  "msg.outOfScale": s6("音階の外に書こうとした", "{key} は {scale}の外の音です（スケール固定 ・ オン）", { key: "D#", scale: "C メジャー" }),
  // ピアノロールの帯（第 3 回）: 道具の列、左右の欄の見出し、ベロシティと CC の札、その場の読み値。
  "pr.clip": e("{number} {track} ・ {clip} ・ 小節 {bars} ・ ノート {count}", "mock:F-PRPlay", { number: "1", track: "ピアノ", clip: "Aメロ", bars: "1–4", count: "23" }),
  "pr.clipSelected": e("{number} {track} ・ {clip} ・ 小節 {bars} ・ ノート {count}（選択 {selected}）", "mock:A-PianoRoll", {
    number: "1",
    track: "ピアノ",
    clip: "イントロ",
    bars: "1–4",
    count: "23",
    selected: "3",
  }),
  "pr.clipRecording": e("{number} {track} ・ {clip} ・ 録音中 ・ ノート {count}", "mock:F-PRRec", { number: "1", track: "ピアノ", clip: "Aメロ", count: "22" }),
  "pr.recordingDot": e("●", "mock:F-PRRec"),
  "pr.toolSelect": e("選ぶ", "mock:A-PianoRoll"),
  "pr.toolDraw": e("書く", "mock:A-PianoRoll"),
  "pr.toolErase": e("消す", "mock:A-PianoRoll"),
  "pr.grid": e("グリッド", "mock:A-PianoRoll"),
  "pr.scaleLock": e("スケール固定", "mock:A-PianoRoll"),
  "pr.noteList": e("ノートの一覧", "mock:A-PianoRoll"),
  "pr.selectedNotes": e("選んだノート", "mock:A-PianoRoll"),
  "pr.ccList": e("CC の一覧", "mock:A-PianoRoll"),
  "pr.velocity": e("ベロシティ", "mock:A-PianoRoll"),
  "pr.velocitySelected": e("選択 {values}", "mock:A-PianoRoll", { values: "80 83 77" }),
  "pr.velocityScale": e("目盛 0–127", "mock:A-PianoRoll"),
  "pr.sustain": e("サステイン", "mock:A-PianoRoll"),
  "pr.sustainDetail": e("CC 64 ・ 踏む {count}", "mock:A-PianoRoll", { count: "5" }),
  "pr.pitchBend": e("ピッチベンド", "mock:A-PianoRoll"),
  "pr.points": e("点 {count}", "mock:A-PianoRoll", { count: "6" }),
  "pr.modulation": e("モジュレーション", "mock:A-PianoRoll"),
  "pr.modulationDetail": e("CC 1 ・ 点 {count}", "mock:A-PianoRoll", { count: "0" }),
  "pr.otherCc": e("そのほかの CC", "mock:A-PianoRoll"),
  "pr.readDraw": e("{key} ・ {position} ・ 長さ {length}", "mock:R3-PRMouse", { key: "D3", position: "1.2.000", length: "0.360" }),
  "pr.readResize": e("長さ {from} → {to}", "mock:R3-PRMouse", { from: "1.000", to: "2.120" }),
  "pr.readMove": e("{ticks} ・ {keys} 半音", "mock:R3-PRMouse", { ticks: "+0.240", keys: "+2" }),
  "pr.readBox": e("{count} ノート", "mock:R3-PRMouse", { count: "3" }),
  "pr.readVelocity": e("{delta}（{count} ノート）", "mock:R3-Lanes", { delta: "+12", count: "3" }),
  "pr.readControl": e("{delta} ・ {position}", "mock:R3-Lanes", { delta: "+0512", position: "1.3.000" }),
  "pr.readSwitch": e("{state}（{value}）", "mock:R3-Lanes", { state: "踏む", value: "127" }),
  "pr.switchOn": e("踏む", "mock:R3-Lanes"),
  // 「そのほかの CC」の一覧の行と、えらんだ番号の帯の名前。
  "pr.ccNumber": e("CC {number}", "user:2026-11-15", { number: "7" }),
} as const satisfies Record<string, Entry>;

export type TextKey = keyof typeof texts;

export interface Term {
  readonly term: string;
  readonly description: string;
  readonly source: Source;
}

const st = (term: string, description: string): Term => ({ term, description, source: "spec:5" });

/** 仕様書 5 章「用語の説明」。形は「用語 — 説明」。 */
export const terms: readonly Term[] = [
  st("なおすためのファイル", "PC が急に終わったときのために、保存していない変更を急いで書いておくファイル。次に起動したとき、ここから戻せる。"),
  st("トラック", "楽器ひとつぶんの列。音源をひとつ持つ。"),
  st("クリップ", "トラックの上に置く、ひとかたまりの演奏や録音。"),
  st("アレンジ", "トラックとクリップを時間にそってならべる、曲の全体図。"),
  st("ピアノロール", "縦が音の高さ、横が時間の方眼。ノートを一つずつ直す場所。"),
  st("ミキサー", "各トラックの音量や響きをまとめて整える卓。"),
  st("ノート", "鍵を押してから離すまでの 1 音。位置、長さ、高さ、ベロシティを持つ。"),
  st("ベロシティ", "鍵盤を押した強さ。1 がいちばん弱く、127 がいちばん強い。"),
  st("MIDI", "音そのものではなく、「どの鍵を、いつ、どの強さで」という演奏の記録。あとから直せる。"),
  st("CC", "コントロールチェンジ。ペダルやホイールなど、鍵盤いがいの動きの記録。番号で種類が決まっている。"),
  st("サステイン", "足のペダル（CC 64）。踏んでいる間、音がのびる。"),
  st("ピッチベンド", "音の高さをなめらかに上下させるホイール。"),
  st("モジュレーション", "音にゆれを足すホイール（CC 1）。"),
  st("テンポ", "曲の速さ。1 分間の拍の数（BPM）。"),
  st("拍子", "1 小節に拍がいくつ入るか。4/4 は 4 拍。"),
  st("小節 . 拍 . ティック", "曲の中の位置。ティックは 1 拍を 480 に分けた細かい目盛り。"),
  st("メトロノーム", "拍に合わせて鳴るクリック音。"),
  st("カウントイン", "録音が始まる前に鳴る合図の拍。"),
  st("ループ", "決めた範囲をくり返し再生する。"),
  st("マーカー", "曲の中の場所につける名札。"),
  st("テイク", "同じ場所を録りなおした 1 回ぶん。前のテイクも消えずに残る。"),
  st("録音待機", "録音ボタンを押したとき、このトラックに録る、という印。"),
  st("グリッド", "ノートやクリップが吸いつく時間の目盛り。1/16 は 16 分音符。"),
  st("三連符", "1 拍を 3 つに分けるグリッド。"),
  st("クオンタイズ", "弾いたタイミングをグリッドにそろえる。強さで、どれだけ寄せるかを決める。"),
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
  st("戻り（リターン）", "センドで送った音が、エフェクトを通って帰ってくるチャンネル。"),
  st("オートメーション", "つまみの動きを時間にそって書いておく機能。"),
  st("プラグイン", "あとから足せる音源やエフェクト。中の画面は、作った人ごとに見た目がちがう。"),
  st("音源", "鍵盤を弾いたときに鳴る楽器の役。"),
  st("サンプル", "その音色で弾いた短い見本。棚の ▶ で聞ける。"),
  st("ためし弾き", "棚で音源をえらんでいる間だけ、鍵盤がその音色で鳴ること。トラックには入らない。"),
  st("お気に入り", "星の印をつけた音源。棚の「お気に入り」でまとめて出せる。"),
  st("エフェクト", "音に響きや変化を足す役。"),
  st("パラメータ", "プラグインが持っている、動かせる値のひとつひとつ。つまみやスイッチにあたる。"),
  st("プラグインの窓", "プラグインが自分で持っている画面。meloet の外に、別の窓としてひらく。"),
  st("形式の印が 2 つ", "同じプラグインが 2 つの形式で入っている。明るいほうを使う。もう一方の印を押すと入れかわる。"),
  st("形式の印", "プラグインの作られかたの種類（LV2 ・ VST3 ・ CLAP）。使いかたはどれも同じ。"),
  st("スキャン", "PC の中のプラグインをさがして、棚にならべる作業。"),
  st("フェード", "音の出だしや終わりを、なめらかに大きく、または小さくする。"),
  st("バッファ", "音をまとめて処理するひとかたまりの大きさ。小さいほど遅れが減り、PC は忙しくなる。"),
  st("遅れ（レイテンシ）", "弾いてから音が出るまでの時間。"),
  st("遅れの補正", "録った音を、遅れのぶんだけ前へ戻して置くこと。"),
  st("サンプルレート", "1 秒を何こまに分けて音を記録するか。48000 Hz がふつう。"),
  st("書き出し", "曲を 1 つの音のファイルにすること。"),
  { term: "仮の音", description: "本物のピアノの音が入るまでの、かわりの音。", source: "user:2026-11-14" },
];

/** 用語と用語の説明をつなぐ記号（仕様書 5 章「用語 — 説明」）。 */
export const TERM_SEPARATOR = " — ";

export function findTerm(term: string): Term | undefined {
  return terms.find((t) => t.term === term);
}

/** {name} の穴に値を入れる。値のない穴はそのままのこす。 */
export function fill(text: string, values: Readonly<Record<string, string | number>> = {}): string {
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in values ? String(values[name]) : whole));
}

export function t(key: TextKey, values?: Readonly<Record<string, string | number>>): string {
  return fill(texts[key].text, values);
}

/** 状態行の左の文の、部分どうしのつなぎ（仕様書 6 章「止まっているときの状態行の左」の「 ・ 」でつなぐ）。 */
export const STATUS_JOIN = " ・ ";
