## 21. Tempo map, markers, automation and the undo history band (round 6)

Research: `docs/history/research/automation-vst3.md`, `automation-lv2.md`, `tempo-and-automation.md`. What shows on the screen but is not in the spec is listed in `docs/history/design-notes.ja.md` (A-19, B-92).

The first rule: **a recorded key is never off by 1 ms, even after 10 minutes of recording, even when the tempo moves.** So every position is worked out by a formula from the head of its stretch, never by adding block after block.

### 21-1. The tempo map (`crates/meloet-model/src/tempo_map.rs`)

- **What the song keeps** (inside `Doc`):

```rust
pub struct TempoPoint { pub tick: u64, pub tempo: Tempo, pub ramp: bool } // ramp: straight line to the next point
pub struct MeterChange { pub bar: u32, pub meter: Meter }                 // bar counts from 0; always 1 or more
pub struct Timeline {
    pub tempo: Tempo,                    // at the head of the song
    pub meter: Meter,                    // at the head of the song
    pub head_ramp: bool,                 // ramp from the head to the first point
    pub tempo_points: Vec<TempoPoint>,   // in tick order; tick >= 1; one per tick
    pub meter_changes: Vec<MeterChange>, // in bar order; one per bar
}
// Doc { timeline: Arc<Timeline>, markers: Arc<Vec<Marker>>, loop_range: Option<TickRange>, … }
```

  - The head's tempo and time signature are kept apart from the lists. Reason: this counts changes the way the spec does ("the head is not counted"), and code that read the old `doc.tempo` / `doc.meter` changed little. `Timeline::check` says what is well formed; `TempoMap::new` still makes the same map from a timeline that is not.
- ~~The tempo number is beats of the time signature's lower note per minute.~~ **Superseded by 22-2**: a tempo is always quarter notes per minute, whatever the time signature, and a quarter note is 1920 ticks (22-1).
- **A ramp is a straight line over ticks** (as Ardour does; it is the only form that gives the spec's example: 104 at bar 5 → 120 at bar 9 gives 112.00 at bar 7). `ramp` on the last point does nothing.
- **The built map** (`TempoMap`, made from a `Timeline` and never changed; the engine, the core and the meter thread share one): a list of **pieces**, each the overlap of a tempo stretch and a time signature stretch. A piece has its first tick `tick0`, its first time `sec0`, its tempo at the head `a` (the value on the line if inside a ramp) and its slope `s` (tempo per tick; 0 without a ramp).
- **Formulas** (u ticks from the head of the piece; time in minutes, × 60 for seconds; `Q = TICKS_PER_QUARTER` since 22-2):
  - tick → time: `t(u) = u / (Q·a) · L(s·u/a)`, `L(x) = ln_1p(x) / x`, `L(0) = 1`
  - time → tick: `u(t) = a·Q·t · E(s·Q·t)`, `E(y) = exp_m1(y) / y`, `E(0) = 1`
  - tempo there: `a + s·u`.
  - Check by hand: 104 → 120 over four 4/4 bars takes `ln(120/104)` min = 8.586 s.
- **No threshold for "almost flat" pieces.** In the `ln_1p(x)/x` and `exp_m1(y)/y` form the answer is right however small x or y is; only exactly 0 is split off. Reason: a threshold makes a small step in the line where the formula changes, and a form without steps is easier to test.
- **`sec0`** is summed from the front once, when the map is made (f64). With hundreds of pieces the error is about 1e-13 s, far under one frame (2e-5 s). Everybody reads the same `sec0`, so there is nobody to drift from.
- **Before tick 0** (count-in, keys just before recording starts) the head's tempo and time signature go on backwards; a ramp is not carried back.
- **Functions** (none allocates; pieces are found by binary search): `new`, `seconds_at`, `tick_at`, `tempo_at` (the transport's number, 112.00 at bar 7), `meter_at`, `tick_of_bar`, `bar_beat_tick`, `nearest_beat`, `bar_at_or_after`, `next_beat` (next beat and whether it is beat 1), `next_click` (the metronome's beat, 22-2), `next_change`, and
  - `frame_of(tick, from_seconds, sample_rate)` = `ceil((seconds_at(tick) − from_seconds) · sample_rate − 1e-6)`: the first frame at or past `tick`. The **1e-6 frame guard** stops a position that is exactly on a frame (one beat at 96.00 and 48 kHz = 30000 frames) from coming out one frame late from the last bit of the f64. If the guard is ever wrong, the error is one frame (21 µs) and does not add up.
- **Where it lives**: `meloet-model`. **`meloet-engine` depends on `meloet-model`** (new in round 6). It does not break report 7-1 (no I/O or threads in model; no screen or plugin formats in engine). Reason: one place for the maths. If the engine had its own copy, the two would drift.
- **Screen side** (`ui/src/time/tempoMap.ts`; outside the bridge since it knows nothing of Tauri): time display, grid times and the transport's tempo (60 Hz, no diff) need the same formulas. A Rust test writes a table (several maps × ticks, seconds, bar and beat) to `ui/src/time/tempoMap.table.json`; a TS test checks against it to a relative 1e-9 (JS `Math.log1p` / `Math.expm1` can differ in the last bit). The Rust test also fails if the committed table is out of date.
- **Tests**: the spec's example; at a constant tempo `frame_of` equals the old integer formula for every tempo, at 44.1 / 48 / 96 kHz, over 2 hours; ramps agree with a fine f64 sum to 1e-9 s and stay smooth as `s` → 0; `tick_at(seconds_at(k))` rounds back to k; `frame_of` never goes back.

### 21-2. Adding a time signature change earlier in the song

- Notes, clips, automation points, markers, tempo points and the loop range **keep their ticks**. A time signature change is kept by bar number and always sits on a bar line, so adding one earlier moves later changes in ticks but not in bar number.
  - Reason: chapter 11, "the meaning of a position does not change". A recorded key's tick is never rewritten.
- In x/8 a beat is the lower note (6/8 has 6 beats).
- Note: "a tempo point is on a beat" is a **snapping rule for placing and moving**, not something always true. After adding a 7/8 or 1/16 bar earlier, an old tempo point can sit inside a beat (shown as「12. 3. 240」). The formulas are right wherever a point is. Markers are the same. Points are not moved back onto beats, since that would change their distance to the notes.
- Bar number → tick (`tick_of_bar`) is integer arithmetic adding bars from the front. Changes are always on bar lines, so there are no part bars.
- Tests: before and after adding, deleting or moving an earlier change, the ticks of notes, clips, points and markers are the same, and the bar numbers of later changes are the same.

### 21-3. The engine's position (`crates/meloet-engine/src/{engine.rs,clock.rs}`)

- **Anchor**: where play started, where the loop went back, where the position was set, where the count-in ended, where the map changed, where the sample rate changed. `Transport` (in `engine.rs`) keeps `anchor_tick`, `anchor_sec` (the anchor in the map's seconds) and `frames` since the anchor (an integer). The position is always `map.tick_at(anchor_sec + frames / sample_rate)`. **Only frames are added, never ticks.**
- **The frame of a note, a beat or the loop end** is `map.frame_of(tick, anchor_sec, sample_rate)`: one step from the anchor, so the answer is the same however many minutes have passed. Track look-ahead (20-8) compares in frames: `frame_of(tick) ≤ frames + L_t`.
- **Handing over the map**: the core sends an `Arc<TempoMap>` (`EngineCommand::SetTempoMap { map, map_gen }`) through a ring, and old maps come back through a ring (nothing is freed on the audio thread). The engine and its readers (the core, the meter thread) share the one `Arc`. The core sends `EngineOp::Timeline { timeline: Arc<Timeline> }` whenever the `Doc`'s timeline changes, and `service.rs` builds the `TempoMap`. `map_gen` is given out by `EngineHandle::set_tempo_map`, and goes on counting in a rebuilt engine. At the swap, the current tick (a fraction) is turned into seconds on the new map and becomes the new anchor, so ticks run on without a jump. A position within 1e-6 ticks of an integer is snapped to it, so a note or beat at exactly that frame is not "already past" from the last bit of the f64; a beat due in the frame of the swap is played in that frame.
- **Loop**: the end is `frame_of(loop_end)`. Going back makes "the loop start, exactly" the new anchor (each turn drops less than one frame, as before; the real position is written to the clock stretch, so recorded keys do not drift).
- **Metronome**: the next beat is `map.next_click` (22-2). The strong click counts from the head of the bar in the current time signature stretch.
- **Count-in**: the tempo and time signature **at `record_from`**, with a constant tempo (inside a ramp, the value at that point). It is not put on the map; it is a straight stretch of "end tick, ticks per second". The last-half-beat key (chapter 12) uses this straight formula too.
- **Clock stretches (`Segment` in `clock.rs`)** carry `sec0` (the map seconds of their first frame) and `map_gen`. A reader uses `map.tick_at(sec0 + (t − start_us) / 1e6)`. `tick` and `ticks_per_frame` stay.
  - **The straight formula (`tick + elapsed × speed`) is used only for**: the count-in, stopped stretches, and when the reader's `map_gen` differs from the stretch's (1–2 blocks right after a tempo edit).
  - Reason: stretches follow blocks and are not cut at tempo points. Reading a 256-frame stretch across a 96 → 192 point with the straight formula is 4 ticks (3 ms) off; inside a strong ramp (30 → 240 in one beat) a 1024-frame block is up to 0.8 tick off.
  - The meter thread also holds the map (`Arc` from the core at each swap; a lock is fine there). The position and "ticks per µs" in the meter data are exact for the send time. The screen still extends them straight for one frame (16 ms, the error does not show). The meter binary did not change.
  - The engine cannot keep the readers' map (`crates/meloet-engine/clippy.toml` bans Mutex). `TransportClock::heard_at(t_us, map, map_gen)` takes the map as an argument; the core keeps it under a lock (`Shared::tempo_map`).
- **Render cuts** (the split points inside one render) also fall on tempo points and time signature changes (not inside ramps). They are rare, so they cost nothing, and plugins and the delay hear the new tempo exactly at the point.
- **To plugins** (`PluginTransport`): the values at the head of the cut: `tempo_bpm = tempo_at`, `position_quarters = tick / TICKS_PER_QUARTER`, `bar_start_quarters` = head of that bar / `TICKS_PER_QUARTER`, time signature as written (6/8 is 6 and 8), `bar` (the spare field became the bar number, because LV2 `time:bar` cannot be found from the bar's start position when the time signature changes), `position_frames = round(seconds_at(tick) × sample rate)` (the real frame position). No averaging inside ramps (as Ardour; Surge XT runs on from the head value).
- **LV2 `time:Position`**: sent to plugins that support it (`atom:supports time:Position`), **every block, once at time 0** (the JUCE LV2 wrapper forgets the position at the head of each block), and once at the head of each cut when a block is split. All fields: `frame`, `speed`, `bar`, `barBeat`, `beat`, `beatUnit`, `beatsPerBar`, `beatsPerMinute`. Control ports with `lv2:designation time:beatsPerMinute` / `time:speed` are written every block (as jalv and Ardour). These ports belong to the host: not shown in the parameter list, not automated (meloet writes them each block, so a hand change would jump back). Knobs such as the BPM of qmidiarp or gx_digital_delay are no longer shown in the panel.
  - ~~Beats are counted in `beatUnit` notes as the LV2 spec says.~~ **Superseded by 22-2**: `beatsPerMinute` and `beat` are in quarter notes (as JUCE and Ardour read them).
- **Built-in delay**: `EffectContext::tempo_bpm` = `tempo_at` at the head of the cut. Returns and the master are not split by cuts, so they get the value at the head of the render (at most 1024 frames).
- When the sample rate changes, a position before 0 becomes 0.
- **Tests** (`crates/meloet-engine/tests/`): 10 minutes with ramps and time signature changes; every note and click is on the frame `frame_of` gives, for any block size. 1000 loop turns. Map swaps during play do not jump the tick and do not allocate.

### 21-4. The position of recorded keys

- The rule of chapter 12 holds. **Frame → tick uses the map**: `stamp()` finds the clock stretch with the key's time and returns `map.tick_at(sec0 + (t − start_us) / 1e6)`, across tempo points and inside ramps alike.
- The audio thread compares its map's `map_gen` with the stretch's; if they differ, it uses the straight formula (21-3).
- The stop position (the core's `heard_at`) uses the same path.
- Tests: a fake clock with keys around tempo points, inside ramps, around time signature changes and around the loop turn, all within 0.01 tick of the formula, also at 10 minutes. `tools/measure/record-timing.sh` also runs on a song with a ramp.
- ~~Tick rounding goes over 1 ms at slow tempos and in x/16.~~ **Superseded by 22-1**: with 1920 ticks per quarter and quarter-note tempos, rounding is within ±0.52 ms in every time signature.

### 21-5. Automation data (`crates/meloet-model/src/automation.rs`)

```rust
pub struct InsertId(pub u32);            // id of what was put in a slot; from next_id
pub enum AutoTarget {
    Volume, Pan, Send(Bus), Mute,
    Instrument { param: u32 },                    // plugin instrument: the plugin's own id
    Insert { insert: InsertId, param: u32 },      // built-in: table index; plugin: its own id
}
pub struct AutoPoint { pub tick: u64, pub value: f32 }   // value: knob position 0–1
pub struct LaneId(pub u32);
pub struct Lane {
    pub id: LaneId,             // new id whenever the points change (like a take)
    pub target: AutoTarget,
    pub step: bool,             // steps (stepped values, on/off, mute)
    pub points: ChunkedVec<AutoPoint>, // tick order; one per tick
    pub resting: bool,
}
// Track { automation: Vec<Arc<Lane>>, … } in the order added
```

- **Keys**: the track by `TrackId`; an insert by **`InsertId`, not by slot number** (given when put in, kept when reordered, new when replaced); a parameter by table index for built-in effects (by name in the file) and the plugin's own id for plugins (as `SetPluginParam`).
  - Reason: slot numbers change on reordering; with `InsertId` the lane just follows.
  - Removing or replacing an effect, or replacing the instrument, removes that parameter's lanes **in the same undo step** (undo brings them back). `remove_effect`, `replace_effect`, `set_instrument` and `RemoveMissing` in model do this, and a `Doc` test checks "every lane has its target". Lanes of plugins that are not found are kept as read.
- **Every value is the knob position the user sees, 0–1.** The line is straight in that position, and straight in ticks across (the lane's x axis is ticks, so it is the line on the screen).
  - Volume and send: the fader scale of spec chapter 2 (+6 = 1.0, 0 dB = 0.8, −60 = 0; position 0 is −∞).
  - Pan: `(pan + 100) / 200`. Plugins: as is.
  - Built-in effects: the bar position in the params panel. These formulas were only in TS (`ui/src/components/params/paramRows.ts` `fractionOf` / `valueAt`, `ui/src/components/mixer/scale.ts` `dbToFrac` / `fracToDb`); the same formulas are in model (`ParamSpec::position` / `value_at`, `Gain::position` / `from_position`), and a Rust table tests the TS (as in 21-1).
  - Steps: a stepped value is n / (steps − 1); on/off and mute are 0 and 1. The value is that of the nearest point at or before the position.
  - Before the first point: the first value. After the last: the last value.
- **The hand-set value stays where it always was** (`Channel` fader, `Insert` values, plugin state …).
  - A lane that is not resting → the lane's value sounds (**also when stopped**, at the play line).
  - Resting → the hand-set value (spec: "the value stays where it was set by hand").
  - A lane with no points → the hand-set value (a flat line).
  - Reason: resting needs no new place for values, and hand intents (`SetGain` …) write the same place as before.
- **A jump is two points one tick apart** (never two points on one tick). Reason: one point per tick keeps order and index ranges clear. Sound is smoothed over 20 ms, so a 1-tick slope cannot be heard.
- **"● write" is not in the song** (outside undo, not saved, off on stop; the core's `Play` keeps it). **Resting is in the song** (spec: one undo step, saved). Folding a lane is in `Session`.
- **Point selection is kept by the core** (like notes, chapter 16: editing changes the `LaneId`). Intents name a `LaneId` and index ranges; an intent with an old `LaneId` does nothing.
- **No lanes for returns A/B or the master.** Lanes live only on `Track`.
- **Undo steps**: `StepKind::{AddLane, DeleteLane, AddAutoPoint, MoveAutoPoints, DeleteAutoPoints, WriteAutomation, RestLane, RestoreLane}` (all with `track` and `target`, and `count` where there is a number).
- **Song file**: each track has `"automation": [{ "target": {…}, "step": false, "resting": false, "points": [[tick, value], …] }]`, each insert an `"id"`. `LaneId` is not written (given out on load). (Added in version 5; **now version 6, 22-3**.)
- **Points are a `ChunkedVec`** like notes (chapter 16). Copying the list per step made 10 minutes of written volume hold 53.9 MiB of history; sharing chunks makes it 1.34 MB. Every step that changes points goes through `with_points`, which shares unchanged chunks (`ChunkedVec::sharing`); lane add / delete / rest / restore share the `Arc<Lane>` or chunks. `AutoPoint`'s `Eq` relies on "values are never NaN" (`clamp_value`, `check_lanes`, file loading); `Hash` treats 0.0 and −0.0 alike. See `crates/meloet-model/tests/lane_memory.rs` and `docs/history/measurements/2026-11-20-round6-step2-3.md`.

### 21-6. How far ahead each slot reads

- Instrument parameters: `L_t` (the sum of instrument and insert latencies), as far ahead as notes.
- A parameter of insert k: the sum of the latencies of that slot and every slot after it, because the sound entering that slot is that far ahead of the song position. This is the same as a usual DAW (which delays the sound) reading the parameter at "the position of the sound entering the slot".
- Fader, pan, send: not read ahead.
- **Mute is read ahead by `L_t`.** meloet's mute stops the seat's notes (chapter 18, 19-11), so it acts at the **instrument's input**. Notes are sent `L_t` early, so mute must be read at the same place or the change is heard `L_t` late. The engine reads the steps at the note lane position (`lane_pos`).
- **Bypassed slots**: their latency is not in the sum (19-11). A bypassed slot's own parameters are read with only the later slots' sum. From the block after un-bypass, the normal sum.
- A latency change takes effect from the next block (20-8). Inside a loop, the read-ahead position wraps too. The first `L` frames of play cannot be read ahead (20-8).
- Tests: two fake effects with 100 frames of latency each; parameter steps land exactly on the matching sound, frame by frame. Mute steps on a track with latency land at the right output frame.

### 21-7. How finely values move

- **What the engine holds**: the core packs each track's non-resting lane points into one box (`AutoSet`), sends it through a ring, and old boxes come back through a ring (as with `Sequence`; no allocation on the audio thread). Insert parameter lanes are kept with their slot and move with it on `MoveEffect`. A lane the user is holding is paused with `HoldLane {track, target, on}` (21-9; up to `HOLDS` = 16).
- **Render cuts are not made finer for automation.** One set of cuts serves all tracks, and finer cuts would add round trips to plugin instruments (20-2). The only extra cuts are mute steps and the tempo points and time signature changes of 21-3.
- **Volume, pan, send**: the cut is split into steps of `MIX_STEP` = 32 frames; at the end of each step the exact value (map → line → gain) is found, and the gain ramps straight to it (the existing `Ramp`). The ticks of these steps are worked out once per cut (33 values) and shared by all lanes that are not read ahead (`crates/meloet-engine/src/automation.rs`).
  - **Jumps** are limited by speed: at most "end to end in 20 ms" per step (`RAMP_S` of 19-11). A 1-tick slope, a step, and the start and end of writing all become 20 ms ramps.
- **Built-in effects**: only slots with lanes have their `process` split every `EFFECT_STEP` = 64 frames with `set_param` at the head (same process, so no extra round trips). Effects glide 30 ms to a new value themselves (the `Effect` contract), so the sound follows the line a little late. **Open**: measure that delay and read ahead by it if it matters.
- **VST3**: points with their place in the block (`ToPlugin::Param {frame, id, value}`; the buffer in `com.rs` holds 16 points per parameter and 128 parameters per block; on overflow the last point is overwritten, so the final value is kept).
  - Smooth lines: one point at the **last frame** of the cut with that frame's value (JUCE uses only the last point; it also suits plugins that read between points), plus a point at any bend of the line inside the cut. A jump is two points.
  - A value equal to the last one sent is not sent. On a new anchor (play start, loop, locate) and after a plugin's state is restored, the "last sent" values are forgotten and sent again.
- **LV2**: the same points come from the parent; only plughost differs.
  - Control ports: `run` is split only in blocks with points, each piece at least `SHORTEST_PIECE` = 32 frames, with the line's value at the middle frame of the piece. Plugins that want blocks of their own length (`bufsz:coarseBlockLength` and the like) are not split; they get the value at the middle of the block (`crates/meloet-plughost/src/formats/lv2.rs`).
  - `patch:Set`: with time; one per parameter per block, the last (JUCE ignores the time).
- **Limits**: the ring to a plugin holds 256 events per slot (`to_plugin_events` in `hosts.rs`). Automation points: at most `PLUGIN_POINTS` = 128 per plugin per cut. On overflow, middle points are dropped first, keeping each parameter's last point; lanes that still do not fit go to the next cut (starting from a different lane each time so no lane waits forever).
- Tests: the no-allocation guard with 200 lanes and 10000 points; a fake plugin for point places and overflow; fader line against output gain, frame by frame.

### 21-8. Values on the screen

- **Built-in values (volume, pan, send, mute, effect parameters) are worked out by the screen**: lane points + meter position → value on the line → knob. No values flow in diffs or meters.
- **Points are not in the state; they are fetched by `LaneId` like takes** (binary; same idea as 3-4). `song` has only each track's lane headers (target, `LaneId`, point count, steps, resting). The screen keeps points only for lanes that exist (`keepOnly`). The volume / pan / send / mute lanes of tracks the mixer shows are fetched even when folded.
  - Reason: written lanes have thousands of points, and `song` is resent whole on every step.
- **Plugin value text comes in `paramValues` (one per 33 ms)**, also for values moved by automation, but **only for the plugin the params panel shows** (so 100 lanes' worth is not sent every time).
  - The core says `PluginAsk::Watch { on: true }` to that plugin while it is running (`sync_panel` in `play/params.rs`; `on: false` when the panel shows something else or the plugin stops). `Hosts::watch` sends `Request::Watch { slot, on }` to the plugin host and remembers it for `reload`.
  - On plughost's audio thread, only watched slots put engine parameter points (= automation; points for parameters the user is holding in the window were already dropped, 21-10) into a per-slot ring (`Moved` in `rt.rs`, 512, newest dropped on overflow). No seat lock is taken (if the main thread held it, the audio thread would skip that slot's block). Every 33 ms the main thread empties the ring and sends the last value per parameter with the plugin's text (`Controller::text`) as `Notice::ParamAutomated { slot, id, value, text }`.
  - `ParamChanged` is not used for this: it means "the user moved it in the window", which makes the core rest or write (21-10). `ParamAutomated` becomes `PluginSaid::Param` in the core and joins the `paramValues` path. It is not stored as a value for reload (`Hosts::params`); the lane sends it again.
  - `WIRE_VERSION` went to 4 here (**now 5, 22-12**). meloet and `meloet-plughost` are always built together, so there is no version negotiation: a child with a different version in the greeting is not used.
  - **`params.selected.automated`**: the parameters of the shown plugin that have a lane on that track (resting lanes too, like the mark on the screen), in list order (`automated_rows` in `play/params.rs`).
- **Names of plugin parameters in lanes**: the lane heading, the status line and history rows need the parameter name. The core remembers names **only for parameters with lanes** (`LaneNames` in `play/automation.rs`, keyed by plugin key and parameter id).
  - Two sources: the panel's table if it has the same plugin key; otherwise one running plugin with that key is asked once for its list (`PluginAsk::Params { ask: NAMES_ASK }`; panel requests count from 1, so the answer is told apart by number). Only the needed names are kept; a parameter not in the list is remembered as "no name" and not asked again.
  - Reason: before, only the table of the plugin the panel **currently** showed was used, so other plugins' names were null. Keeping every plugin's table would be 45 000 rows for 16 Surge XT. Keying by plugin key means 16 copies of one plugin need one request. Names are not forgotten when the song changes or a lane is deleted (history rows can still name a deleted lane).
  - Headings and the status line are built from this table each time, so `song` is resent with the next diff after a name arrives. History rows (`history_rows`) read the table when made (`play/history.rs`).
- **Plugin windows**: VST3 uses `setParamNormalized` on the main thread; only **the last value per parameter per block** goes into the `to_controller` ring (1024). LV2 uses `port_event`.
- The transport's tempo and time signature numbers are also worked out by the screen from the map (TS) and the meter position.
- The `rec` border of a lane being written: the core gives "the lane being written and the tick writing started" in `automation`, and the screen draws up to the play line.
- Tests: TS line values against the Rust table. A new moving fader needs `tools/measure/webkit-damage.sh`. Core to plugins: `crates/meloet-core/src/service/window_tests.rs` (`lanes_move_the_parameters_of_plugins_through_the_core` with fake plugins, always run; `surge_xt_and_zamcomp_{vst3,lv2}_follow_their_lanes_through_the_core`, `--ignored`, needs X). A core without a stream sends nothing to the engine, so these tests hand the lanes (`send_automation`), map and Play/Locate to the engine themselves. They fail if the `housed.rs` wrapper drops `set_param_at` / `takes_points`.

### 21-9. Where written points go (`crates/meloet-core/src/play/automation.rs`)

- **Rule**: a value written with "● write" goes to the position that was reaching the ear at the moment it was moved (as with keys).
- **The core places it from the time; the engine does not record it.** The core turns the move time (CLOCK_MONOTONIC) into the heard position with `TransportClock` and the map.
  - Mixer and params panel: the time the core received the intent.
  - Plugin windows: plughost stamps the time at `performEdit` (LV2: a write from the window) and sends it with the notice.
  - Reason 1: the engine's own position is the frame it is making now, ahead of the ear by the output latency (10–20 ms or more), and the value reaches the engine only in the next block. Placing it right in the engine would need the move time and `TransportClock` anyway, and the core has them.
  - Reason 2: it is the same formula as keys (21-4); no new ring from the audio thread to the core.
  - Reason 3: the core makes the undo step.
  - The clock ring keeps 512 stretches (2.7 s at 256 frames); the core handles input at once, so that is enough. A value too old to find is dropped.
- **While writing**: the core pauses the lane with `HoldLane` and plays values through the existing "sound only" preview path. The core collects value/position pairs (not the audio thread, so allocating is fine).
- **On release (one step, `WriteAutomation`)**: points in [first position, last position] are removed and the written ones put in. Both ends are jumps: one tick before the first position, the old line's value; one tick after the last, the old line's value (spec: "on release, it goes back to the line").
- **Across a loop turn**: the range is split per turn and put in from the oldest turn (the newer turn wins where they overlap). Still one step.
- **Thinning**: once, on release (`thin` in `crates/meloet-model/src/automation.rs`). Ramer–Douglas–Peucker on the value (0–1) with `THIN_TOLERANCE` = 0.005 (0.5 % of the knob; provisional). Step lanes keep only the points where the value changes. Input rate: the mixer gives one value per frame (existing rule), plugin windows one per 33 ms (existing notice; Ardour uses 30 ms).
- **On stop**: what was written so far becomes one step and write turns off. Moves while still holding are not put in the song; on release the value goes back to the line (provisional, B-92).
- Tests: with a fake clock, "a value moved at time t goes to the position heard at t" (also with other output latencies and across tempo points); the thinned line stays within the tolerance; across a loop; undo after writing gives back the old `LaneId`.

### 21-10. 15-C with write and rest (`play/windows.rs`, `crates/meloet-plughost/src/`)

- **The undo step for "time in the window" is made only if the user did something**: plughost counts user actions (VST3 `performEdit`, LV2 writes from the window, and in both the preset-loaded notices `kParamValuesChanged` / `kParamTitlesChanged`) and tells the core when the window is left. With 0 actions there is no step even if the state bytes differ. Reason: automation moving values always changes the bytes.
- When there is a step, its state also holds the automation values of that moment. That is harmless: lanes that are not resting resend their values in the next block. **For that**: after a plugin's state is restored (undo, redo, history jump, reload), the engine forgets the "last sent" values of that plugin's lanes and sends them again (21-7).
- **Moving a parameter with a lane by hand in the window**:
  - While it is held (VST3 `beginEdit` … `endEdit`; LV2 control `ui:touch`, or 0.5 s after the last write without it; `patch` 0.5 s; provisional numbers), plughost **drops automation points for that parameter itself**. It does not wait for the core, so hand and automation do not fight.
  - Write off, or stopped → **rest**. On the notice the core at once holds the lane in the engine (a "temporary rest" outside the song, like the mixer preview), and **puts the rest into the same step as leaving the window** (not two steps).
  - Write on and playing → **write**. From touch to release is one step (`WriteAutomation`, 21-9), made at once, separate from the window step. These moves are not counted as user actions (so leaving the window does not make another step).
- Dragging in the params panel (`SetPluginParam`, `SetParam`) follows the mixer table in 21-11.
- Tests: the random test of 20-13 (song and plugin agree) with lanes, write and rest. Lanes moving values while the window is open make 0 steps. A hand move gives one rest + window step; undo brings both back.

### 21-11. Mixer drags and writing (`play/mixer.rs`, `ui/src/components/mixer/mixDrag.ts`)

Built on the existing "sound only (`commit: false`) → commit on release (`commit: true`) → Esc sends `CancelMixPreview`". **No new intents**: the core decides from whether there is a lane, whether write is on, and whether it is playing.

| Lane | Write | First `commit: false` | `commit: true` | Esc |
|---|---|---|---|---|
| none | — | as before | as before (one `SetGain`-like step) | as before |
| yes, not resting | off, or stopped | hold the lane, preview | **value + rest in one step** (`SetGain {…, rested: true}`) | lane runs again; no step |
| yes, not resting | on and playing | hold the lane, preview; collect values and positions | **the written points as one step** (`WriteAutomation`); hand-set value unchanged | drop what was collected; no step |
| yes, resting | — | as before | as before (one step) | as before |

- 「もどす」 is `RestoreLane` (one step). A lane that went to rest also loses its armed write (spec: when resting, the button becomes 「もどす」).
- Typing a value (`TypeParam`, the fader number) has no release, so it **rests** even with write on.
- Key moves (86-7: `commit: false` per key, `commit: true` after 1 s) follow the same table.
- Tests: per table row, the step count and the song after undo.

### 21-12. The undo history band (`meloet-model/src/history.rs`, `play.rs`, `play/mixsync.rs`)

- **Jumping N steps**: `History::jump(&mut doc, to)` swaps N times inside (only `Arc` swaps, through the same `swap` path and `next_id` rule as one step), changes `Doc` once and returns the number of steps. Undo, redo and history jump all go through one path (`travel` in `play/history.rs`). **After it, once**:
  - Engine sync: `sync_mix` sends only the difference from "what the engine was last told", so one call. Sequences only for tracks whose clips changed. The map once.
  - **Insert reordering matches by `InsertId`, not slot number**: if the same `InsertId` is in another slot, rotate; if it is missing, create or return. (The old per-step `insert_moved` could not do N steps.) One path for one step and for N. Windows, the chosen slot and the slot the shelf is open for are remapped by `InsertId` too. See also the core's reorder in 19-11 (`inserts_follow` in `play/mixsync.rs`).
  - **An effect in the same slot of the same kind with only a different `InsertId` is kept, not rebuilt** (as in 19-11; values are sent as differences, so the sound is the same).
  - Plugin state: `SetPluginState` once, only for plugins present before and after whose bytes differ. The visit cut (`visit_overtaken`) once. "One per block" (20-3) stays inside `Hosts`.
  - Automation boxes: at most twice per seat (a seat whose plugin state was restored gets the resend of 21-10).
  - Selection: the selection of the step jumped to.
- **What each row says**: `StepKind` carries the numbers and names it needs. Names (track, clip, plugin, marker) are taken by the core from **the `Doc` before and after the step** (`History` keeps the `Doc` before each step; the one after is the next step's "before" or the current song). The screen gets "kind of text + values to fill in" (rule of 3-2). Names of plugin parameter steps (「（パラメータ）を変える」) are remembered by the core by step number from the list at the time (`StepKind` cannot hold names); they are not in the song file, so after reopening they have no name.
- **Rows are fetched**: the `history` part has only counts. Rows come from `history_rows(from, to)` for what is visible. New steps always go at the end and only cut what follows, so the screen refetches only "from the last row on". Undo, redo and jump do not change rows, only the current index. Reason: 1000 rows are not resent on every step (arrow keys on notes make 30 steps a second). 100 rows of a 5000-step song take 6–8 µs; a 500-step jump including engine sync about 75 µs.
- **The loop range is in `Doc` (step `SetLoop`); on/off stays in `Session`.** 「ここから次のマーカーまでをループに」 is one range step + on (not a step). Turning loop on with no range makes one (`DEFAULT_LOOP_BARS` = 4 in `play.rs`), which is a range step.
- **Solo is not a step** (stays in `Session`). The spec's step name 「ソロ」 went back to the design as B-92.
- **"End of song"**: the end of the last clip, rounded up to a bar line. If a marker is later, 4 bars after the marker.
- Status line: undo, redo and jump use the chapter 6 texts. With nothing to redo:「やりなおせる手がありません。」 (first provisional; now the spec's text, `msg.nothingToRedo` in `ja.ts`).
- Tests: random step lists; "undo N times" and "jump N" give the same song, engine state and plugin states; a jump does `SetPluginState` at most once per plugin and rebuilds no effect when only the order changed.

### 21-13. Song file and data

- **Song file** (`songfile.rs`; safe writes and `.prev` as chapter 14; the recovery file has the same form):
  - `headRamp`, `tempoPoints: [{tick, tempo (integer, 0.01 units), ramp}]`, `meterChanges: [{bar (from 0), meter: [top, bottom]}]` (`tempo` / `meter` stay as the head's values)
  - `markers: [{id, tick, name}]`
  - `loop: {start, end} | null` moved out of `session` into the undoable song; `loop_on` stays where it was
  - each track's `automation` (21-5) and each insert's `id`
  - Not readable: a point at tick 0, points out of order or on the same tick, out of range; a change at bar 0, out of order or on the same bar; repeated marker ids; an empty loop.
  - This was version 5. ~~Reading versions 1–4~~ **Superseded by 22-3**: the file is now version 6 and versions 1–5 are not read.
- **`Doc`**: `timeline: Arc<Timeline>`, `markers: Arc<Vec<Marker>>` (each step copies the `Doc`, so the `Arc` avoids copying name strings), `loop_range`. The old `doc.tempo`, `doc.meter`, `TempoChange` and `Session::loop_range` are gone. Steps: `crates/meloet-model/src/timeline.rs`; intents: `crates/meloet-core/src/play/timeline.rs`.
- `StepKind` has 14 timeline steps (tempo point add / move / value / ramp / delete; time signature change add / move / value / delete; marker add / rename / move / delete; loop range). Head values use the old `Tempo` / `Meter` steps.
- **The quantize grid counts from the head of the time signature stretch** (not from each bar). A song without changes gives the same answer as before (so a half-note grid in 3/4 does not change). A step that would cross the next change stops at that change's bar line.
- **A point added inside a ramp takes on the ramp** (the screen sends the value on the line; without that, the second half of the line would change).
- **If undo removes the loop range, loop turns off.** L with no range is "range step + on".
- **Adding a marker and typing its name in the open box is one step** (`History::amend`: only while the box from K is still open and no other step or undo came between).
- The count-in counts beats in the time signature at the record start.

### 21-14. Parameters that only take keyboard knobs (91-3; `crates/meloet-plughost/src/formats/{vst3.rs,mod.rs,lv2.rs}`, `crates/meloet-core/src/play/params.rs`)

- On VST3, each time the parameter list is built, the controller's `IMidiMapping` is asked which parameter each of 16 channels × 130 controllers (CC 0–127, aftertouch, pitch bend) on MIDI input 0 maps to. Those mapped **and** not automatable (no `kCanAutomate`) are "keyboard knobs only". Names of the form "MIDI CC n|n" are treated the same (for plugins without `IMidiMapping`, and LV2).
  - Reason: names alone miss non-JUCE plugins. `IMidiMapping` also points at real sound parameters mapped to a CC (CC 7 → volume), so automatable ones are shown as usual (no such plugin on this PC; not checked with a real one).
- Surge XT VST3: both ways find the same 2080 (`MIDI CC 0|0` … `MIDI CC 15|129`), leaving 774. Surge XT LV2 (775), LSP and ZamComp have none. Asking takes 0.87 ms (debug).
- plughost sets `ParamInfo::midi_cc`; the core puts those rows in `ParamTable::keyboard_knobs` (not in `groups` nor `SelectedPlugin::count`). In the core's table they can still be chosen and moved like other rows (for rows found by search).

### 21-15. Protocol versions 20 and 21

The `timeline` part, tempo / time signature / marker intents and their status line texts came in version 20 (`crates/meloet-protocol/src/timeline.rs`, `lib.rs`); `lanes`, `automation`, `history` and their intents and texts in **version 21** (`crates/meloet-protocol/src/{automation.rs,history.rs,lib.rs}`). (The protocol is now version 27: 22-12, 23-7.)

**State parts**

| Part | Holds |
|---|---|
| `song` | each track's `lanes: [{ target, name, laneId, points (count), step, resting }]` in the order added (`name` is the lane heading / the "what" of texts: `volume` / `pan` / `send {bus}` / `mute` / `builtin {effect, param}` / `plugin {plugin, param?}`); each insert's `id` (`InsertId`). `tempoBpm`, `meterNum`, `meterDen`, `tempoChanges` moved to `timeline` |
| `timeline` | the head's `tempoBpm` / `meterNum` / `meterDen`, `headRamp`, `tempoPoints: [{tick, tempoBpm, ramp}]` (tick order, ≥ 1), `meterChanges: [{bar, meterNum, meterDen}]` (bar from 0, ≥ 1), `markers: [{id, tick, name}]` (tick order, then id), `selected` (at most one of: `{kind: "tempoPoint", tick}`, `{kind: "meterChange", bar}`, `{kind: "marker", id}`), `renaming` (id of the marker whose name box is open), `open` (whether the arrange's 「テンポ ・ 拍子の変更」 row is open). The screen's map (`ui/src/time/tempoMap.ts`) is built from this |
| `automation` (not undoable) | `folded: LaneRef[]`, `armed: LaneRef[]` (「● 書く」 on), `writing: [{lane, fromTick}]`, `held: LaneRef[]` (temporary rest while moved in a plugin window, 21-10), `selected: {laneId, points: IndexRange[]} \| null` (one lane only). `LaneRef` is `{track, target}`. A lane in no list is open, not armed and not writing |
| `history` | `total`, `redo`, `current` (`total − redo`; 0 with no steps or all undone), `newest` (step number of row `total`, or 0). No rows. Rows the screen keeps are fixed by `index` and `step`; if `newest` changes only the last row is refetched, rows after `total` are dropped |
| `transport` | unchanged (tempo and time signature numbers come from `timeline` and the meter position) |
| `params` | uses the existing `automated` field of the plugin view |

**Fetched (invoke, not diffs)**

- `lane(laneId)` → binary points, version 1, little endian (`LanePoints`; read by `ui/src/bridge/lane.ts`; a TS test reads `ui/tests/fixtures/lane-v1.bin` written by Rust). 16-byte head: `u8` type = 3, `u8` version = 1, `u8` flags (1 = steps), `u8` 0, `u32` laneId, `u32` point count, 4 bytes 0. 16 bytes per point: `u64` tick, `f32` value (0–1), 4 bytes 0. Not read if the length does not match the count. No answer unless it is a current `laneId`. The screen keeps lanes in the song plus the newest 16 (`LaneStore.keepOnly`).
  - The head carries `laneId` and the step flag so the answer alone says which lane and which line shape (no mix-up if the lane changes while fetching). Points are 16 bytes so `u64` positions are 8-byte aligned.
- `history_rows(from, to)` → rows `from..=to` (as many as exist, oldest first): `{index (from 1), step (step number, not 0), name: StepName, what: StepWhat, place: StepPlace | null}`. `name` is the kind of step name (spec chapter 3 and B-92; only `SetParam {effect, param}`, `SetPluginParam {plugin, param?}`, `PluginWindow {plugin}` carry values); `what` holds the values for 「何を」 (note count, clip count, take, track, strip, place, name, tempo / time signature before and after, loop bars, dB before and after, pan, send, effect, sound, lane and point count, count removed); `place` is the right column (arrange / piano roll / mixer / track / plugin window). Texts are on the screen (`ui/src/components/history/stepText.ts`, `ja.ts`).
  - Step names are in one place, `StepName::every_kind()`. Rust checks "every `StepKind` maps to a `StepName`", TS checks "every `StepName` has a name in `ja.ts`"; one missing fails.

**Intents**

- Tempo and time signature: `SetTimelineOpen {open}`, `AddTempoPoint {tick, bpm}`, `MoveTempoPoint {tick, toTick, bpm}` (one drag moves place and value; typing a value sends `toTick = tick`), `SetTempoRamp {tick, on}`, `AddMeterChange {bar, num, den}`, `MoveMeterChange {bar, toBar}`, `SetMeterChange {bar, num, den}`, `SelectTimeline {target?}`, `DeleteTimeline {target}` (`target` has the same form as `selected`; markers are chosen and deleted with these too). The transport's `SetTempo` / `SetMeter` change the point or change in effect at the play line (the start point inside a ramp, else the head).
- Markers: `AddMarker {name}` (at the play line; the core snaps to the nearest beat), `OpenMarkerName {id}`, `RenameMarker {id, name}`, `CloseMarkerName`, `MoveMarker {id, tick}`, `GoToMarker {id}`, `StepMarker {forward}` (Alt → / ←), `LoopFromMarker {id}`.
- **Tempo points are named by tick and time signature changes by bar number**, not by index or id. There is one per place, so the place is the name. Tick 0 / bar 0 is the head (always there; can be chosen and changed, not moved or deleted). If nothing is there, the intent does nothing (as "an intent with an old id does nothing", chapter 16).
  - Reason: indexes shift with undo or other points coming and going, so an intent just sent could hit the neighbour. Ids would add an id field to `Timeline` (21-1) and the file, and the map (held by the engine and meters too) would carry a field used only for choosing. With tick and bar, an old intent either hits the point now at the place the user aimed at, or does nothing; never a point elsewhere.
  - Moving a point changes its name (tick). The core reselects the moved point; the screen reads `selected` after release.
- **Tempos are f64 BPM on a 0.01 grid everywhere**: in the part (`tempoBpm`), intents (`bpm`) and status line (`tempoBpm`). Reason: the existing protocol (`SetTempo {bpm}`, `NewSong {tempoBpm}`) and the answer table (`tempoMap.table.json`) use this form and the screen's map puts the f64 straight into the formulas. Rust `Tempo::bpm()` (integer ÷ 100) and the JS number read from JSON are the same f64. Only the core turns it into a 0.01 integer (`Tempo::from_bpm`).
- **Snapping to beats is done by both**: the screen snaps with the map (`nextBeat`) for the look while dragging, and the core snaps again by the same rule (nearest beat; at exactly half, the later). K for a marker sends no position (only the core knows the heard position while playing).
- **The name 「マーカー 4」 is made by the screen** (`AddMarker {name}`; text `marker.newName` in `ui/src/text/ja.ts`; number = current marker count + 1). Reason: screen text is not kept in Rust (3-2).
- **The core owns the name box** (`renaming`): only the core knows to open it right after adding, so double-click, F2 and the context menu all go the same way (`OpenMarkerName`). The text being typed is only on the screen.
- Automation: `AddLane {track, target}`, `DeleteLane {track, target}`, `FoldLane {track, target, folded}`, `AddAutoPoint {lane, tick, value?}` (no `value` = the value on the line), `MoveAutoPoints {lane, points, dTick, dValue}`, `DeleteAutoPoints {lane, points}`, `SelectAutoPoints {lane, points}`, `SetLaneWrite {track, target, on}`, `RestoreLane {track, target}`.
- History: `HistoryJump {index, step}` (`step` is the step number; does nothing if stale, or during count-in / recording, like undo).
- Loop: existing intents (now steps). Mixer and params intents unchanged (21-11).
- Status line texts (`StatusMessage` in `crates/meloet-protocol/src/lib.rs`): tempo / time signature / marker selections and edits, `Undone` / `Redone` (Mod Z / Mod Shift Z name the step), `HistoryJumped`, `NothingToUndo`, `AutomationWriting` / `Rested` / `Restored` (spec chapter 6), `AutomationMoving` (mock F6-MixerAuto). `bar` and `beat` are shown numbers (from 1). A ramp on the last point does nothing, so it is told with `TempoPointSelected`.

`target` is `{kind: "volume" | "pan" | "send" | "mute" | "instrument" | "insert", bus?, insert?, param?}` (`insert` is `InsertView.id`; `param` is the table index for built-ins and `ParamRow.id` for plugins). `points` in point intents are indexes in the lane (tick order, from 0); chosen points are index ranges.

**Model steps added in version 21**: the 8 of 21-5, plus `RemoveInstrument {track}` (「外す」 a stopped plugin instrument; made in `crates/meloet-model/src/edit.rs`), and `field` on `SetNotesField` (start / key / length / velocity, to tell step names apart).

---

## 23. Finishing and shipping (round 8): sound pack download, development hooks, audio thread priority, memory, the WebKit CPU path

Research: `docs/history/research/distribution-formats.md`, `webkitgtk-2404-2604.md`, `sound-pack-download.md`, `dev-hooks-release.md`. Design: spec edition 12 (43-B, 44-A, 45-A).

### 23-1. The form of the sound pack (changes 19-1 and 19-8)

- **One file per sound** (so 45-A "sounds play as they arrive" works, and a new pack only refetches the sounds that changed). One sound = one tar, `<id>-<version>.tar`, holding `instrument.json` and `samples/*.flac` at the top (no folder above). FLAC is already compressed, so the tar is not. Credits and license texts (`credits.json`, `licenses/<id>/…`) are in a small shared `common-<version>.tar`.
- **Catalog** `catalog.json` (versioned JSON): `{"format": "meloet-sounds-catalog", "version": 1, "pack": "2026.12.0", "common": {id, version, file, url, bytes, sha256}, "instruments": [{…, "credits": […]}]}`. `id` is the sound name used in song files (`grandPiano`). A sound's version is the first 16 hex digits of a sha256 over its files' paths and sha256s, so the same content gives the same version and rebuilding the pack does not refetch unchanged sounds.
- **No signature during v0.x** (the owner's decision). Instead, a catalog may only point under `https://github.com/wintermaples/meloet/releases/download/` (`RELEASES` in `crates/meloet-core/src/sounds/catalog.rs`); a catalog with any other URL is not used. What this protects: TLS (the other end is GitHub), the catalog's sha256 (broken or changed bytes on the way), immutable releases (uploaded files cannot be replaced), and the built-in catalog (the first 240 MB are right). What it does not: someone who takes over the account or a token and rewrites the catalog on `main` (they can only make meloet fetch sound files; no program runs). Catalog checking is in one place (`check` in `catalog.rs`) so a minisign signature (`minisign-verify`, MIT) can be added there later.
- **Where**: `$XDG_DATA_HOME/meloet/sounds/<id>/<version>/` (`~/.local/share` if unset; `XDG_DATA_HOME` is used only if absolute). The kept catalog is `…/sounds/catalog.json`. Reason: it can be fetched again but is 240 MB and cannot be fetched offline, and `~/.cache` is expected to be cleaned by tools. Copies of audio files (`audiofiles/`) can be rebuilt, so they stay in `XDG_CACHE_HOME`.
- **Search order** (changes 19-1): see 23-9.
- **Building**: `tools/sounds/build.py` makes one folder as before; `tools/sounds/split.py` makes the per-sound tars and the catalog (23-9). One GitHub Release per pack version (tag `sounds-<pack version>`, immutable). If an earlier Release has the same version of a sound, the catalog uses its URL and the file is not uploaded again. Ask the owner before uploading.
- **The built-in catalog** is put into the executable at build time (`include_str!` of `tools/sounds/catalog.json`), so the first fetch does not wait for the catalog on the net.

### 23-2. The fetcher (43-B, 44-A, 45-A)

- One thread in the core (`crates/meloet-core/src/sounds/fetch.rs`), far from the audio thread. HTTP is `ureq` 3 (MIT OR Apache-2.0, blocking), TLS is rustls + ring, certificates from the PC (`rustls-platform-verifier`), falling back to `webpki-roots`. `https_only`, up to 10 redirects, User-Agent only `meloet/<version>` (no OS, no name). Proxies from environment variables.
- **Nothing is fetched until the user answers** (43-B). The answer is kept in settings (per PC). 「あとで」 asks again at the next start.
- **Order**: as 45-A, grand piano and studio kit first, then B-95-1 of `docs/history/design-notes.ja.md`. One at a time (not in parallel, so the grand piano arrives sooner).
- **Resume**: write to `<file>.part`; after a cut, ask for the rest with `Range`. **Always check size and sha256 at the end**; on a mismatch, delete and silently fetch once more; a second mismatch is "broken" (chapter 6 text). `If-Range` is not relied on. Details in 23-9.
- **Placing**: after the check, unpack into a work folder on the same file system and `rename` into `<id>/<version>/` (safe write, same idea as chapter 14).
- **Free space**: checked with `statvfs` before starting (rustix, already in Cargo.lock), and `ENOSPC` while writing means the same "disk full". The exact amount is in 23-9.
- **Sounds play as they arrive**: once placed, the core loads it (the existing "when it loads, rebuild every seat with that sound" flow in `play/sounds.rs`) and swaps the seats; engine swaps still overlap 10 ms. **Seats before that use the sine stand-in** (chapter 3, 「取れるまで鳴らす音」). The song file holds the real sound name.
- **Stop**: the file being fetched stays as `.part` and the next fetch resumes.
- **This is the only place that goes on the net.** The page (WebView) cannot go out because of CSP `connect-src`.

### 23-3. The check at start

- Whether there is a new pack is checked **without holding up start** (after the screen's greeting, on the fetcher thread) by **fetching one catalog** (one `GET`; nothing about the user is sent).
- **At most once per 24 hours.** The time of the last check is written in the normal state folder, and no check is made until 24 hours have passed. Reason: the pack changes less than once a month; checking more only leaves more IP logs at GitHub. Sparkle (macOS updates) also defaults to one day.
- **Offline, it says nothing** (no notice, no log line; next start follows the 24-hour rule).
- No check for people who have no sounds yet (people who said 「あとで」); the question comes first.
- The catalog on the net needs a fixed URL outside Releases (Releases are immutable per version): `https://raw.githubusercontent.com/wintermaples/meloet/main/tools/sounds/catalog.json` (`ONLINE` in `catalog.rs`). It is the same file as the built-in catalog; to ship a new pack, rewrite this file on `main` and older meloet versions find it. There is no `.minisig` (23-1). Details in 23-9.

### 23-4. Release builds versus development builds

- Rust: feature **`dev-hooks`** on `meloet-app`, passed on to `meloet-core/dev-hooks` and others, not in `default`. Hooks are removed with `#[cfg(feature = "dev-hooks")]` attributes on modules, items and statements (also on `generate_handler!` entries). `cfg!(…)` is not used (its strings stay in the binary); `cfg(debug_assertions)` is not used (release could not be measured). The full list is in 23-11.
- Screen: the `__DEV_HOOKS__` `define` (true only with `MELOET_DEV_HOOKS=1`); `ui/src/dev/` is loaded only inside `if (__DEV_HOOKS__)`. (The planned `resolve.alias` fallback was not needed, 23-11.)
- Measuring tools (`tools/measure/`) are built with `--features dev-hooks` and `MELOET_DEV_HOOKS=1` (`tools/measure/build-dev.sh`).
- **Guard test** `scripts/check-release.sh` (23-11). A shipping build names `-p meloet-app` alone (`--workspace` would unite features and pull hooks in).

### 23-5. Package form and version names

- **Form: deb** (the owner's decision). Built on Ubuntu 24.04 (in a container, `packaging/Dockerfile`, `scripts/build-deb.sh`) with hand-written dependencies that work on both 24.04 and 26.04. The current list is in `crates/meloet-app/tauri.conf.json` (`bundle.linux.deb.depends`: `libc6 (>= 2.39)`, `libasound2t64 | libasound2`, `libpipewire-0.3-0t64 | libpipewire-0.3-0`, `liblilv-0-0`, `libdbus-1-3`, `libxcb1`, `libstdc++6`; `recommends: pipewire, wireplumber`), which replaces the first plan. `meloet-plughost` goes next to the main binary in `/usr/bin/` (`bundle.externalBin`, passed by `build-deb.sh`) and is found next to `current_exe()` (`launch::spawn`). `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE` go to `/usr/share/doc/meloet/`.
  - No AppImage (it breaks when the host's Mesa / NVIDIA differ). No Flatpak (host plugins are not visible).
- **Version**: the first shipped version is `0.1.0` (as in `Cargo.toml`), shown in 「meloet について」. It is a different number from the song file version and the protocol version. The pack version (`2026.12.0` …) is date-shaped and separate again.
- **WebKitGTK**: 24.04 also has 2.52.6, so the behaviour meloet relies on (redraw, GC, the WebGL limit of 16) is the same in source. The WebKitGTK on this PC went from `.1` to `.2` on 2026-11-21, which changed the drawing path (see chapter 7 and 23-13).

### 23-6. Decisions with reasons (summary)

- No signature in v0.x; the fetch URLs are limited instead (23-1, the owner's decision).
- Once per 24 hours (23-3).
- Sounds in DATA, not cache (23-1).
- `ureq`: no tokio; one thread is enough. `reqwest` 0.13 defaults to aws-lc (many C parts).
- One file at a time: the simplest form that keeps 45-A (grand piano first). Parallel fetches do not make the line faster.

### 23-7 Protocol 27: the shape

This step only added the shapes, TS types and fake scenes; 23-8 and 23-9 made the core do it.

**1. Two or more audio clips chosen (39-B, 40-A)**

- `ParamsView::clips: Option<SelectedClips>` (`crates/meloet-protocol/src/audio.rs`). `Some` when two or more clips are chosen and **all** are audio clips (a keyboard clip in the mix gives `None`); then `selected` is `None`. Choosing an insert or instrument takes over the panel as with `selected`, and choosing clips again brings it back.
  - `SelectedClips { clips, tracks, gain_db, follow, semitones, cents, keep_formant }`. `clips` in the order of `ArrangeView::selected_clips`; `tracks` are their track ids (once each, in track order: 「6 歌 ・ 7 歌 2」). Each value is the shared value, or `None` if they differ (the panel shows 「—」).
  - Not a new arm of `SelectedParams`: that would break the screen's type checks (`selected.kind` in `RightRail.svelte`), and this step did not touch screen parts.
- Intents (each one step; step names `SetClipGain` / `SetClipFollow` / `SetClipPitch`, `StepWhat::Clips { count }`):
  - `ShiftClipGains { clips, by_db }`: on release of a gain-line drag; all move by the same dB, each stopped at −60…+12. 0, a non-number, or no audio clips: nothing.
  - `SetClipGains { clips, db }`: the value typed in 「まとめて変える」's 「音量」, for all.
  - `SetClipsStretch { clips, change: StretchChange }`: 「ついていく」, 「音の高さ」 (semitones and cents apart), 「声の感じを保つ」. `SourceBpm` does nothing. Clips already at the value stay; if all are, no step.
  - Drags move only the screen and send one intent on release. A single clip still uses `SetClipField` / `SetClipStretch`.
- The mock status line for F8-MultiClip is made by the screen from `params.clips` (no new notice).

**2. The check before export (41-A, 42-C)**

- `ExportCheck::Counting` and `Done` carry `over: u32`: the number of places so far above 0 dB (counted as B-95-2: a run of frames over is one place; after 100 ms not over, the next one is a new place). `peak_db` is the highest point (inside an over place if there is one). `over` is counted for every bit depth. `clips` (whether the chosen format will clip) is as before: `false` for 32-bit even with `over` ≥ 1.
  - 「用意をしています」 is the existing `Waiting`; 「たしかめています 38 %」 and the peak so far are `Counting`.
- `ExportDialogView::solo: Option<ExportSolo { first: TrackRef, others }>`: the soloed tracks (first in track order and how many others), for 「いま 2 ベース をソロにしています。…」. Solo is not applied to exports (as before).
- The status line parts 「 ・ たしかめています 38 %」 and 「 ・ 0 dB をこえる所があります」 (F8-ExportCheck, F8-ExportClip) are made by the screen from `ExportDialogOpen` and `export.dialog.check`.

**3. Notices from the core (chapter 6, edition 12)** (`StatusMessage`; texts in `ui/src/text/ja.ts`, made into text by `message` in `StatusLine.svelte`): `InputLost { input, device }`, `RecordingGaps { count }`, `AudioFound { file, clips }`, `AudioFoundMismatch`, `AudioBroken { file }` / `AudioTooManyChannels { file }` (instead of `AudioUnreadable`), `SavingAudio { done, total, done_bytes, total_bytes }`, `DiskTooSlow`, and `same` on `SourceTempoFound`. When each is sent: 23-8.

- 「遅れを測れない ・ ばらつき」 and 「… ・ 入口がない」 are dialog texts on the existing `LatencyFailure::Unsteady` / `NoInput` (`latency.failedUnsteady`, `latency.failedNoInput`).
- `msg.sourceTempoFound` is joined with `msg.sourceTempoSet` or `msg.sourceTempoSame`, because chapter 6 writes the first half of the "same" text as 「…」.

**4. Sound pack (43-B, 44-A, 45-A, 23-1 to 23-3)** (`crates/meloet-protocol/src/pack.rs`)

- A new part `AppState::sounds` (`Change::Sounds`) = `SoundsView`. Reason: the numbers change many times a second while fetching; inside `shelf` or `workspace` they would resend the shelf or bands each time (same idea as `audioWork` and `export`). The core sends it at most 4 times a second.
- `SoundsView`:
  - `ask`: the band asks 「音の素材（約 240 MB）を取りにいきます」. True when something is missing and there is no answer yet (first start, and every start after 「あとで」). While the flow 1–4 hints are shown (no keyboard, no output yet, sound came out …), the screen waits; it can tell from `keyboards`, `outputs`, `workspace.chooser`, `workspace.hint`, so nothing was added to the protocol.
  - `fetch: PackFetchView` = `Idle` / `Fetching { instrument, done_bytes, total_bytes, remaining_s }` / `Failed { failure, done_bytes, total_bytes }`. % is `done_bytes / total_bytes`; `instrument` is the sound being fetched (`None` for the common file); `done_bytes` includes earlier runs and `.part`; `remaining_s` is `None` until there is an estimate. The band (44-A) shows unless `Idle`; while flow hints show, it moves to the status line (44-B; the screen decides).
  - `PackFailure` = `Offline` / `CutOff` / `DiskFull { needed_bytes, free_bytes }` / `Corrupt` (after one silent refetch).
  - `have` / `total`: shelf sounds that play with their own samples (the 5 computed ones count) and all (31). Advanced settings row: 「ぜんぶあります ・ 240 MB」 when equal, else 「まだ 12 / 31」.
  - `pack_bytes`, `new_pack_bytes` (size of a new pack found at start; `None` once fetched), `place` (the folder, for advanced settings and 「meloet について」).
- `ShelfView::not_yet: Vec<InstrumentKind>`: sounds not yet fetched (shelf order), not in `available`. The shelf lists both in shelf order; `not_yet` rows have the name in gray.8 and 「まだ」 (「取ってきています」 when it is `fetch.instrument`), no star, ▶ or audition.
- A track on the sine stand-in is the existing `StandIn::PackMissing`; whether it is not fetched yet or broken is told by `sounds` (no new `StandIn` arm, since `standInTip` in `sounds.ts` switches on it).
- Intents (not steps): `FetchPack` (「取りにいく」, 「もう一度」, 「続きから」 and the other fetch buttons; stores the answer; nothing while fetching or with nothing to fetch), `PackLater` (「あとで」; `ask` false, `fetch` `Idle`; asks again next start), `StopPackFetch` (「やめる」; what arrived stays, the next `FetchPack` resumes).
- Notices: `PackAsking` (mock F8-PackAsk, while `ask`), `SoundReady { instrument }` (45-A, when more are still coming), `PackFetched { count }`, `PackFailed { failure, done_bytes, total_bytes }` (button 「もう一度」, or 「続きから」 for `CutOff`; both `FetchPack`), `NewPack { bytes }` (once, when the start check finds one).
- Screen texts are under `pack.*`, `shelf.pack*`, `settings.pack*` and `button.fetchPack` / `resumeFetch` / `packLater` in `ja.ts`. MB is 1000 × 1000 bytes.

**5. TS and fakes**: types are written by `cargo test -p meloet-protocol` (exported from `ui/src/bridge/types.ts`); fakes and scenes (`packAsk`, `packGet`, `packFail*`, `packSettings`, `packNew`, `multiClip`, `exportCheck`, `exportClip`) are in `ui/src/bridge/fakeRound8.ts`; tests in `crates/meloet-protocol/src/tests27.rs` and `ui/tests/bridge.protocol27.test.ts`.

### 23-8 Protocol 27: the core

The protocol shapes are those of 23-7. The sound pack is in 23-9.

**1. Two or more audio clips**

- Model: `meloet_model::set_clips_shape(doc, clips, change, step)` (`crates/meloet-model/src/audio/clips.rs`) changes the `AudioShape` of the chosen audio clips in one `Edit`, skipping keyboard clips, missing ids and clips that do not change. If none changes: `Unchanged` (no step).
- Steps: `StepKind::SetClipsGain { count }`, `SetClipsFollow { count }`, `SetClipsPitch { count }` (`count` = clips that really changed). Rows use `StepName::SetClipGain` / `SetClipFollow` / `SetClipPitch`, `StepWhat::Clips { count }`, place `StepPlace::Arrange` (the clips may span tracks, so no track name).
- Core (`crates/meloet-core/src/play/audio.rs`): `Play::set_clip_gains` (`ClipGains::By` for the line drag, `To` for a typed value; both clamp to −60…+12 through `ClipGain::from_db`), `Play::set_clips_stretch` (semitones ±12, cents ±50; `SourceBpm` does nothing). Nothing during recording or count-in.
- Panel: `Play::selected_vocal_clips` makes the `SelectedClips`; `params_view` in `state/audio.rs` puts it in `ParamsView::clips` by the same rule as one clip, with `selected` `None`.

**2. The check before export**

- Counting (B-95-2): `OverPlaces` in `export/job.rs`. It looks at the frames that go into the file (the head latency is not in it, the tail is). A frame where either channel is above 1.0 is over (exactly 1.0 is not). After 4800 frames not over (100 ms at 48 kHz), the next over frame starts a new place.
- Per-track export sums the counts of the files. `ExportNews::Counted` carries `over`. `clips` = `over > 0` and not 32-bit. What is counted is the master before the final cut: 23-16.
- Solo: `ExportDialogView::solo` from `export::Scene::soloed`. Solo is not in `Asked`, so changing it does not restart the export job.

**3. Notices from the core**

| Notice | Where |
|---|---|
| `InputLost` | `Input::InputLost` while recording vocals (`state.rs`). `input` is the input of the first vocal track being recorded; `device` is the input name remembered when opened (`EndpointView::name`; works even after it left the list; `wanted_name` in `state/vocal.rs`) |
| `RecordingGaps` | `Input::VocalsWritten` (after stop, when the disk worker has finished), if `gap_count + dropped_blocks` ≥ 1. The output-gone notice and `InputLost` come first |
| `AudioFound` / `AudioFoundMismatch` | `found: Finding` of `AudioNews::Found` (`Taken` / `Mismatch` / `Unreadable`). `Mismatch`: readable but sample rate or channel count differ (also 3+ channels). `file` is the name the song is looking for; `clips` counts clips in the shown takes that use it |
| `AudioBroken` / `AudioTooManyChannels` | `Brought::Unreadable(name, Unread)`. `ReadError { why: Unread }` in `audiofiles/decode.rs`: 3+ channels → `TooManyChannels`; else a name ending in wav / mp3 / flac / ogg (`AUDIO_EXTENSIONS`) → `Broken`, otherwise `NotAudio` (the old `AudioUnreadable`) |
| `SavingAudio` | while a save brings audio into the song folder (`Settling` of `AudioStore::settle`): silent for the first 250 ms, then at most once per 250 ms. Files that are renamed add their size when done. Gone when the save ends |
| `DiskTooSlow` | when frames the vocal read-ahead missed go up (`EngineMeters::stream_missing`, `log_missing` in `service.rs`), `Input::DiskTooSlow`. Once while running; can be said again after stop |

- **One change to saving**: a song that uses audio is written on its own thread even without plugins (`save_song` in `service.rs`); on the core thread it could not send `SavingAudio` while copying. The answer comes back later as `Msg::Input`, as for songs with plugins.

### 23-9 The sound pack fetcher (built)

This is how 23-1 to 23-3 were built. No signature (23-1); catalog checking is in one place, `check` in `sounds/catalog.rs`.

**The pack (tool)**

- `tools/sounds/split.py <build.py output> --out <dir> --pack 2026.12.0 [--previous <older catalog>] [--catalog tools/sounds/catalog.json]` makes the per-sound tars, the common tar and `catalog.json` (form in 23-1). The tars are ustar, uncompressed, in name order, with no owner or time, so the same folder gives the same bytes. With `--previous`, a sound with the same version keeps its old URL (not uploaded again). URLs are `https://github.com/wintermaples/meloet/releases/download/sounds-<pack version>/<file>`.
- The catalog built from `target/sounds/dev` (26 sounds + common, 238,909,440 bytes) is `tools/sounds/catalog.json`, built into the executable. Whether the dev pack is the shipped pack, and uploading the Release, are decisions for the lead / the owner.
- Removed: `tools/sounds/pack.json`, `tools/sounds/pack.py` (the one-tar form), `scripts/fetch-sounds.sh` (meloet fetches by itself). Development stays `build.py` → `target/sounds/dev` → `ln -sfn ../sounds/dev target/debug/sounds`. The old Release `sounds-2026.11.1` remains.

**Search order and places** (`crates/meloet-core/src/sounds.rs`, `sounds/fetch.rs`)

- `MELOET_SOUNDS_DIR` (development only; if set, only it, and no fetching) → `sounds/` beside the executable when it has a `pack.json` (the `target/*/sounds` of a development build; does nothing in a deb) → `$XDG_DATA_HOME/meloet/sounds/<id>/<version>/`. Per sound: sounds in the development folder are not fetched (`Disk::covered`); others come from DATA (the catalog's version first, else any other version, so an old version plays until the new one arrives). The old `~/.cache/meloet/sounds/<pack version>/` is not read.
- The catalog used is whichever of the built-in one and the one kept from the net (`…/sounds/catalog.json`) has the higher pack version. A kept one is not used unless it passes `check`.
- While fetching: `…/sounds/<file>.part`, unpacked in `…/sounds/.work-<file>/` (same file system), then `rename` to `<id>/<version>/`. At the start of the fetcher thread, leftover `.work-*`, `.part` files not in the catalog, and other versions of sounds whose catalog version is present are deleted (not while running, since they may be in use).

**The fetcher** (`Fetcher` in `sounds/fetch.rs`; one thread `meloet-pack`; the only code that goes on the net)

- ureq 3.5.2 (rustls + ring, `platform-verifier`, no gzip). `https_only`, 10 redirects, User-Agent `meloet/<version>` only, `http_status_as_error(false)`. 15 s for connect and response head, 30 s per read (then "cut off"). Only if the PC has no readable CA certificates at all (`rustls-platform-verifier`'s "No CA certificates") is `webpki-roots` used. Proxies: ureq's defaults (`ALL_PROXY`, `HTTPS_PROXY`, `NO_PROXY` …).
- Order: common → B-95-1 (grand piano, studio kit, finger bass, strings, electric piano, organ → shelf order). One at a time.
- One file: `Range: bytes=N-` from the `.part` size. 206 with `Content-Range` starting at N: append (otherwise delete and start over). 200: start over. 416: delete and start over. 403 / 410 (expired signed URL): follow the catalog URL again (twice at most, then `CutOff`). Any other answer (404, 5xx …): `CutOff` (the net is there, so do not say 「つながっていません」). A cut in the middle: wait 1 s and silently resume (twice at most, then `CutOff`, keeping `.part`). At the end, always size and sha256. Mismatch, cannot unpack, or no `instrument.json` (`credits.json` for common) → delete `.part`, silently fetch once more; again → `Corrupt`. The unpacker (`sounds/untar.rs`) accepts only ustar files and folders; links, pax, names that leave the folder, and `\` are refused per file.
- Free space: before starting, `statvfs` (rustix) `f_bavail × f_frsize` is compared with "what is left + the largest file left (two copies exist while unpacking) + 64 MiB" (`ROOM_BYTES`). `StorageFull` / `QuotaExceeded` while writing or unpacking also give `DiskFull { needed_bytes: same formula, free_bytes: free now }`.
- Numbers: `total` is the whole pack (minus what the dev folder covers) if any sound is missing, or only what changed for a new pack. `done` = `total` − what is left (`.part` counts as arrived). Notices at most every 250 ms (to the screen at most 4 a second). `remaining_s` from 3 s on, at this run's speed.
- 「やめる」: `stop(round)` stops that run's read (`.part` stays). The core bumps the run number and ignores late news from a stopped run (sounds already placed are taken).

**Check at start** (23-3): on the screen's greeting (`Core::subscribe` → `Input::ScreenUp`, once), `Job::Check`. No check if DATA has nothing fetched, or if `sounds-checked` (Unix ms) in the normal `MELOET_STATE_DIR` place is less than 24 hours old. Once it decides to check, it first writes the time (also when offline). One `GET` (up to 1 MiB); offline, not 200, or unreadable → silent. If the catalog passes `check` and is newer, it is kept in `catalog.json` and used. If an older version of a sound is here: `NewPack { bytes: what changed }`.

**Core rules** (`state/pack.rs`)

- The answer is `pack_fetch` in `settings.json`: true on 「取りにいく」; false on 「あとで」, 「やめる」 or when all have arrived. At start, if true and something is missing, it silently resumes after the greeting (after the check). If false and some sound is missing (not just a new pack's difference): `ask`. After `Failed` it stays true (resume next start).
- During `ask` the status line says `PackAsking` (after the flow 1–4 lines, no keyboard, no output; before the new song line). `SoundReady` when a sound that was not here arrives (not for a new version of one), then `PackFetched { count: have }`, `PackFailed`, `NewPack` (cleared by the next action).
- An arriving sound: the service calls `Library::add` (new path) → `Play::sound_installed` → `LoadSound` if the song or shelf uses it → when loaded, rebuild every seat (existing flow; engine swaps overlap 10 ms). Until then seats keep the sine. A new version is `DropSound` then `LoadSound`.
- New song: the first track is grand piano even before fetching (if the catalog has it and the last fetch was not `Corrupt`; otherwise the sine 「シンセ」 as before), and the file says `grandPiano`. The untouched new song at start is rebuilt once the catalog is known (`Play::start_pack`).
- Shelf: `available` = the 5 computed sounds + sounds here; `not_yet` = in the catalog but not here (shelf order). Only with no catalog and no pack is it sine only. `LibraryView::pack_missing` as before.
- `SoundsView`: `have` = number of `available` (with the 5 computed), `total` 31, `place` like `~/.local/share/meloet/sounds` (home shown as `~`). Without a fetcher (tests, `MELOET_SOUNDS_DIR`, no home): "all there" (31 / 31).

**Numbers**: 64 MiB spare, 24 h between checks, 15 s connect, 30 s read, 2 silent resumes (1 s apart), 2 re-follows on 403/410, 1 refetch when broken, notices every 250 ms, remaining time from 3 s, net catalog up to 1 MiB, versions of 16 hex digits.

**Test hooks** (removed in release, 23-11): `MELOET_PACK_ORIGIN` (e.g. `http://127.0.0.1:<port>/`; replaces the allowed URL prefix and the net catalog `<origin>catalog.json`, and drops `https_only` for `http://`), `MELOET_PACK_CATALOG` (a file instead of the built-in catalog), and `MELOET_SOUNDS_DIR` / `MELOET_STATE_DIR`. Free space, `ENOSPC` and time are replaced only through `cfg(test)` `fetch::Hooks` (no env vars).

**Tests**: `sounds/fetch_tests.rs` with a small local HTTP server (never the real net) covers each answer above, cuts, broken files, disk full, offline, stop, the net catalog and catalogs with URLs outside the rule; `state/pack_tests.rs` the core rules; `sounds/untar.rs` the unpacker. In the real app, stop and fetch again resumed with `Range`, and key → DAC while fetching stayed 11.4–15.4 ms.

### 23-11 Development hooks removed from release builds

How 23-4 was built. Research: `docs/history/research/dev-hooks-release.md`.

**Features**

- `meloet-app/dev-hooks` = `meloet-core/dev-hooks` + `dep:cairo-rs` (used only by `MELOET_SNAPSHOT`). `meloet-core/dev-hooks` = `meloet-midi/dev-hooks` (and `meloet-engine/dev-hooks` for `MELOET_ENGINE_RT`, 23-10). `meloet-plughost/dev-hooks` is a separate binary and is turned on by itself. None is in `default`.
- Everything removed uses `#[cfg(feature = "dev-hooks")]` (modules, functions, statements, struct fields, `generate_handler!` entries). No `cfg!(…)` or `cfg(debug_assertions)`. plughost uses `#[cfg(any(test, feature = "dev-hooks"))]` (its own tests use the fake plugin).
- Tests: `meloet-core` lists itself with the feature in `[dev-dependencies]` (like `rt-check` in `meloet-engine`). Binary-only crates (`meloet-app`, `meloet-plughost`) cannot (Cargo warns that a dependency without a lib is ignored), so plughost's integration tests and `meloet-core`'s tests run `cargo build -p meloet-plughost --features dev-hooks` themselves (`exe()` in `tests/common/mod.rs`, `meloet-core/tests/plughost/mod.rs`). `devhooks.rs` tests in `meloet-app` run only with `--features dev-hooks`. `scripts/check-all.sh` runs `cargo clippy --all-targets --features dev-hooks`, `cargo clippy` (no feature, the shipped form) and `cargo test --features dev-hooks` (on the workspace, the feature is set only on crates that have it).
- Screen: `__DEV_HOOKS__` in `define` of `ui/vite.config.ts` (true with `MELOET_DEV_HOOKS=1` or under vitest; type in `ui/src/dev/flag.d.ts`). Only the measuring part of `gl/Surface.svelte` (`measureEnabled` → `KeyToScreen`, `FrameProfile`, `uiScript` → `import("../dev/start")`) is inside `if (__DEV_HOOKS__)`. With that alone the production `vite build` (Vite 8.3.8 / Rolldown 1.2.19) drops the `start-*.js` chunk and the strings `measure_enabled`, `measure_report`, `ui_script`, `__meloetMeasure`, `key-to-screen` (unused bridge functions and `gl/measure.ts` / `profile.ts` are tree-shaken). `resolve.alias` was not needed. `bridge/tauri.ts` was not changed.
- Measuring tools are built by `tools/measure/build-dev.sh` (`MELOET_DEV_HOOKS=1 npm run build` and `cargo build --release -p meloet-app -p meloet-midi -p meloet-plughost --features dev-hooks`). `latency.sh`, `long-record.sh`, `ui-script.sh` and `tools/design/shoot_app.py` call it; other tools use the `target/release` it made.

**Removed names** (not in the shipped `meloet` / `meloet-plughost`)

- `meloet-app`: all of `devhooks.rs` (`MELOET_WINDOW`, `MELOET_SNAPSHOT`, `MELOET_SNAPSHOT_AFTER_MS`, `MELOET_COMMANDS`, `MELOET_EVAL`, `MELOET_PIPE`, `MELOET_EXIT_AFTER_MS`, `MELOET_CLOSE_AFTER_MS`, `MELOET_PCWINDOW_SHOT`, `MELOET_OPEN_SONG`, `MELOET_UI_SCRIPT`), commands `measure_enabled`, `measure_report`, `ui_script`, and `MELOET_DEBUG_SCREEN`.
- `meloet-core`: the whole `measure` module (`MELOET_MEASURE_DIR`, writing `key-to-sound.json`, `plugins.json`, `vocal-faults.log`, `CoreOptions::measure_dir`, sound load timing), the place overrides `MELOET_STATE_DIR`, `MELOET_CONFIG_DIR`, `MELOET_SF2_DIR`, `MELOET_SONGS_DIR`, `MELOET_SOUNDS_DIR`, the pack test hooks `MELOET_PACK_ORIGIN`, `MELOET_PACK_CATALOG`, `MELOET_INPUT_SINK_MONITOR`, and (added later) `MELOET_PLUGHOST_LOG_DIR` (23-14).
- `meloet-midi`: `MELOET_ACCEPT_VIRTUAL`, `MELOET_ONLY_VIRTUAL`, `MELOET_VKEYS_NAME`, `MELOET_MIDI_FAIL_AFTER` read by `MidiOptions::from_env` (without the feature: `MidiOptions::default()`, real keyboards only).
- `meloet-engine`: `MELOET_ENGINE_RT` (23-10).
- `meloet-plughost`: the fake plugin (`formats/fake.rs`, `MELOET_PLUGHOST_FAKE`, `urn:meloet:fake:`), `MELOET_PLUGHOST_RT`, `MELOET_PLUGHOST_NO_DUMP`, and the crash log (`crashlog.rs`, 23-14).
- Screen: the `ui/src/dev/` chunk, `gl/measure.ts`, `gl/profile.ts`, and calls to the measuring commands.

**Kept, and why**

- `JSC_useGenerationalGC` and `WEBKIT_DISABLE_COMPOSITING_MODE`: they are how the shipped meloet behaves (memory and drawing path). That a value set from outside wins is kept for comparisons.
- The `MidiOptions` fields (`accept_virtual` …) and spotting virtual keyboards by the `meloet-vkeys` name (`meloet-midi/src/rules.rs`): tests set the fields directly. Without the feature nothing can set them from the environment, so they are always off.
- `sounds/` beside the executable (with `pack.json`): not an env var; it finds `target/*/sounds` of development builds. In a deb there is no `/usr/bin/sounds/pack.json`, so it does nothing.
- `XDG_*`, `HOME` and proxy variables: normal PC rules.
- Test-only names (`cfg(test)`, `MELOET_TIMING_*` / `MELOET_VTIMING_*` in `tests/` …) never reach the shipped binary.

**Guard test** `scripts/check-release.sh` (`--no-dev` skips the reverse test)

1. Reverse test (first): the `MELOET_DEV_HOOKS=1` `ui/dist` and the `--features dev-hooks` release **do** contain the names (so the search list is not stale): in `meloet`, `MELOET_PIPE`, `MELOET_UI_SCRIPT`, `MELOET_MEASURE_DIR`, `MELOET_STATE_DIR`, `MELOET_ACCEPT_VIRTUAL`, `MELOET_PACK_ORIGIN`, `devhooks`, `measure_report`; in `meloet-plughost`, `MELOET_PLUGHOST_FAKE`, `MELOET_PLUGHOST_RT`, `urn:meloet:fake`; in the page, `measure_report`, `ui_script`, `pr-scroll`.
2. Shipped form (after, so `ui/dist` and `target/release` are left in the shipped form): `npm run build` without `MELOET_DEV_HOOKS`, and `cargo build --release` without features for `-p meloet-app` and `-p meloet-plughost` **one at a time** (together, features would unite). `strings` finds none of `MELOET_` (every `MELOET_` name is a hook), `devhooks`, `measure_enabled`, `measure_report`, `ui_script`, `key-to-sound.json`, `vocal-faults.log`, `urn:meloet:fake`. `ui/dist` has no `start-*.js` and none of `MELOET_`, the command names, `uiScript`, `__meloetMeasure`, `key-to-screen`, `frame-times`, `pr-scroll`.
3. The page inside the executable: the `meloet-app` test `embedded_page::write_the_embedded_page` (`#[ignore]`) decodes the brotli `assets()` of `tauri::generate_context!()` into `MELOET_ASSETS_OUT`, which is searched as in 2 (closes the hole of a stale `ui/dist`).
- It takes about 3 minutes the first time and 44 s after, so `scripts/check-all.sh` runs it only with `--release`.

### 23-10 Audio thread priority

- **Form**: the engine raises it itself (`crates/meloet-engine/src/priority.rs`). The output and input callbacks write their own thread id (`gettid`) to an atomic once, on the first call. A helper thread per stream, `meloet-rt-ask` (started when the stream opens), looks every 10 ms, raises the thread when it sees the id, writes one log line and ends. The callback never makes calls that may wait (D-Bus, rlimit). If the stream closes before the first callback, the helper ends quietly (it watches the count of the `Arc` the callback holds).
- **How**: the same order as the plugin host (chapter 20). If `RLIMIT_RTPRIO` ≥ 1, `sched_setscheduler(tid, SCHED_FIFO | SCHED_RESET_ON_FORK, 20)` (or the limit if lower). Otherwise rtkit `MakeThreadRealtime(tid, 20)` (setting `RLIMIT_RTTIME` to rtkit's `RTTimeUSecMax` first, or rtkit refuses). If both fail, it runs at normal priority. Log lines: `meloet: the audio output thread runs at real-time priority 20 (limit)` / `… (rtkit)` / `meloet: no real-time priority for the audio output thread: <reason>` (`audio input` for input). No protocol change.
- **Why 20**: the same as the audio threads of the plugin hosts that play. The engine waits for their answers, so it must not be below them (it would wait its turn to wake). Above them gains nothing, since it sleeps while waiting. rtkit's maximum is also 20. It stays below PipeWire's own threads (88 with limits).
- **Why not cpal's `realtime` feature** (read: cpal 0.18.2 `Cargo.toml`, `src/host/pipewire/stream.rs`, `rt_promote.rs`; audio_thread_priority 0.35.1 `src/lib.rs`, `src/rt_linux.rs`): (1) `realtime` alone does nothing on Linux (audio_thread_priority uses `rt_linux` only with its `dbus` feature); `realtime-dbus` would add a second dbus (0.6; meloet has 0.9.12). (2) rtkit only, no `RLIMIT_RTPRIO` path, and the priority is fixed at 10. (3) audio_thread_priority is MPL-2.0 (usable unchanged, but one more part). (4) Failure arrives at cpal's error callback and would mix into meloet's xrun counts. Doing it ourselves adds no parts (dbus 0.9.12 and libc are already there), matches the plugin host, and needs one log line.
- **On a normal Ubuntu**: the `pipewire-bin` package ships `/etc/security/limits.d/25-pw-rlimits.conf` (`@pipewire - rtprio 95`), but normally nobody is in the `pipewire` group. On this PC the `audio` group's rtprio 95 comes from `30-ubuntustudio-audio.conf` (not checked whether a normal Ubuntu has it; confidence C), so other PCs will usually take the rtkit path. rtkit's polkit rule (`org.freedesktop.RealtimeKit1.acquire-real-time`) says yes for active and inactive seat sessions and no otherwise; a meloet started from the desktop should pass (confidence C). The agent user on this PC has no seat and is refused by rtkit, so **the rtkit path has not been checked for real**.
- **Test hook**: `MELOET_ENGINE_RT` (only with `meloet-engine`'s `dev-hooks`, passed from `meloet-core/dev-hooks`): `off` asks nothing; `rtkit` asks rtkit even with limits. Same form as `MELOET_PLUGHOST_RT`. Tests: two in `priority.rs` (raised; helper ends if the stream closes before the first callback). The no-allocation callback guard in `tests/device.rs` (`--ignored`) still passes.
- **Measured** (`docs/history/measurements/2026-11-22-round8-rt.md`): `pw_out` runs FIFO 20 in the real app. With 16 Surge XT + compressor on a busy PC, blocks that missed the plugin answer went from 1458 (without) to 0–20 (with). Key → DAC did not change (p95 15.88 ms).

### 23-14 The Surge XT host stopping at the start of play

- **Found** (`docs/history/measurements/2026-11-22-round8-surge.md`): it did not crash; the engine was stopping a live host. The engine's watch (20-2, `EngineEnd::await_overdue` in `meloet-plugin`) took "no request picked up and the host's `progress` unmoved over two deadlines" as "stopped or gone". A freshly loaded Surge XT (1.3.4) takes 4–5 ms for its first block; with 16 in one host (the grouping rule of 20-1) all making their first sound in the same block, the host's 4 audio threads are all inside Surge XT and `progress` stops. With short blocks (always at 128 frames in tests) all 16 were stopped at the start of play. That this was the cause of the two earlier quiet-PC cases is not proven (no logs), but Surge XT never crashed in 59 real-app and 100+ test runs (confidence C).
- **Decided**: "dead" also requires that no slot of the host has a thread inside the plugin (`taken` ≠ `done`) for less than `HANG_MS`. A host with a thread inside a plugin is slow, not dead. Reason: more heavy work than the host's threads (at most 4) is normal under the grouping rule. A stopped host whose thread happened to be inside a plugin is found after `HANG_MS` (5 s, `crates/meloet-plugin/src/ipc/ends.rs`), like a hanging plugin. Gone hosts are still caught by the core's pidfd.
- **Not taken**: at most 4 plugins per host (more hosts and memory; breaks 20-1). Moving `progress` from the host's main thread too (useless if the audio threads are the ones stuck).
- **Logs**: `MELOET_PLUGHOST_LOG_DIR` (core `dev-hooks`) writes each host's stderr to a file; the host then writes a stack on fatal signals (`crates/meloet-plughost/src/crashlog.rs`, `dev-hooks`). If a host did not end by the core's SIGKILL, the core always logs `meloet: the process of <key> ended: <how>`.
- **Left**: Surge XT's heavy first block still drops a few blocks of the first chord (as before; it just no longer stops). On rtkit PCs the host's `RLIMIT_RTTIME` becomes 200 ms, and a long patch-loading block might get the host SIGKILLed (not checked).

### 23-12 Memory

Numbers: `docs/history/measurements/2026-11-22-round8-memory.md`; tool `tools/measure/memory.py`.

- **meloet's memory is mostly samples.** Started without samples: RSS 238 MB (PSS 99 MB; shared libraries and the NVIDIA mapping count in RSS but are shared). One grand piano adds 333 MB; 8 tracks of mixed sounds reach 908 MB (674 MiB of samples). 16 Surge XT weigh in the plugin host (290 MB, Surge's own); 16 vocal tracks add 18 MB (read-ahead rings).
- **Freed sample memory goes back to the PC** (fixed). The sample loading thread puts samples in its glibc malloc arena; memory freed there was kept by malloc, so RSS did not drop (closing an 8-track song left 1051 MB).
  - Form: `SampleBank`'s `Drop` counts freed bytes (`meloet_sound::memory`). The core's 100 ms `housekeeping` checks: at 4 MiB or more, a separate thread `meloet-give-back` calls `malloc_trim(0)` once (glibc; every arena, including free pages in the middle). Never two at once.
  - Reason: samples are freed in many places (closing a song, replacing a sound, deleting a track, ending a shelf audition, reloading an SF2, a new sound version arriving), and where the last `Arc` drops (core, an instrument back from the engine, the shelf) is not fixed. Counting in `Drop` works on every path. `malloc_trim` takes 0.2–25 ms (24.8 ms for 334 MiB) and holds arena locks in turn, so it runs off the core thread. The audio thread does not allocate, so it never waits for those locks.
  - When samples are held and let go (not kept for undo; the shelf's "one before" only while open, etc.) is unchanged (`sync_sounds` in `play/sounds.rs`). Only "RSS goes down after freeing" changed.
  - Not taken: (1) `mallopt(M_MMAP_THRESHOLD)` to keep big samples outside malloc: changes malloc for the whole process, set at `meloet-app` start. (2) Our own mmap container: more unsafe in meloet-sound. (3) `M_ARENA_MAX`: does not change whether memory is returned.
- **"10 minutes of recording grows meloet from 102 to 232 MB" was not growth**: 102 MB is the 0 s sample before the page shows; from 30 s to 600 s it is flat (231.7 → 232.4 MB).
- ~~WebKitWebProcess grows on the CPU path even when stopped (+130 MB in 10 minutes of recording).~~ **Superseded by 23-15**: not reproduced; both paths grow alike and level off.

### 23-13 Arrange scrolling on the CPU path

Numbers: `docs/history/measurements/2026-11-22-round8-scroll.md`.

- **Where the cost is**: a scroll frame makes the whole time side (1676×904 at 1920×1080) one bounding rectangle. Everything moves, so fewer rectangles do not mean less area. Nearly half the drawing is canvases (WebKit reads the WebGL picture to the CPU, puts it into a GPU ImageBuffer, then reads it back to draw) and filling the background (new shared memory each time). On the page side, what could be cut was the wheel JS (3–4.5 ms each) and rewriting HTML on each scroll.
- **Form**:
  1. **Clip face rectangles are not rebuilt on each scroll** (`lanePicture` in `Arrange.svelte`). They are built for a window 3 view-widths wide (1 left, 2 right), and the offset from the window's left edge is applied with the `rects` slot's `shiftX` (`gl/slot.ts`; the engine draws rectangles and waveforms shifted left). The window is rebuilt when the view's left edge has moved one width, and on zoom or content change. Clips outside the window are not built, so zoom costs about the same as before.
  2. **Only waveforms are built each scroll** (`laneWaves`): their level (`framesPerPx`) and cut points (`straightParts`) depend on the visible part, so building them for the window would change the picture. They are drawn with the same `shiftX`.
  3. **Note shapes are remembered per take** (`laneShapesOf`): takes never change, so shapes are keyed by take and clip placement (start, end, offset), and only notes touching the window are taken by binary search (`notesNear`, starting back by the longest note). `keyRange` is over all the clip's notes, as before.
  4. **Ruler numbers move as one strip with `transform`** (`.bar-strip`); each number's `left` is from the song head and is not rewritten.
  5. **Labels at the left edge do not change width each scroll**: a label of a clip that reaches past two view-widths to the right is cut there (no name is longer). A width change ran layout inside the label every frame.
- **Not taken**: two label sets each moved by `transform` (crossing the left edge rebuilt elements and was slower); showing clip faces through a `bitmaprenderer` copy (no change).
- **Reading busyMs**: on the CPU path a frame comes as soon as the last one is drawn (it does not wait 16.7 ms on a 144 Hz screen). busyMs is "until this frame's picture is drawn" and can cover two frames' drawing. Whether 60 Hz holds is read from `frameGapMs` (mean and p95) in `arrange_parts.py`.
- **Result**: side scroll busyMs 20 → 13 ms mean, frame gap p95 15–16 → 12–13 ms; most zoomed-out scroll 24 → 14–16 ms. Zoom is 19–20 ms, **not inside 16.7 ms** (mean frame gap 11 ms). A song with 192 audio clips did not change (label text cost; the font task of 22-18). Key → screen while playing did not change.
- **Left**: zoom and scrolling songs with many audio clips still pay for canvas and text drawing. (`set_enable_accelerated_2d_canvas(false)` was thought of here; **23-15 found it does nothing in WebKitGTK 2.52.6**.)

### 23-15 WebKit memory on the CPU path

Numbers: `docs/history/measurements/2026-11-22-round8-webkit-memory.md`; tool `tools/measure/webkit_memory.py`.

- **Decision: keep the CPU path (`WEBKIT_DISABLE_COMPOSITING_MODE=1`) and add nothing.** It keeps key → drawn at p50 0.4–0.7 ms. The "+130 MB in 10 minutes even when stopped" of 23-12 did not appear in the current version. The CPU path and the compositing path grow about the same in the same scene (play 25 min: +18 / +16 MB; record 13 min: +20 / +17 MB) and level off after 20–25 minutes; both are flat when stopped. CPU-path play for 60 minutes: +38 MB (in steps). RSS is 40–170 MB lower on the CPU path. The memory target "10 minutes of recording: WebKit within +40 MB" is met.
- What grows is WebKitWebProcess's libpas (`fastMalloc` and JavaScriptCore) in-use memory, not memory freed and kept. Shared memory (`memfd` of each frame's `ShareableBitmap`) does not pile up. meloet itself (the UI process) does not grow.
- **Tried, no effect**: glibc malloc tunables (`GLIBC_TUNABLES` `trim_threshold` / `arena_max` / `mmap_threshold`); `Malloc=1` (no bmalloc; worse); the CPU path by setting (`set_hardware_acceleration_policy(Never)`: same path as the env var, same growth and time, and since it changes after the WebView is made a `Core: Scrolling` thread is left); `enable-2d-canvas-acceleration` false (in 2.52.6 a WebGL canvas on the CPU path always goes through `createImageBufferForWebGLContextReads`, which ignores this setting). `set_enable_accelerated_2d_canvas` of `webkit2gtk` 2.0.2 does nothing in 2.52.6.
- WebKitWebProcess disappearing after about 280 s did not happen again in about 4 hours of CPU-path runs.
- Note: on the CPU path, play keeps drawing 60+ frames a second even when the window is hidden or the screen is off (the compositing path stops). When measuring the compositing path, check from the page's frame count (`frames` in `frame-profile.json`) that the screen did not go off. Open: whether to stop drawing while hidden; memory beyond 2 hours of play.

### 23-16 The export check uses the sound before the final cut

Fixes "F8-ExportClip cannot be made" from `docs/history/measurements/2026-11-22-round8-end.md`.

- **Why**: the export engine (`ExportEngine` in `export/render.rs`) calls the same `Engine::process` as playback. At its end (after the master in `engine.rs`, where the output is written) the sound was cut to ±1.0. The check (`peak_of` and `OverPlaces` of `Taker` in `export/job.rs`) and the 32-bit work file got the cut sound, so the peak stopped at 0.0 dB and `over` was always 0. The cut had been there since round 1 and export since round 7 (the bug was old).
- **Fix**: `Engine::keep_overs()` (do not cut where the output is written; non-numbers still become 0). Only the export engine calls it; the playing engine still cuts (to protect ears and speakers).
  - The check's peak and `over` are from the master before the cut (after the master inserts; the metronome does not play in exports), after the limiter if there is one.
  - The work file (32-bit float) is not cut. The final 32-bit file is written as is (B-95-10, "32-bit does not clip"). 16 / 24-bit (and FLAC) are cut to ±1.0 where the file is written (`Whole::of` in `export/sink.rs`), as before.
  - No protocol change (`peak_db`, `over`, `clips` of `ExportCheck` mean the same; `clips` stays `false` for 32-bit).
- **"The same as the output, bit for bit", read again**: a song that never goes over 0 dB is still identical (`what_is_exported_is_what_plays_sample_for_sample`). A song that goes over is identical once the export is cut to ±1.0 (`what_goes_over_0_db_is_exported_as_it_is_and_cut_as_it_plays`). 16 / 24-bit values are cut before turning into integers, so they equal what would be made from the output.
- **Real app**: `shoot_app.py exportClipApp` (limiter removed, all faders +6 dB) shows 「いちばん大きい所 +11.7 dB」 and 「0 dB をこえる所が 1 か所あります（いちばん大きい所 +11.7 dB）。音が割れます。」 (a dense song never has 100 ms below 0 dB, so one place).
