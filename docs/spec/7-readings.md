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
| Automation of returns A and B and the master | No bands (bands are only under tracks). The 「オートメーションに書く」 mark of return and master inserts has the disabled look. Changed in round 12 (M5, A-29): the master has one lane, of its volume; the returns, the master's pan and the master's inserts still have none. | the developers' decision; round 12: the owner's request (user:2026-11-23) |
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
| mp3 in export | Not shown. Round 12 F2 built mp3 with `rusty_mp3` 1.2.2 on the owner's condition (2026-11-23: offered only if it meets the acceptance conditions of F2). It did not: at 192 kbps pink noise comes back 14.4 dB (44100 Hz) and 13.4 dB (48000 Hz) below the source, where the rule asks 20 dB (LAME 3.101 gives 13.7 and 12.8 dB on the same signal, so the rule itself may be too strict). The mp3 work is kept on the side branch `fea/round12-f2-mp3`; the owner decides (open question B-98-1). The song's information of F2 is offered for wav and flac. Read mp3 in the format lists of F8-ExportCheck, F8-ExportClip and R12-Fit the same way. (Before round 12: never shown, because there was no mp3 encoder that is not LGPL, except one that was very new.) | the owner's decision (C-11) and user:2026-11-23 |

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
| Pasted notes that go outside the clip | Now in the body (spec 3, 82; the owner's decision, user:2026-11-23): the clip grows in whole bars. | the owner's decision |
| Mod V and 「編集 ▸ 貼りつけ」 | Paste what was copied or cut last. If that was notes and the piano roll is open with a clip shown, the notes go into that clip even when the keys point at the arrange. If it was clips, clips are pasted as before. The 「貼りつけ」 of a context menu pastes the kind of the place that was pressed. | the developers' reading |
| Where Mod V puts notes | At the play line (snapped to the grid) when the play line is inside the shown clip; otherwise at the start of that clip. | the developers' reading |

## A-26 Keyboard buttons and the fader (round 10)

| Where | Rule | Basis |
|---|---|---|
| CC 123 and CC 121 while recording | The messages themselves are not recorded, but the key-ups, pitch bend centre, modulation 0 and sustain up that they cause go the same way as played keys, so a take never keeps a note or the pedal held down. | the developers' reading |
| The fader after the volume changed somewhere else | When the selected track's volume is changed on the screen or by undo, the fader waits again until it passes the new volume. | the developers' reading |
| The fader when it moves on another track before resting | The step of the earlier track is made first. | the developers' reading |
| Two keyboards with faders | Their fader values are not kept apart; picking up is shared. | the developers' provisional form |
| Picking up on a track with automation | The fader is compared with the volume set by hand, not with the value the automation plays. | the developers' provisional form |
| The second and later ports of a device | Every port after the lowest one is a button port, also on a sound card with two MIDI inputs: notes 86–100 there work as buttons, other notes are not used (before round 10 those ports were not read at all). | the developers' reading |

## A-27 Details of the round 10 mocks (round 10)

| Where | Rule | Basis |
|---|---|---|
| 「重ねる」 while recording | Stays accent (the ● is rec), as in P10-Overdub. No key and no row in the menu bar for now. When the transport is narrow, the other folding parts fold first and 「重ねる」 hides last (`transportWithOverdub` in `ui/src/components/fit.ts`). | the developers' decision |
| Overdub where no clip is | Recording starts a new clip, as without 「重ねる」. The take tag of the piano roll does not change. | the developers' decision |
| The chord stamp | The shadow before pressing is a 1px accent border. The list shows the notes of each shape with C as the root. Shape names are the usual symbols (7, M7, m7, sus4, dim, aug). Changing to another tool keeps the shape. No key for now. | the developers' decision |
| The chord table | Pressing a chord and then a row with the draw tool puts the chord with its root on the nearest row of that note name. Degrees (Ⅰ, Ⅱm …) are not shown. In a minor key the list starts from the tonic (Am, Bdim, C, Dm, Em, F, G). The table chord and the stamp shape share one choice: the last one chosen. A chord sounds while it is pressed. | the developers' decision |
| The clip name box | It fills the name strip and grows to the right with a long name (spec 1 has no name for the 120 of the mock). Pressing outside decides, the same as the track name box. In the params panel, the 「選んだクリップ」 label is hidden while typing. | the developers' decision (P10-ClipName) |
| Joining with another clip between | When a clip that is not chosen lies between the chosen ones, nothing is joined, and the rows 「つなげる」 and 「クリップをつなげる」 are disabled. | the developers' decision |
| Notes hidden by a shortened clip, when joining | Only the notes inside each clip's range are joined; a note that goes past a clip's end is cut there. | the developers' decision |
| 「AI とつなぐ」 | No status line text and no 「切る」 button: turning it off closes the port and cuts the AI. How to set up the AI app (starting `meloet-mcp`) is told in the user guide, not on this screen. Remembered as a meloet setting, not in the song. | the developers' decision |
| Chord help on a drum track | The ▾ of the draw tool and the chord table are disabled in the drum piano roll. | the developers' decision |
| Overdub in loop recording | Every pass that has notes is one undoable step 「録音」, so its notes sound from the next pass on, and undo takes away the last pass. | the developers' decision |
| Lines of the CC lanes (themes) | The CC lanes use the same line names as the piano roll (bar `color.line.bar`, beat `color.line.beat`, fine `color.line.grid`), one step stronger than the mock R3-Lanes, so lines read the same in every lane and theme. | the developers' decision (the owner chose line-only names, 2026-11-23) |
| Off borders of M / S / ● (themes) | Spec 2 says "normal border gray.6", spec 1's Contrast table puts the M, S, ● toggles on gray.5. The toggles keep gray.5, so accent and rec stay 3:1 against the off border after gray.6 got stronger in the dark themes. | the developers' reading |
| Lines under track rows (themes) | `color.line.thin`, not `color.line.row`: they were gray.5, and line.row would make them weaker than before. | the developers' decision |

## A-28 The AI link (round 11)

| Where | Rule | Basis |
|---|---|---|
| Key names of an LV2 plugin (midnam) | Read when the plugin is loaded and when its state is restored, never at other times. The midnam update feature is not offered, so a plugin that renames its keys while it runs keeps the old names until its state is restored again. | the developers' decision (architecture 24-16 E) |
| Measuring a muted track with `ignore_mute` | Only the mute button is ignored, for the track measured alone; a moving mute lane is still followed, and the mix keeps the song's mutes. The song does not change. | the developers' decision (architecture 24-16 D) |
| A recalled mute set and a moving mute lane | When a recall changes the mute of a track that has a moving mute lane, the lane is rested in the same step, as pressing M does; tracks whose mute does not change keep their lane as it is. | the developers' decision (architecture 24-16 F) |
| Solo in a mute set | A set remembers solo and a recall sets it, but solo is not part of the step (as solo is never a step): undo does not bring the old solo back. | the developers' decision (architecture 24-16 F) |

## A-29 Round 12 (the owner's request of 2026-11-23)

The rules are in spec 3, "Rules added in round 12"; how each is built is architecture chapter 25 (25-4 to 25-35). Below: what the developers decided where the request said nothing, what they changed from it and why, and every text they made up. Wording with the source `builder:<date>` waits for the owner's approval (open question B-98-28); everything else new is `user:2026-11-23` from the request. In "Basis", "the lead" is the developers' lead.

### How the round was run

| Where | Rule | Basis |
|---|---|---|
| Protocol versions | The request planned one bump per wave (31, 32, 33, 34). Waves 3 and 4 share 33 (the shapes of both were made in one pass), so the features of the round end at protocol 33; the owner's answers after them made it 34 (A-30). | the developers' decision (the lead, 2026-11-24) |
| Documents during the round | Each feature helper wrote a notes file (`docs/round12/<ID>.md`) instead of editing the shared documents; one document helper merged them at the end (this table, spec 3, architecture 25, progress, the READMEs). The notes files were then deleted (the history has them). | the developers' decision (the lead) |
| Measurements | Speed and latency were measured once, after the last merge, on a quiet PC, not per feature (helpers built in parallel and the PC was not quiet). Results: `docs/measurements/2026-11-24-round12.md`. | the developers' decision (the lead, 2026-11-24) |
| A new control in a full row (P2, R1; also R2, E2, E7) | A control added in this round never pushes out a part the spec and the mocks already show: it takes only room that is left, and it is the first thing to hide. So at 1280 px wide the label of 「ほかのトラックも見る」 and the button 「さっきのを残す」 are hidden (the feature stays reachable: the ▾ list, Shift R and the menu). Pictures at 1280 × 800 stay as before round 12. Question: B-98-3, B-98-4. | the developers' decision (the lead, 2026-11-24) |
| `ebur128` and FTZ (F1, M3) | Accepted: the crate sets FTZ only inside its own call, on threads that are not the audio thread (the export job, the analysis thread), and restores it; architecture 19-2 has the exception. | the developers' decision (the lead, 2026-11-24) |
| The line for the release skill (D2) | The D2 helper was refused permission to edit `.claude/skills/bump-version/SKILL.md`, so its line was not added. For the owner to add, before step 8: "If the owner asked for the AppImage too (a trial, `docs/development/appimage.md`): `MELOET_DOCKER_SG=1 scripts/build-appimage.sh` makes `target/appimage/meloet_X.Y.Z_amd64.AppImage`. The script changes its name in the docs together with the deb's." (`scripts/bump-version.sh` already changes the AppImage's name.) | the developers' note |
| Order of the new rows of 「表示」 | E1 and E7 each put their row last; merged, the menu ends 「テーマ ▸」, 「鳴っている音を光らせる」, 「音の動きを見せる」, with no separator. | the developers' decision |

### G1 The key list and the user guide

| Where | Rule | Basis |
|---|---|---|
| Edit keys in the key list | The groups have no 「編集」. Cut, copy, paste, duplicate, select all and Delete work on the arrange or the piano roll, whichever was pressed last, so they are listed under both 「アレンジ」 and 「ピアノロール」; undo and redo under 「曲とファイル」. | the developers' reading |
| Where the other keys went | Esc, the shelf and the history band keys: 「帯」. Markers, the tempo box, drags: 「アレンジ」. Note list Enter / Tab: 「ピアノロール」. Fader / pan / send keys and the parameters panel: 「ミキサー」. Zoom, screen scale, vertical zoom, F10, Tab: 「表示」. The output chooser's ↑ ↓ Enter are not listed (its own hint says them). | the developers' decision |
| Rows that join two menu rows | Joined with 「 ／ 」 (from the request's picture 「再生 ／ 止める」): 「次のテイク ／ 前のテイク」, 「横に拡大 ／ 横に縮小」, 「縦に拡大 ／ 縦に縮小」; keys the same way (「T ／ Shift T」). | the developers' reading |
| Q | Listed as 「クオンタイズをかけなおす」 as in the request's picture; the menu row stays 「クオンタイズ」. | the request's picture |
| The link and the filter box | The link is accent text in the foot, not a button; the filter box is a row of its own under the heading (as About's second page), not in the heading. | the developers' decision |
| Wording made up (`keyList.*`, `builder:2026-11-23`), rows of the key list | 「ひとつ戻る ／ 閉じる」, 「メニューバーへ」, 「次のボタンや欄へ ／ 前へ」, 「トラック ／ クリップの名前を変える」, 「道具：選ぶ ／ 書く ／ 消す」, 「ノートを半音 ／ 1 オクターブ動かす」, 「ノートをグリッド 1 つ動かす」, 「ノートの一覧：欄に打ちこむ ／ 右の欄へ」, 「複製しながら動かす」, 「グリッドに合わせずに動かす」, 「ドラッグをやめて元に戻す」, 「マーカーを足す」, 「次のマーカーへ ／ 前のマーカーへ」, 「マーカーの名前を変える」, 「選んだ点 ／ マーカーを消す」, 「テンポ ／ 拍子の箱で 1 ／ 10 ずつ変える」, 「棚：上 ／ 下の行を選ぶ」, 「棚：選んだ音を入れる」, 「棚：サンプルを鳴らす ／ 止める」, 「棚：お気に入りの印」, 「履歴：上 ／ 下の行を選ぶ」, 「履歴：いまのステップへ」, 「履歴：ここまで戻す」, 「選んだインサートをとばす ／ 戻す」, 「選んだインサートを外す」, 「フェーダー ／ センドを動かす（細かく）」, 「パンを動かす（大きく）」, 「フェーダー ／ パン ／ センドを最初の値に戻す」, 「フェーダー ／ パン ／ センドに値を打ちこむ」, 「パラメータ：上 ／ 下の行を選ぶ」, 「パラメータ：値を動かす（細かく）」, 「パラメータ：値を打ちこむ」, 「パラメータ：さがす箱へ」 (33 rows). Other rows reuse the menu wording. | builder:2026-11-23 |

### A3 Duplicating a track

| Where | Rule | Basis |
|---|---|---|
| The copy's name | The base is the name without a trailing " N" when N is a number the naming rule makes (2 or more, no leading 0); then the smallest free number from 2. The copy always has a number: 「リード 1」 → 「リード 1 2」, 「リード 02」 → 「リード 02 2」. | the developers' decision |
| ● of a vocal copy | "Not copied" is read as "the source's ● does not carry over"; the copy gets its ● because it is selected (24-C). A source that was armed stays armed, so R records the same input to both. | the developers' reading |
| Monitor, mute sets | The monitor is not copied (session only). The copy is in no mute set, as any track added later. | the developers' decision |
| Plugin state | The plugins are asked for their state first and the copy gets it in the same step (the request said to copy the state object); reason: architecture 25-6. A plugin that is loading, failed, stopped or missing: the copy has the song's state. | the developers' decision (changed from the request) |
| The menu row at 64 tracks | Usable; core answers on the status line. The notice `TooManyTracks` is new and is said only for 「トラックを複製」, 「音のクリップにする」 and 「パターンを置く」; the shelf's new track and 「＋ 歌のトラック」 still say nothing at 64. | the developers' decision |
| Wording | 「トラックは 64 本までなので、これ以上足せません。」 (`msg.tooManyTracks`, status line). | builder:2026-11-23 |

### A5 Recently opened songs

| Where | Rule | Basis |
|---|---|---|
| 「曲の名前」 | The song file's name without `.meloet` (the name inside the file would need every file read; normally the same). Question B-98-17. | the developers' reading |
| 「場所」 | The folder that holds the file, `~` for home, with a final `/`. | the developers' reading |
| The song open now, files that do not read | Choosing the song open now does nothing (no reload). A gone file is found when the read fails, after the unsaved-changes question. A file that is there but broken stays in the list. 「一覧を消す」 can be used while recording. | the developers' decision |

### R3 Metronome settings and tap tempo

| Where | Rule | Basis |
|---|---|---|
| The window's look | Rows `size.toolbar` high, `space.3` left and right, a `line.row` line below; the label left (gray.8; the first row in the name look), buttons right; at least `part.list.width` wide; gray.3 on a gray.8 border; a chosen choice has the transport buttons' on look. | the developers' decision (the mock gives only the rows) |
| The volume bar | The bar of a parameter row (spec 2), not the meter-like bar of the mock (it is a value the person moves). Whole dB; typed values rounded and put into range; no plus sign (「3 dB」). | the developers' decision |
| Texts | The first row says 「オン」 when on and 「オフ」 when off. Before two taps the tempo is 「—」. 「この速さにする」 sits on the tap row right of the tempo found; after it the taps start again and the tempo goes back to 「—」. | the developers' decision |
| Taps | "The last 8" is the last 8 taps (7 gaps); "2 seconds apart" is a gap of 2000 ms or more (a tap earlier than the one before also starts again). | the developers' reading |
| The count-in row | Shown in the window while the count-in button is hidden (spec 3: the count-in then lives in the ▾). The ▾'s name for screen readers is 「メトロノーム」, or 「メトロノームとカウントイン」 then. | the developers' reading |
| The sounds | Choosing a sound does not play it; it is heard on the next click. 「カチ」 and 「カウベル」 were designed by the developers (numbers in architecture 25-8); the owner should listen (B-98-11). | the developers' decision |
| Wording | `metronome.tapNone` 「—」 (as `inspector.scaleNone`). | builder:2026-11-23 |

### P1 Scale highlight and nine scales

| Where | Rule | Basis |
|---|---|---|
| The toolbar toggle | The text says 「色」 (the mock shows 「色をつける」; the text wins); its screen-reader name is 「色をつける」. It sits right after the scale ▾ (the toolbar has no separate 「固定」 button; the lock is the 「スケール固定 …」 button). | the request's text |
| Drum tracks | No new wording: 「色」 reuses the tip 「ドラムには音階がないので、スケール固定は使えません。」 and the right panel 「使えない」. A tip of its own would need new wording (B-98-20). | the developers' decision |
| The menu bar | 「スケール固定 ▸」 lists the nine kinds but has no 「色をつける」 row (the request names only the toolbar and the right panel). | the request's text |
| Pictures | 「色をつける」 is on at first, so every picture with the lock off now shows the rows outside C major in `row.out`; scenes compared with mocks drawn before round 12 (F-PRPlay, F-PRRec, F-PREmpty …) differ there. | the request ("on at first") |

### P3 Velocity and length tools

| Where | Rule | Basis |
|---|---|---|
| First values of 「だんだん」 | 40 and 110 (the request's picture; the text gives none). | the request's picture |
| Out of range | Put at the nearest end. For 「だんだん」 the status line says so (as the other velocity boxes); for 「割合」 and 「ばらつき」 nothing is said and an unreadable value only turns the box red (B-98-22). | the developers' decision |
| Box values | Kept while the panel is shown (tool settings, not of the selection); not saved. The notices are said also when no note changed, with no step (as 「そろえる」). | the developers' decision |
| Wording (status line) | 「{count} ノートのベロシティに {percent} % をかけました。」, 「{count} ノートのベロシティを {from} から {to} へだんだん変えました。」, 「{count} ノートのベロシティを ± {amount} でばらつかせました。」, 「{count} ノートを次の音までのばしました。」 | builder:2026-11-23 |

### P6 Chord progressions

| Where | Rule | Basis |
|---|---|---|
| Which major the degrees are read in | Minor, harmonic minor and melodic minor in their relative major (+3). **Dorian and Mixolydian in the major scale with the same notes** (Dorian +10, Mixolydian +5): reading them by "minor-like +3 / major-like +0" gave D Dorian a B♭ chord and G Mixolydian chords of D major, outside the scale. Pentatonics and blues through their parent. | the developers' decision (the lead, 2026-11-23, reading 「平行調」) |
| The clip's first bar | When the clip does not start on a bar line: the first bar whose head is inside the clip (no growth to the front). | the developers' reading |
| No clip shown | The chords go from the play line's bar onto the play-target track (as the write tool does). | the developers' decision |
| Chords left out | Not said on the status line (「書けませんでした」 would be wrong when some were written). | the developers' decision |
| Sounds and names | ▶ plays at C4..B4 (as the chord row), placing is at C3..B3 (the request). Chord names as the chord row's. The chosen row starts at 「王道」 each time. The play line is the position of the last state (not the moving line while playing). | the developers' decision |

### A9 Reverse and normalize

| Where | Rule | Basis |
|---|---|---|
| "The range the shown take uses" | The frames the clip plays from its start to its end (not the whole take); a cut or trimmed clip is measured and reversed only where it sounds. Normalize measures the file's samples, not what pitch, stretch or fades make of them. | the developers' reading |
| Silent and limited together | When one normalize has both, the status line says the silent one. | the developers' decision |
| A take whose tempo changes, on a clip that does not follow | Its backwards take has no tempo (「—」). | the developers' decision |
| Files that cannot be read or written | The rows are not disabled for a clip whose file is missing (Rust passes it over). A file that cannot be read, or a reverse that cannot be written, does nothing and says nothing (no wording; the shape `clipWorkFailed` exists but is not used yet). | the developers' decision |
| mp3, ogg, flac | Reversed into a 32-bit float wav; a wav keeps its own kind of samples. | the developers' decision |
| With a comp | Both rows are disabled with a reason, and Rust passes such a clip by (see A8). | the developers' decision (A8) |
| Wording (status line) | 「{count} つのクリップは、いちばん大きい音を −1.0 dB にできませんでした（クリップの音量は −60 〜 +12 dB まで）。」, 「{count} つのクリップは音がないので、そのままにしました。」, 「{count} つのクリップは、もとの音のテンポが途中で変わるので、「曲のテンポについていく」をオフにしました。」 | builder:2026-11-23 |

### F1 Loudness of the export

| Where | Rule | Basis |
|---|---|---|
| The mock's texts | The mock's 「いちばん大きい所 −3.2 dBTP」 is the request's 「トゥルーピーク」; the mock's 「。音割れはありません」 after the result is not used. The mock's numbers would pass −1 dBTP; the fake scenes use numbers that keep the rule. | the request's text |
| Place of the lines | The loudness line after the peak line and its clipping lines, the result line right under it; the row 「音の大きさ」 after 「形」, before 「保存する所」. | the mock |
| Numbers | One decimal with 「−」; a loudness not measured is 「−∞」 when the check is done and 「—」 while it counts; a true peak above 0 has 「+」. A gain that rounds to 0 reads 「合わせると 0.0 dB 大きくなります」. | the developers' decision |
| Before the check ends | 「公開先に合わせる」 can be chosen (whether it can be matched is not known yet). The dialog shows what will happen, not the kept choice. | the developers' decision |
| Clipping lines with matching | With 「公開先に合わせる」 in force there are no clipping lines (the file is at −1 dBTP or lower); the peak line still shows the peak as it plays. | the developers' decision |
| `Done`'s numbers | The measured numbers plus the gain, not measured again after a rate conversion (a few tenths of a dB off at another rate). | the developers' decision |
| Wording | 「ここまでの大きさ {lufs} LUFS ・ トゥルーピーク {peak} dBTP」 (the check while counting), 「合わせると {db} dB 小さくなります」 (the request gives only the louder sentence). | builder:2026-11-23 |

### E9 「書き出しました」

| Where | Rule | Basis |
|---|---|---|
| How it closes | Core holds it: 「閉じる」, Esc and Enter close it, and so does any other command (as the status line's 「書き出しました」 always went). Keeping it open while the person plays would need a new shape. | the developers' decision |
| Sizes | The waveform is `size.overview` high (40 at 100 %; the mock's 36 is not a named value), bars centred; the dialog is `part.dialog.width` (the mock's 420 is a picture width); padding space.3 / space.4; space.2 above the waveform and the loudness line. 「閉じる」 is the bright button with 「Enter」. | the developers' decision |
| Numbers | One decimal with 「−」, 「+」 above 0; 「−∞ LUFS」 when there is a true peak but no loudness; no loudness line for silence; seconds cut off. | the developers' decision |
| Wording | 「{seconds} 秒」 (`exported.lengthSeconds`, a file under a minute, instead of 「0 分 45 秒」). | builder:2026-11-23 |

### D1 The notice of a new version

| Where | Rule | Basis |
|---|---|---|
| 「止まっているとき」 | The song is not moving: playing also waits, not only the count-in and recording (question B-98-18). | the developers' reading |
| "At most once a run" | It stays while stopped until 「あとで」 or 「ページをひらく」, or until the song is started after it was shown; it does not come back in that run. | the developers' reading |
| Once a day | 24 hours since the last look (as the sound pack's catalog look), not the calendar day. | the developers' decision |
| Turning it off | Also takes away what was found; an answer that comes while off is dropped; on again later in the run looks once if it has not looked yet. With the development hooks nothing is remembered. | the developers' decision |
| Order in the status line | After every notice, before 「鍵盤をさがしています」 and the keyboard / chooser lines (else a person without a keyboard would never see it). | the developers' decision |
| 「meloet について」 | The line sits under 「バージョン」 in the value column with an empty label, gray.10, no button. | the developers' decision |
| Wording | 「オン」, 「オフ」 of the settings row (the words of 「AI とつなぐ」). | the developers' decision (user:2026-11-23 tag, the neighbours' words) |

### R1 Keeping what was just played

| Where | Rule | Basis |
|---|---|---|
| Runs that end without play / stop | A jump while playing, 「再生」 while playing and a change of the loop while playing also close the phrase (one line of positions and one loop per phrase). | the developers' decision |
| A key near a start | Belongs to the run the engine read it in; a key pressed in the few ms between the play command and its first heard frame lands just before the start, like a pickup. Keys read in the same block as a stop belong to the stopped run and start a new phrase. | the developers' decision |
| "Nothing held" | Keys and CC 64 (≥ 64 is down); two bars at the tempo and meter of the play line when the key arrives. | the developers' reading |
| The head of a phrase | A phrase starts at a key press; controller values before it are written at the head of the take (a pedal pressed before the first chord holds it). Overflow drops whole notes from the old side. | the developers' decision |
| The clip's end, overdub, unplugging | The bar line at or after the end. 「重ねる」 is read when the button is pressed. Keys held when the keyboard is pulled out end at that moment. | the developers' decision |
| The button | Lit while something can be kept, `aria-disabled` otherwise, no key shown on it. A click while disabled sends nothing; Shift R always sends (so 「残せる演奏がありません」 can be said). It hides first (see "How the round was run"). | the developers' decision |
| Not built | The request picture's status line 「さっき弾いた 2 小節ぶん（ノート 14）を残せます」: the text does not ask for it and there is no shape for it. | the request's text |
| Shift R | No clash (a test that pinned "Shift R does nothing" now checks Shift L). | the developers' check |

### R2 Playing with the PC keyboard

| Where | Rule | Basis |
|---|---|---|
| Shift + a letter | Shift is a modifier, so Shift R, Shift M, Shift S, Shift T work while it is on, and Shift + A plays nothing; Ctrl, Alt and Meta the same. Digits are the row's and the number pad's. | the developers' reading |
| Octave | Z at C0 and X at C7 do nothing (still not a shortcut). The octave is kept while meloet runs (also off and on), C3 at first. | the developers' decision |
| The 「音が出ました」 hint | Not put away by the PC keyboard's keys (they are a keyboard). | the developers' reading |
| Input method | `isComposing` keys and keys whose `key` is `Process` are neither notes nor shortcuts, and their default is left to the input method. | the developers' decision |
| Notices, time limit, names | A typed key does not put a notice away (a MIDI key does not either); the switch does. Times older than 250 ms are taken as now. Rust sends the PC keyboard with an empty name; the screen names it 「パソコンのキーボード」. | the developers' decision |
| Where the switch is | Right of the keyboard's name (the request's picture), only in room left over (see "How the round was run"); question B-98-5. | the developers' decision |
| The menu row | The last row of 「再生」, with Mod K and a check while on. | the developers' decision |
| Not built from the request's picture | 「Z ／ X で上下 ・ Esc でやめる」 next to the octave, the line under the keys and the picture of the 13 keys (the status line says the same; the text asks only for the switch and the octave). | the request's text |
| Turned on with no output chosen | The output chooser comes up (it counts as a keyboard); keys do not sound until an output is open. | the developers' reading |
| Wording | 「日本語入力がオンだと鳴りません」 (`msg.typedKeysIme`), joined to the request's status text with 「 ・ 」 so that text stays as written. | builder:2026-11-24 |

### P2 Other tracks' notes, faintly

| Where | Rule | Basis |
|---|---|---|
| Toggle and ▾ | 「ほかのトラックも見る ▾」 is read as the existing toggle + ▾ form: the label is the toggle, the ▾ a button of its own. Placed right after the spacer, before the tools. The chosen track names in the request's picture are not shown. | the developers' reading |
| Folded form | Made up: only the ▾, whose list starts with the switch row. The list is a check list (stays open after a pick). | the developers' decision |
| What is kept | `hidden` (tracks taken off), so a track added later is shown; forgotten with another song; on / off kept. A disabled ▾ when there is nothing to offer; the toggle stays usable. | the developers' decision |
| The 20 000 limit | Counted from the play line when the picture is made. Audio clips on a keys track are skipped. | the developers' decision |
| Wording | 「見せるトラック」 (the ▾'s name and the list heading), 「ドラムのトラックでは、ほかのトラックは見られません。」 (the tip on drum rows). | builder:2026-11-23 |

### E1 Sounding notes light up

| Where | Rule | Basis |
|---|---|---|
| Look | The note's face in the track color with the 2 px gray.10 edge kept, as a selected note keeps it (the request's picture fills the whole note). A lit note covers the selection's accent while it sounds. | the developers' reading |
| Drum rows | The square lights while its note sounds (the note's own length). | the developers' decision |
| Mute, count-in, recording, drags | Only the mute button and solo count (a mute lane is not read). Not lit in the count-in. Nothing lights while the clip being recorded is shown; recording on another track lights the shown clip. Not lit while notes are dragged. | the developers' reading |
| The box and the cap | The lit box is the note area's full height (chapter 7 rule), and the cap is a width (15 % of the window's area over the note area's height) with a two-frame change, in place of the request's "32 notes at most": the cost is set by the width, not by the count. Reasoned, not measured (B-98-9). | the developers' decision |
| The box stays put (after measuring, 2026-11-24) | The canvas box is not as wide as the lit notes: a new one leaves one to two steps of 32 px to their right and stays while they stay inside it, so most changes only draw the canvas again (no layout, no new size, fewer changed rectangles). Not visible. The lit notes and the cap are as above; the count is not narrowed, and the setting stays on at first (measurements, "E1 after the fix"). | the developers' decision |
| The menu row's place | Last in 「表示」 (now followed by E7's row). | the developers' decision |

### P4 Drum patterns

| Where | Rule | Basis |
|---|---|---|
| Drum track | The instrument's group is ドラム (the three kits, an SF2 drum kit). A plugin drum track is not one: 「ここに置く」 makes a studio kit track and ▶ plays the studio kit (meloet does not know a plugin's key map). Question B-98-21. | the developers' decision |
| Over a clip | The existing 「ほかのクリップと重なるので、書けませんでした。」 (of the overlap notices, the one about writing). | the developers' reading |
| After placing | The clip is selected and shown in the piano roll (as pasted clips). The tempo of ▶ is the one at the bar where it would be placed. ▶ does not choose the row. | the developers' decision |
| What the list shows | No mode line, search box, 「すべて ／ お気に入り」, stars or 「0:01 / 0:02」 time (the mock shows none); no status line while a pattern plays (■ on the row shows it). The headings start open. The bottom line's bar is the play line's start position. | the mock |
| The patterns | Written by the developers (which drum where, velocities), checked by rules in a data test; not heard by the owner yet (B-98-21). | the developers' decision |
| Wording | 「トラック {number}（{instrument}）を作って、{bar} 小節めに置きます」 (the bottom line when a studio kit track will be made), 「パターンを聞く」 (the ▶ button for screen readers). | builder:2026-11-23 |

### A1 Repeating clips

| Where | Rule | Basis |
|---|---|---|
| ⟳ | Shown only while the pointer is on the handle (or it is held), not on the whole clip: the request's sentence is in the handle's item (B-98-12). | the developers' reading |
| "The name strip does not show" | The clip is narrower than `part.take.tag-min-width` (28). The handle's width is `space.3` (the named value equal to 12 px). | the developers' reading |
| Cursor, count | The copy cursor (`not-allowed` when no copy fits); a drag's count is rounded to the nearest whole copy. | the developers' decision |
| The frames | `gray.2` face, `gray.7` border, a `line.mark` accent bar at the end of the last copy; the mock's dashed top in the track color is not drawn. | the developers' reading |
| The notice | The existing 「ほかのクリップと重なるので、複製できませんでした。」, also past the song's end. | the developers' decision |
| The box | At the end of the block on the right-clicked clip's name strip (else the upper clip that ends last), in the look of the clip name box; out-of-range counts put at the nearest end without words (no field for it); full-width digits read. Disabled in the overview. A double click on the handle opens the name box. | the developers' decision |
| Mod R | Also works while typing (as Mod D, Mod J). | the developers' decision |
| Wording | 「くり返す回数」 (the box's name for screen readers). | builder:2026-11-23 |

### M1 Presets

| Where | Rule | Basis |
|---|---|---|
| Reverb and delay | A factory preset leaves their mix as it is (it depends on the strip). A user preset sets every kept value, the mix too (B-98-23). | the developers' decision |
| The button row and the list | The row holds only the button, on the right (the panel already shows the effect's name). The list is drawn in the panel and scrolls with the parameter list. The order is the request's text order. User presets in the order first kept; writing over keeps the place. | the developers' decision |
| Factory ids | `eq4` vocalClear, unmuddy, cleanLows, fatBass; `compressor` vocalEven, drumPunch, bassSolid, gentleGlue; `reverb` smallRoom, hall, behindVocal; `delay` echo, slapback, pingPong; `chorus` subtleWide, deepSwirl; `limiter` finish, gentle. | the developers' decision |
| Names and states | A name over 40 characters is cut to 40. The button is accent whenever the values are a preset. Applying is refused while counting in or recording (the song-change rule), though the mixer's own parameter moves work then. | the developers' decision |
| Factory values | Chosen by the developers (architecture 25-22, `crates/meloet-model/src/presets.rs`), not heard (B-98-23). | the developers' decision |
| Wording | 「{name} ▾」, 「「{name}」を上書きしました」, 「レーンが動かしている {count} つのパラメータは、そのままにしました」, 「{effect}のプリセットは {max} 個までなので、これ以上残せません。」 | builder:2026-11-24 |

### M6 Gate, de-esser, saturation

| Where | Rule | Basis |
|---|---|---|
| Gate 「戻る速さ」 | The time to fall 60 dB, at a steady rate in dB (a smaller 「下げる量」 is reached sooner). The 30 ms count from the last frame above the closing level, not only from the opening. Opening is a straight line of the gain in 1 ms. | the developers' reading |
| De-esser | Built as a moving high shelf, not two bands added back (no bump below; untouched when idle). 「周波数」 is the shelf's middle: a sound at the frequency gets half the cut in dB; the full cut is about an octave and a half above (B-98-24). Attack 1 ms and release 50 ms fixed; left and right linked. | the developers' decision |
| Saturation 「ひずみ」 | The lift of quiet sound in dB, the curve bending around −18 dBFS (B-98-24). 「ひずみ」 0 dB is exactly clean. 「明るさ」 20000 Hz is off (fading in between 18 and 20 kHz). 「出る大きさ」 is the last step, after the mix. | the developers' decision |
| Value texts | dB with one place, Hz and ms with none (the existing rule). The scene `presetsNone` is now a gate whose values are not a preset. | the developers' decision |
| The AI link | The text of the insert tool's `effect` argument names the nine effects; no new tool. | the developers' decision |
| Wording (preset names) | 「雑音を消す」, 「余韻を短く切る」, 「サ行をやわらかく」, 「サ行をしっかりおさえる」, 「あたたかく」, 「荒くひずませる」. | builder:2026-11-24 |

### M2 「音が大きすぎます」

| Where | Rule | Basis |
|---|---|---|
| The sentence's number and the button's | The sentence has the measured amount to a tenth (「5.6 dB」), the button the whole dB it lowers by (「6 dB」, rounded up from the shown amount). | the developers' reading (the lead) |
| A's amount | The mean over the time the limiter held 3 dB or more, not over the whole 4 s (the protocol comment says the 4 s; B-98-7). | the developers' decision |
| B's 「3 回」 | Three telemetry readings (about 60 a second) with the master's peak over 1.0. "Half of the 4 s" is the time the readings stand for. | the developers' reading |
| A lasting line | Not a notice (Space would put a notice away, and the buttons must stay usable after stopping); it stays until 「閉じる」, 「下げる」, the next play or another song. | the developers' decision |
| Lowering | With every fader at −∞: no step, the line goes. 0, negative or not a number: nothing. Lane points are each lowered N dB on the fader scale (not with `move_points`, which is not in dB); −60 dB stops just above −∞. `count` is the tracks that changed. The master's reduction is not shown on the screen. | the developers' decision |

### F2 The song's information

| Where | Rule | Basis |
|---|---|---|
| mp3 | Not offered: the acceptance condition 2 failed (A-23, C-11; B-98-1). | the owner's condition (user:2026-11-23) |
| Place and look | The three rows in 「くわしく」 after 「実時間で書き出す」 under the small heading 「曲の情報」 (gray.8), each a row of the usual form (name 120 wide); the boxes are the key list's input box. 「はずす」 before 「えらぶ…」. There is no mock. | the developers' decision |
| What belongs to what | The title and picture belong to the song (kept while it is open, also after closing the dialog); an empty title writes no title. 5 MB is 5 MiB. The picture window starts in the last picture's folder, else the song's, else home. png or jpg is decided by the bytes. The artist is saved as soon as it changes. The AI link's exports write none. | the developers' decision |
| wav | RIFF INFO after the audio data, only `INAM` and `IART` (ffmpeg and lofty read it there; symphonia does not). | the request ("題名と作者だけ") |
| Wording | 「はずす」, 「なし」, 「「{file}」は大きすぎるので使えません（絵は {mb} MB まで）。」, 「「{file}」は絵として読めません（使えるのは png ・ jpg）。」. In the mp3 branch only: 「{rate} Hz ・ {kbps} kbps ・ しっぽは音が消えるまで（最大 {seconds} 秒）」, 「{rate} Hz ・ {kbps} kbps ・ しっぽなし」, 「{length} ・ {rate} Hz ・ {kbps} kbps」, 「{file} を書き出しました（{length} ・ {rate} Hz ・ {kbps} kbps）。」 | builder:2026-11-24 |

### E2 Arpeggio

| Where | Rule | Basis |
|---|---|---|
| Stop and hold | Stop does not end a held chord (the request's list does not name it); it goes on at the tempo (B-98-6). | the developers' reading |
| Count-in, punch-in | The arpeggio sounds through the count-in on the grid that leads to the start; nothing of it is recorded before the start. Starting a recording while playing: the note sounding then is not in the take. | the developers' decision |
| Order, shapes | Notes sorted by pitch over all octaves, each once. 「上下」 does not say the top and bottom twice; 「ばらばら」 never says one note twice in a row (a fixed seed that runs on; it is recorded as notes). | the developers' decision |
| Keys that come and go | The step count runs on; a change is heard from the next step. A short step before a bar line ends at the bar line. A rate change is taken at the next step of the new grid. A 17th key is left out. With hold, a key after all were let go replaces the chord. Octaves out of range are brought into 1..3. | the developers' decision |
| The mixer strip | No free seat above 「音源」 was laid out, so 「アルペジオ」 is at the right end of the strip's heading, only while on, and can only turn it off; the track name is then cut to about one character on a 108 px strip (B-98-6). | the developers' decision |
| The input meter | Takes left-over room before 「キーで弾く」; at 1280 × 800 at 150 % it is not shown and there is no other way to turn the arpeggio on (B-98-6). While counting in or recording the button is shown disabled. | the developers' decision |
| Wording | 「アルペジオの設定」 (the screen-reader name of ▾ and of the window). | builder:2026-11-24 |

### E3 The knobs of a built-in sound

| Where | Rule | Basis |
|---|---|---|
| 「のび」 of a drum kit | Read as "how long a hit rings": electro kit, the decays × 1/4 to × 4; sampled kits, only shorter below 50 % and no change at 50 % and above (a recording cannot ring longer than it was recorded) (B-98-10). | the developers' reading |
| 「立ち上がり」, 「のび」, 「ゆれ」 | 「立ち上がり」 above 50 % adds up to 400 ms, at 0 % one frame; a sound whose own attack is 0 does not change below 50 %. 「のび」 changes the release of voices let go (the piano's release and pedal sounds are not changed). 「ゆれ」 starts at the note's pitch going up, up to 50 cents. | the developers' decision |
| Typing | "35", "35 %", "35%" are read; a text with no number sends nothing and says nothing. 「音源を入れかえる…」 stays under the four rows. | the developers' decision |
| The scene `paramsSound` | Left as it was (a grand piano with no knobs, which Rust no longer sends); it still pictures the panel an SF2 preset gets. | the developers' decision |
| Wording | 「{track} ・ 「{knob}」をえらんでいます」 (status line), 「{track} ・ {knob} {from} → {to} %」 (history row). | builder:2026-11-24 |

### E7 The picture of the sound

| Where | Rule | Basis |
|---|---|---|
| The FFT | The FFT meloet already had (it allocates only when made), not `realfft`: no new crate. | the request ("use the FFT there is when it allocates only when made") |
| When it works | No reader, the setting off, or a silent window: no FFT. When the screen hides the bars for lack of room the analysis still runs (core does not know the width). | the developers' reading |
| A band's level | The power in the band, so a full-scale sine reads 0 dB in its band; narrow low bands read at their middle (a sine there reads −1.8 dB). The fall is in sound time; frames the ring dropped restart the bands from silence. | the developers' decision |
| Sizes | The gap to 「マスター」 is space.1 (space.2 did not fit at 1280 × 800 100 %); one device pixel of ground at the right of each 4 px band (the mock's 5 + 2 px would not make 96); the height is the master's whole box. The bars may show while 「さっきのを残す」 or the song name is hidden, when the room left is enough. | the developers' decision |

### M3 Finishing meters

| Where | Rule | Basis |
|---|---|---|
| Two meters | 「いまの大きさ」 and 「曲ぜんたい」 come from two meters, not one (so 「曲ぜんたい」 keeps the last play's value and 「いまの大きさ」 shows what is heard now). | the developers' decision |
| What the meters hear with mono on | Every meter of the master shows the mono sound, as heard (the request puts mono before the meter); 「曲ぜんたい」 of a play with mono on part of the time is partly mono (B-98-8). | the developers' reading |
| The level of mono | `(L + R) / 2` on both sides (a centred sound keeps its level; one side only is 6 dB lower; no new clipping). | the developers' decision |
| The bar of 「いまの大きさ」 | On the meters' scale (−14 LUFS at 37 %), on its own line the whole width of the box (the picture's other scale is not given anywhere) (B-98-8). | the developers' decision |
| Layout and colors | Rows level with the master's meter, the bars filling the rest; 「低い音 ← → 高い音」 under the bars, 「公開の目安は …」 at the bottom; the button in the head row; labels gray.8, 「曲ぜんたい」's number gray.10, other numbers gray.9, the box gray.3 with a gray.10 top line. No sound: 「−∞ LUFS」 / 「−∞ dBTP」. | the developers' decision |
| The play | Playing or recording, not the count-in; a recording after a count-in starts anew when it starts. Moving the play line or a loop going round is not a new play. While the box is scrolled out of view it is still measured. | the developers' reading |

### A2 Whole sections

| Where | Rule | Basis |
|---|---|---|
| 「この前に空きを入れる…」 | Needs a usable section like the other two (so not before the last marker at the song's end) (B-98-13 asks). | the developers' decision (the lead's request) |
| Details | Markers on one tick share a section. The mock's first row 「この区間をループに」 is not used (the existing row stays). The reason of a disabled row is shown in the row under the pointer. The loop is not copied. A section longer than `u32::MAX` bars is not usable. | the developers' decision |
| Empty bars and lanes, meter | The bars hold the value at the cut (for a jump on the cut, the value the jump goes to; for a stepped lane, the value before the step); a meter change on the cut's bar stays, so the bars are in the new meter. | the developers' reading |
| Seams | A ramp or slope that meets another value is ended one tick before the seam. Points at a seam that the line is the same without are left out (also ones that were there before), so bars in and out again give the song back (exceptions in architecture 25-30). | the developers' decision |
| Selection, out of range | Everything selected is let go (the piano roll keeps its clip when it is still there). A typed count outside 1–64 is put to the nearer end and the status line says 「小節は 1〜64 なので、64 にしました。」 (the existing sentence). | the developers' decision |
| Wording | 「このマーカーより後ろにクリップがないので、区間がありません」 (disabled rows), 「{count} 小節の空きを入れます」 (status line while the box is open), 「曲が長くなりすぎるので、できませんでした。」 (status line), 「入れる小節の数」 (the box's screen-reader name), 「{name} ・ {bars} 小節」 (the history row). | builder:2026-11-24 |

### A7 Turning keys into audio

| Where | Rule | Basis |
|---|---|---|
| The range of 「音のクリップにする」 | The track over that range: clips between the selected ones that were not selected are in the audio too. | the developers' reading |
| Muting | 「元のトラックは…ミュートします」 is followed for 「音のクリップにする」 too: clips outside the range go silent (B-98-14). | the request's text |
| What is copied | Volume, pan and sends with their automation lanes; instrument, insert and mute lanes stay in the render. The returns are emptied and muted in the render (a plugin there would be started for nothing). | the developers' reading |
| The new track | The color of its place (as 「＋ 歌のトラック」), not selected (selecting would arm it). | the developers' decision |
| Refusals | While the export dialog is open the status line says the dialog is open (so 「書き出しか…」 is not seen then). The AI link's export and measuring are refused while bouncing. A file that comes too late is removed from the work place. | the developers' decision |
| Progress | A status line message with a button (as 「音を作っています」), not the export's bar; the percent is the range's, 99 until the step. `clip_doc` of the AI link was not widened; `bounce_doc` is its own. | the developers' decision |
| Wording (status line) | 「{track} に音のクリップを置きました。もとのトラックはミュートしました。」, 「音にするのをやめました。」, 「音にできませんでした。{plugin} が止まりました。」, 「音にできませんでした。作業の場所に書けません。」, 「もとのトラックがなくなったので、できた音はすてました。」, 「書き出しか「音にする」の途中なので、終わってからもう一度ためしてください。」, 「このトラックにはクリップがありません。」, 「音にしているあいだは書き出せません。終わってからひらいてください。」 | builder:2026-11-23 |

### M5 The master's lane and the fade-out

| Where | Rule | Basis |
|---|---|---|
| Shape of the fade | Lines are straight in the fader's place (not in dB), so half way the fader is half way. No curves. | the developers' decision |
| Again with fewer bars | Changes nothing heard (the new start takes the lane's value, which lies on the old fade); to shorten, undo or drag the points. A longer one replaces the old one (B-98-16). | the developers' decision |
| A resting lane, a lane with no points | The fade starts from the fader, and the lane moves again (points before the fade play again too). A lane with no points is as no lane. A point on the start is replaced. Bars outside 1 to 16 reaching core do nothing. | the developers' decision |
| The export's end | 「曲ぜんぶ」 ends at the last clip's end, not at the next bar head: when the last clip ends inside a bar, the file ends before the fade does (B-98-16). | the developers' note |
| Screen | One row 「マスター 音量」; folded it stays as a low row 「▸ マスター 音量」 (the master has no 「＋ 帯を足す」); the mixer's mark at the right end of the master's value row (there was no disabled mark before) (B-98-16). The box opens in the bar ruler with its right edge at the song's end; disabled in the whole-song view. 「全部のトラックを下げる」 leaves the master alone. | the developers' decision |
| Wording | 「曲の終わりの {count} 小節をフェードアウトにします」 (status line while the box is open), 「フェードアウトの小節の数」 (screen-reader name), 「クリップがないので、フェードアウトをつけられませんでした。」 (status line). | builder:2026-11-24 |

### A8 Comping

| Where | Rule | Basis |
|---|---|---|
| A9 and M7 with a comp | 「逆にする」 and 「ノーマライズ」 are refused (the loudest sample of the shown take is not what the clip plays). 「音程を寄せる」 stays disabled; a tune set before the comp stays and is heard; turning it on for comps is one line (B-98-15). | the developers' decision (the lead) |
| Stretch with a comp | Made right (one made file per take that sounds), not refused. | the developers' decision |
| Takes going and coming | Deleting a take is refused while there is a comp; removing a missing file takes the comp only when a piece names its takes; a recording over the clip keeps the comp, and the new takes are there to choose (the other choice would lose work silently) (B-98-15). | the developers' decision |
| No smallest piece | One tick is allowed; seams shrink beside a short piece. The screen cannot make a piece shorter than a grid step without Shift. A list that says what the clip plays already is no step. | the developers' decision |
| The band | Right under the track's row, above its lanes; rows `size.lane` (the mock's are thinner), the heading `size.row`; 「▾ いいとこ取り」 in the head column, the rest on the time side (the mock's heading is one row). 「歌 1 ・ テイク 3」 is the clip's name and the shown take. The newest take on top (the request's text; the mock shows take 1 on top). The menu row is with the take rows. Opening scrolls the band into view. No audition. Waves drawn at volume 1 without fades. | the developers' decision |
| The cross-fade | A straight line over 10 ms (the request says only "10 ms"); an equal-power curve would bump 3 dB on like audio. | the developers' decision |
| Wording | 「テイクが 2 つ以上あるクリップで使えます」, 「いいとこ取りがあるあいだは使えません」 (disabled menu rows on hover). The step name 「いいとこ取りを外す」 is provisional. | builder:2026-11-24 |

### M7 Pulling the pitch toward the scale

| Where | Rule | Basis |
|---|---|---|
| The detector and the smoothing | YIN, as in the paper (25 ms, 0.15; "not sure" above 0.25 is the developers' number). "Smooth over 50 ms" is a zero-lag smoothing whose step response goes 18 % → 82 % in 50 ms; a 5.5 Hz vibrato keeps about half its depth at 100 %. | the developers' reading |
| Limits and unvoiced parts | No limit in the request; ±200 cents in the code. Points within 60 ms of a voice move with it (else the first 50 ms of a note would be half tuned); further away exactly 0. | the developers' decision |
| A separate preset | The transpose is set every 256 frames but the library uses it once per block, so tuned clips use a denser preset (block 0.12 s, interval 0.02 s); a stretched or pitched clip sounds slightly different once also tuned (B-98-19). | the developers' decision |
| The rows | The strength is a dragged number (`line.mark` per percent; the protocol comment says "bar"); the scale row has no label of its own; the group count went from 4 to 5. `tune_usable`: a vocal clip of one take sounding, whose file is found; there is no "kind of audio", so the tip is the only warning. Strength 0 is allowed even when not usable. | the developers' decision |
| Wording | 「音程を寄せる {percent} %（ドラッグ中）」 (status line while dragging; in the form of 「音の高さ …（ドラッグ中）」). | builder:2026-11-23 |

### D2 AppImage

| Where | Rule | Basis |
|---|---|---|
| Children's environment | `GTK_THEME` is removed for children (the gtk hook always sets it); `xdg-open` gets the cleaned environment too. | the developers' decision |
| `--mcp` | Not limited to the AppImage (harmless for the deb). | the developers' decision |
| A relative song argument | Read from `OWD`; without it a relative path did not open. | the developers' decision |
| Not done | The LGPL source offer and the patched WebKit library (B-98-2): stopped rather than ship something unclear; nothing is published. | the request ("stop rather than ship something unclear") |

### D4 English wording

| Where | Rule | Basis |
|---|---|---|
| Where the setting is | The request says "in the settings" without a place; the theme, its model, is a row of 「表示」, so 「ことば / Language ▸」 is the row after 「テーマ ▸」 with 「日本語」 and 「English」. Disabled while counting in or recording (the core refuses then). | the developers' decision |
| The row's words | 「ことば / Language」, 「日本語」, 「English」 are the same in both tables, so a person who cannot read the current language finds them. | the request's 「ことば / Language」, 「日本語／English」 (user:2026-11-23) |
| The PC's language | The first of `LC_ALL`, `LC_MESSAGES`, `LANG` that is set and not empty decides (the POSIX order for messages); none set, or `C`, is English. Until a language is chosen it is read at every start, so the setting stays "not chosen". | the developers' decision |
| Names of new things | 「はじめての曲」 "My first song"; track names: Piano, Guitar, Bass, Drums, Pad, Winds, Synth, Organ, Mallets; SF2 families: Strings, Ensemble, Brass, Reeds, Pipes, Ethnic, Percussion, Sound effects, Synth; 「歌」 "Vocal". At the first start the untouched new song is made in the PC's language; after a change only new things follow it. | the developers' translation |
| PC file window titles | "Open", "Choose a folder", "Save as", "Export", "Choose a folder to export to". | the developers' translation |
| Joints | 「 ・ 」 → " · ", 「、」 → ", ", two sentences one after the other get a space; 「（…の帯も消えました）」 goes before the first "." as before the first 「。」. | the developers' decision |
| Numbers | A text whose noun follows a number has a form for 1 and one for any other number (`n(by, one, other)` in en.ts); Japanese is unchanged. | the request |
| Term tips | Looked up by the Japanese word as before; in English a name shown in English (drum names, …) finds its term too. Explanations that said "also called 「ハイカット」" where the English word is that name now leave it out. | the developers' decision |
| Shortened to fit (1280×800 at 150 %) | 「アレンジ 全体」 "Arrange · all"; 「公開の目安は…」 "Target …"; 「センド B の戻り」 "Send B return"; 「出口 ・ …」 (mixer) "Out · …"; 「もとの位置へ戻す」 (quantize panel) "Put back"; 「新しい版をたしかめる」 "Check for updates"; 「いま … ・ 点 5」 "Now … · 5 pts"; 「録った音の位置」 "Recorded position"; 「次の音までのばす」 "Stretch to next"; 「さがす枠を空にすると…」 "Clear the search to go back to the groups"; the output chooser's help; 「鍵盤を USB でつなぐと…」 "Plug in a USB keyboard and it shows here. Or write with the mouse."; 「音源 202」 (shelf tab) "Instr. 202"; 「選択 80 83」 (velocity band) "Sel. 80 83"; 「止」 (stopped insert) "■". | the developers' decision (the request: reword, do not widen) |
| shoot_ui.py | `--lang=en` adds `&lang=en`, names the files `-en`, puts no mock beside them, and turns the Japanese words the clicks look for into the English ones (`EN_WORDS`). | the developers' decision |

## A-30 After the features: the owner's answers (2026-11-24)

The owner's answers to the open questions are in `docs/history/round12-answers-brief.md`; how they are built is architecture 25-37.

### Mute sets and the batch row (B-97-1, B-97-2, B-97-3, B-97-5)

| Where | Rule | Basis |
|---|---|---|
| N of 「まとめて変える（N 個）」 | The number of steps the batch made one, which is the number of its operations that changed the song (each makes one step). An operation that changed nothing, and solo or the loop's switch (no steps, not undone), are not counted, so N can be smaller than the number of operations the AI sent. A batch that made one step keeps that step's own row. | the developers' reading of the owner's answer (Fable agreed) |
| The rest of the batch row | Nothing follows the name (no 「（何を）」) and the right column is empty: the changes can be anywhere. | the developers' decision |
| 「（何を）」 of a rename | Both names, 「サビ → 大サビ」, as a marker or track rename; the other three mute set steps keep the set's name. | the developers' decision |
| A name taken by a rename | Refused, as a save that is not to write over a set is (the rule a save already had); renaming to the name it has is no change and no step. | the developers' decision (the lead: follow the rule saving uses) |
| A set's name | The spaces at its ends are taken off; 1 to 64 characters (the AI tools' limit already); no control characters. The same for a save and a rename, from the screen and from the AI. | the developers' decision |
| How the screen names a set | By its name (a song has one set of a name; the AI names them so; no ids in the file). An intent whose name is no set does nothing; one that does not fit (a name taken, no name, 64 sets) does nothing too, with no status line: the menu has the list and the numbers to prevent it. | the developers' decision |
| `asNow` | True when a recall would change nothing: the tracks the set names that the song still has are muted and soloed as it says, and so are the returns; tracks added since do not count. The same as the AI tools' `as_now`. | the developers' decision |
| English step names | 「ミュートのセットを保存する」 "Save a mute set", 「ミュートのセットを呼び出す」 "Recall a mute set", 「ミュートのセットを消す」 "Delete a mute set", 「ミュートのセットの名前を変える」 "Rename a mute set", 「まとめて変える（N 個）」 "Change together (N)" (the count is always 2 or more; 「まとめて変える」 is "Change together" in the clips panel already). | the developers' translation |
