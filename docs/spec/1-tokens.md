# 1 List of values

Colors are called by name. The grays are 11 steps in which R, G and B are the same number; no other gray is used. Color is painted only on small places that have a meaning; large areas stay gray.

## Colors

|  | Name | Value | R=G=B | Used for |
|---|---|---|---|---|
|  | `color.gray.0` | `#0A0A0A` | 10 | Background of meters and readings, input meter, status line |
|  | `color.gray.1` | `#121212` | 18 | Work area (arrange, piano roll background) |
|  | `color.gray.2` | `#161616` | 22 | Open band, white-key rows |
|  | `color.gray.3` | `#1A1A1A` | 26 | Panels, track heads, shelf |
|  | `color.gray.4` | `#222222` | 34 | Folded headings, button face, selected row |
|  | `color.gray.5` | `#2C2C2C` | 44 | Thin divider lines |
|  | `color.gray.6` | `#3A3A3A` | 58 | Strong divider lines, button border |
|  | `color.gray.7` | `#5E5E5E` | 94 | Disabled things, the position before aligning |
|  | `color.gray.8` | `#9A9A9A` | 154 | Labels |
|  | `color.gray.9` | `#D6D6D6` | 214 | Body text, line drawings |
|  | `color.gray.10` | `#F2F2F2` | 242 | Emphasis, numbers |
|  | `color.accent` | `#FFB000` |  | Things that are live now (pressed keys, selection, loop, on, play head, the band chosen with the keyboard) |
|  | `color.rec` | `#FF3B30` |  | Recording, record arm, broken things |
|  | `color.meter.ok` | `#4CD964` |  | Meter up to −3 dB (70 % of the length) |
|  | `color.meter.hot` | `#FFD60A` |  | Meter above −3 dB |
|  | `color.track.1` | `#69E4E4` |  | Track color 1 (only the 4px-wide strip and the top 2px edge of a clip) |
|  | `color.track.2` | `#5ABDF2` |  | Track color 2 |
|  | `color.track.3` | `#6F8BED` |  | Track color 3 |
|  | `color.track.4` | `#8C54C3` |  | Track color 4 |
|  | `color.track.5` | `#FFD9F1` |  | Track color 5. From the 6th track, back to 1 |
|  | `color.tip.bg / color.tip.fg` | `#F2F2F2` |  | The term tip background is gray.10, the text is gray.0 |

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
