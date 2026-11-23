# 4 List of keys

<kbd>Mod</kbd> is <kbd>Ctrl</kbd> on Linux and Windows, and <kbd>Cmd</kbd> on Mac. This one table is read for both.

- Keys marked “one letter” (「一文字」) do not work while typing text (when the cursor is in a name, a search box, or a number box). Then they go in as plain characters.
- Even while typing, <kbd>Esc</kbd> (stop typing), <kbd>Enter</kbd> (confirm), and keys with <kbd>Mod</kbd> work. <kbd>Space</kbd> does not work and goes in as a space. <kbd>Delete</kbd> and <kbd>Backspace</kbd> do not work either; they delete only the text in the box (they do not delete the chosen clips or notes).
- While a plugin window is in front, all keys, <kbd>Space</kbd> and keys with <kbd>Mod</kbd> too, belong to the plugin. meloet does not receive them. To play and stop, go back to the meloet window first.
- Keys are shown small at the right end of band tabs and buttons.
- While 「キーで弾く」 is on (round 12, R2), keys marked one letter and the digit keys do not work (they play notes, or nothing); keys with <kbd>Mod</kbd> or <kbd>Shift</kbd>, <kbd>Space</kbd>, the arrows, <kbd>Delete</kbd>, <kbd>Home</kbd>, <kbd>Enter</kbd>, <kbd>Tab</kbd>, the F keys and <kbd>Esc</kbd> work as before. The keys by their place: <kbd>A</kbd> <kbd>S</kbd> <kbd>D</kbd> <kbd>F</kbd> <kbd>G</kbd> <kbd>H</kbd> <kbd>J</kbd> <kbd>K</kbd> <kbd>L</kbd> <kbd>;</kbd> are the white keys C D E F G A B C D E; <kbd>W</kbd> <kbd>E</kbd> are C# D#; <kbd>T</kbd> <kbd>Y</kbd> <kbd>U</kbd> are F# G# A#; <kbd>O</kbd> <kbd>P</kbd> are C# D# of the next octave; <kbd>Z</kbd> / <kbd>X</kbd> move the octave down / up (C0 to C7). See spec 3, "Rules added in round 12".
- A key added to the app must also be added to the key list dialog (`ui/src/keys/keyList.ts`; round 12, G1).

| Action | Linux, Windows | Mac | While typing |
|---|---|---|---|
| Play / stop | <kbd>Space</kbd> | <kbd>Space</kbd> | Does not work (goes in as a space) |
| Record | <kbd>R</kbd> | <kbd>R</kbd> | Does not work |
| Back to the start (with the loop on: to the start of the loop; pressed there again: to the start of the song) | <kbd>Home</kbd> | <kbd>Home</kbd> | Works |
| Loop on / off | <kbd>L</kbd> | <kbd>L</kbd> | Does not work |
| Metronome on / off | <kbd>C</kbd> | <kbd>C</kbd> | Does not work |
| Back to the arrange | <kbd>A</kbd> | <kbd>A</kbd> | Does not work |
| Open / fold the piano roll | <kbd>E</kbd> | <kbd>E</kbd> | Does not work |
| Open / fold the mixer | <kbd>M</kbd> | <kbd>M</kbd> | Does not work |
| Open / fold the undo history | <kbd>H</kbd> | <kbd>H</kbd> | Does not work |
| Open / fold the shelf | <kbd>P</kbd> | <kbd>P</kbd> | Does not work |
| Open / fold the parameters | <kbd>U</kbd> | <kbd>U</kbd> | Does not work |
| In the parameters: choose the row above / below | <kbd>↑</kbd> <kbd>↓</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> | Works |
| In the parameters: move the value / move it finely | <kbd>←</kbd> <kbd>→</kbd> / <kbd>Shift</kbd> <kbd>←</kbd> <kbd>→</kbd> | <kbd>←</kbd> <kbd>→</kbd> / <kbd>Shift</kbd> <kbd>←</kbd> <kbd>→</kbd> | Works |
| In the parameters: type a value | <kbd>Enter</kbd> | <kbd>Enter</kbd> | Works |
| In the parameters: go to the search box | <kbd>Mod</kbd> <kbd>F</kbd> | <kbd>Mod</kbd> <kbd>F</kbd> | Works |
| In the shelf: choose the row above / below (choosing starts audition) | <kbd>↑</kbd> <kbd>↓</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> | Works |
| In the shelf: play / stop the sample | <kbd>Space</kbd> | <kbd>Space</kbd> | Does not work (goes in as a space) |
| In the shelf: set / clear the favorite mark | <kbd>F</kbd> | <kbd>F</kbd> | Does not work |
| In the shelf: put in the chosen instrument | <kbd>Enter</kbd> | <kbd>Enter</kbd> | Works |
| Go back one (close) | <kbd>Esc</kbd> | <kbd>Esc</kbd> | Works |
| Undo | <kbd>Mod</kbd> <kbd>Z</kbd> | <kbd>Mod</kbd> <kbd>Z</kbd> | Works |
| Redo | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>Z</kbd> | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>Z</kbd> | Works |
| Save | <kbd>Mod</kbd> <kbd>S</kbd> | <kbd>Mod</kbd> <kbd>S</kbd> | Works |
| Export | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>E</kbd> | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>E</kbd> | Works |
| Sound settings | <kbd>Mod</kbd> <kbd>,</kbd> | <kbd>Mod</kbd> <kbd>,</kbd> | Works |
| Cut / copy / paste | <kbd>Mod</kbd> <kbd>X</kbd> / <kbd>Mod</kbd> <kbd>C</kbd> / <kbd>Mod</kbd> <kbd>V</kbd> | <kbd>Mod</kbd> <kbd>X</kbd> / <kbd>Mod</kbd> <kbd>C</kbd> / <kbd>Mod</kbd> <kbd>V</kbd> | Does not work (used for the text in the box) |
| Duplicate | <kbd>Mod</kbd> <kbd>D</kbd> | <kbd>Mod</kbd> <kbd>D</kbd> | Works |
| Select all | <kbd>Mod</kbd> <kbd>A</kbd> | <kbd>Mod</kbd> <kbd>A</kbd> | Does not work (used for the text in the box) |
| Rename the selected track (with a clip chosen: the clip; round 10) | <kbd>F2</kbd> | <kbd>F2</kbd> | Works |
| Remove the chosen insert | <kbd>Delete</kbd> | <kbd>Delete</kbd> | Does not work (mixer) |
| Add a marker | <kbd>K</kbd> | <kbd>K</kbd> | Does not work (one letter; at the play line) |
| To the next marker / to the previous marker | <kbd>Alt</kbd> <kbd>→</kbd> / <kbd>Alt</kbd> <kbd>←</kbd> | <kbd>Option</kbd> <kbd>→</kbd> / <kbd>Option</kbd> <kbd>←</kbd> | Works |
| Rename the chosen marker | <kbd>F2</kbd> | <kbd>F2</kbd> | Does not work (the same key as the track name; what is chosen decides) |
| Delete the chosen points / marker | <kbd>Delete</kbd> | <kbd>Delete</kbd> | Does not work |
| History band: choose a row / go to the current step / go back to here | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Home</kbd> / <kbd>Enter</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Home</kbd> / <kbd>Enter</kbd> | Does not work (when the band has focus) |
| Bypass / restore the chosen insert | <kbd>B</kbd> | <kbd>B</kbd> | Does not work (one letter; mixer) |
| Move a fader / send (finely) | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Shift</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Shift</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | Does not work (when it has focus) |
| Move the pan (in big steps) | <kbd>←</kbd> <kbd>→</kbd> / <kbd>Shift</kbd> <kbd>←</kbd> <kbd>→</kbd> | <kbd>←</kbd> <kbd>→</kbd> / <kbd>Shift</kbd> <kbd>←</kbd> <kbd>→</kbd> | Does not work (when it has focus) |
| Fader / pan / send back to the first value | <kbd>Home</kbd> | <kbd>Home</kbd> | Does not work (when it has focus) |
| Type a value into a fader / pan / send | <kbd>Enter</kbd> | <kbd>Enter</kbd> | Does not work (when it has focus) |
| In the shelf (effects): favorite mark | <kbd>F</kbd> | <kbd>F</kbd> | Does not work |
| Close a dialog / go back one | <kbd>Esc</kbd> | <kbd>Esc</kbd> | Does not work (on the second page, to the first page) |
| Save as | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>S</kbd> | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>S</kbd> | Works |
| Quit meloet | <kbd>Mod</kbd> <kbd>Q</kbd> | <kbd>Mod</kbd> <kbd>Q</kbd> | Works |
| To the menu bar | <kbd>F10</kbd> | <kbd>F10</kbd> | Works |
| Zoom the piano roll in / out vertically | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>＋</kbd> <kbd>ホイール</kbd> | <kbd>Mod</kbd> <kbd>Shift</kbd> <kbd>＋</kbd> <kbd>ホイール</kbd> | Works |
| New song / open | <kbd>Mod</kbd> <kbd>N</kbd> / <kbd>Mod</kbd> <kbd>O</kbd> | <kbd>Mod</kbd> <kbd>N</kbd> / <kbd>Mod</kbd> <kbd>O</kbd> | Works |
| In the tempo / time signature box, change by 1 / by 10 | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Shift</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Shift</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | Works |
| Note list: type into a field / go to the field on the right | <kbd>Enter</kbd> / <kbd>Tab</kbd> | <kbd>Enter</kbd> / <kbd>Tab</kbd> | Works |
| Move clips or notes while duplicating them | <kbd>Alt</kbd> <kbd>＋</kbd> <kbd>ドラッグ</kbd> | <kbd>Option</kbd> <kbd>＋</kbd> <kbd>ドラッグ</kbd> | Works |
| While dragging, do not snap to the grid | <kbd>Shift</kbd> <kbd>＋</kbd> <kbd>ドラッグ</kbd> | <kbd>Shift</kbd> <kbd>＋</kbd> <kbd>ドラッグ</kbd> | Works |
| Cancel the drag and go back to before the move | <kbd>Esc</kbd> | <kbd>Esc</kbd> | Works |
| Delete | <kbd>Delete</kbd> | <kbd>Delete</kbd> | Does not work (deletes the text in the box) |
| Quantize | <kbd>Q</kbd> | <kbd>Q</kbd> | Does not work |
| Chosen clip: next take / previous take | <kbd>T</kbd> / <kbd>Shift</kbd> <kbd>T</kbd> | <kbd>T</kbd> / <kbd>Shift</kbd> <kbd>T</kbd> | Does not work |
| Tools: select / draw / erase | <kbd>1</kbd> / <kbd>2</kbd> / <kbd>3</kbd> | <kbd>1</kbd> / <kbd>2</kbd> / <kbd>3</kbd> | Does not work |
| Join the chosen keyboard clips of one track (round 10) | <kbd>Mod</kbd> <kbd>J</kbd> | <kbd>Mod</kbd> <kbd>J</kbd> | Works |
| Split (the chosen clip; if there is none, the clip of the selected track that the play line crosses. Keyboard clips too) | <kbd>S</kbd> | <kbd>S</kbd> | Does not work |
| Mute / solo the selected track | <kbd>Shift</kbd> <kbd>M</kbd> / <kbd>Shift</kbd> <kbd>S</kbd> | <kbd>Shift</kbd> <kbd>M</kbd> / <kbd>Shift</kbd> <kbd>S</kbd> | Does not work |
| Move notes by a semitone / an octave | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Shift</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Shift</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | Does not work (used for the text in the box) |
| Move notes by one grid step | <kbd>←</kbd> <kbd>→</kbd> | <kbd>←</kbd> <kbd>→</kbd> | Does not work (used for the text in the box) |
| Zoom in / out horizontally | <kbd>Mod</kbd> <kbd>+</kbd> / <kbd>Mod</kbd> <kbd>−</kbd> | <kbd>Mod</kbd> <kbd>+</kbd> / <kbd>Mod</kbd> <kbd>−</kbd> | Works |
| Screen scale 100 / 125 % | <kbd>Mod</kbd> <kbd>Alt</kbd> <kbd>1</kbd> / <kbd>2</kbd> | <kbd>Mod</kbd> <kbd>Option</kbd> <kbd>1</kbd> / <kbd>2</kbd> | Works |
| To the next part / to the previous part | <kbd>Tab</kbd> / <kbd>Shift</kbd> <kbd>Tab</kbd> | <kbd>Tab</kbd> / <kbd>Shift</kbd> <kbd>Tab</kbd> | Works |
| Keep what was just played (「さっきのを残す」; round 12) | <kbd>Shift</kbd> <kbd>R</kbd> | <kbd>Shift</kbd> <kbd>R</kbd> | Does not work |
| Play with the PC keyboard on / off (「キーで弾く」; round 12) | <kbd>Mod</kbd> <kbd>K</kbd> | <kbd>Mod</kbd> <kbd>K</kbd> | Works |
| Repeat the chosen clips (a box of digits; round 12) | <kbd>Mod</kbd> <kbd>R</kbd> | <kbd>Mod</kbd> <kbd>R</kbd> | Works |
| Close 「書き出しました」 (round 12) | <kbd>Esc</kbd> / <kbd>Enter</kbd> | <kbd>Esc</kbd> / <kbd>Enter</kbd> | Works |
| In the metronome window: move the volume by 1 dB / tap the tempo on 「タップ」 (round 12) | <kbd>←</kbd> <kbd>→</kbd> / <kbd>Space</kbd> <kbd>Enter</kbd> | <kbd>←</kbd> <kbd>→</kbd> / <kbd>Space</kbd> <kbd>Enter</kbd> | Works (when it has focus) |
| In the shelf's 「パターン」: choose a row / play or stop it / place it (round 12) | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Space</kbd> / <kbd>Enter</kbd> | <kbd>↑</kbd> <kbd>↓</kbd> / <kbd>Space</kbd> / <kbd>Enter</kbd> | Works |
