# 1 List of values

Colors are called by name. The grays are 11 steps in which R, G and B are the same number; no other gray is used (the pink and mint themes tint them; see “Themes”). The keys and rows of the keyboards and the piano roll have names of their own (`color.key.*`, `color.row.*`): in the dark themes they are grays of the 11 steps, in the light themes some of them lie between the steps (user:2026-11-23). Lines (`color.line.*`), the diagonal lines on rows that cannot be written (`color.row.hatch`) and the grounds of meters (`color.meter.ground`, `color.meter.ground.mic`) have names of their own too (user:2026-11-23): the lines and the diagonal lines lie between the steps, at the weakest value that reaches their contrast (see “Contrast”). Color is painted only on small places that have a meaning; large areas stay gray.

## Colors

|  | Name | Value | R=G=B | Used for |
|---|---|---|---|---|
|  | `color.gray.0` | `#000000` | 0 | Background of readings, input meter, status line |
|  | `color.gray.1` | `#1A1A1A` | 26 | Work area (arrange, piano roll background) |
|  | `color.gray.2` | `#212121` | 33 | Open band |
|  | `color.gray.3` | `#282828` | 40 | Panels, track heads, shelf |
|  | `color.gray.4` | `#2F2F2F` | 47 | Folded headings, button face, selected row |
|  | `color.gray.5` | `#3C3C3C` | 60 | Border of disabled parts, of the M, S, ● toggles and of empty insert slots; the drum row name column |
|  | `color.gray.6` | `#686868` | 104 | Strong divider lines, button border |
|  | `color.gray.7` | `#717171` | 113 | Disabled things, the position before aligning |
|  | `color.gray.8` | `#9A9A9A` | 154 | Labels |
|  | `color.gray.9` | `#D6D6D6` | 214 | Body text, line drawings |
|  | `color.gray.10` | `#F2F2F2` | 242 | Emphasis, numbers |
|  | `color.accent` | `#FFB000` |  | Things that are live now (pressed keys, selection, loop, on, play head, the band chosen with the keyboard) |
|  | `color.rec` | `#FF6158` |  | Recording, record arm, broken things |
|  | `color.meter.ok` | `#4CD964` |  | Meter up to −3 dB (70 % of the length) |
|  | `color.meter.hot` | `#FFD60A` |  | Meter above −3 dB |
|  | `color.meter.ground` | `#000000` | 0 | Ground of the meters (the two bars, also when there is no sound) |
|  | `color.meter.ground.mic` | `#2F2F2F` | 47 | Ground of the mic meter |
|  | `color.meter.clip` | `#FF6158` |  | The clip mark at the loud end of a meter (2px for 2 seconds) |
|  | `color.meter.peak` | `#F2F2F2` |  | The line of the 1-second peak on the mic meter |
|  | `color.track.1` | `#69E4E4` |  | Track color 1 (only the 4px-wide strip and the top 2px edge of a clip) |
|  | `color.track.2` | `#5ABDF2` |  | Track color 2 |
|  | `color.track.3` | `#6F8BED` |  | Track color 3 |
|  | `color.track.4` | `#9561C7` |  | Track color 4 |
|  | `color.track.5` | `#FFD9F1` |  | Track color 5. From the 6th track, back to 1 |
|  | `color.tip.bg / color.tip.fg` | `#F2F2F2` |  | The term tip background is gray.10, the text is gray.0 |
|  | `color.key.white` | `#3C3C3C` | 60 | White keys of the piano roll keyboard |
|  | `color.key.black` | `#000000` | 0 | Black keys of the piano roll keyboard |
|  | `color.key.meter.white` | `#282828` | 40 | White keys of the keyboard picture (input meter, sound settings) |
|  | `color.key.meter.black` | `#000000` | 0 | Black keys of the keyboard picture |
|  | `color.row.white` | `#212121` | 33 | White-key rows of the piano roll. Drum rows, in turns with row.black |
|  | `color.row.black` | `#1A1A1A` | 26 | Black-key rows of the piano roll. Drum rows, in turns with row.white |
|  | `color.row.out` | `#000000` | 0 | Rows that cannot be written while scale lock is on, with row.hatch diagonal lines (chapter 2, “Note”) |
|  | `color.row.hatch` | `#5A5A5A` | 90 | The diagonal lines on rows that cannot be written (12 apart, with smooth edges; user:2026-11-23) |
|  | `color.line.bar` | `#6B6B6B` | 107 | Bar lines (piano roll, drum rows, CC lanes; the arrange when beat lines are shown too) |
|  | `color.line.beat` | `#3E3E3E` | 62 | Beat lines; the bar lines of the arrange when they are shown alone |
|  | `color.line.grid` | `#2F2F2F` | 47 | Fine grid lines (1/16 and finer), on the drum rows too |
|  | `color.line.thin` | `#4A4A4A` | 74 | Thin dividers and borders of bands, panels, headings and track rows; the lines of the keyboard picture |
|  | `color.line.row` | `#353535` | 53 | Lines between the rows of lists, menus, dialogs and panels |

The values above are the 「ダーク」 (dark) theme. gray.0 to gray.7, rec and track.4 were changed so that the pairs below reach their contrast (user:2026-11-23, the owner's request: the on and off rows of scale lock in the piano roll and the lines were hard to tell apart). Before: gray.0 `#0A0A0A`, gray.1 `#121212`, gray.2 `#161616`, gray.3 `#1A1A1A`, gray.4 `#222222`, gray.5 `#2C2C2C`, gray.6 `#3A3A3A`, gray.7 `#5E5E5E`, rec `#FF3B30`, track.4 `#8C54C3`. The mock boards are still drawn with the values from before.

The key and row names were added for the keyboards and the piano roll rows (user:2026-11-23, the owner's request: in the light themes the black keys and their rows had become the lightest). In the dark theme they have the values of the grays those places used before: key.white gray.5, key.black gray.0, key.meter.white gray.3, key.meter.black gray.0, row.white gray.2, row.black gray.1, row.out gray.0. The drum rows were gray.2 and gray.3 in turns, and the thin grid lines (gray.3) could not be seen on every second row; they now take row.white and row.black in turns.

The line, hatch and meter ground names were added (user:2026-11-23, the owner's choice 「B plan 2」: names only for lines, so that each theme gives lines their own strength without changing the faces). Before, the lines used grays: bar lines gray.6, beat lines gray.4, fine grid lines gray.3 (in the CC lanes bar lines gray.4 and other lines gray.2; in the arrange, bar lines alone gray.4), thin dividers gray.5, lines between rows gray.4, the diagonal lines gray.7, the meter grounds gray.0 and gray.4. gray.6 was `#4A4A4A`; the button borders were made a little stronger (user:2026-11-23). The usable M, S, ● toggles had a gray.6 border when off; it is gray.5 now, so that the on border keeps its 3:1 (see “Contrast”).

## Themes

There are 6 themes (user:2026-11-23): 「ダーク」 `dark`, 「ライト」 `light`, 「ピンク ・ ダーク」 `pinkDark`, 「ピンク ・ ライト」 `pinkLight`, 「ミント ・ ダーク」 `mintDark`, 「ミント ・ ライト」 `mintLight`. Every theme gives a value to every color name above, and the names keep their role in every theme. In the light themes the brightness runs the other way: gray.0 is the lightest ground and gray.10 the darkest text, so gray.0 to gray.4 are still the grounds from the back to the front and gray.7 to gray.10 still the text from weak to strong. In the pink and mint themes the grays carry a light tint of their color (both grounds and text), at the same brightness as the plain grays, so they meet the same contrast.

- `color.accent` follows the theme: yellow in dark, a dark amber in light (dark enough for text on the light grounds), pink in pinkDark, a dark plum pink in pinkLight, mint green in mintDark, a dark green in mintLight.
- `color.rec` is red in every theme. Only its brightness follows the ground.
- Track colors: the same 5 hues, lighter or darker for the ground. Where a track color would look like the accent, it is changed for a yellow: track.5 in the pink themes, track.1 in the mint themes.
- `color.tip.bg` is gray.10 and `color.tip.fg` is gray.0 in every theme, so the term tip is always the opposite of the screen.
- `color.meter.ok` and `color.meter.hot` are the same in every theme (user:2026-11-23: in the light themes the darker hot looked too brown). In the light themes the meters lie on a dark ground: `color.meter.ground` and `color.meter.ground.mic` are gray.9 of the theme there (user:2026-11-23). In the dark themes they are gray.0 and gray.4, as the meters had. The marks on those grounds have names of their own too (user:2026-11-23): `color.meter.clip` (the clip mark) and `color.meter.peak` (the peak line of the mic meter) are rec and gray.10 of the theme in the dark themes, and the dark values of rec and gray.10 in the light themes.
- Keys and rows (user:2026-11-23): in every theme a white key is lighter than a black key and a white-key row lighter than a black-key row, as on a real keyboard, and the rows that cannot be written are the darkest rows. The dark themes keep the grays these places had. In the light themes the white keys and the white-key rows are gray.0, the black keys gray.6 (the darkest gray on which a pressed key, in accent, still has 3:1), the black-key rows lie between gray.0 and gray.1, and the rows that cannot be written are gray.2 (so the beat and sub-beat lines are still seen on them).
- Lines (user:2026-11-23): each theme gives every `color.line.*` name the weakest value, with the theme's tint, that reaches its contrast against every ground it is drawn on (see “Contrast”). `color.row.hatch` is the darkest such value that reaches 3:1 against row.out in the dark themes, and gray.7 in the light themes.
- gray.6 (user:2026-11-23): about 2.4:1 against gray.4 in `dark` and `mintDark`. In `pinkDark` it stops at about 1.85:1, the strongest value against which accent (the on border) still has 3:1. The light themes keep their gray.6, which is already at that limit.
- Values are in `ui/src/design/themes.ts`; `dark` is `colors` of `ui/src/design/tokens.ts`.

| Name | `dark` | `light` | `pinkDark` | `pinkLight` | `mintDark` | `mintLight` |
|---|---|---|---|---|---|---|
| `color.gray.0` | `#000000` | `#FFFFFF` | `#040404` | `#FBF9F9` | `#040404` | `#F8FAF9` |
| `color.gray.1` | `#1A1A1A` | `#EAEAEA` | `#211B1E` | `#EEE0E6` | `#171F1C` | `#DBE7E3` |
| `color.gray.2` | `#212121` | `#E1E1E1` | `#292225` | `#E8D6DE` | `#1D2723` | `#D0DFDA` |
| `color.gray.3` | `#282828` | `#D8D8D8` | `#30282C` | `#E2CCD6` | `#232E2A` | `#C4D7D1` |
| `color.gray.4` | `#2F2F2F` | `#CDCDCD` | `#392F33` | `#DBC1CD` | `#293531` | `#B6CEC6` |
| `color.gray.5` | `#3C3C3C` | `#BBBBBB` | `#483B41` | `#CFADBD` | `#33423D` | `#A0BDB3` |
| `color.gray.6` | `#686868` | `#A7A7A7` | `#65555C` | `#BF98AA` | `#5B7068` | `#89AB9F` |
| `color.gray.7` | `#717171` | `#7A7A7A` | `#806E77` | `#866F7A` | `#637971` | `#617C73` |
| `color.gray.8` | `#9A9A9A` | `#575757` | `#A7969E` | `#5D5056` | `#8D9E98` | `#485852` |
| `color.gray.9` | `#D6D6D6` | `#333333` | `#DBD5D7` | `#373134` | `#D3D7D6` | `#2D3432` |
| `color.gray.10` | `#F2F2F2` | `#111111` | `#F3F2F2` | `#121011` | `#F1F3F2` | `#0F1111` |
| `color.accent` | `#FFB000` | `#735000` | `#FF7FC8` | `#8E2879` | `#5CF0B4` | `#0B603F` |
| `color.rec` | `#FF6158` | `#A5261F` | `#FF685F` | `#9F251E` | `#FF6A61` | `#A0251E` |
| `color.meter.ok` | `#4CD964` | `#4CD964` | `#4CD964` | `#4CD964` | `#4CD964` | `#4CD964` |
| `color.meter.hot` | `#FFD60A` | `#FFD60A` | `#FFD60A` | `#FFD60A` | `#FFD60A` | `#FFD60A` |
| `color.meter.ground` | `#000000` | `#333333` | `#040404` | `#373134` | `#040404` | `#2D3432` |
| `color.meter.ground.mic` | `#2F2F2F` | `#333333` | `#392F33` | `#373134` | `#293531` | `#2D3432` |
| `color.meter.clip` | `#FF6158` | `#FF6158` | `#FF685F` | `#FF685F` | `#FF6A61` | `#FF6A61` |
| `color.meter.peak` | `#F2F2F2` | `#F2F2F2` | `#F3F2F2` | `#F3F2F2` | `#F1F3F2` | `#F1F3F2` |
| `color.track.1` | `#69E4E4` | `#007F7F` | `#69E4E4` | `#007B7B` | `#FFD54A` | `#8F6800` |
| `color.track.2` | `#5ABDF2` | `#1977B6` | `#5ABDF2` | `#1873B0` | `#5ABDF2` | `#1874B0` |
| `color.track.3` | `#6F8BED` | `#4668E1` | `#6F8BED` | `#4164E0` | `#6F8BED` | `#4165E0` |
| `color.track.4` | `#9561C7` | `#9551CD` | `#9664C8` | `#924CCC` | `#9765C9` | `#924DCC` |
| `color.track.5` | `#FFD9F1` | `#C23F87` | `#FFD54A` | `#8E6800` | `#FFD9F1` | `#BD3D84` |
| `color.tip.bg` | `#F2F2F2` | `#111111` | `#F3F2F2` | `#121011` | `#F1F3F2` | `#0F1111` |
| `color.tip.fg` | `#000000` | `#FFFFFF` | `#040404` | `#FBF9F9` | `#040404` | `#F8FAF9` |
| `color.key.white` | `#3C3C3C` | `#FFFFFF` | `#483B41` | `#FBF9F9` | `#33423D` | `#F8FAF9` |
| `color.key.black` | `#000000` | `#A7A7A7` | `#040404` | `#BF98AA` | `#040404` | `#89AB9F` |
| `color.key.meter.white` | `#282828` | `#FFFFFF` | `#30282C` | `#FBF9F9` | `#232E2A` | `#F8FAF9` |
| `color.key.meter.black` | `#000000` | `#A7A7A7` | `#040404` | `#BF98AA` | `#040404` | `#89AB9F` |
| `color.row.white` | `#212121` | `#FFFFFF` | `#292225` | `#FBF9F9` | `#1D2723` | `#F8FAF9` |
| `color.row.black` | `#1A1A1A` | `#F6F6F6` | `#211B1E` | `#F5EEF0` | `#171F1C` | `#EBF2EF` |
| `color.row.out` | `#000000` | `#E1E1E1` | `#040404` | `#E8D6DE` | `#040404` | `#D0DFDA` |
| `color.row.hatch` | `#5A5A5A` | `#7A7A7A` | `#66575E` | `#866F7A` | `#4B5F57` | `#617C73` |
| `color.line.bar` | `#6B6B6B` | `#808080` | `#7A6971` | `#8F7582` | `#5D736B` | `#668279` |
| `color.line.beat` | `#3E3E3E` | `#B9B9B9` | `#4A3D43` | `#CDAABB` | `#35453F` | `#9DBAB0` |
| `color.line.grid` | `#2F2F2F` | `#CECECE` | `#382F33` | `#DBC1CD` | `#293531` | `#B7CEC7` |
| `color.line.thin` | `#4A4A4A` | `#A7A7A7` | `#56494F` | `#BF98AA` | `#3F514A` | `#8AABA0` |
| `color.line.row` | `#353535` | `#C5C5C5` | `#3F3438` | `#D6B8C6` | `#2E3B36` | `#ACC6BE` |

## Contrast

Every theme must meet WCAG 2 AA. Contrast is the WCAG 2 contrast ratio of the two colors. `tools/design/contrast.py` checks every theme against the list of pairs in `tools/design/contrast-pairs.json` (which color is drawn on which, and where) and fails if a pair is under its minimum. When a part starts to draw a color on a new color, add the pair there.

| Kind | Minimum | Pairs |
|---|---|---|
| text | 4.5 | Text on its ground (WCAG SC 1.4.3). gray.8, gray.9, gray.10, accent and rec on gray.0 to gray.4; gray.9 and gray.10 on gray.5; tip.fg on tip.bg |
| ui | 3 | A mark, a meaningful line or border, or an on mark, against the color next to it (WCAG SC 1.4.11). Notes and the original position frame on the rows, bar lines (line.bar), the lines on rows that cannot be written (row.hatch), play head, record ring, track colors, meter colors on the meter grounds, keyboard focus line |
| state | 3 | The on color against the off color of the same mark, where only color shows the change: accent against gray.6 (button border), accent and rec against gray.5 (M, S, ● toggles; their off border is gray.5, and the usable ones are told from the disabled ones by the text, gray.8 against gray.7), gray.10 against gray.7 (star, plugged-in mark), accent against the white and black keys (a pressed key) |
| disabled | 2 | Text of 「使えない」 (disabled) parts. WCAG asks for no ratio here; they must still be readable |
| line | 1.5 | Lines that show structure but are not the only way to tell it: beat lines (line.beat), thin dividers and borders (line.thin), strong dividers, button, clip and box borders (gray.6) |
| grid | 1.2 | Thin lines that only have to be seen clearly: fine grid lines (line.grid), lines between rows (line.row) |
| faint | 1.08 | The faintest decoration: neighbouring fills (white-key and black-key rows, white and black keys, panels and bands) |
| fill | 1.2 | Large neighbouring fills that carry a meaning: the rows that can be written (row.white, row.black) against the rows outside the scale (row.out) while scale lock is on |

- Text sizes are all under the WCAG large size (18.66 px bold, 24 px), so all text uses 4.5.
- Where the words or a border also change (「オン」 and 「オフ」, the border of an on button), the on and off text colors need no ratio between them. Accent and rec, and accent and the track colors, are told apart by hue.
- The drum row name that sounds (accent) or is being recorded (rec) on the gray.5 name column is counted as a mark (3), not as text: the same name is shown in gray.9 or gray.10 just before.
- Scale lock: 3:1 between the rows cannot be reached by values alone without making the whole work area light. The values give 1.2 or more, and the rows outside the scale also get thin diagonal lines in row.hatch, 3:1 or more against row.out in every theme, so they can be told apart without seeing the difference in color (user:2026-11-23). In the dark themes row.hatch is just 3:1, so that the diagonal lines are not stronger than the bar lines (user:2026-11-23).
- Lines (user:2026-11-23): line.bar is a ui line (3), line.beat and line.thin are line (1.5), line.grid and line.row are grid (1.2), each against every ground it is drawn on (the rows of the piano roll and the drum rows, gray.1 and gray.2 under the arrange and the CC lanes, gray.0 to gray.4 under dividers, gray.1 and gray.3 under rows).
- Meters (user:2026-11-23): meter.ok and meter.hot keep their dark values in the light themes, and the dark meter grounds there (meter.ground, meter.ground.mic) give them 3:1. The clip mark (meter.clip) and the peak line of the mic meter (meter.peak) take the dark values of rec and gray.10 in the light themes, so they have 3:1 on those grounds too.

## Text

Japanese is IBM Plex Sans JP; numbers, positions and values are IBM Plex Mono (fixed-width digits). There are only 3 sizes. No text smaller than 12px is used, for Japanese or for numbers. The numbers in the table are px at 100 % / 125 % / 150 %.

| Name | Size | Line height | Weights allowed at that size | Used for |
|---|---|---|---|---|
| `text.s` | 12 / 15 / 18 | 16 / 20 / 24 | 400 / 500 / 600 | Body text, labels, buttons, numbers in tables. The smallest size, for Japanese and for numbers |
| `text.m` | 16 / 20 / 24 | 20 / 25 / 30 | 400 / 500 | Numbers of readings (position, time, tempo, velocity, dB) |
| `text.l` | 20 / 25 / 30 | 24 / 30 / 36 | 600 | Document headings, the count-in number. Dialog headings do not use this; they use text.s at 600 |

## Space

The spaces and sizes in the tables below are built only from multiples of 4. At 125 % and 150 % they are always whole numbers. Small sizes inside parts (icon 10, mark 6, bar handle 6×12, window mark 10, and so on) need not be multiples of 4. Multiply them by the scale and round (.5 rounds up). Example: 10 → 13 → 15, 6 → 8 → 9.

| Name | 100 / 125 / 150 % | Used for |
|---|---|---|
| `space.1` | 4 / 5 / 6 | Between buttons, between an icon and text |
| `space.2` | 8 / 10 / 12 | Between parts in a row, inside a panel |
| `space.3` | 12 / 15 / 18 | Left and right inside a band, gaps in a toolbar |
| `space.4` | 16 / 20 / 24 | Between readings, inside a dialog |
| `space.5` | 20 / 25 / 30 | Between readings of the input meter |
| `space.6` | 24 / 30 / 36 | One indent step, the height of a band tab |

## Sizes

| Name | 100 / 125 / 150 % | Used for |
|---|---|---|
| `size.menubar` | 24 / 30 / 36 | Menu bar |
| `size.transport` | 48 / 60 / 72 | Transport band |
| `size.meter.full` | 52 / 65 / 78 | Input meter, full |
| `size.meter.slim` | 24 / 30 / 36 | Input meter, slim |
| `size.meter.name` | 108 / 135 / 162 | Name field of the input meter (148 for a long name) |
| `size.meter.name.slim` | 176 / 220 / 264 | Upper limit of the name when slim (128 in a narrow window) |
| `size.bandrow` | 24 / 30 / 36 | Band tab row, where the folded headings line up |
| `size.status` | 24 / 30 / 36 | Status line |
| `size.row` | 24 / 30 / 36 | Headings, table rows, inspector rows, buttons |
| `size.row.s` | 20 / 25 / 30 | Rows of the note list, the M, S, ● toggles |
| `size.note` | 12 / 15 / 18 | One row of the piano roll (one semitone) |
| `size.toolbar` | 32 / 40 / 48 | Toolbar of a band, dialog headings |
| `size.track` | 64 / 80 / 96 | Track row (160 when editing a vocal) |
| `size.lane` | 48 / 60 / 72 | Automation band |
| `size.lane.vel` | 72 / 90 / 108 | Velocity band |
| `size.overview` | 40 / 50 / 60 | The shrunk arrange |
| `size.col.head` | 220 / 275 / 330 | Width of the track heads in the arrange |
| `size.col.keys` | 104 / 130 / 156 | Width of the keyboard in the piano roll |
| `size.panel.list` | 216 / 270 / 324 | Note list |
| `size.panel.insp` | 240 / 300 / 360 | Selected notes |
| `size.shelf` | 320 / 400 / 480 | Shelf |
| `size.tab.v` | 24 / 30 / 36 | Vertical headings of the folded left and right panels and of the shelf |
| `size.strip` | 108 / 135 / 162 | One mixer strip (124 for the master) |

## Lines and corners

| Name | Value | Rule |
|---|---|---|
| `line.hair` | 1 | Dividers, borders, grid, play position. At every scale it stays one screen dot (physical 1px) and never gets thicker |
| `line.mark` | 2 | Left of a selected row, top edge of an open band tab, top edge of a clip. 2 at 125 %, 3 at 150 % |
| `line.track` | 4 | Track color strip. 5 at 125 %, 6 at 150 % |
| `radius.0` | 0 | Panels, bands, rows, clips, notes, meters |
| `radius.1` | 1 | Buttons, input boxes, toggles, term tips. Stays 1 when scaled up |
| `radius.round` | 50 % | Only the record button and the pan knob |

## Rules for the scale

- 3 steps: 100 %, 125 %, 150 %. The same scale is applied to text, space and sizes. Lines follow the table above.
- When the width is not enough after scaling up, hide things in this order. (1) Transport: song name (leave a 28-wide ▾), CPU load, count-in (leave a 20-wide ▾ on the metronome), position labels. (2) Input meter when slim: modulation, pitch bend. (3) Band tabs: counts and current values. (4) Piano roll: start with the left and right panels folded.
- Names, keys, link marks, held notes, and the numbers of position and time are never removed at any scale.
- Example: in a 1280×800 window at 150 %, the piano roll shows 16 rows of notes. With the velocity band folded, 22 rows.
- The narrow form of the transport after the position labels are hidden: the position box is 32 high, space.2 inside, space.3 between bar and time. The master is 100 wide (space.1 between 「マスター」 and the value).
- The key hints written next to buttons (Space, R, L, C, 1, 2, 3, and so on) are hidden before anything else when the width is not enough. At 100 % in a 1280-wide window, the keys of the transport are not shown. The keys of the toolbar are shown.
- When the input meter is full, the value of 「弾くと鳴る先」 shows the track and instrument names, like 「1 ピアノ ・ グランドピアノ」. If the width is not enough, hide in this order: modulation → pitch bend → the keyboard picture to thin keys (width 8) → the instrument name of the target (show only 「1 ピアノ」) → the 「弾くと鳴る先」 label → sustain. Never remove the link mark, the name, the keyboard picture, held notes, velocity, and the target track. During audition, never remove 「ためし弾き ・ （音源の名前）」.
- While a hint text is shown at the right end of the input meter (no keyboard, no output yet), hide in this order: modulation → pitch bend → the keyboard picture to thin keys → the second sentence of the hint (「下でえらんでください」「つながなくても、マウスで音を書けます。」) → sustain → cut the end of the remaining first sentence to 「…」. The hint says what to do now, so it stays longer than the readings.
- When the vertical headings at the right rail do not fit vertically, hide the counts and names first. Keep the heading names and keys.
- The smallest window size, in px at 100 %, is 853×448. For the window itself: 853×448, 1067×560, 1280×672. Even at 150 %, it fits on a screen 768 high. The window cannot be made smaller than this.
- Because the menu bar (24) was added, the piano roll works from 452 and the undo history from 432. The mixer from 700. In the smallest window (448), the piano roll does not fit all its bands: notes are 9 rows, and only the content scrolls vertically. In windows lower than that, the mixer toolbar does not move, and only the strips scroll vertically. At first the bottom (faders and meters) is shown; scrolling up shows the inserts.
- A scale at which even the smallest window does not fit on the screen is shown with the 「使えない」 (disabled) look, with the reason 「この画面には入りません」.
