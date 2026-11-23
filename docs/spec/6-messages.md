# 6 Error and hint wording

There are three rules for the wording. Say first what happened. If the song is safe, say that it is safe. Make the next thing the user can do into a button. Do not write 「エラー」「失敗」「不正」.

| Scene | Wording | Where shown | Button |
|---|---|---|---|
| Keyboard missing | 鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。 | Input meter | — |
| Keyboard found | 鍵盤 49 が見つかりました。鍵は届いています。音の出口をえらぶと鳴ります。 | Status line | — |
| No output yet | Right of the input meter: 「音の出口がまだです。下でえらんでください」. Above the arrange: 「音をどこから鳴らしますか」 and the list of outputs. | Right of the input meter, above the arrange | ここから鳴らす（Enter）/ a test sound on each row |
| Nothing plugged into the chosen output | （出口の名前）には、いま何もささっていません。ささると、ここから鳴ります。 | Status line | 音の設定をひらく |
| Buffer is fixed (status line) | バッファが PC 全体の設定で 1024 に固定されています ・ 音が出るまで 約 53 ms（256 なら 約 13.5 ms） | Status line. Show it when a song is opened and when the number changes. The right end always shows 「バッファ 1024 ・ 頼んだ数 256」 | くわしく (opens the deep part of the sound settings) |
| Buffer is fixed (deep in the sound settings) | バッファが PC 全体の設定で 1024 に固定されています。meloet は 256 を頼んでいますが、1024 で動いています。鍵を押してから音が出るまで 約 53 ms。256 で動けば 約 13.5 ms です。速いフレーズでは、音が指より遅れて聞こえます。録った音の位置は、遅れの補正で正しく置かれます。meloet は PC 全体の設定を変えません。変える場所は PipeWire の設定（quantum の固定）です。 | Deep in the sound settings, under 「バッファ ・ 動いている数」 | — (no button to fix it) |
| Sound came out | Two sentences: 「音が出ました。」 and 「赤い丸の録音ボタンを押すと、カウントインのあとに録りはじめます。」. Show the second sentence only when the record button can be used. | Above the arrange | — |
| Unusable output: the PC switches to it | さすと、PC がこちらに切りかえます | Right of the output row | — |
| Unusable output: stopped by the PC settings | PC の設定で、いまは使えません | Right of the output row | — |
| Text on the output and input rows | Whether something is plugged in: 「ささっています」, 「つながっています」 (for things that are connected rather than plugged in, like HDMI and USB), 「何もささっていません」. When it is not known, show nothing. Plain names in place of device names: 「ヘッドホンの穴」「うしろのスピーカーの穴」「画面のスピーカー」「うしろのマイクの穴」. On unusable rows, and on rows that have the same name as another row, show the device name in place of the plain name. | Right of the row, in the output chooser and the sound settings | — |
| Heading of unusable inputs | いまは使えない入口 2 (the reason texts are the same two as for outputs) | Sound settings | — |
| Recording in a loop | 1 ピアノ へ録音 ・ ループ 5–9 の 2 周目 ・ 周ごとにテイクを分けています | Status line | — |
| Take switched | 1 ピアノ ・ Aメロ をテイク 2 / 3 に切りかえました | Status line | — |
| Scale that does not fit the screen | この画面には入りません | The 「拡大率」 list in the status line. That scale is shown as 「使えない」 | — |
| Scale that does not fit, chosen with a key | 150 % は、この画面には入りません。125 % のままです。 | Status line. The scale is not changed | — |
| Scale changed and the window made larger | 拡大率を 125 % にしました。窓を、いちばん小さい大きさまで広げました。 | Status line | — |
| Screen became smaller | 画面が小さくなったので、拡大率を 150 % から 125 % にしました。 | Status line | — |
| Band folded automatically | 窓が低くなったので、ミキサーをたたみました。帯札を押すと、またひらきます。 | Status line | — |
| Opened a band that does not fit | ピアノロールをひらくために、ミキサーをたたみました。窓を高くすると、両方ひらけます。 | Status line | — |
| Heading of unusable outputs | いまは使えない出口 8 ・ PC が切りかえる物 1 ・ PC の設定で止まっている物 7 (in the sound settings: 「meloet は、PC 全体の設定を変えません」) | Output chooser, sound settings | — |
| Counting in | カウントイン中 ・ あと 2 拍で録音が始まります。止めるには Space | Status line | — |
| Recording | 1 ピアノ へ録音 ・ 止めると テイク 2 として残ります | Status line (when the piano roll is not open) | — |
| Recording, piano roll | 1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C3 E3 G3 C4 (when no key is held, leave out 「押している音」) | Status line (when the piano roll is open) | — |
| Playing, piano roll | 1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4 (when there is only one take, leave out 「テイク」; when no note is sounding, leave out 「鳴っている音」) | Status line | — |
| Notes selected | 選択 3 ノート ・ D4 E4 F4 (the same names are put together: 「選択 2 ノート ・ スネア ×2」. With 5 or more names: 「選択 7 ノート ・ C3 E3 G3 C4 …」. While keys are held, add 「・ 押している音 キック スネア」) | Status line | — |
| Piano roll where there is no clip | Two sentences: 「ここにはクリップがありません。」 and 「書く道具（2）で押すと、そこから 4 小節のクリップを作って書きます。鍵盤で弾いて録っても作られます。」 | Upper left of the note area | — |
| Set together | 3 ノートのベロシティを 100 にそろえました。 ／ 3 ノートの長さを 0.240 にそろえました。 | Status line | — |
| Clips overlap | 「ほかのクリップと重なるので、動かしませんでした。」「ほかのクリップと重なるので、貼りつけませんでした。」「ほかのクリップと重なるので、複製できませんでした。」 | Status line | — |
| Note outside the scale (reading) | When 「スケール固定」 is off, the reading is 「オフ」 | The 「選んだノート」 panel | — |
| Reading of an on/off CC | 「踏む（127）」「OFF（0）」 | CC band, CC list | — |
| No armed track | 録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。 | Status line (when the record button is pressed) | — |
| Keyboard unplugged | 鍵盤 49 とのつながりが切れました。曲はそのままです。さしなおすと、自動で戻ります。 | Input meter and status line | — |
| Output gone | ヘッドホンが見つからなくなりました。再生を止めています。 | Status line | 音の設定をひらく |
| Mic too loud | 入ってくる音が大きすぎて割れています。マイクから少し離れるか、マイクがわの音量を下げてください。 | Status line | — |
| Scanning | スキャン中 37 / 112 ・ 調べ終えた分から、もう使えます | Below the shelf | — |
| Scan finished | スキャンが終わりました ・ 667 個を棚に入れました。4 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。 | Status line, below the shelf | 止まった 4 個を見る |
| Plugin whose window cannot be opened yet | この子の窓は、まだひらけません。パラメータはここで動かせます。meloet が、この型の窓にまだ対応していないためです。音は、いつもどおり鳴ります。 | Top of the params view | — |
| Plugin that has no window | この子は窓を持ちません。パラメータはここで動かせます。 | Top of the params view | — |
| Windows are open | プラグインの窓が 2 つひらいています | Status line. 「窓 2」 at the right end | 窓 (opens the list) |
| Window takes time to open | （名前）の窓をひらいています… | Status line. Show it only when the window has not opened after 0.2 seconds | — |
| Window larger than the screen | （名前）の窓がひらいています（1597×995 ・ この画面より大きい窓です）。パラメータは右でも動かせます。 | Status line | — |
| Search finds nothing | 「（ことば）」に合うパラメータはありません。 | Parameter list | さがす枠を空にする |
| Before switching the format | この曲では VST3 版を使っています。LV2 版に入れかえると、いまのパラメータの値は引きつがれないことがあります。 | Dialog | 入れかえる / やめる |
| Auditioning | 鍵盤を弾くと、この音色で鳴ります。まだトラックには入っていません。 | Below the chosen row of the shelf, right of the input meter, status line | トラック（次の番号）を作る / やめる |
| Sample playing | サンプルを再生中 ・ （名前）。行をえらぶと、鍵盤でためし弾きできます。 | Status line | — |
| Instrument to audition is not loaded yet | （名前）を読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。 | Below the chosen row of the shelf | やめる |
| Plugin not found | 前はここにありました: （場所）。曲の中の設定は残しています。 | Below that row of the shelf | 場所をさがす / もう一度スキャン / 曲から外す |
| Plugin stopped | （名前）が止まりました ・ （トラック）。曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。 | Below the shelf, insert slot, status line | もう一度読みこむ / 外す |
| Plugin failed in the scan | （名前）は調べている途中で止まったので、棚には入れていません。ほかのプラグインは使えます。 | Below the shelf | もう一度ためす / このまま外しておく |
| PC sound system stopped | PC の音のしくみ（PipeWire）が止まりました。曲はそのままです。動きだしたら、自動でつなぎなおします。 | Status line | — |
| PC sound system stopped while recording | PC の音のしくみ（PipeWire）が止まったので、録音を止めました。そこまでの演奏は テイク 2 に残っています。 | Status line | — |
| Output gone while recording | ヘッドホンが見つからなくなったので、録音を止めました。そこまでの演奏は テイク 2 に残っています。 | Status line | 音の設定をひらく |
| Only the sound stream of meloet stopped | While not recording, reconnect up to 4 times without showing any text. If all 4 times fail, show the same text as "Output gone". While recording, the same text as "Output gone while recording". | Status line | 音の設定をひらく |
| Input from the keyboard stopped | 鍵盤からの入力が止まりました。曲はそのままです。いま、つなぎなおしています。 | Status line | — |
| Opened a third band | 取り消しの履歴をひらいたので、いちばん前にひらいたミキサーをたたみました。下の帯は、2 つまでひらけます。 | Status line | — |
| Band folded after raising the scale | 拡大率を 150 % にしたので、ミキサーをたたみました。帯札を押すと、またひらきます。 | Status line | — |
| Typed a value out of range | 「ベロシティは 1〜127 なので、127 にしました。」「拍子の下は 2 ・ 4 ・ 8 ・ 16 なので、4 にしました。」 (the same form for 「テンポ」 30.00〜300.00 and 「拍子の上」 1〜12) | Status line. The value is set to the nearer end of the range | — |
| Position rounded | Say only the part that was rounded. 「拍は 1〜4 なので、3. 4. 000 にしました。」「ティックは 0〜1919 なので、…」 (0〜0959 in 6/8) 「小節は 1 からなので、1. 1. 000 にしました。」. For the position of a note that went outside the clip: 「位置は 1. 1. 0000〜4. 4. 1919 なので、…」 | Status line | — |
| Not readable as a position | 位置は「5」「5.2」「5.2.240」の形で打ちます。もとの 1. 1. 000 のままです。 | Status line. The box stays 「こわれた」 | — |
| Not readable in the note field | 音は C-1〜G9 です。もとの C4 のままです。 | Status line. The box stays 「こわれた」 | — |
| Reading of the selected notes | 「位置」 「3.1.000 〜」 (when they differ, the earliest position and 「〜」), 「長さ」 「ばらばら」 (when they differ), 「ベロシティ」 「77 – 83」 (when they differ, the smallest and the largest value). Empty when nothing is selected | The 「選んだノート」 panel | — |
| Muted | 4 パッド はミュート (for 2 or more tracks: 「4 パッド ・ 5 ベース はミュート」) | Left of the status line while stopped | — |
| Not readable as a number | ベロシティは数字で打ちます。もとの 77 のままです。 (the same form for the other fields) | Status line. The box stays 「こわれた」 | — |
| Tried to write outside the scale | D# は C メジャーの外の音です（スケール固定 ・ オン） | Status line | — |
| Status line labels | 「■ 停止中」「▶ 再生中」「● 録音中」「● カウントイン中」 | Left end of the status line | — |
| Left of the status line while stopped | Join only the things that exist, with 「 ・ 」. Example: 「ループ 5–9 ・ 選択なし」 | Status line | — |
| Number of count-in bars | 「なし」「1 小節」「2 小節」 | Count-in list | — |
| Song name list | 「新しい曲」「ひらく…」「保存」 | The list that opens when the song name is pressed | — |
| Title of the PC file window | 「ひらく」 when opening, 「場所をえらぶ」 when choosing a folder | PC file window | — |
| Restored from the recovery file | Two sentences: 「PC が終わる前の演奏を戻しました。まだ保存していません。」 and 「11 月 15 日 22:41 に PC が終わったときの「新しい曲」です。テイク 2 つ。」. When meloet ended suddenly or the power failed: 「meloet が急に終わる前の演奏を戻しました。まだ保存していません。」 and 「11 月 15 日 22:41 の「新しい曲」です。テイク 2 つ。」. When there are 0 takes, leave out 「テイク N つ。」 | The hint above the arrange (the status line shows 「なおすためのファイルから戻しました ・ まだ保存していません」) | 保存する（Mod S）/ 戻さない |
| 「戻さない」 pressed | 戻した変更をすてて、最後に保存した形にしました。取り消せます（Mod Z） | Status line. The undo history shows 「戻した変更をすてる（最後に保存した形にする）」 | — |
| Recovery file cannot be read | Two sentences: 「前に保存していない変更がありましたが、なおすためのファイルが読めませんでした。」 and 「最後に保存した「はじめての曲」をひらいています。ファイルは消さずに残してあります。」. For a song that was never saved, the second sentence is 「新しい曲をひらいています。ファイルは消さずに残してあります。」 | The hint above the arrange (also in the status line) | ファイルの場所をひらく / 閉じる |
| Track added | 5 シンセ を足しました。 | Status line | — |
| Track deleted | 3 ドラム を消しました。取り消せます（Mod Z） | Status line | — |
| Auditioning to replace | 鍵盤を弾くと、この音色で鳴ります。再生すると、録ってあるノートもこの音色で鳴ります。まだ入れかえていません。 | Below the chosen row of the shelf (the status line shows 「エレクトリックピアノ ・ 1 ピアノ の音源は、まだグランドピアノです ・ 入れかえるには Enter」) | 入れかえる（Enter）/ やめる（Esc） |
| Instrument replaced | 1 ピアノ の音源を エレクトリックピアノ にしました。取り消せます（Mod Z） | Status line | — |
| Drums and 「スケール固定」 | ドラムには音階がないので、スケール固定は使えません。 | On hover over 「スケール固定」 | — |
| Cannot write outside the clip | ほかのクリップと重なるので、書けませんでした。 | Status line | — |
| Cannot move outside the clip | ほかのクリップと重なるので、動かしませんでした。 | Status line | — |
| Cannot copy outside the clip | ほかのクリップと重なるので、複製できませんでした。 | Status line (Alt ＋ drag, Mod D) | — |
| Cannot change the length | ほかのクリップと重なるので、長さを変えられませんでした。 | Status line | — |
| Take deleted | テイク 2 を消しました。テイク 1 / 2 にしました。取り消せます（Mod Z） | Status line | — |
| Dialog is open | 音の設定をひらいています ・ meloet について をひらいています | Left of the status line | — |
| Putting into an insert | 2 ベース のインサート 2 に入れます。 | Below the chosen row of the shelf | 入れる（Enter）/ やめる（Esc） |
| Sound pack not found (song opened) | 音の素材が見つからないので、4 本のトラックをサイン波で鳴らしています。 | Status line | — |
| Sound pack not found (shelf) | 音の素材が見つかりません。いまはサイン波だけ使えます。meloet を入れなおすと、もとに戻ります。 | Above the instrument list of the shelf | — |
| Dragging in an SF2 | ここに落とすと、棚に SF2 を足します。ファイルは meloet の中にうつします。 (below it: 「やめるには、窓の外で離すか Esc」) | The box in the middle of the window. The status line shows 「SF2 のファイルを持ってきています」 | — |
| Dragging in a file that is not SF2 | SF2 のファイルではないので、足せません。 | The box in the middle of the window | — |
| SF2 added | FluidR3_GM.sf2 を足しました。音色 128 ・ ドラムのキット 9 | Status line (the shelf opens, and the heading of that file opens. The numbers are those of that file) | — |
| SF2 cannot be read | FluidR3_GM.sf2 は読めませんでした。ファイルがこわれているか、SF2 ではないかもしれません。 | Status line | — |
| SF2 not found | 曲で使っている FluidR3_GM.sf2 が見つかりません。2 本のトラックはサイン波で鳴らしています。 | The SF2 place of the shelf (the notice of missing things), and the status line when a song is opened | SF2 を足す… / 曲から外す |
| Effect put in | 2 ベース のインサート 2 に リバーブ を入れました。 | Status line | — |
| Effect removed | 2 ベース のインサート 1 の コンプレッサー を外しました。取り消せます（Mod Z） | Status line | — |
| Inserts reordered | 2 ベース のインサートを並べかえました（EQ 4 バンド → コンプレッサー）。 | Status line | — |
| Replacing an insert | 2 ベース のインサート 1 の コンプレッサー を、EQ 4 バンド に入れかえます。いまの設定は残りません（取り消せます）。 | Below the chosen row of the shelf | 入れかえる（Enter）/ やめる（Esc） |
| Insert replaced | 2 ベース のインサート 1 を EQ 4 バンド に入れかえました。取り消せます（Mod Z） | Status line | — |
| Effect bypassed or brought back | 2 ベース の コンプレッサー をとばしました。 (when brought back: 「…をもどしました。」) | Status line | — |
| Parameter chosen (track, master) | 2 ベース ・ コンプレッサーの「しきい値」をえらんでいます | Status line | — |
| Typed value goes over the range | レシオは 1〜20 なので、20 : 1 にしました。 | Status line | — |
| Params panel with nothing chosen | ミキサーでインサートの枠をえらぶと、ここにパラメータが出ます。 | Params panel | — |
| No output | 出口なし ▸ (on hover: 「音の出口がありません。押すと音の設定をひらきます。」) | The output box of the master | — |
| New plugins found | 新しいプラグインが 3 個見つかりました。棚の「あとから入れたもの」に入れました。 | Status line (from the second start on) | — |
| List of plugins that stopped in the scan | この 4 個は、調べている途中で止まったので、棚に入れていません。meloet と曲は無事です。次のスキャンからは、とばします。 | The dialog 「スキャンで止まったプラグイン」 | もう一度ためす (on each row) / プラグインの場所… / 閉じる |
| Tried again and it got in | Granular Fog を、もう一度ためしました。今度は入りました。 | Status line | — |
| Tried again and it stopped again | Granular Fog は、また止まりました。棚には入れません。 | Status line | — |
| Changed in a plugin window (history) | Surge XT の窓で変えた所 | A row of the undo history (its place is 「5 Surge XT ・ プラグインの窓」) | — |
| Loading a plugin | Surge XT を読みこんでいます… | Below the chosen row of the shelf, status line | やめる（Esc） |
| Plugin cannot be loaded | Surge XT を読みこめませんでした。 | Below the chosen row of the shelf | — |
| Shelf open (headings) | 音色 31 ・ ピアノ ・ 鍵盤 と ギター をひらいています (with 3: 「ピアノ ・ 鍵盤、ギター と ベース」; with 5 or more: 「ピアノ ・ 鍵盤 ほか 4 つ」. When nothing is open: 「音色 31」) | Status line (when the shelf opens, and when a heading opens or folds. For effects: 「エフェクト 512 ・ リバーブ をひらいています」) | — |
| Dragging in a non-SF2 file | SF2 ではないファイルを持ってきています（足せません） | Status line | — |
| Dragging in many files | 3 つのファイル ・ SF2 は 2 つ | The box in the middle of the window | — |
| Many SF2 files added | SF2 を 2 つ足しました（FluidR3_GM.sf2 ほか 1 つ）。1 つは SF2 ではないので、足していません。 | Status line | — |
| Plugin not found (song opened) | 曲で使っている Surge XT が見つかりません。1 本のトラックはサイン波で鳴らしています。 | Status line | — |
| Plugin cannot be loaded (song opened) | 曲で使っている Surge XT を読みこめませんでした。1 本のトラックはサイン波で鳴らしています。 | Status line | — |
| Plugin cannot be loaded (shelf) | Surge XT を読みこめませんでした。こわれているか、この PC では動かないのかもしれません。 | Below the chosen row of the shelf (the box of the notice of missing things) | やめる（Esc） |
| Instrument plugin removed | 5 Surge XT の音源を外しました。サイン波で鳴らします。取り消せます（Mod Z） | Status line | — |
| Song dialog is open | 「保存していない変更」のダイアログをひらいています | Left of the status line | — |
| Mixer opened (with bypassed or stopped slots) | 4 パッド の MVerb をとばしています ・ 3 ドラム の Brick EQ は止まっています（音は、その枠をとばして流しています） | Status line (once, when it opens) | — |
| Plugin window opened | Surge XT の窓がひらいています (with 2 or more: 「プラグインの窓が 3 つひらいています」) | Status line (also when a window opens) | — |
| Plugin window cannot be shown | Surge XT の窓をひらけませんでした。パラメータの欄では動かせます。 | Status line | — |
| Undone | 「ノートを動かす」を取り消しました（3 ノート）。やりなおすには Mod Shift Z | Status line | — |
| Redone | 「ノートを動かす」をやりなおしました（3 ノート）。 | Status line | — |
| Went back in the history | 11 手目「マーカーを足す」まで戻しました（5 手）。 | Status line | — |
| No step to undo | 取り消せる手がありません。 | Status line | — |
| Writing automation | 1 ピアノ ・ 音量を書いています（● 書く ・ 止めるとオフに戻ります） | Status line | — |
| Automation put to rest | 4 パッド の音量を手で動かしたので、オートメーションを休ませました。「もどす」で線のとおりに戻ります | Status line | — |
| Automation brought back | 4 パッド の音量を、オートメーションのとおりに戻しました。 | Status line | — |
| Tempo ramp chosen | 5 小節目の変更をえらんでいます ・ 104.00 から 9 小節目の 120.00 へ、だんだん速く | Status line | — |
| Tempo at the song start chosen | 曲の頭のテンポをえらんでいます ・ 96.00 (for the time signature: 「曲の頭の拍子をえらんでいます ・ 4/4」) | Status line | — |
| Bands went away too | 2 ベース の コンプレッサー を外しました（そのパラメータの帯 2 本も消えました）。取り消せます（Mod Z） | Status line (removing, replacing, replacing the instrument) | — |
| History heading: no undone step | 14 手 ・ いまは 14 手目（いちばん新しい手） | Heading of the history band | — |
| History heading: everything undone | 14 手 ・ ぜんぶ取り消してあります ・ やりなおすには Mod Shift Z | Heading of the history band | — |
| History heading: no steps | まだ手がありません ・ 何かすると、ここに 1 手ずつならびます | Heading of the history band | — |
| No step to redo | やりなおせる手がありません。 | Status line | — |
| No parameters to move | グランドピアノには、動かせるパラメータがありません | Status line (the params panel also shows the same text and 「音源を入れかえる…」) | 音源を入れかえる… |
| Vocal track added | 6 歌 を足しました ・ 入口 1 から録ります ・ 歌のトラックの ● は、選んでいなくても自分でつけます | Status line | — |
| No input | 音の入口がありません。マイクをつなぐか、音の設定で入口をえらんでください。 | Status line (the input of a vocal track shows 「入口なし ▾」) | 音の設定… |
| Monitor turned on for the first time | 自分の声が少し遅れて聞こえます（いまは 13 ms）。スピーカーだと音がまわって「キーン」となるので、ヘッドホンで聞いてください。 | Status line | — |
| Mic too loud | マイクの音が大きすぎます（0 dB に届きました）。マイクを少しはなすか、PC の入口の音量を下げてください。 | Status line (the right end of the meter is rec) | — |
| Vocal tracks have no piano roll | 歌のトラックにはピアノロールがありません。波形はアレンジで直します。 | Status line | — |
| Recording vocals | ● 録音中 ・ 6 歌（入口 1）・ モニター オン ・ 止めるには Space | Status line | — |
| Recording keyboard and vocals | ● 録音中 ・ 1 ピアノ（鍵盤）と 6 歌（入口 1）に録っています ・ 止めるには Space | Status line | — |
| Vocals being recorded were kept | 録っていた歌 1 つを、止まる直前（9. 3. 0000 ごろ）まで残しました。 | The hint of the recovery file | — |
| Audio files not found | 曲で使っている音のファイルが 2 つ見つかりません。そのクリップは鳴りません。 | Status line (when a song is opened) | — |
| Audio file not found (panel) | 歌 テイク 1.wav が見つかりません。前はここにありました: はじめての曲/音/歌 テイク 1.wav | Params panel (the notice of missing things) | さがす… / 曲から外す |
| Moving the clip gain | 歌 テイク 1 のクリップの音量 −3.0 dB（ドラッグ中）・ 離すと決まります ・ やめるには Esc | Status line | — |
| Cut | 歌 テイク 1 を 7. 1. 0000 で切りました。 | Status line | — |
| Making the sound | 歌 テイク 1 の音を作っています 34 % ・ できるまでは、もとの音で鳴らします | Status line, below the clip | — |
| Audio file brought in | ドラム.wav を入れました（6 歌 ・ 5. 1. 0000）。 | Status line | — |
| Loading an audio file | 読みこんでいます 40 % | Below the clip | — |
| File that cannot be read | 「声.aac」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。 | Status line | — |
| Dragging in audio files | 音のファイルを 8 つ持ってきています ・ 左に落とすと別々のトラック、右に落とすと 1 本にならべる | Status line (the box in the middle is split into left and right) | — |
| Dragging in SF2 and audio files together | SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ | The box in the middle | — |
| Measuring the latency | 測っています 3 / 5 | The 「遅れを測る」 dialog | やめる（Esc）/ この数を使う |
| Latency cannot be measured | 音が小さくて測れませんでした。ヘッドホンの音量を上げて、もう一度測ってください。 | The 「遅れを測る」 dialog | もう一度 |
| Not running at the asked size | 頼んだ 128 ではなく、256 で動いています（PC の設定で決まっています）。meloet は PC 全体の設定は変えません。 | Sound settings ▸ advanced settings | — |
| Exporting | 書き出しています 46 % ・ はじめての曲.wav ・ 残り 約 40 秒 ・ 弾けます（書き出しの音は聞こえません） | Status line (while the time left is not known yet, 「残り 約 …」 is taken out: 「はじめての曲.wav ・ 弾けます（書き出しの音は聞こえません）」 after the percent. The time left is 「N 秒」 under one minute, at least 1, and 「N 分」, rounded, otherwise; user:2026-11-24) | やめる |
| Exporting in real time | 速く書き出せないプラグインがあるので、実時間で書き出します（3:02）。 | Status line (when the person turned on 「実時間で書き出す」: 「実時間で書き出しています（3:02）。」; user:2026-11-24) | やめる |
| Exported | はじめての曲.wav を書き出しました（3:02 ・ 48000 Hz ・ 24 ビット）。 | Status line | フォルダをひらく |
| Export stopped | 書き出しをやめました。途中までのファイルは消しました。 | Status line | — |
| Tempo at the song start (start of a ramp) chosen | 曲の頭のテンポをえらんでいます ・ 96.00 から 5 小節目の 120.00 へ、だんだん速く | Status line | — |
| Latency cannot be measured: spread | 測るたびに数が大きくちがったので、決められませんでした。まわりの音を小さくして、もう一度測ってください。 | The 「遅れを測る」 dialog | もう一度 |
| Latency cannot be measured: no input | 音の入口がないので測れません。マイクをつなぐか、入口をえらんでください。 | The 「遅れを測る」 dialog | 音の設定… |
| Mic too loud (how long) | マイクの音が大きすぎます（0 dB に届きました）。… | Status line (for 4 seconds after the last time it reached 0 dB, or until another notice comes. The rec at the right end of the meter: 2 seconds) | — |
| Input unplugged | 入口 1（USB マイク）が抜けました。録音を止めました。録れた所までは残しています。 | Status line | — |
| Sound broke up while recording | 録っている間に音が 3 回とぎれました（PC がいそがしかったため）。とぎれた所は無音で残しています。バッファを大きくすると、とぎれにくくなります。 | Status line (when the recording stops) | — |
| Recording 2 or more vocal tracks | ● 録音中 ・ 6 歌（入口 1）、7 歌 2（入口 2）・ モニター オン ・ 止めるには Space (when the monitor is off, take out 「モニター オン ・ 」) | Status line | — |
| Found by searching | 歌 テイク 1.wav を見つけました。2 つのクリップがまた鳴ります。 | Status line | — |
| Searched file does not match | えらんだファイルは使えません（サンプルレートかチャンネルの数がちがいます）。 | Status line | — |
| Broken file, 3 or more channels | 「声.wav」はこわれているので読めません。 ・ 「8ch.wav」は 3 チャンネル以上なので読めません（読めるのはモノとステレオ）。 | Status line | — |
| Copying the audio while saving | 保存しています ・ 音のファイルを写しています 3 / 12（80 / 240 MB） | Status line | — |
| Dragging in one audio file | ここに落とすと、そのトラックの落とした位置に入れます。 (when not over a vocal track: 「新しい歌のトラックに入れます。」) | The box in the middle (under the name). The status line shows 「音のファイルを持ってきています ・ 落とすと、枠の所に入れます」 | — |
| Moving a fade | 歌 テイク 1 の前のフェード 0. 1. 0480（ドラッグ中）・ 離すと決まります ・ やめるには Esc | Status line (the reading in place is 「0. 1. 0480」. For the fade at the end, 「後」 of mock F7-AudioEdit in place of 「前」: 「歌 テイク 1 の後のフェード …」; user:2026-11-24) | — |
| Disk too slow | 音のファイルを読むのが間に合わず、歌が少し切れました（ディスクがいそがしかったため）。 | Status line | — |
| Searched for the tempo of this sound | この音のテンポは 120.00 くらいです（さがした数を入れました）。 ・ When it is the same number: 「…くらいです（いまの数と同じです）。」 ・ When none is found: 「この音のテンポは見つかりませんでした。数を打ちこんでください。」 | Status line | — |
| Moving the pitch | 音の高さ +2 半音 +30 セント（ドラッグ中） | Status line (the reading in place is 「+2 ・ +30」) | — |
| Input plugged in but not used | ささっています ・ PC の設定でほかの入口をえらんでいます（設定 ▸ サウンド ▸ 入力 で変えられます） | The row in 「いまは使えない入口」 of the sound settings (text decided by the owner) | — |
| Recording while exporting | ● 録音中 ・ … ・ 書き出し 46 % | Status line | — |
| Fetching the sound pack | 音の素材を取りにいっています ・ 38 % ・ 92 / 240 MB ・ のこり 約 2 分 | The place decided in choice 44 | やめる |
| Sound pack fetched | 音の素材をぜんぶ取りました。31 の音色が鳴ります。 | Status line | — |
| Sound pack: not connected | 音の素材を取りにいけませんでした（インターネットにつながっていません）。つながったら「もう一度」を押してください。 | Status line, the place where it fetches | もう一度 |
| Sound pack: cut off on the way | とちゅうで切れました。92 / 240 MB まで取ってあります。続きから取ります。 | Status line, the place where it fetches | 続きから |
| Sound pack: disk full | ディスクがいっぱいで、音の素材を置けません（あと 150 MB 要ります）。 | Status line, the place where it fetches | もう一度 |
| Sound pack: broken | 取った音の素材がこわれていました。取りなおしても直りませんでした。 | Status line (after fetching again once without a word) | もう一度 |
| Sound pack: new pack | 音の素材の新しい包みがあります（12 MB）。 | Status line (once, at start) | 取りにいく |
| Marker added | マーカー 4 を足しました。名前を打って Enter | Status line | — |
| There are undone steps | 2 手ぶん取り消してあります。新しく何かすると、その 2 手はやりなおせなくなります。 | Status line | — |
| Quantize applied | クオンタイズ 1/16（75 %）を 3 ノートに。うすい枠が、そろえる前の位置。 | Status line | もとの位置へ戻す |
| Not saved | 「はじめての曲」に、まだ保存していない変更があります。 | Dialog (when closing) | 保存する / 保存しない / やめる |
| Cannot save | ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。べつの場所をえらんでください。曲は開いたままです。 | Dialog | 場所をえらぶ / やめる |
| Song file is broken | このファイルは読めませんでした。自動で残してある 1 つ前の状態からなら開けます。 | Dialog | 1 つ前を開く / やめる |
| Processing cannot keep up | PC の処理が追いつかず、音がとぎれました。音の設定でバッファを大きくすると、とぎれにくくなります。 | Status line | 音の設定をひらく |
| Before export | いちばん大きい所 −6.2 dB ・ 音割れはありません | The export dialog | — |
| Export would clip | いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。 | Status line, when clipping is found only after the export. In the export dialog the check row uses the 41-A texts of chapter 3 instead (user:2026-11-24) | — (status line) / このまま書き出す / やめる (the export dialog, with the 41-A texts) |
| Export finished | 書き出しました: \~/ミュージック/はじめての曲/はじめての曲.wav | Status line | フォルダをひらく |
| Way back to the arrange | Esc ・ ひとつ戻る。何度か押すとアレンジへ | Right end of the band tab row (when a band is open) | — |
