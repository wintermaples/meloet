# 7 Readings and decisions

This chapter is about the places where chapters 1–6 say nothing, and the places where their text can be read in more than one way. In each place the developers decided, and the screen works that way now. This chapter is part of the spec, the same as chapters 1–6. When it does not agree with chapters 1–6, chapters 1–6 are right (when you find such a place, fix this chapter).

The original records are in [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (in Japanese). The headings keep the numbers of that file (A-1, A-2, …), because comments in the code point to them by number. Rows that no longer apply were left out; the old file still has them.

In the "Basis" column: "spec 3" is chapter 3 of this spec; "mock Main" is a mock picture; "the owner's decision" was decided by the owner; "the developers' decision" and "the developers' reading" were decided by the developers where the spec and the mocks say nothing or can be read in two ways.

## A-1 Mock fixes (round 1)

| Where | Decision | Basis |
|---|---|---|
| F-Settings: device names of outputs that cannot be chosen | The real names that the PC gives (for example 「GB206 High Definition Audio Controller」), not the mock's 「内蔵オーディオ」「グラフィック 1」「グラフィック 2」. A name that does not fit is cut with 「…」, and hovering shows all of it | the owner's decision |
| F-Settings: columns of an output row that cannot be chosen | 168 and 150 (the mock has 160 and 96), the same as the output and input row | spec 2 |

## A-2 Reading the spec text

| Where | Reading | Basis |
|---|---|---|
| Example values inside the texts of spec 6 (「鍵盤 49」「ヘッドホン」「1024」「256」「53」「13.5」「1」「7」, and so on) | They are holes for the real values. The text around them is not changed by even one character. A test puts the example values of the spec into the holes and checks that the result is the spec text | the developers' reading |
| 「rec の 2px」 (the clipping mark of a meter) | A small size inside a part: multiplied by the scale and rounded (2 / 3 / 3) | spec 1 |
| 「最後に弾いた鍵盤」 (the keyboard played last) | Counted only when a key is pressed (NoteOn). Pedals and wheels do not change it. Before anything is played, it is the keyboard plugged in last | the developers' reading |
| 「同じ装置の中で PC が切りかえる穴」 and 「PC の設定で止まっている出口」 | If the output has a node and the PC is using another jack of the same device, it is the first kind. If it has no node, it is the second kind (even when another jack of the same device has a node) | the developers' reading |
| The right of the heading of outputs that cannot be chosen (「PC が切りかえる物 1 ・ PC の設定で止まっている物 7」) | The counts are the real numbers. A kind with none is shown as 0 | the developers' reading |
| How keys are written on the screen (「Mod Alt 1」 and so on) | The screen never shows 「Mod」. On Linux and Windows it shows 「Ctrl」 (「Ctrl Alt 1」「Ctrl +」「Ctrl −」「Ctrl Shift S」「Shift M」); on Mac it shows 「Cmd」 and 「Option」 (「Cmd Option 1」) | spec 4 (Mod, and the Mac column of the table) |

## A-3 Small details that the spec and the mocks do not have

| Where | Decision | Basis |
|---|---|---|
| Scroll bar | The shortest handle is as long as the bar is wide (8). The width 8 includes the 1px line on the left. Pressing outside the handle moves by one screen | the developers' decision |
| Output chooser that does not fit in the window | The space below it is also 24, the same as for dialogs. In a low window it moves up | the developers' decision |
| Folding of 「いまは使えない出口」 | Remembered only by the screen. Each time the place opens, it starts folded | the developers' decision |
| Place of the 「いまは使えない入口」 heading | In the sound settings, in 「歌の入口」, under the 「いまの大きさ」 row (the same order as the heading of the outputs). No text on its right | the developers' decision |
| Outputs with the same name | Compared only among the outputs that can be chosen. The device name goes in the friendly-name column. When the device name is not known, the friendly name stays | the developers' decision |
| Two or more keyboards | The rule that widens the name field for a name over 8 characters counts the whole 「名前 +N」. Hovering shows all the names joined with 「 ・ 」. 「入口:」 on the status line shows the name of one keyboard only | the developers' decision |
| Black keys with an even width (the thin keys at 125 % and 150 %) | They cannot sit on the middle of a line, so the extra 1 dot goes on the right | the developers' decision |
| A lower band that does not fit even alone | Its lowest height is its toolbar, 32 | the developers' decision |

## A-4 Round 2 (recording, transport, arrange)

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| Status line during the count-in (F-Connect4) | Spec 6: 「カウントイン中 ・ あと 2 拍で録音が始まります。止めるには Space」. The mock puts the left tag 「● カウントイン中」 before 「あと 2 拍で…」 | The left tag 「● カウントイン中」, then the spec text after 「カウントイン中 ・ 」. The two together make the spec text | the developers' reading |
| Record button with no armed track | Spec 2: disabled (no armed track). Spec 6: 「録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。」 when the record button is pressed | It has the disabled look, but pressing it shows the spec 6 text | the developers' reading (keeps both) |
| Status line of F-Connect3 and F-Empty | 「… {track} が録音待機…」 | When there is no armed track, only the part before it: 「出口を{output}にしました。」, 「新しい曲 ・ テンポ … ・ …」 | the owner's decision |
| 「マーカー ・ ループ 5–9」 in the heading (Main) | — | Only while the loop is on. When it is off, 「マーカー」 | the developers' decision |
| Line of the loop range | Main: 3px at the top of the ruler | `mock.arrange.loop` (3). The line.mark table of spec 1 has no loop | mock Main |
| Notes drawn inside a clip | Main: 2px high, 3px space above and below inside the clip | `mock.arrange.note`, `mock.arrange.clip-inset`, rounded at each scale | mock Main |
| Play position line | Spec 1: accent 「再生ヘッド」. F-Connect4 and F-ArrangeRec: rec | accent when stopped and playing; rec during the count-in and recording. The line is line.hair | spec and mocks |
| Count-in squares | Only a picture of one 4-beat bar | As many squares as the metronome clicks in one bar (4 in 4/4, 2 in 6/8, 5 in 5/8), counted again from the first square in each bar. 「あと N 拍」 on the status line is the whole number of clicks left | the developers' decision |
| Digit font | Spec 1: numbers, positions and values are IBM Plex Mono | The digits of 「テイク 2 / 3」「2/3」「ループ 5–9」 are Mono; 「テイク」 and 「/」 are the Japanese font | spec 1 |
| Count-in bars, recorded clip names, first save | — | When the count-in is 「なし」, the button value is 「なし」. A recorded clip gets the track name (「ピアノ」). The first save does not ask: it goes to 「はじめての曲/はじめての曲.meloet」 in the PC's Music folder (「はじめての曲 2」 if that name is already used) | the owner's decision |
| Loop 「5–9」 | The accent band on the ruler of Main | From the start of bar 5 to the start of bar 9 (the numbers of the bar lines) | the developers' reading of the mock |
| Behaviour around recording (where play and record start, the last half beat of the count-in, not playing the earlier takes of the track being recorded, a recording with no notes, take switching that goes round, actions not taken during the count-in and recording, and so on) | — | Chapters 10, 12 and 13 of [`docs/architecture.md`](../architecture.md) | the developers' decision |

## A-5 Round 3 (piano roll and clips)

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| Bar of quantize strength and swing | A-PianoRoll fills it with accent | Fill gray.9 (a value that moves smoothly in the parameter row of spec 2) | spec 2 |
| Horizontal zoom of the piano roll | A-PianoRoll and F-PR150: 40 px a beat. F-WidePR: 80 px | When it opens, the largest step at which the clip fits | the developers' decision (all 3 mocks fit) |
| Border of 「そろえる前の位置」 | Spec 2: gray.7 border only | Drawn only for notes whose position or length differs from the original note | so that the borders do not cover each other |
| Notes inside a trimmed clip | Spec 3: the trimmed part is kept inside a gray.7 border | Only the notes that start inside the range are shown and sound. Only while dragging, the kept part is shown in the gray.7 border | so that what sounds and what is seen are the same |
| Moving a clip before the start of the song | Spec 3: 「できない」 on a track where it cannot be placed | Not 「できない」 before the start; it stops at 0 | the same as moving notes in the piano roll |
| − ＋ on the toolbar of the piano roll | Spec 3: − ＋ on the toolbar, no picture | Before the grid (the same order as the heading row of the arrange in R3-Arrange) | mock R3-Arrange |
| A list that goes outside the window | — | Moved inside the window | the developers' decision |

## A-6 Round 3, part 0 (states of parts)

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| Hover and pressed of list rows and note list rows | Spec 2: hover = border gray.8, text gray.10; pressed = bg gray.2, border gray.8. A selected list row has a 2px accent on the left and background gray.4 | The border is drawn 1px inside the row on all 4 sides (the row height does not change). A selected row has only background gray.4 and the left line.mark, so the border and the background tell them apart. Hovering a selected row shows both | the developers' reading |
| Pressed and hover of band tabs, vertical headings, open band headings, CC tabs and list rows | The button rules of spec 2 | These parts have no border, so the border is drawn 1px inside. Hover of band tabs and vertical headings is text gray.10 only | the developers' reading |
| Keyboard focus where the box around cuts the edge | Spec 2: a 1px gray.10 line, 1px outside | For band tabs, vertical headings, panel headings, CC tabs, list rows and − ＋ of the arrange, the outside would be cut, so the line is drawn 1px inside | the developers' reading |
| Hover of the tempo and time signature numbers | Spec 2: hover = border gray.8 | A 1px border is laid around the digits, so the size and place of the reading do not change (the text is already gray.10) | the developers' reading |
| Keys of the strength and swing bars | Spec 4: 「パラメータの中: 値を動かす ・ 細かく動かす ← → ・ Shift ← →」, no amounts | Reached with Tab. ← → move by 10 %, Shift ← → by 1 %. Each press sends once | the parameter form of spec 4 |
| Hover and moving of the strength and swing bars | Spec 2 parameter row: hover, moving (row background gray.4, left 2px accent) | Hover: a gray.8 border inside the row and the label gray.10. Moving: row background gray.4 and an accent line.mark on the left | spec 2 |
| Place of the mark of the band that keys go to, in the arrange | Spec 2: for the arrange, its heading row | Only the top edge of the 「アレンジ − ＋ グリッド」 row in the left column. With the shrunk overview, the top edge of the 「アレンジ 全体」 panel. When the piano roll is not open, the mark is always on the arrange | the developers' reading |
| Color of the keys written on buttons | Spec 4: shown small at the right end; no color | Mono text.s, gray.8, the same as band tabs. It stays gray.8 when the button is on and when it is disabled (mock F-Zoom) | the developers' reading |
| Where term tips go | Spec 5: a term that appears for the first time | On a term inside a text, only on that word: 「ノート」 in 「ノートの一覧」 and 「選んだノート」, 「ノート」 in the band tab 「ノート 23」, 「テイク」 on the switch tag, 「CC」 in 「そのほかの CC」, 「アレンジ」 in 「アレンジ 全体」. 「トラック」 on the track name in the track head, 「dB」 on the master and track values, 「MIDI」 on the 「MIDI の入口」 label of the sound settings | the developers' reading |
| Place of the scale ▾ list | Spec 2: opens just below or above the thing pressed | Just below or above, on the side with more room, as high as fits (the content scrolls), the same as the scale list in the panel on the right | the panel on the right |

## A-7 Round 3, part 1 (piano roll)

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| Note height at each vertical zoom step | 「行 − 1」 | The row height − (size.note − part.note.height). At every step and every scale, that is the row height − 1 | the table of spec 1 and spec 2 |
| 「10 行」 for folding the CC bands | Spec 3 | Counted in rows of the current step (with row height 24, the bands fold more easily) | the developers' decision |
| Mark while the left end of a note is moved | 「右はしと同じ」 | At the original start, a short gray.7 line, the same as at the end | the right end |
| Pressing a note to sound it | 「ノートを押したとき」 | Only presses that select or move. Not the 6px ends, not Mod + press, not the erase tool | the developers' reading |
| Strength when a chord sounds while dragging or with ↑ ↓ | — | Dragging: the velocity of the note held. ↑ ↓: the largest velocity of the notes moved | the developers' reading |
| Loop mark on the piano roll ruler | Only the arrange ruler has one | The same accent band as the arrange, while the loop is on and while dragging | the arrange |
| Paste position from the right-click on an empty place | 「押した所」 | The nearest grid line (the same snapping as pressing the ruler) | the developers' reading |
| Right-click during recording and count-in | Spec 4 | Rows that change the song (cut, paste, duplicate, delete, quantize, back to the original position) are disabled. Copy and select all can be used | architecture 13 |
| Input box of the change-all row | Spec 2 「入力の枠」 | Border gray.6 normally, gray.8 on hover, gray.10 while typing, gray.5 with text gray.7 when disabled, rec when broken. The heading can be opened with nothing selected (the rows inside are disabled) | spec 2 |
| Wording of a rounded value | — | In the note fields, the form of the field (「3.4.0000」). When 2 or more parts were rounded, only one is reported, in the order clip range → bar → beat → tick | the developers' reading |
| What is read out of range | — | Velocity: negative numbers and decimals (rounded half up). Position: bar 0 and negative numbers, beat 0 and beats above the top number, ticks of one beat or more. Note names: C10, C-2 and so on. None of these is made 「こわれた」 | the developers' reading |

## A-8 Round 3, part 1 (arrange, transport, input meter)

| Where | Decision | Basis |
|---|---|---|
| 「見えている線」 of the arrange ruler | When bar numbers are thinned out, every numbered bar; usually every bar. When one beat is at least the smallest space between numbers, beats. Snaps to the nearest line | the developers' reading |
| Selecting with a box | It becomes a box after moving space.1 or more from the press. Clips touched even a little by the box are selected. Shift is read at the press | the box of the piano roll |
| Pressing one of many selected clips | At the press, all stay selected; when released without moving, only that one is selected | the developers' reading |
| Moving together | Only the clip held snaps to the grid; the others follow at the same distance. Rows move only by the row distance of the mouse. If any clip would go outside the tracks, 「できない」 | the developers' decision |
| 「貼りつけ」 from the right-click on an empty place | At the nearer bar start on the track pressed (with Shift, the tick pressed). Disabled below the tracks, when there is nothing copied, and while recording | the developers' decision |
| 「貼りつけ」 from the right-click on a clip | The same as Mod V (at the start of the bar of the play position, on the selected track) | the developers' reading |
| 「ピアノロールでひらく」 | Selects the clip and opens the piano roll if it is folded. If it is open, nothing more (E would fold it) | the developers' reading |
| Rows disabled while recording | Cut, paste, duplicate, delete, next take, previous take. Rows that can be used: open in the piano roll, copy, solo, record arm, mute | architecture 10 |
| Both the top and the bottom of the time signature rounded | Only the top is reported | the same as 「only the largest unit」 for positions |
| Look and place of 「固定」 | Background gray.1, border gray.6, text gray.8 (the same as ● in the arrange); smallest width the same as its height. On: border and text accent. When the input meter is full, to the right of the 96-wide reading and the bar; when slim, just to the right of the reading | spec 2 |
| Input meter with no keyboard | 「ベロシティ」 and 「固定」 are shown. The reading only while fixed is on (the fixed value). No bar. This differs from mock F-Connect1 | the developers' reading (velocity lock can be used with no keyboard) |
| ↑ ↓ in the field of the fixed value | By 1; with Shift, by 10. It does not go outside 1–127 | the tempo field |
| Position input box | 10 characters wide. Readable forms: bar[.beat[.tick]]. Spaces around the dots and full-width characters are fine | the developers' decision |

## A-9 Round 3, part 1 (menu bar and context menus)

| Where | Decision | Basis |
|---|---|---|
| Names in the menu bar | Text-only buttons (text.s, gray.9). space.1 inside at the left and right ends of the bar. Hover: text gray.10 | other text-only buttons, such as band tabs |
| Place of the list | Its left edge lines up with the name, just below the menu bar | spec 2 |
| Width of the list | 220. Only at 150 %, when a row does not fit its name and key (「名前をつけて保存… Ctrl Shift S」), wide enough to fit | the developers' reading |
| A row with ▸ | ▸ (gray.8) in the place of the key. While its children are open, it looks the same as hover | the hover of list rows |
| Place of the child list | To the right of the parent row (see A-10 for the gap and the top). If it does not fit in the window, to the left of the parent. If it goes out at the bottom, moved up | the developers' reading |
| An on mark in a disabled row | Mark border gray.5, dot inside gray.7 | spec 2 (disabled) |
| Stacking order | Lists (menu bar, context menu) are above dialogs and below term tips | the lists before |
| Space | When text is not being typed, both the press and the release of Space are stopped (a button with focus is not pressed) | the table of spec 4 |

## A-10 Version 5 of the design

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| Under the song name | Spec 3: 「未保存の変更 ・ 取り消し 1 / 1」, the same as in other scenes | 「保存済み ・ 取り消し N / M」 and 「未保存の変更 ・ 取り消し N / M」 (N is the steps that can be undone, M all steps, the same as the band tab). A new song with nothing done: 「新しい曲 ・ 取り消し 0 / 0」. Also during recording | spec 3; the new song is the owner's decision |
| 「ほかの CC はありません」 | Only in R5-Lists | Not shown. With no other CCs, the 「そのほかの CC」 button is disabled, so the list does not open | spec 3 |
| 「この画面には入りません」 in the scale child list of the menu | R5-Menus: shown on hover. The status line list (F-Zoom) always shows it | In the menu child list, shown inside the row only while hovered. The status line list always shows it | each mock |
| Space between the vertical − ＋ | A-PianoRoll and others: 2px (not a value of the space table) | The mock value 2 under the name `mock.pianoroll.row-zoom-gap` | mock |
| When the mute text is shown | Main is stopped with no notice | Only when stopped, and only when there is no notice or there is a summary such as 「選択なし」. Not shown during other notices or while moving | the developers' reading |
| Kind headings of the CC list | Spec 3: grouped by kind, each group folds by its heading | The order is the same as the CC tabs (sustain, pitch bend, modulation, then the others by number). All open at first. The folded state is kept when the band folds. Moving with keys skips folded rows | the developers' reading |
| Hover on the slim master | Spec 3: 「マスター ・ −6.2 dB」 on hover (no picture) | Shown with `use:tip`, the same as the song ▾. In the slim form, the term tip of 「dB」 on the value is removed | the song ▾ |
| Place of the CC list | In A-PianoRoll, a heading under the note list | When it opens, it shares the height half and half with the note list | the developers' reading |
| Value field of the CC list | Spec 3: on/off CCs are 0 or 127 | On/off CCs snap to 0 or 127 when the value is decided (snapping a value inside the range is not reported). ↑ ↓ go between 0 and 127 | the developers' reading |
| Mark of rows that do something | R5-Menus: an empty 12px place with no border | No border; only the place is kept. The band rows of 「表示」 show whether the band is open | R5-Menus, spec 3 |
| Inside the list, place of the child | R5-Menus | space.1 above and below inside the list. The child is right next to the parent, with no space.1, and its first row lines up with the parent row | R5-Menus |
| Width of the scale child list with 「この画面には入りません」 | R5-Menus: width 220, shown on hover | The width of the text is kept from the start, so when some scale does not fit, it is wider than 220 (about 311 at 100 %). The width does not change on hover | the developers' reading |

## A-11 Version 6 of the design (before round 4)

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| 「トラック（次の番号）を作る」 | The button row of spec 6. Mock F4-ShelfAudition: 「トラック 5 を作る」 | The form with a hole for the number, 「トラック {n} を作る」. The source is the mock | the developers' reading |
| Status line when ● of another track is pressed | R7-Arrange6: 「R を押すと 2 ベース に録ります」 (only in the mock) | The mock text as it is. Only when chosen from ● (not when the heading is pressed) | mock |
| Changing the selected track during recording and count-in | — | It does not change (so that where it records and where it sounds do not split). Pressing a clip does not move the track either | the developers' decision (the same idea as 「not taken during recording」) |
| After the selected track is deleted | — | The track just below; if it was the bottom one, the one above; if none, nothing | the developers' decision |
| M, S, ● on returns and the master | The mocks draw them on every strip. The spec gives no meaning | Only M of the returns can be used (it silences that return; saved in the song; one step). S and ● of the returns, and M, S, ● of the master are disabled | the owner's decision |
| Parameters of EQ, compressor, chorus, limiter | Spec 3 has only reverb and delay | The table in 19-3 of [`docs/architecture.md`](../architecture.md) | the owner's decision |
| Key of the mid tom in the built-in kits | Spec 3 gives only the piece names. GM has 2 keys for a mid tom, 47 and 48 | 47 (high 50, low 45) | the developers' decision |

## A-12 Round 4, part 1 (tracks and the shelf)

| Where | Decision | Basis |
|---|---|---|
| Track name input box | Decided with Enter and when pressing outside the box (when the focus leaves). Esc stops. Spaces at both ends are removed. An empty name or the same name is not sent. It does not open during recording and count-in | pressing outside is the owner's decision |
| Selecting by pressing a heading | Selected on release (moving more than space.1 is reordering, and does not select) | to keep it apart from the reorder drag |
| Reorder line | At the border nearest to the middle of the head being moved. Not shown at the borders around itself (where nothing moves). The line is only inside the head column. The lifted head moves only up and down (mock R6-Tracks is a little to the right) | the developers' reading |
| Mark of 「録音待機」 in the right-click menu | The right-click selects that track before opening, so it is on, in the form after selecting | spec 3 (right-clicking something not selected selects only it first) |
| Dividers of the 「トラック」 menu | 「ミュート」「ソロ」「録音待機」 / 「次のテイク」「前のテイク」「テイクを消す」 / 「鍵盤のトラックを足す…」 to 「下へ動かす」 / 「トラックを消す」 | the order of spec 3, with dividers added |
| Place of 「ヘルプ」 | Just to the right of 「トラック」 (not pushed to the right end of the window) | mock R7-Arrange6 |
| Keys going to the shelf | They go there when the shelf opens, when its mode changes, and when the shelf is pressed. They leave when another place is pressed. Not while a dialog is open. No mark (accent line) is drawn (「キーが向く帯の印」 of spec 2 is only for the arrange and the piano roll) | the developers' reading |
| P | If the shelf is folded, it opens as `newTrack` (the shelf for a new track). If it is open, the usual band rule (to the front if it is behind; folds if it is in front). The mode does not change | spec 3 |
| Esc | During audition, it stops the audition. The next Esc folds the shelf. If there is a dialog, the dialog comes first | spec 3 (Esc stops and goes back to the sound of the original track) and the Esc order |
| Space and F in the shelf | They act on the chosen row. If no row is chosen, nothing (Space does not start play either). Space on that row while its sample plays stops it | spec 4 |
| Search (built-in sounds) | Rows whose name or group name contains the text (「弦」 gives the 2 of 「パッド ・ 弦」). Upper and lower case letters are not told apart | the developers' reading |
| While loading | Under 「ためし弾き中」, only 「（名前）を読みこんでいます…」 and 「やめる」 | spec 6 |
| Left tag of the status line | Only when stopped and auditioning: 「♪ ためし弾き中」 in accent (mock F4-ShelfAudition). While playing, 「▶ 再生中」 stays | mock |
| Record button during audition | The mock keeps the red ring. Recording cannot be done during audition, so it is disabled | spec 3 (recording cannot be done) |
| 「ためし弾き ・ （名前）」 in the input meter | Mock F4-ShelfReplace breaks it into 2 lines. It stays 1 line; if it does not fit, 「…」 | spec 1 (it is never removed) |
| Instrument name in the heading during the audition of a replace | The name of the instrument being tried, in accent (「…」 if it does not fit) | mock F4-ShelfReplace |

## A-13 Round 4, part 2 (drum piano roll, 「meloet について」)

| Where | Decision | Basis |
|---|---|---|
| Brightness of the names in the drum key column | Rows with hits are gray.10; rows not in use are gray.9 (not darker) | mock R7-Drum, spec 3 (normal brightness) |
| Row names and note names | Hovering a row name shows the note name. On a term inside the name (「タム」 in 「ハイタム」), the term tip comes first | spec 3, spec 5 |
| First row height of the drums | Until the vertical − ＋ is pressed, it follows the height of the note area. After it is pressed, it is fixed | the developers' reading |
| Moving drum boxes up and down | Only when all the notes moved can move by the same number of rows and stay on keys of the kit. If a note on a key not in the kit is among them, they do not move vertically. ↑ ↓ move by one row; Shift ↑ ↓ do nothing | the developers' reading |
| Draw tool for drums | One grid cell. Moving right while held does not change the length | spec 3 |
| Where there is no clip | If the instrument of the selected track is a drum kit, the drum rows are shown | the developers' reading (drawing makes a clip on that track) |
| Typing 「音」 for drums | Both piece names and note names are read. The piece name is shown | the developers' reading |
| Third sentence of the F-Empty hint | 「トラックを増やすには左下の「＋ トラックを追加」」 is shown. It is not shown during recording and count-in | the hint rules of spec 3 |
| Who holds 「meloet について」 | Only the screen (a read-only dialog that touches neither the song nor the sound). Esc comes after menus and before the core's back. While another dialog is open, the menu bar is disabled | the developers' reading |
| Version | The version in `crates/meloet-app/Cargo.toml`. The mock's 「（例）」 is not shown | spec 3 (counted and put in at build time) |
| Version on a component row | The full version (1.0.228), because a crate with the same name can be in with 2 versions | the developers' reading |
| License headings | A plain OR is put in name order and joined with 「 または 」. AND, WITH and brackets stay in English. Headings with more components first. The mock's 「そのほか」 is not used; every expression gets its own heading | spec 3 (fold by license headings, with a count) |
| License names of the sound materials | CC-BY-x.y → 「CC BY x.y」, CC0-1.0 → 「CC0 1.0」, MIT as it is | mock |
| Font rows | Like component rows, pressing one opens the box with the full text (OFL) | OFL asks for the text to be shipped |
| Search box | Upper and lower case are not told apart. No text is shown when nothing matches (the spec has none) | the developers' reading |
| Each time the second page is entered | The search text, headings and rows go back to the first form (all folded) | the developers' reading, spec 3 |
| Counting components | Only what goes into the program (`cargo tree -p meloet-app -e normal,no-proc-macro`; for the screen, the npm packages in the vite bundle). The full license texts are made by `npm run build` into `ui/dist/licenses/texts.json` and read when the second page opens (not in the repository). The list and `NOTICE` are in the repository, and `licenses/check.mjs` checks that they are not old | `cargo metadata` alone also brings in about 60 components that are not in the program |

## A-14 Round 4, part 3 (mixer)

| Where | Decision | Basis |
|---|---|---|
| Where sends are taken | After the fader and after the pan (the sound heard is sent as it is; the effect of an instrument panned left also leans left) | spec 3 Y9 (after the fader); the pan is the developers' decision |
| Inserts of the master | Only the master has them after the fader (raising the fader does not go over the limiter's ceiling). Tracks and returns have them before the fader | spec 3 Y9 (the limiter stops clipping), the developers' decision |
| Pan of the master | Shown, and it works like the other strips | only in mock F4-Mixer (as in the mock) |
| Bottom of the fader | −∞. −60.0 dB can be kept by typing it | spec 1 (below −60 dB is 0 %, −∞) |
| 「間隔」 of the delay | One row. When 「テンポに合わせる」 is on, steps (1/8 and so on); when off, ms | the delay parameters of spec 3 |
| Status line 「…をえらんでいます」 | Also shown while playing. Not during recording and count-in | mock F4-Mixer (playing) |
| The mixer during recording | Fader, pan, send, parameters, mute (tracks and returns), adding and removing effects are taken during recording and count-in too. Undo and redo are not | the developers' decision (they do not touch the recorded position or passes; so that the monitor can be set while recording) |
| A track whose instrument is loading, when played | The same text as for audition, 「（名前）を読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。」. It goes away when loading ends | spec 6 text as it is; the owner's decision |
| The shelf during recording | Only the shelf for putting in an effect (from 「空き ＋」) opens during recording. The instrument shelf does not (audition does not go with recording) | the developers' decision |
| Pan law | Balance: in the middle, left and right are both 1.0; panning lowers only the other side (the middle sounds as before, and the left and right of a stereo sound are not mixed) | the developers' decision (`docs/architecture.md` 19-11) |
| Test sound of the sound settings | Added after the master strip (so that 「does sound come out」 can be checked even when the master is low) | the developers' decision |
| Strip sizes found only in the mock (inside 6, space between slots 2, scale label column 28, and so on) | Built from near names in the value tables, or with calc of names (no value is added) | the rule of `ui/src/design/` |
| Output slot of the master | The real output name, the same as the status line (the mock has 「ヘッドホン」) | spec 3 (outputs and inputs are shown with the real names) |
| Digits of parameter values | s: 2 decimals; dB, oct and 「: 1」: 1; ms, Hz and %: 0 (the ms of 「おさえるまで」 has 1, the Hz of 「ゆれの速さ」 has 2). Minus is 「−」; only ranges that also have minus values put 「+」 on plus values | the examples of spec 3, the table in architecture 19-3 |
| Scale of parameter bars | Hz is always logarithmic. Times are logarithmic only when the range is 50 times or more; other values are straight | the developers' reading (the knob positions in the mock are example pictures) |

## A-15 Version 8 of the design (before round 5)

| Where | Decision | Basis |
|---|---|---|
| Sources of 「エレキギター」, 「ひずんだギター」 and 「ブラシキット」 | 「エレキギター」 is GM 28 of FluidR3 (Electric Guitar (clean)), 「ひずんだギター」 is GM 31 (Distortion Guitar). The other sounds are the GM sounds of their names. 「ブラシキット」 is preset 40 (Brush) in bank 128 of the drums | the developers' decision (checked by the owner's ears) |
| Rows of 「ブラシキット」 | The same 12 sounds as 「スタジオキット」 and 「エレクトロキット」 (the pack has only those 12 keys). The piano roll rows are the same | the developers' decision (the built-in kits match) |

## A-16 Round 5, first part and part 0 (sounds, plugins, SF2)

| Where | Spec or mock | Decision | Basis |
|---|---|---|---|
| Text for bypassing and bringing back an effect | Spec 6 example: 「2 ベース の コンプレッサー をとばしました。」 (when brought back: 「…をもどしました。」) | `{target}をとばしました。` and `{target}をもどしました。` (target is 「2 ベース の コンプレッサー 」) | the developers' reading |
| Key 39 of 「ブラシキット」 | The row name is 「クラップ」 | It keeps the sound of the FluidR3 Brush kit (another brush snare sound) | the owner's ears |
| End of a scan with no stopped plugins | The spec 6 text is the form with stopped plugins (「…667 個を棚に入れました。4 個は…」) | Only the first half: 「スキャンが終わりました ・ 667 個を棚に入れました。」 | the developers' decision |
| Title of the PC window of 「場所を足す…」 | — | 「場所をえらぶ」, which already exists | the developers' decision |
| Name of a stopped plugin | F5-ScanStopped: 「Old Chorus」 | In this order: the name read during the scan, the name read in the last scan, the file name of the bundle | the developers' decision |
| Kind of a plugin put together in 1 row | Spec 3 14-A | The first kind that is not 「そのほか」 among the formats put together (Surge XT is 「そのほか」 in LV2 and 「シンセ」 in VST3, so 「シンセ」) | the developers' decision |
| One plugin in a bundle stops during the scan | Spec 3 Y13: things that stopped are skipped from the next scan on | Only the plugin that stopped is skipped. The rest of the same bundle goes in (the child process is started again to check the rest) | the developers' decision |
| Enter in 「スキャンで止まったプラグイン」 | — | 「閉じる」 (the bright button at the right end, the same as the rule of the save dialogs) | the developers' decision |
| Shelf when the sound pack is not found | Spec 3 85: 「サイン波」 only. R8-Missing | Only 「サイン波」 (the 4 computed sounds are not shown either) | spec 3 |
| Nearest group of an SF2 sound | Spec 3 83: 「ギター」 and 「民族楽器」 → 「ピアノ ・ 鍵盤」 and so on | For the families with no rule: 「ピアノ」, 「マレット」, 「オルガン」 → 「ピアノ ・ 鍵盤」; 「ベース」 → 「ベース」; 「弦」, 「アンサンブル」, 「シンセパッド」 → 「パッド ・ 弦」; 「シンセリード」, 「シンセの効果」 → 「シンセ」. The sends of a new track use the same group | the developers' reading |
| Group of a plugin instrument | — | Treated as 「シンセ」 (send A −15 dB; ▶ uses the phrase of the plugin's kind). Even when its kind is 「ドラム」, it does not get the drum piano roll (the key names are not known) | the developers' reading |
| The right of the 「あとから入れたもの」 row | Spec 3 puts both 「SF2 を足す…」 and 「もう一度スキャン」 there | The instrument list has 「SF2 を足す…」, the effect list has 「もう一度スキャン」 (2 do not fit in width 320; the form of F5-SF2Shelf and F5-ShelfFx) | mock |
| When nothing has been added later | — | At the bottom of the list, 「あとから入れたもの ・ 0 ［SF2 を足す…］」 | mock F5-SF2Drop |
| 「ミックスは、はじめ 30 %。」 | F5-ShelfFx (a plugin reverb) | Shown when a reverb or a delay (for plugins, those whose kind is 「リバーブ」 or 「ディレイ」) is put on a track or the master | the developers' reading |
| Buttons when a plugin stops | Spec 6: 「もう一度読みこむ / 外す」 below the shelf, in the insert slot and on the status line | A box with a red border below the shelf (the form of A-Mixer), and the right-click menu of the insert. The status line has only the text (it can hold only one button) | the developers' reading |

## A-17 Plugin windows, parameters and inserts (round 5, part 3, first pass)

| Where | Rule | Basis |
|---|---|---|
| 「（名前）」 in the spec 6 texts 「（名前）の窓をひらいています…」 and 「（名前）の窓がひらいています（1597×995 ・ この画面より大きい窓です）。…」 | 「（居場所）・（プラグインの名前）」, with a space after it (「1 ピアノ ・ Surge XT 」). For a plugin being auditioned on the shelf, only the name. The one-window text 「Surge XT の窓がひらいています」 uses only the name (spec 6). | mock F-ParamMany |
| The window count text while the 「窓」 list is open | While the 「窓」 list on the status line is open, the screen shows 「プラグインの窓が 2 つひらいています」 (or the one-window text) in place of the usual summary on the left, in the same form as 「ループ 5–9 ・ …」. | the owner's decision, mock F-WindowOpen |
| A window larger than the screen opens | Only 「…（1597×995 ・ この画面より大きい窓です）…」 is said, in place of the window count text of spec 3 (90). | the owner's decision, the developers' reading |
| The 0.2 seconds of 「（名前）の窓をひらいています…」 | Counted from when the intent to open the window is received. The text goes away when the window opens or when it cannot be opened. | the developers' reading |
| 「（名前）の窓はまだひらけません。パラメータは右で動かせます」 | Said on the status line when a plugin whose window cannot be opened yet is chosen (`PluginWindowNotYet`). A plugin that has no window gets no status line text. | mock F-ParamNoWindow |
| 「（名前）の（パラメータ）を動かしています」 | Shown by the screen while a row of the params panel is being dragged (the value during a drag does not come back from the core). | mock F-ParamFew |
| 「パラメータをさがしています ・「cutoff」・ 24 個」 | Shown by the screen while the search box has text (the screen does the search). | mock F-ParamSearch |
| Parameters in no group (「そのほか 1200」 in F-ParamMany) | A 「そのほか」 heading only when the plugin also gives named groups. A plugin that gives no groups gets one list with no heading. | spec 3, mock F-ParamMany |
| Nested groups (the unit tree of VST3) | Put together by the name at the top level. | mock F-ParamMany (one level only) |
| What the parameter list shows | Parameters that the plugin hides, and read-only ones, are not shown. | spec 3 |
| Reordering inserts (spec 3, 86-2) | The slots between the moved slot and the place it goes to each move by one (empty slots too), the same as reordering tracks. | the developers' reading |
| The brackets of 「インサートを並べかえました（EQ 4 バンド → コンプレッサー）」 | The filled slots of that strip after the reorder, from the top (empty slots are skipped). | the developers' reading |
| When the master shows 「出口なし ▸」 (spec 3, 86-8) | When no output is chosen, when the chosen output is gone, or when the name of the chosen output is not known. An output that only has nothing plugged in shows its name. | the developers' reading |
| 「窓にいた間」 of 15-C (spec 3, Y15) | While a plugin window has the keyboard input (also while a menu or dialog that the plugin opens itself is in front). A plugin being auditioned on the shelf is not in the song yet, so no step is made for it. | spec 3, the developers' reading |
| The plugin process stops while its window has the input | No step while it is stopped. When 「もう一度読みこむ」 brings it back with the values changed in the window, that becomes 1 step. | the developers' reading |
| 「（範囲）」 and 「（値）」 when a value typed for a plugin goes over the range | The texts that the plugin gives for the two ends, joined with 「〜」 (「0 %〜100 %」). The value is also the plugin's text. For a plugin that gives no text, a number from 0 to 1 with 2 decimals. | spec 6 (the form), the developers' reading |
| Another row is moved during a drag in the plugin panel | The earlier drag ends and its value goes back (the same as the preview of a built-in effect, which ends when the song changes). | the developers' reading |
| A parameter whose value is not known (「—」 in spec 3) | The bar is not filled and has no handle. Once it is moved, its value is shown from there. | the owner's decision |
| Closing a plugin window | meloet's screen has no part for it. A window is closed with its own × button. | the developers' decision (the comment on `ClosePluginWindow` in the protocol says the owner confirmed it, user:2026-11-19) |
| A band is folded when the mixer opens | The 「…たたみました」 text is shown, not `MixerPassesBy` (that a band went away is said first). | the developers' decision |
| The bypass of a new effect that replaced another (spec 3, 86-3) | It comes in not bypassed (the mark of the earlier slot is not kept; undo brings back the earlier effect with its mark). | the developers' decision |
| A plugin that is bypassed and also stopped | In the summary when the mixer opens, it is only in the 「止まっています」 part. | the developers' decision |
| The mixer summary and the window texts while a stopped notice stays | The summary when the mixer opens, and the window texts (「…この画面より大きい窓です」 too), are shown once above the stopped notice; the next action brings back the stopped notice. Reason: the summary names the stopped slots itself, and if it were under the notice, half of 「止まっています」 would never be seen; a window text answers what was just pressed. Other notices stay under the stopped notice. | the developers' decision |
| Text that is not a number, typed in the plugin panel (spec 3, 86-6) | For VST3, it is read only when it is the same as a text that the plugin shows itself (step names, On, Off). Note names for Surge XT (「c4」) are treated as not readable (JUCE plugins answer even to text they cannot read, so meloet cannot tell). | the developers' decision |

## A-18 The params panel and the mixer on the screen (round 5, part 3, second pass)

| Where | Rule | Basis |
|---|---|---|
| The first folding for more than 40 parameters | All named groups start folded; 「オートメーションに書いている」 is open. Read the open 「Macros」 in F-ParamMany as opened by hand. | spec 3 |
| Texts under the list | 「40 個をこえる子は、グループをたたんで始めます」 and 「さがす枠を空にすると、グループの一覧に戻ります Esc」 are shown as screen texts (the mock wording). Esc in the search box empties it when it has text (if not, the usual Esc order). | mock F-ParamMany, F-ParamSearch |
| 「とばす B」 in the panel heading | Shown for every insert, built-in effects too. Not shown for an instrument plugin (it cannot be bypassed). | spec 3 (86-1) |
| The button that brings the window to the front | It has the window mark only while the window is open. 「窓をひらく」 is text only. | mock F-ParamMany, F-ParamFew |
| The text of a plugin whose window cannot be opened yet | The spec 6 text is split at 「。」 into the first 2 sentences (gray.10) and the rest (gray.8), with a gray.7 line.mark on the left. The rest also wraps at the panel width. The text of a plugin with no window (2 sentences) uses the same box. | mock F-ParamNoWindow |
| The first text in the input box | For a plugin, the text of the current value (「35 %」). For a built-in effect, the number only (「20.0」), with the unit after the box (「: 1」 in R9-Params). | mock R9-Params |
| Enter on a stepped value | The list of steps opens at the lower right of the row, in the same form as a context menu (with choice marks). | the developers' reading |
| What can be typed | Only smooth values get the input box. Stepped values get the list; on/off values switch. Pressing a value twice works only for smooth values. | spec 3 (86-6) |
| The range in 「レシオは 1〜20 なので、20 : 1 にしました。」 | The range is numbers without units (「1〜20」). The value is written as in the row (「20.0 : 1」; digits as in A-14). The name is the name in the panel (for the ratio, 「おさえる強さ」). | A-14 |
| 「もとの（値）」 when typed text cannot be read | For a built-in effect, the number from the core (`"-24"`) written with the unit and digits of the row (「−24.0 dB」). For a plugin, the text as it came. | the developers' decision |
| A space after plugin names | On the status line, a name given by a plugin (a plugin name or a parameter name) is followed by a space: 「Surge XT の「Macro 1」」, 「Mix は 0 %〜100 % なので」, 「Surge XT が止まりました」. | mock F-ParamMany |
| 「とばす」 in the context menu of an insert | Under 「下へ動かす」, with a choice mark (marked while bypassed) and the key B. Under it, a divider and 「外す Delete」. | mock R9-Inserts |
| Pressing the chosen slot again | For a plugin, the slot is chosen again and its window comes to the front. For a built-in effect, nothing happens. | spec 3 |
| Hover over an insert name | A bypassed slot always shows 「名前（とばしている）」. Other slots show the name only when it is cut with 「…」. | spec 2, spec 3 (86-1) |
| Dragging to reorder inserts | The slot itself follows the pointer (the same as track heads). The drag starts after the pointer moves `space.1`. No gray.7 border at the original place (the same as reordering tracks). | the developers' reading |
| The key B | Works only when an insert is chosen and the mixer or the params panel can be seen. | the developers' reading |
| Enter on a fader, pan or send (spec 3, 86-7) | The value text becomes an input box. For dB it reads 「−3」, 「+2 dB」, 「−∞」; for pan, 「C」, 「L 20」, 「R20」, 「-20」. Text that cannot be read keeps the box open (no status line text, because the spec has none for it). | the developers' reading |
| Where the 「窓」 list opens | Its right end lines up with the right end of the 「窓」 button. | the developers' reading |
| The order on the right side of the status line | Output, input, windows, buffer, scale. | mock F-WindowOpen, spec 3 |
| The 30 Hz of the mixer meters | The left half and the right half of the meters are drawn on alternate frames (so every meter is 30 Hz). A frame in which the scroll or the size changed draws both at once. | spec 3, architecture 7 |
| Right-click on an insert slot | It does not choose the slot (no `SelectInsert`, because that would open a plugin window). The menu rows act on that slot by its `InsertRef`. No choice mark. Right-click on the instrument slot does not choose it either. | the developers' decision |
| The panel after undo | While the panel shows the same item in the same slot (the slot and `PluginView.key`), the search text, the open groups and the scroll stay, even if the row numbers change. Until the new table comes, the earlier table is shown. If a table with different groups (names and counts) comes, only the folding goes back to the start. | the developers' decision |
| The font of names in status line texts | The name holes (`{name}`, `{strip}`, `{track}`, `{effect}`, `{plugin}`, `{param}`, `{target}`, `{order}`) use the same font as the text around them. Number, value and key holes stay in the digit font. Older texts (「1 ピアノ へ録音」) follow the same rule. | mock F5-MixerBypass, F-ParamNoWindow, F-ParamFew |

## A-19 Version 9 of the design (before round 6)

| Where | Rule | Basis |
|---|---|---|
| 「1 ピアノ と 4 パッド の音量は、オートメーションで動いています」 (only in mock F6-MixerAuto) | The mock wording (`mock:F6-MixerAuto`). Said once when the mixer opens, only when a volume band is moving some track (not resting). If the summary of bypassed and stopped slots (spec 3, 90) is shown, that comes first and this text is not shown. | the developers' decision |
| Typing the tempo or time signature in the transport, in a song with changes | It changes the value of the point in effect at the play line (the start of the song if there is none). Inside a ramp, the point at the start of that ramp. The status line says which point was changed. | the developers' decision (spec 3: 92-11 to 92-16 stay) |
| Automation of returns A and B and the master | No bands (bands are only under tracks). The 「オートメーションに書く」 mark of return and master inserts has the disabled look. | the developers' decision |
| Removing or replacing an effect that has bands, replacing an instrument that has bands | The parameter bands of that slot (or instrument) go away in the same step (undo brings them back). A replaced effect gets a new `InsertId`, so its bands are not kept even for the same kind. Volume, pan, send and mute bands belong to the track and stay (architecture 21-5). | the developers' decision (spec 3, 92-19) |
| "The end of the song" for the last marker (spec 3, Y18) | The end of the last clip, rounded up to the start of a bar. If no clip is after the marker, 4 bars from the marker (the same length as turning on the loop when there is no range). | the developers' decision |
| Choosing a tempo point that is not a ramp, or a time signature flag | 「5 小節目の変更をえらんでいます ・ 120.00」, 「9 小節目の変更をえらんでいます ・ 3/4」 (after the first half of the spec 6 ramp text; `builder:2026-11-20`). | the developers' decision |

### The shelf, SF2 and missing things (round 6, part 0-2)

| Where | Rule | Basis |
|---|---|---|
| When the shelf heading text is shown (spec 6) | Said at the times in spec 6, and kept until the notice from the core changes. Also said when switching between the instrument list and the effect list. | the developers' reading |
| 4 open headings | 「A、B、C と D」. | the developers' reading |
| Many files dragged in, no SF2 | The name area is 「3 つのファイル」, the text is 「SF2 のファイルではないので、足せません。」, and the box is gray.8. | the developers' reading |
| Many files dragged in, 1 SF2 added | 「FluidR3_GM.sf2 を足しました。音色 128 ・ ドラムのキット 9 ・ 1 つは SF2 ではないので、足していません。」 | the developers' reading |
| Many files dragged in, an SF2 cannot be read | It is counted in 「足していません」. If none could be added, the first file that cannot be read is told with 「…読めませんでした」. | the developers' reading |
| The count in 「1 本のトラック」 | When the plugin is not found, the number of tracks that use it as their instrument. When it cannot be loaded, the number of seats that could not be loaded. | the developers' reading |
| An effect that could not be loaded | 「止」 (red border), as for a stopped plugin. Not told on the status line. | the developers' reading |
| The order of telling missing things when a song opens | Sound pack, then SF2, then plugins. Plugins are not told during a scan. | the developers' reading |
| A window could not be opened (spec 3, 91-1) | The params panel is not brought to the front by itself. | the developers' reading |
| Groups inside 「最初から入っているもの」 | They start folded. The groups inside are indented one step, the same as the SF2 families. | mock F5-PluginLoading, the developers' reading |
| After 「〜」 in the keyboard knob text (「MIDI CC 0\|0 〜 15\|129」) | The last name loses its front part, up to the last space of the first name. | spec 3 (the form) |

## A-20 Tempo and time signature changes, markers (round 6, part 1)

| Where | Rule | Basis |
|---|---|---|
| The height 72 of the open band | `size.row` + `size.lane` (24 + 48; the way the mock splits the rows). | mock F6-Tempo |
| A tempo point, 6×6 | The same shape as an automation point (normal gray.10, chosen accent, an accent 1px around a hovered point). | spec 2 |
| The vertical scale of the tempo | The highest tempo is `space.3` from the top, the lowest is `space.3` from the bottom. If all tempos are the same, the middle of the row, at 1.00 per 1px. | mock (96 to 120 is 24px) |
| The line color and the value text | The line is a gray.9 line.hair. The value is at the lower right of the point; the value of the chosen point is accent. | mock |
| The original place of a dragged tempo point | An accent border (the same as automation points). | spec 2 |
| The reading while a point or flag is dragged | Only the position 「9. 2. 0000」 (for tempo, the value text of the point moves with it). | the same as the marker reading |
| A chosen flag | Line accent, background gray.4. | after the marker tag |
| Pressing outside the name box | The box closes and the name does not change. | the developers' reading (the comment in the protocol) |
| 「96.00 ・ 4/4」 on the folded row | The values at the play line. | spec 3 |
| Horizontal zoom of the arrange | The px of one bar is counted with a bar of the time signature at the start of the song (a 3/4 bar is 3/4 of the width of a 4/4 bar). | positions are straight in ticks |

## A-21 Automation (round 6, part 2)

| Where | Rule | Basis |
|---|---|---|
| The top and bottom of a band | `space.1` is left empty above and below; the larger value is at the top. The range itself follows spec 3 (21-B). | the developers' reading |
| Snapping of added and moved points | The start of a beat. With Shift, ticks. | mock R10-AutoEdit (「+0. 1. 0000」) |
| The lower line of the band heading | Usually 「● 書く」 and 「いま 値 ・ 点 N」; while writing, 「書いています」; for a plugin band whose value cannot be shown, only 「点 N」. | mock |
| The 「オートメーション」 and 「休み もどす」 rows in the mixer | When any track has a volume band, every strip gets the row, so that the heights line up. The text is only for volume bands. Pan and send only get the accent border. | mock F6-MixerAuto, F6-MixerRest, the developers' reading |
| The resting value of a band of a built-in effect (「休み ・ （いまの値）」) | The value is shown only while the params panel shows that slot; otherwise only 「休み」. | the developers' reading |
| Delete | Works on the thing pressed and chosen last (clips, tempo points and flags, markers, automation points). | the developers' decision |
| 「→」 in the reading | No unit before 「→」, the unit after it. A plus dB value has no 「+」. | mock R10-AutoEdit, the fader numbers |
| The step when moving by hand puts a band to rest | The value and the rest go into one step such as 「音量を変える」 (the rest goes only into the song of that step). If the value does not change, it is 1 step 「オートメーションを休ませる」. | architecture 21-11 |
| Moving by hand a band with no points | It does not rest (the band moves nothing). It can be written with 「● 書く」. | the developers' reading |
| Values when a band is deleted | Volume, pan, send, mute and built-in effects stop at the value at the play line. A plugin keeps the value that the band sent last. | the developers' reading |

## A-22 The undo history band (round 6, part 3)

| Where | Rule | Basis |
|---|---|---|
| Hover over the row of the current step | No 「ここまで戻す」; it stays 「いまここ」. | the developers' reading |
| Hover over or choose the row of an undone step | 「ここまで戻す Enter」 in place of 「やりなおせます」. | mock R10-History |
| The chosen row | Dropped each time the band opens. When the band opens, it scrolls until the current step can be seen (after that, it does not scroll when steps are added). | the developers' reading |
| Esc while the band has focus | Folds the history band, even when the shelf is open. | spec 4 (read before the Esc order of spec 3) |
| When all steps are undone | ↑ chooses step 1. Home has no row to choose. From the band, you cannot go to 「0 手目」. | the developers' reading |

## A-23 Versions 10 and 11 of the design (before round 7)

| Where | Rule | Basis |
|---|---|---|
| 「▸ パン」「▸ プラグインのパラメータ」 in the band table of spec 3 and in the Y16 rule | Read as the 20-A row 「＋ 帯を足す ▾」, which is newer. | spec 3 (20-A) |
| The squares during the count-in | As many as the metronome clicks in one bar (6/8 has 2, 9/8 has 3, 12/8 has 4, 5/8 has 5, 4/4 has 4). The squares light with the clicks; with 2 clicks and 6 squares, some squares would never light. | the developers' decision (spec 3: 94-2 stays) |
| mp3 in export | Never shown (there is no mp3 encoder that is not LGPL, except one that is very new). Read mp3 in the format lists of F8-ExportCheck, F8-ExportClip and R12-Fit the same way. | the owner's decision (C-11) |

## A-24 Version 12 of the design (before round 8)

| Where | Rule | Basis |
|---|---|---|
| The heading of the version 12 rules in spec 3 says that choices 35–45 are not decided yet | The next line, "What the owner chose", is newer: 35 is 35-A, and the others follow the recommendation (36-B, 37-A, 38-B, 39-B, 40-A, 41-A, 42-C, 43-B, 44-A, 45-A). | spec 3 |
| The 44-B form named in 44-A | Read from the 44-B picture of Opt8-PackShow: on the left of the status line, 「音の素材を取りにいっています」, 「38 %」 and a bar, then 「やめる」. | mock Opt8-PackShow |
| Where the spec 6 scene "Fetching the sound pack" is shown (spec 6 still says "the place decided in choice 44") | The 44-A hint band. The one-line spec 6 text is the 2 lines of the band joined: 「音の素材を取りにいっています」 and 「38 % ・ 92 / 240 MB ・ のこり 約 2 分」. The button is 「やめる」. When it moves to the status line, the 44-B form above. | spec 3 (44-A) |
| Texts that are only in the mocks (used as written, with the source `mock:…`) | F8-PackAsk, status line: 「音の素材がまだありません ・ いまはサイン波で鳴ります」. F8-FirstLane, status line while the 「帯」 list of a track head is open: 「2 ベース にオートメーションの帯を足します ・ えらぶと、帯がひらきます」. F8-MultiClip, status line: 「選択 3 クリップ ・ 欄の「音量」に打つと、3 つともその音量になります」. F8-ExportCheck: 「 ・ たしかめています 38 %」 after the status line text 「「書き出し」のダイアログをひらいています」. F8-ExportClip: 「 ・ 0 dB をこえる所があります」 after the same text. F8-PackFail: the 2 lines of the hint band, 「音の素材を取りにいけませんでした」 and 「インターネットにつながっていません ・ 92 / 240 MB まで取ってあります」, with the buttons 「もう一度」「あとで」. R12-Fit: when the small text of the asking band does not fit (width 853), only 「取っている間も弾けます」. | the mocks |
| Examples in the F8 pictures | 「音源 31」, 「音源 202」, which rows are 「まだ」 or 「取ってきています」, and the place 「~/.local/share/meloet/音の素材」 in F8-PackSettings are examples. The real numbers and the real place are shown. The 31 names and the group counts on the shelf are the real ones. | the developers' reading |
| Sounds made by calculation | 「シンセベース」, 「エレクトロキット」, 「あたたかいパッド」, 「シンセリード」 and 「サイン波」 are made by calculation and always sound. They are not fetched and are never 「まだ」 or 「取ってきています」. The order in which the other sounds are fetched is in [open questions](open-questions.md) (B-95-1). | the developers' reading |
| Two texts for clipping in export | The check row of the dialog uses the 41-A texts of spec 3 (rec 「0 dB をこえる所が 3 か所あります（いちばん大きい所 +1.2 dB）。音が割れます。」 and gray.8 「マスターのフェーダーを下げるか、リミッターの上限を見てください。」). The spec 6 text 「いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。」 is used on the status line when clipping is found only after the export (41-A in Opt8-ExportCheck). How 「3 か所」 is counted is in [open questions](open-questions.md) (B-95-2). | the developers' decision |
| 「まだの音色」 on the shelf and Y2 | While the pack has not been fetched, or is being fetched, sounds that are not there yet are shown as 45-A rows. "Not shown on the shelf" in Y2 applies only when the fetched pack does not have that sound (for example, an old pack). | the developers' decision |
| 「（何を）」 of history rows whose right column shows the track name (spec 3, 94-6) | 「音源を入れかえる」 also loses the track name: 「グランドピアノ → オルガン」 (not 「1 ピアノ ・ グランドピアノ → オルガン」 of 92-3). Steps whose right column is 「アレンジ」 or 「ミキサー」 (color, volume, pan, send, mute, vocal input) keep the track name. The window row, where 「プラグインの窓」 shows twice, is in [open questions](open-questions.md) (B-95-3). | the developers' decision |
| The bottom end of the clip gain line (spec 3, 39-B) | The range stays −60 to +12 dB (93-4). The line does not go below the −60 place of the fader scale; 「−∞」 is only the name of the bottom of the scale. | the developers' decision |
| Numbers in "the developers' provisional decisions stay" (spec 3) that also got a text in spec 6 or 41-A (94-27, 36, 44, 46, 51, 53, 75) | The behavior stays; the text is the one in spec 6 or 41-A. | the developers' reading |
| The slim input meter split into left and right (the spec 3 example, and R12-Meters, have no track name and no monitor) | The current form, with the track name, stays (spec 3 says that the developers' form stays). | the developers' reading |

## A-25 Pasting notes (round 10)

| Where | Rule | Basis |
|---|---|---|
| Pasted notes that go outside the clip | They are kept, and the clip grows in whole bars, the same as writing outside the clip (spec 3, 82). | the developers' reading (the owner's report: notes could not be pasted into a clip of another track) |
| Mod V and 「編集 ▸ 貼りつけ」 | Paste what was copied or cut last. If that was notes and the piano roll is open with a clip shown, the notes go into that clip even when the keys point at the arrange. If it was clips, clips are pasted as before. The 「貼りつけ」 of a context menu pastes the kind of the place that was pressed. | the developers' reading |
| Where Mod V puts notes | At the play line (snapped to the grid) when the play line is inside the shown clip; otherwise at the start of that clip. | the developers' reading |
