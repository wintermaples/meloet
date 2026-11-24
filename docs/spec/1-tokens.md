# 1 List of values

Colors are called by name. The grays are 11 steps in which R, G and B are the same number; no other gray is used, in any theme (user:2026-11-24; see “Themes”). The keys and rows of the keyboards and the piano roll have names of their own (`color.key.*`, `color.row.*`): in the dark themes they are grays of the 11 steps, in the light themes some of them lie between the steps (user:2026-11-23). Lines (`color.line.*`) and the grounds of meters (`color.meter.ground`, `color.meter.ground.mic`) have names of their own too (user:2026-11-23): the lines lie between the steps, at values chosen by eye (they are not checked for contrast, user:2026-11-24; see “Contrast”). Color is painted only on places that have a meaning (marks, notes, on buttons); large faces stay gray.

## Colors

|  | Name | Value | R=G=B | Used for |
|---|---|---|---|---|
|  | `color.gray.0` | `#000000` | 0 | Background of readings, input meter, status line |
|  | `color.gray.1` | `#161616` | 22 | Work area (arrange, piano roll background) |
|  | `color.gray.2` | `#1E1E1E` | 30 | Open band |
|  | `color.gray.3` | `#252525` | 37 | Panels, track heads, shelf |
|  | `color.gray.4` | `#2C2C2C` | 44 | Folded headings, button face, selected row |
|  | `color.gray.5` | `#383838` | 56 | Border of disabled parts, of the M, S, ● toggles and of empty insert slots; the drum row name column |
|  | `color.gray.6` | `#606060` | 96 | Strong divider lines, button border |
|  | `color.gray.7` | `#707070` | 112 | Disabled things, the position before aligning |
|  | `color.gray.8` | `#969696` | 150 | Labels |
|  | `color.gray.9` | `#DADADA` | 218 | Body text, line drawings |
|  | `color.gray.10` | `#F5F5F5` | 245 | Emphasis, numbers |
|  | `color.accent` | `#FFA92B` |  | Things that are live now, as a fill or a thick mark: ordinary notes and drum squares of the piano roll (as strong as their velocity), the face of an on button and toggle, pressed keys |
|  | `color.accent.ink` | `#FFA92B` |  | The accent as text and as thin lines and small marks (1–2 px): selection frames, loop, play head, 「オン」 and other on text, the band chosen with the keyboard, the line.mark of selected rows, choice dots, progress and slider fills (user:2026-11-24) |
|  | `color.accent.fg` | `#000000` | 0 | Text and marks on an accent fill (the on buttons and toggles) (user:2026-11-24) |
|  | `color.rec` | `#FF5852` |  | Recording, record arm, broken things |
|  | `color.meter.ok` | `#2BE263` |  | Meter up to −3 dB (70 % of the length) |
|  | `color.meter.hot` | `#FBDA10` |  | Meter above −3 dB |
|  | `color.meter.ground` | `#000000` | 0 | Ground of the meters (the two bars, also when there is no sound) |
|  | `color.meter.ground.mic` | `#2C2C2C` | 44 | Ground of the mic meter |
|  | `color.meter.clip` | `#FF5852` |  | The clip mark at the loud end of a meter (2px for 2 seconds) |
|  | `color.meter.peak` | `#F5F5F5` | 245 | The line of the 1-second peak on the mic meter |
|  | `color.track.1` | `#09E0DB` |  | Track color 1 (only the 4px-wide strip and the top 2px edge of a clip) |
|  | `color.track.2` | `#3CB5FE` |  | Track color 2 |
|  | `color.track.3` | `#7584FE` |  | Track color 3 |
|  | `color.track.4` | `#BA66E9` |  | Track color 4 |
|  | `color.track.5` | `#FFA3CF` |  | Track color 5. From the 6th track, back to 1 |
|  | `color.tip.bg / color.tip.fg` | `#F5F5F5` | 245 | The term tip background is gray.10, the text is gray.0 |
|  | `color.key.white` | `#383838` | 56 | White keys of the piano roll keyboard |
|  | `color.key.black` | `#000000` | 0 | Black keys of the piano roll keyboard |
|  | `color.key.meter.white` | `#252525` | 37 | White keys of the keyboard picture (input meter, sound settings) |
|  | `color.key.meter.black` | `#000000` | 0 | Black keys of the keyboard picture |
|  | `color.row.white` | `#212121` | 33 | White-key rows of the piano roll. Drum rows, in turns with row.black |
|  | `color.row.black` | `#1A1A1A` | 26 | Black-key rows of the piano roll. Drum rows, in turns with row.white |
|  | `color.row.out` | `#000000` | 0 | Rows that cannot be written while scale lock is on, with nothing more drawn on them (chapter 2, “Note”; user:2026-11-24) |
|  | `color.line.bar` | `#686868` | 104 | Bar lines (piano roll, drum rows, CC lanes; the arrange when beat lines are shown too) |
|  | `color.line.beat` | `#4E4E4E` | 78 | Beat lines; the bar lines of the arrange when they are shown alone |
|  | `color.line.grid` | `#2C2C2C` | 44 | Fine grid lines (1/16 and finer), on the drum rows too |
|  | `color.line.thin` | `#464646` | 70 | Thin dividers and borders of bands, panels, headings and track rows; the lines of the keyboard picture |
|  | `color.line.row` | `#333333` | 51 | Lines between the rows of lists, menus, dialogs and panels |

The values above are the 「ダーク」 (dark) theme. All six themes were made again from the owner's choice 「B」 among three candidates (user:2026-11-24, the owner's request: give up WCAG AA, more vivid and beautiful, with a minimum contrast kept): deeper blacks, more saturated accent, meters and track colors, and stronger beat lines. Before that, the dark theme had gray.1 `#1A1A1A`, gray.2 `#212121`, gray.3 `#282828`, gray.4 `#2F2F2F`, gray.5 `#3C3C3C`, gray.6 `#686868`, gray.7 `#717171`, gray.8 `#9A9A9A`, gray.9 `#D6D6D6`, gray.10 `#F2F2F2`, accent `#FFB000`, rec `#FF6158`, meter.ok `#4CD964`, meter.hot `#FFD60A`, line.beat `#3E3E3E`, and the five track colors `#69E4E4`, `#5ABDF2`, `#6F8BED`, `#9561C7`, `#FFD9F1`. Earlier still, gray.0 to gray.7, rec and track.4 had been changed so that the pairs below reach their contrast (user:2026-11-23, the owner's request: the on and off rows of scale lock in the piano roll and the lines were hard to tell apart). Before: gray.0 `#0A0A0A`, gray.1 `#121212`, gray.2 `#161616`, gray.3 `#1A1A1A`, gray.4 `#222222`, gray.5 `#2C2C2C`, gray.6 `#3A3A3A`, gray.7 `#5E5E5E`, rec `#FF3B30`, track.4 `#8C54C3`. The mock boards are still drawn with the values from before.

The key and row names were added for the keyboards and the piano roll rows (user:2026-11-23, the owner's request: in the light themes the black keys and their rows had become the lightest). In the dark theme they have the values of the grays those places used before: key.white gray.5, key.black gray.0, key.meter.white gray.3, key.meter.black gray.0, row.white gray.2, row.black gray.1, row.out gray.0. The drum rows were gray.2 and gray.3 in turns, and the thin grid lines (gray.3) could not be seen on every second row; they now take row.white and row.black in turns.

The line and meter ground names were added (user:2026-11-23, the owner's choice 「B plan 2」: names only for lines, so that each theme gives lines their own strength without changing the faces). Before, the lines used grays: bar lines gray.6, beat lines gray.4, fine grid lines gray.3 (in the CC lanes bar lines gray.4 and other lines gray.2; in the arrange, bar lines alone gray.4), thin dividers gray.5, lines between rows gray.4, the meter grounds gray.0 and gray.4. gray.6 was `#4A4A4A`; the button borders were made a little stronger (user:2026-11-23). The usable M, S, ● toggles had a gray.6 border when off; it is gray.5 now, so that the on border keeps its 3:1 (see “Contrast”).

## Themes

There are 6 themes: 「ダーク」 `dark`, 「ライト」 `light`, 「桜 ・ ダーク」 `pinkDark`, 「桜 ・ ライト」 `pinkLight`, 「ライム ・ ダーク」 `mintDark`, 「ライム ・ ライト」 `mintLight` (user:2026-11-23; the pink and mint themes became sakura and lime, and their names changed, user:2026-11-24; the ids stay, so saved settings keep working). Every theme gives a value to every color name above, and the names keep their role in every theme: the themes differ only in values, never in what is drawn where. In the light themes the brightness runs the other way: gray.0 is the lightest ground and gray.10 the darkest text, so gray.0 to gray.4 are still the grounds from the back to the front and gray.7 to gray.10 still the text from weak to strong. No theme uses brownish colors (user:2026-11-23, 「茶色っぽい色はNG(特にライトモード)」).

- Sakura and lime (user:2026-11-24): their grounds, text grays, lines, keys and rows are exactly those of `dark` (`pinkDark`, `mintDark`) and `light` (`pinkLight`, `mintLight`), with no tint. Only the accent, and what must change with it, is theirs: a soft, slightly cool cherry-blossom pink and a fresh yellow-green lime.
- `color.accent` is the bright accent, for fills and thick marks. It is the same in the dark and light theme of a pair: yellow-orange in dark, a clear blue in light, the same sakura pink in pinkDark and pinkLight, the same lime in mintDark and mintLight (user:2026-11-24: the owner does not want a deep rose or a deep leaf green in the light themes).
- `color.accent.ink` is the accent as text, thin lines and small marks. Where the bright accent already reaches the contrast of text, it is the same value (dark, light, pinkDark, mintDark); in pinkLight and mintLight it is a deep rose and a deep green that reach it on the light grounds. A bright accent fill on a light ground always has an accent.ink border, frame or mark next to it, so that the thing is seen (see “Contrast”).
- `color.accent.fg` is the text on an accent fill: black in the dark themes, gray.10 in pinkLight and mintLight, white in light (on the blue).
- `color.rec` is red in every theme. Only its brightness follows the ground; in the sakura themes it leans a little to orange, so it is not taken for the pink.
- Track colors: in the dark themes cyan, sky blue, blue, violet and pink; in the light themes teal, sky blue, violet, magenta and green, dark enough for the light grounds (no orange, which turns brown there). Where a track color would look like the accent, it is changed for another hue: track.5 is apricot in pinkDark, track.4 blue in pinkLight, track.5 blue in mintLight (user:2026-11-24).
- `color.tip.bg` is gray.10 and `color.tip.fg` is gray.0 in every theme, so the term tip is always the opposite of the screen.
- `color.meter.ok` and `color.meter.hot` are the same in every theme (user:2026-11-23). In the light themes the meters lie on a dark ground: `color.meter.ground` and `color.meter.ground.mic` are gray.9 of the theme there (user:2026-11-23). In the dark themes they are gray.0 and gray.4. `color.meter.clip` and `color.meter.peak` are rec and gray.10 of the theme in the dark themes, and the dark values of rec and gray.10 in the light themes.
- Keys and rows (user:2026-11-23): in every theme a white key is lighter than a black key and a white-key row lighter than a black-key row, as on a real keyboard, and the rows that cannot be written are the darkest rows. In the light themes the white keys and the white-key rows are gray.0, the black keys gray.6, the black-key rows lie between gray.0 and gray.1, and the rows that cannot be written lie between gray.3 and gray.4.
- Lines: beat lines are stronger than before (user:2026-11-24). Lines are not checked for contrast any more; their values are chosen by eye, bar lines the strongest.
- Values are in `ui/src/design/themes.ts`; `dark` is `colors` of `ui/src/design/tokens.ts`.

| Name | `dark` | `light` | `pinkDark` | `pinkLight` | `mintDark` | `mintLight` |
|---|---|---|---|---|---|---|
| `color.gray.0` | `#000000` | `#FFFFFF` | `#000000` | `#FFFFFF` | `#000000` | `#FFFFFF` |
| `color.gray.1` | `#161616` | `#F0F0F0` | `#161616` | `#F0F0F0` | `#161616` | `#F0F0F0` |
| `color.gray.2` | `#1E1E1E` | `#E7E7E7` | `#1E1E1E` | `#E7E7E7` | `#1E1E1E` | `#E7E7E7` |
| `color.gray.3` | `#252525` | `#DEDEDE` | `#252525` | `#DEDEDE` | `#252525` | `#DEDEDE` |
| `color.gray.4` | `#2C2C2C` | `#D6D6D6` | `#2C2C2C` | `#D6D6D6` | `#2C2C2C` | `#D6D6D6` |
| `color.gray.5` | `#383838` | `#C4C4C4` | `#383838` | `#C4C4C4` | `#383838` | `#C4C4C4` |
| `color.gray.6` | `#606060` | `#B0B0B0` | `#606060` | `#B0B0B0` | `#606060` | `#B0B0B0` |
| `color.gray.7` | `#707070` | `#7C7C7C` | `#707070` | `#7C7C7C` | `#707070` | `#7C7C7C` |
| `color.gray.8` | `#969696` | `#606060` | `#969696` | `#606060` | `#969696` | `#606060` |
| `color.gray.9` | `#DADADA` | `#2E2E2E` | `#DADADA` | `#2E2E2E` | `#DADADA` | `#2E2E2E` |
| `color.gray.10` | `#F5F5F5` | `#0F0F0F` | `#F5F5F5` | `#0F0F0F` | `#F5F5F5` | `#0F0F0F` |
| `color.accent` | `#FFA92B` | `#0150DE` | `#FF92B5` | `#FF92B5` | `#B7F321` | `#B7F321` |
| `color.accent.ink` | `#FFA92B` | `#0150DE` | `#FF92B5` | `#B30275` | `#B7F321` | `#326A02` |
| `color.accent.fg` | `#000000` | `#FFFFFF` | `#000000` | `#0F0F0F` | `#000000` | `#0F0F0F` |
| `color.rec` | `#FF5852` | `#B71D0F` | `#FF5A3A` | `#B32700` | `#FF5852` | `#B71D0F` |
| `color.meter.ok` | `#2BE263` | `#2BE263` | `#2BE263` | `#2BE263` | `#2BE263` | `#2BE263` |
| `color.meter.hot` | `#FBDA10` | `#FBDA10` | `#FBDA10` | `#FBDA10` | `#FBDA10` | `#FBDA10` |
| `color.meter.ground` | `#000000` | `#2E2E2E` | `#000000` | `#2E2E2E` | `#000000` | `#2E2E2E` |
| `color.meter.ground.mic` | `#2C2C2C` | `#2E2E2E` | `#2C2C2C` | `#2E2E2E` | `#2C2C2C` | `#2E2E2E` |
| `color.meter.clip` | `#FF5852` | `#FF5852` | `#FF5A3A` | `#FF5A3A` | `#FF5852` | `#FF5852` |
| `color.meter.peak` | `#F5F5F5` | `#F5F5F5` | `#F5F5F5` | `#F5F5F5` | `#F5F5F5` | `#F5F5F5` |
| `color.track.1` | `#09E0DB` | `#048681` | `#09E0DB` | `#048681` | `#09E0DB` | `#048681` |
| `color.track.2` | `#3CB5FE` | `#0481B3` | `#3CB5FE` | `#0481B3` | `#3CB5FE` | `#0481B3` |
| `color.track.3` | `#7584FE` | `#8F45DD` | `#7584FE` | `#8F45DD` | `#7584FE` | `#8F45DD` |
| `color.track.4` | `#BA66E9` | `#CC2880` | `#BA66E9` | `#3959DA` | `#BA66E9` | `#CC2880` |
| `color.track.5` | `#FFA3CF` | `#068B3F` | `#FEC675` | `#068B3F` | `#FFA3CF` | `#3959DA` |
| `color.tip.bg` | `#F5F5F5` | `#0F0F0F` | `#F5F5F5` | `#0F0F0F` | `#F5F5F5` | `#0F0F0F` |
| `color.tip.fg` | `#000000` | `#FFFFFF` | `#000000` | `#FFFFFF` | `#000000` | `#FFFFFF` |
| `color.key.white` | `#383838` | `#FFFFFF` | `#383838` | `#FFFFFF` | `#383838` | `#FFFFFF` |
| `color.key.black` | `#000000` | `#B0B0B0` | `#000000` | `#B0B0B0` | `#000000` | `#B0B0B0` |
| `color.key.meter.white` | `#252525` | `#FFFFFF` | `#252525` | `#FFFFFF` | `#252525` | `#FFFFFF` |
| `color.key.meter.black` | `#000000` | `#B0B0B0` | `#000000` | `#B0B0B0` | `#000000` | `#B0B0B0` |
| `color.row.white` | `#212121` | `#FFFFFF` | `#212121` | `#FFFFFF` | `#212121` | `#FFFFFF` |
| `color.row.black` | `#1A1A1A` | `#F5F5F5` | `#1A1A1A` | `#F5F5F5` | `#1A1A1A` | `#F5F5F5` |
| `color.row.out` | `#000000` | `#DDDDDD` | `#000000` | `#DDDDDD` | `#000000` | `#DDDDDD` |
| `color.line.bar` | `#686868` | `#8A8A8A` | `#686868` | `#8A8A8A` | `#686868` | `#8A8A8A` |
| `color.line.beat` | `#4E4E4E` | `#AAAAAA` | `#4E4E4E` | `#AAAAAA` | `#4E4E4E` | `#AAAAAA` |
| `color.line.grid` | `#2C2C2C` | `#D6D6D6` | `#2C2C2C` | `#D6D6D6` | `#2C2C2C` | `#D6D6D6` |
| `color.line.thin` | `#464646` | `#B0B0B0` | `#464646` | `#B0B0B0` | `#464646` | `#B0B0B0` |
| `color.line.row` | `#333333` | `#CDCDCD` | `#333333` | `#CDCDCD` | `#333333` | `#CDCDCD` |

## Contrast

The themes no longer aim at WCAG 2 AA (user:2026-11-24, the owner's request: 「WCAG AA はあきらめて、もっと鮮やかで綺麗に。ただ最低限のコントラストは保つ」). What is kept is the minimum below. Contrast is the WCAG 2 contrast ratio of the two colors. `tools/design/contrast.py` checks every theme against the list of pairs in `tools/design/contrast-pairs.json` (which color is drawn on which, and where) and fails if a pair is under its minimum. When a part starts to draw a color on a new color, add the pair there.

| Kind | Minimum | Pairs |
|---|---|---|
| text | 4.5 | Body text on its ground: gray.9, gray.10, accent.ink and rec on gray.0 to gray.4; gray.9 and gray.10 on gray.5; tip.fg on tip.bg; accent.fg on accent (the on buttons) |
| weak | 3 | Weak text: labels, units, key hints, note names (gray.8 as text) on gray.0 to gray.4 (user:2026-11-24) |
| ui | 3 | A mark with a meaning against the color next to it: the frame of notes and drum squares (accent.ink) and selected notes (gray.10) on the rows, the original position frame, play head, record ring, track colors, meter colors on the meter grounds, keyboard focus line, on marks |
| state | 3 | The on color against the off color of the same mark, where only color shows the change: accent.ink against gray.6 (the border of an on button) and gray.5 (M, S toggles), rec against gray.5 (●), gray.10 against gray.7 (star, plugged-in mark), accent.ink against the white and black keys (the mark of a pressed key) |
| disabled | 2 | Text of 「使えない」 (disabled) parts |
| fill | 1.2 | Large neighbouring fills that carry a meaning: the rows that can be written (row.white, row.black) against the rows outside the scale (row.out) while scale lock is on |
| faint | 1.08 | The faintest decoration: neighbouring fills (white-key and black-key rows, white and black keys, panels and bands) |
| line | — | Lines, borders and dividers (`color.line.*`, gray.6 button and clip borders, line.thin): not checked (user:2026-11-24). Listed so that they stay in sight |

- Text sizes are all under the WCAG large size (18.66 px bold, 24 px); body text uses 4.5 and weak text 3.
- The bright accent (`color.accent`) is a fill. A fill needs 3:1 against its ground, or an accent.ink border, frame or mark that has 3:1. The notes and drum squares have an accent.ink frame, the on buttons and toggles an accent.ink border, a pressed key an accent.ink mark on its left (user:2026-11-24).
- Where the words or a border also change (「オン」 and 「オフ」, the border of an on button), the on and off text colors need no ratio between them. Accent and rec, and accent and the track colors, are told apart by hue.
- The drum row name that sounds (accent.ink) or is being recorded (rec) on the gray.5 name column is counted as a mark (3), not as text: the same name is shown in gray.9 or gray.10 just before.
- Scale lock: 3:1 between the rows cannot be reached by values alone without making the whole work area light. The values give 1.2 or more. The rows outside the scale get nothing more on them (the thin diagonal lines they had were removed, user:2026-11-24); the cursor and the status line still say that they cannot be written (chapter 3).
- Meters (user:2026-11-23): meter.ok and meter.hot keep their dark values in the light themes, and the dark meter grounds there (meter.ground, meter.ground.mic) give them 3:1. The clip mark (meter.clip) and the peak line of the mic meter (meter.peak) take the dark values of rec and gray.10 in the light themes, so they have 3:1 on those grounds too.

## Text

Japanese is IBM Plex Sans JP; numbers, positions and values are IBM Plex Mono (fixed-width digits). There are only 3 sizes. No text smaller than 12px is used, for Japanese or for numbers. The numbers in the table are px at 100 % / 125 %.

| Name | Size | Line height | Weights allowed at that size | Used for |
|---|---|---|---|---|
| `text.s` | 12 / 15 | 16 / 20 | 400 / 500 / 600 | Body text, labels, buttons, numbers in tables. The smallest size, for Japanese and for numbers |
| `text.m` | 16 / 20 | 20 / 25 | 400 / 500 | Numbers of readings (position, time, tempo, velocity, dB) |
| `text.l` | 20 / 25 | 24 / 30 | 600 | Document headings, the count-in number. Dialog headings do not use this; they use text.s at 600 |

## Space

The spaces and sizes in the tables below are built only from multiples of 4. At 125 % they are always whole numbers. Small sizes inside parts (icon 10, mark 6, bar handle 6×12, window mark 10, and so on) need not be multiples of 4. Multiply them by the scale and round (.5 rounds up). Example: 10 → 13, 6 → 8.

| Name | 100 / 125 % | Used for |
|---|---|---|
| `space.1` | 4 / 5 | Between buttons, between an icon and text |
| `space.2` | 8 / 10 | Between parts in a row, inside a panel |
| `space.3` | 12 / 15 | Left and right inside a band, gaps in a toolbar |
| `space.4` | 16 / 20 | Between readings, inside a dialog |
| `space.5` | 20 / 25 | Between readings of the input meter |
| `space.6` | 24 / 30 | One indent step, the height of a band tab |

## Sizes

| Name | 100 / 125 % | Used for |
|---|---|---|
| `size.menubar` | 24 / 30 | Menu bar |
| `size.transport` | 48 / 60 | Transport band |
| `size.meter.full` | 52 / 65 | Input meter, full |
| `size.meter.slim` | 24 / 30 | Input meter, slim |
| `size.meter.name` | 108 / 135 | Name field of the input meter (148 for a long name) |
| `size.meter.name.slim` | 176 / 220 | Upper limit of the name when slim (128 in a narrow window) |
| `size.bandrow` | 24 / 30 | Band tab row, where the folded headings line up |
| `size.status` | 24 / 30 | Status line |
| `size.row` | 24 / 30 | Headings, table rows, inspector rows, buttons |
| `size.row.s` | 20 / 25 | Rows of the note list, the M, S, ● toggles |
| `size.note` | 12 / 15 | One row of the piano roll (one semitone) |
| `size.toolbar` | 32 / 40 | Toolbar of a band, dialog headings |
| `size.track` | 64 / 80 | Track row (160 when editing a vocal) |
| `size.lane` | 48 / 60 | Automation band |
| `size.lane.vel` | 72 / 90 | Velocity band |
| `size.overview` | 40 / 50 | The shrunk arrange |
| `size.col.head` | 220 / 275 | Width of the track heads in the arrange |
| `size.col.keys` | 104 / 130 | Width of the keyboard in the piano roll |
| `size.panel.list` | 216 / 270 | Note list |
| `size.panel.insp` | 240 / 300 | Selected notes |
| `size.shelf` | 320 / 400 | Shelf |
| `size.tab.v` | 24 / 30 | Vertical headings of the folded left and right panels and of the shelf |
| `size.strip` | 108 / 135 | One mixer strip (124 for the master) |

## Lines and corners

| Name | Value | Rule |
|---|---|---|
| `line.hair` | 1 | Dividers, borders, grid, play position. At every scale it stays one screen dot (physical 1px) and never gets thicker. Bar lines are two line.hair side by side (2 screen dots at every scale; the second dot on the right) in the piano roll, the drum rows, the CC lanes and the arrange; beat and fine grid lines stay one (user:2026-11-24) |
| `line.mark` | 2 | Left of a selected row, top edge of an open band tab, top edge of a clip. 2 at 125 % |
| `line.track` | 4 | Track color strip. 5 at 125 % |
| `radius.0` | 0 | Panels, bands, rows, clips, notes, meters |
| `radius.1` | 1 | Buttons, input boxes, toggles, term tips. Stays 1 when scaled up |
| `radius.round` | 50 % | Only the record button and the pan knob |

## Rules for the scale

- 2 steps: 100 % and 125 % (user:2026-11-24). The same scale is applied to text, space and sizes. Lines follow the table above.
- When the width is not enough after scaling up, hide things in this order. (1) Transport: song name (leave a 28-wide ▾), CPU load, count-in (leave a 20-wide ▾ on the metronome), position labels. (2) Input meter when slim: modulation, pitch bend. (3) Band tabs: counts and current values. (4) Piano roll: start with the left and right panels folded.
- Names, keys, link marks, held notes, and the numbers of position and time are never removed at any scale.
- Example: in a 1280×800 window at 125 %, the piano roll shows 23 rows of notes. With the velocity band folded, 29 rows.
- The narrow form of the transport after the position labels are hidden: the position box is 32 high, space.2 inside, space.3 between bar and time. The master is 100 wide (space.1 between 「マスター」 and the value).
- The key hints written next to buttons (Space, R, L, C, 1, 2, 3, and so on) are hidden before anything else when the width is not enough. At 100 % in a 1280-wide window, the keys of the transport are not shown. The keys of the toolbar are shown.
- When the input meter is full, the value of 「弾くと鳴る先」 shows the track and instrument names, like 「1 ピアノ ・ グランドピアノ」. If the width is not enough, hide in this order: modulation → pitch bend → the keyboard picture to thin keys (width 8) → the instrument name of the target (show only 「1 ピアノ」) → the 「弾くと鳴る先」 label → sustain. Never remove the link mark, the name, the keyboard picture, held notes, velocity, and the target track. During audition, never remove 「ためし弾き ・ （音源の名前）」.
- While a hint text is shown at the right end of the input meter (no keyboard, no output yet), hide in this order: modulation → pitch bend → the keyboard picture to thin keys → the second sentence of the hint (「下でえらんでください」「つながなくても、マウスで音を書けます。」) → sustain → cut the end of the remaining first sentence to 「…」. The hint says what to do now, so it stays longer than the readings.
- When the vertical headings at the right rail do not fit vertically, hide the counts and names first. Keep the heading names and keys.
- The smallest window size, in px at 100 %, is 1024×640 (user:2026-11-24). For the window itself: 1024×640 at 100 % and 1280×800 at 125 %. The window cannot be made smaller than this.
- The mixer works from 700. In windows lower than that, the mixer toolbar does not move, and only the strips scroll vertically. At first the bottom (faders and meters) is shown; scrolling up shows the inserts.
- A scale at which even the smallest window does not fit on the screen is shown with the 「使えない」 (disabled) look, with the reason 「この画面には入りません」.
