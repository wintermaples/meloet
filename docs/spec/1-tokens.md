# 1 List of values

Colors are called by name. The grays are 11 steps in which R, G and B are the same number; no other gray is used (the pink and mint themes tint them; see “Themes”). Color is painted only on small places that have a meaning; large areas stay gray.

## Colors

|  | Name | Value | R=G=B | Used for |
|---|---|---|---|---|
|  | `color.gray.0` | `#000000` | 0 | Background of meters and readings, input meter, status line |
|  | `color.gray.1` | `#1A1A1A` | 26 | Work area (arrange, piano roll background) |
|  | `color.gray.2` | `#212121` | 33 | Open band, white-key rows |
|  | `color.gray.3` | `#282828` | 40 | Panels, track heads, shelf |
|  | `color.gray.4` | `#2F2F2F` | 47 | Folded headings, button face, selected row |
|  | `color.gray.5` | `#3C3C3C` | 60 | Thin divider lines |
|  | `color.gray.6` | `#4A4A4A` | 74 | Strong divider lines, button border |
|  | `color.gray.7` | `#717171` | 113 | Disabled things, the position before aligning |
|  | `color.gray.8` | `#9A9A9A` | 154 | Labels |
|  | `color.gray.9` | `#D6D6D6` | 214 | Body text, line drawings |
|  | `color.gray.10` | `#F2F2F2` | 242 | Emphasis, numbers |
|  | `color.accent` | `#FFB000` |  | Things that are live now (pressed keys, selection, loop, on, play head, the band chosen with the keyboard) |
|  | `color.rec` | `#FF6158` |  | Recording, record arm, broken things |
|  | `color.meter.ok` | `#4CD964` |  | Meter up to −3 dB (70 % of the length) |
|  | `color.meter.hot` | `#FFD60A` |  | Meter above −3 dB |
|  | `color.track.1` | `#69E4E4` |  | Track color 1 (only the 4px-wide strip and the top 2px edge of a clip) |
|  | `color.track.2` | `#5ABDF2` |  | Track color 2 |
|  | `color.track.3` | `#6F8BED` |  | Track color 3 |
|  | `color.track.4` | `#9561C7` |  | Track color 4 |
|  | `color.track.5` | `#FFD9F1` |  | Track color 5. From the 6th track, back to 1 |
|  | `color.tip.bg / color.tip.fg` | `#F2F2F2` |  | The term tip background is gray.10, the text is gray.0 |

The values above are the 「ダーク」 (dark) theme. gray.0 to gray.7, rec and track.4 were changed so that the pairs below reach their contrast (user:2026-11-23, the owner's request: the on and off rows of scale lock in the piano roll and the lines were hard to tell apart). Before: gray.0 `#0A0A0A`, gray.1 `#121212`, gray.2 `#161616`, gray.3 `#1A1A1A`, gray.4 `#222222`, gray.5 `#2C2C2C`, gray.6 `#3A3A3A`, gray.7 `#5E5E5E`, rec `#FF3B30`, track.4 `#8C54C3`. The mock boards are still drawn with the values from before.

## Themes

There are 6 themes (user:2026-11-23): 「ダーク」 `dark`, 「ライト」 `light`, 「ピンク ・ ダーク」 `pinkDark`, 「ピンク ・ ライト」 `pinkLight`, 「ミント ・ ダーク」 `mintDark`, 「ミント ・ ライト」 `mintLight`. Every theme gives a value to every color name above, and the names keep their role in every theme. In the light themes the brightness runs the other way: gray.0 is the lightest ground and gray.10 the darkest text, so gray.0 to gray.4 are still the grounds from the back to the front and gray.7 to gray.10 still the text from weak to strong. In the pink and mint themes the grays carry a light tint of their color (both grounds and text), at the same brightness as the plain grays, so they meet the same contrast.

- `color.accent` follows the theme: yellow in dark, a dark amber in light (dark enough for text on the light grounds), pink in pinkDark, a dark plum pink in pinkLight, mint green in mintDark, a dark green in mintLight.
- `color.rec` is red in every theme. Only its brightness follows the ground.
- Track colors: the same 5 hues, lighter or darker for the ground. Where a track color would look like the accent, it is changed for a yellow: track.5 in the pink themes, track.1 in the mint themes.
- `color.tip.bg` is gray.10 and `color.tip.fg` is gray.0 in every theme, so the term tip is always the opposite of the screen.
- Values are in `ui/src/design/themes.ts`; `dark` is `colors` of `ui/src/design/tokens.ts`.

| Name | `dark` | `light` | `pinkDark` | `pinkLight` | `mintDark` | `mintLight` |
|---|---|---|---|---|---|---|
| `color.gray.0` | `#000000` | `#FFFFFF` | `#040404` | `#FBF9F9` | `#040404` | `#F8FAF9` |
| `color.gray.1` | `#1A1A1A` | `#EAEAEA` | `#211B1E` | `#EEE0E6` | `#171F1C` | `#DBE7E3` |
| `color.gray.2` | `#212121` | `#E1E1E1` | `#292225` | `#E8D6DE` | `#1D2723` | `#D0DFDA` |
| `color.gray.3` | `#282828` | `#D8D8D8` | `#30282C` | `#E2CCD6` | `#232E2A` | `#C4D7D1` |
| `color.gray.4` | `#2F2F2F` | `#CDCDCD` | `#392F33` | `#DBC1CD` | `#293531` | `#B6CEC6` |
| `color.gray.5` | `#3C3C3C` | `#BBBBBB` | `#483B41` | `#CFADBD` | `#33423D` | `#A0BDB3` |
| `color.gray.6` | `#4A4A4A` | `#A7A7A7` | `#57494F` | `#BF98AA` | `#3F514A` | `#89AB9F` |
| `color.gray.7` | `#717171` | `#7A7A7A` | `#806E77` | `#866F7A` | `#637971` | `#617C73` |
| `color.gray.8` | `#9A9A9A` | `#575757` | `#A7969E` | `#5D5056` | `#8D9E98` | `#485852` |
| `color.gray.9` | `#D6D6D6` | `#333333` | `#DBD5D7` | `#373134` | `#D3D7D6` | `#2D3432` |
| `color.gray.10` | `#F2F2F2` | `#111111` | `#F3F2F2` | `#121011` | `#F1F3F2` | `#0F1111` |
| `color.accent` | `#FFB000` | `#735000` | `#FF7FC8` | `#8E2879` | `#5CF0B4` | `#0B603F` |
| `color.rec` | `#FF6158` | `#A5261F` | `#FF685F` | `#9F251E` | `#FF6A61` | `#A0251E` |
| `color.meter.ok` | `#4CD964` | `#2D813C` | `#4CD964` | `#2C7D3A` | `#4CD964` | `#2C7E3A` |
| `color.meter.hot` | `#FFD60A` | `#906C00` | `#FFD60A` | `#8B6900` | `#FFD60A` | `#8C6900` |
| `color.track.1` | `#69E4E4` | `#007F7F` | `#69E4E4` | `#007B7B` | `#FFD54A` | `#8F6800` |
| `color.track.2` | `#5ABDF2` | `#1977B6` | `#5ABDF2` | `#1873B0` | `#5ABDF2` | `#1874B0` |
| `color.track.3` | `#6F8BED` | `#4668E1` | `#6F8BED` | `#4164E0` | `#6F8BED` | `#4165E0` |
| `color.track.4` | `#9561C7` | `#9551CD` | `#9664C8` | `#924CCC` | `#9765C9` | `#924DCC` |
| `color.track.5` | `#FFD9F1` | `#C23F87` | `#FFD54A` | `#8E6800` | `#FFD9F1` | `#BD3D84` |
| `color.tip.bg` | `#F2F2F2` | `#111111` | `#F3F2F2` | `#121011` | `#F1F3F2` | `#0F1111` |
| `color.tip.fg` | `#000000` | `#FFFFFF` | `#040404` | `#FBF9F9` | `#040404` | `#F8FAF9` |

## Contrast

Every theme must meet WCAG 2 AA. Contrast is the WCAG 2 contrast ratio of the two colors. `tools/design/contrast.py` checks every theme against the list of pairs in `tools/design/contrast-pairs.json` (which color is drawn on which, and where) and fails if a pair is under its minimum. When a part starts to draw a color on a new color, add the pair there.

| Kind | Minimum | Pairs |
|---|---|---|
| text | 4.5 | Text on its ground (WCAG SC 1.4.3). gray.8, gray.9, gray.10, accent and rec on gray.0 to gray.4; gray.9 and gray.10 on gray.5; tip.fg on tip.bg |
| ui | 3 | A mark, a meaningful line or border, or an on mark, against the color next to it (WCAG SC 1.4.11). Notes and the original position frame on the rows, play head, record ring, track colors, meter colors, keyboard focus line |
| state | 3 | The on color against the off color of the same mark, where only color shows the change: accent against gray.6 (button border), accent and rec against gray.5 (M, S, ● toggles), gray.10 against gray.7 (star, plugged-in mark) |
| disabled | 2 | Text of 「使えない」 (disabled) parts. WCAG asks for no ratio here; they must still be readable |
| line | 1.5 | Lines that show structure but are not the only way to tell it: bar lines, strong dividers, button, clip and box borders (gray.6) |
| grid | 1.2 | Thin lines that only have to be seen clearly: beat lines (gray.4), thin dividers (gray.5) |
| faint | 1.08 | The faintest decoration: sub-beat lines (gray.3), neighbouring fills (white-key and black-key rows, panels and bands) |
| fill | 1.2 | Large neighbouring fills that carry a meaning: the rows that can be written (gray.1, gray.2) against the rows outside the scale (gray.0) while scale lock is on |

- Text sizes are all under the WCAG large size (18.66 px bold, 24 px), so all text uses 4.5.
- Where the words or a border also change (「オン」 and 「オフ」, the border of an on button), the on and off text colors need no ratio between them. Accent and rec, and accent and the track colors, are told apart by hue.
- The drum row name that sounds (accent) or is being recorded (rec) on the gray.5 name column is counted as a mark (3), not as text: the same name is shown in gray.9 or gray.10 just before.
- Scale lock: 3:1 between the rows cannot be reached by values alone without making the whole work area light. The values give 1.2 or more; a stronger difference needs a change in how the rows are drawn.

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
