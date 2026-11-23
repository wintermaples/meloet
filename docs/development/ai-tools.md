# The AI link's tools

The MCP tools of `meloet-ai` (94 after round 11), and for each tool added or changed in round 11 its arguments and its answer. Taken from the code (`crates/meloet-ai/src/tools.rs`, `crates/meloet-ai/src/tools/*.rs`): the argument structs and the `json!` answers. How they are built: [architecture 24-12, 24-14 and 24-16](../architecture.md). The schema each tool sends (`tools/list`) is the full truth; this page is a map.

## How every tool answers

- Every tool: `openWorldHint: false`; read tools `readOnlyHint: true`; tools that delete or put a whole state `destructiveHint: true`. Each description is at most 60 words (`every_tool_says_what_it_is` in `crates/meloet-ai/tests/link.rs`).
- An answer is a `CallToolResult` with a text and `structuredContent`. The text is one or a few lines for a person, then the same JSON as `structuredContent`. A failure is `isError: true` with a text that says why.
- **A write that worked** answers at least `{changed: true, step, undo_steps, clip, take, track}` (`done_value`): `step` is the history row made (null when the step comes later, as for plugin states and imports), `undo_steps` the number of steps that can be undone now, `clip` / `take` / `track` where the tool gives them (else null).
- **"No change"** (new in round 11) is a success, not an error: text "No change: the song already is like that.", `{changed: false, step: null, undo_steps}`. Tools that turn a value into a toggle (`set_mute`, `set_solo`, `bypass_effect`, `set_loop_on`, `add_lane` when the lane is there) answer the same when it is so already.
- **Tracks** (new in round 11): every track argument (`track`, `tracks`, `to_track`, `alone`, `compare`, 40 in all) takes the number from 1, as `song_overview` lists them, or the stable id as the string `"id:7"`. Ids do not move when tracks are added, moved or deleted. Answers give the id beside each number: `id` in a track object, `track_id` beside `track`, `<key>_ids` beside a list (`tracks` → `track_ids`). Not track arguments: `move_clips` `tracks` (rows to move), `move_track` `to`, `import_midi` `midi_track`.
- Places: "bar.beat.tick" (bar and beat from 1, tick from 0) or ticks from the song's start (1920 a quarter note). Where an argument is a `Place` it takes either.
- Plugin place (the tools of parameters, states, presets and Surge XT): `track` (number or id) or `bus` (`"master"`, `"return_a"`, `"return_b"`), and `slot` 1–4 for an insert; leave `slot` out for the track's instrument plugin.
- Long lists come in pages: `limit`, `offset`, and `total`, `count`, `next_offset` (null on the last page) in the answer.

## 5 and 11: batches and "no change"

### `batch` (new; destructive, not idempotent)

- Arguments: `operations`: 1–200 of `{tool: string (1–64), arguments?: object}`, run in order; each sees what the ones before it did. In `arguments`, a value may be `{"result_of": N, "field": "clip"}`: the field (`clip`, `take`, `track`, `track_id`) of operation N's result (from 1).
- Refused before anything runs: read tools, `batch`, `undo`, `save_song`, `save_song_as`, `open_song`, `new_song`, `import_audio`, `export_song`, `set_plugin_state`, `set_plugin_state_text`, `load_preset`, `save_preset`, `surge_set_modulation`, `surge_remove_modulation`, `surge_set_patch_params` ("Operation 2 (undo) cannot be in a batch: …. Nothing was run."). Plugin parameter changes are refused by meloet when their turn comes, and all is taken back.
- Answer when all worked: text "Done: N operations as one undo step." (or that someone else changed the song, so it is not one step), then `{changed, step (the one row, or null), undo_steps, operations, results: [{tool, result}]}`; each `result` is that tool's answer without `step` and `undo_steps`.
- Answer when one failed (`isError`): "Batch stopped at operation 5 of 6 (set_volume): …" and whether everything was taken back; `{changed, taken_back, failed: {operation, tool, error}, undo_steps, done_before}`. When meloet itself closed the batch (idle 2 s, too many held intents, quitting): "meloet closed the batch … took all of it back: nothing was changed. Run it again." with `{changed: false, taken_back: true}`.
- While a batch runs, what the person does in meloet waits and is done right after it.

## 3 and 10: notes in bulk, MIDI files

The note writes (`write_notes`, `copy_notes`, `import_midi`) answer the write fields, then:

| Field | What |
|---|---|
| `written` | Notes written |
| `clip_place` | `{start, end, id, track, track_id}` of the clip the notes went into, as it is now |
| `created_clip` | The same when the clip was made by this call, else null |
| `grew_to` | `{start, end}` when an existing clip grew, else null |
| `not_written` | Up to 200 of `{index, start, length, key, reason}`; reasons `past_new_clip_end`, `past_song_end`, `before_song_start`, `key_out_of_range`, `outside_source_clip` |
| `not_written_count`, `not_written_more` | How many in all, how many more than listed |
| `changed` | Up to 200 cut notes `{index, start, key, length_asked, length, reason}` (`cut_at_new_clip_end`, `cut_at_song_end`); note: here `changed` is this list, not the `true` / `false` of other writes |
| `changed_count`, `changed_more` | As for `not_written` |

### `write_notes` (changed)

- Arguments: `track` (number or id) **or** `clip` (id); `notes`: 1–2000, each an object `{at | start, length, key, velocity}` **or the array `[start, length, key, velocity]`** (new; velocity 100 when left out; start in ticks or "bar.beat.tick"); `origin` (new, a place: starts count from there).
- With `track`: into the clip that holds the first note (it grows by bars), or a new clip from the head of that note's bar, 4 bars or up to the next clip and longer when the notes need it. Another clip in the way: nothing is written and the answer names that clip.
- Answer: the note write fields above.

### `create_clip` (changed)

- Arguments: `track`, `at` ("bar.beat.tick"), and new: `bars` (1–9999) or `end` (a place, brought on to a bar head); neither: 4 bars or up to the next clip.
- Answer: the write fields plus `start`, `end`. A clip in the way is named in the error.

### `copy_notes` (new; not destructive, not idempotent)

- Arguments: `clip` (the source keys clip); `take` (the take id from `get_notes`, needed with `notes`: when the clip shows another take now, nothing is done); `notes` (indexes in the take) or `from` + `to` (places: notes that start in that range), or neither for every note; `to_track` (number or id) or `to_clip`; at most one of `shift_ticks`, `shift_bars` (−9999 to 9999, counted from the first note's bar), `at` (where the source clip's start, or `from`, goes); `transpose` (−127 to 127).
- Copies only notes that sound in the source clip, as long as they sound. Into a track: the clip that holds the first copied note, else a new clip exactly as long as the notes need.
- Answer: the note write fields (`index` is the index in the source take).

### `import_midi` (new; not destructive, not idempotent)

- Arguments: `path` (absolute, a standard MIDI file of formats 0 or 1, up to 8 MiB and 50 000 notes); `track` (number or id; not needed with `list_only`); `at` or `tick` (where the file's start goes; default the song's start); `midi_track` (from 1); `channel` (1–16); `transpose` (−127 to 127); `list_only`.
- `list_only`: "…: format F, N ticks a quarter note, T tracks, N notes. Nothing was changed." and `{path, format, ticks_per_quarter, scaled_exactly, notes, tracks: [{number, name, channels, notes, lowest_key, highest_key, length_ticks, length_quarters, hanging_notes}], tempo: [{tick, bpm}], tempo_count, meters: [{tick, meter}], meter_count}`.
- Import: one step; into the clip that holds the first note, else a new clip as long as the notes need. Answer: the note write fields plus `file` (the listing above with `tracks: null`), `asked`, `origin`, `hanging_notes`, `tempo_and_meter_applied: false` (the file's tempo and meter are told, not applied).

### `export_midi` (new; not destructive, idempotent)

- Arguments: `clip` (a keys clip), or `from` / `to` (places) and `tracks` (numbers or ids; default every keys track), or nothing for the whole song; `path` (absolute `.mid` in a folder that is there; default a file in the private temporary folder, deleted after an hour); `overwrite`; `ticks_per_quarter` 480, 960 or 1920 (default 1920, meloet's own ticks, so no note moves).
- Answer: `{file, bytes, format: 1, ticks_per_quarter, origin, end, notes, notes_cut_at_range_end, tracks: [{number, id, name, notes, channel}], tempo_events, meter_events, tempo_ramps_as_steps}`. Drum kits are on channel 10, other tracks on 1.

## 6 and 7: measuring

### `measure_mix` (changed; read only)

- Arguments: `from_bar`, `to_bar`; `tracks` (each measured alone; the mix holds only them); new: `alone` (each measured alone while the mix stays every track; not with `tracks`), `ignore_mute` (a track measured alone sounds even when muted in the song; a mute lane is still followed; the mix keeps the song's mutes).
- Answer: as before (`from_bar`, `to_bar`, `sample_rate`, `master`, `tracks`) plus new `mix` (for example "all tracks, mutes as in the song"), `solo_ignored: true`, `ignore_mute`; in `master` and each track new `lufs` (integrated, BS.1770-4) and `short_term_max_lufs` (the loudest 3 s); each track new `id` and `muted`. Each level: `peak_dbfs`, `rms_dbfs`, `lufs`, `short_term_max_lufs`, `over_0db_places`, `seconds`. A muted track's text line says it is silent and how to measure it with `ignore_mute`.

### `analyze_bands` (new; read only)

- Arguments: `from_bar`, `to_bar`, `tracks`, `alone`, `ignore_mute` (as `measure_mix`); `bands`: `"five"` (default: low < 120 Hz, low_mid 120–500, mid 500–2k, high_mid 2k–6k, high 6k and up) or `"octave"` (31.5 Hz to 16 kHz); `compare`: 2–8 of the tracks measured alone (default: those tracks when they are 2 to 8); `sections` (cut at markers); `every_bars` (1–256; cut every N bars).
- Answer: `{from_bar, to_bar, sample_rate, mix, solo_ignored, ignore_mute, bands: [{name, low_hz, high_hz}], method, master, tracks, overlap, sections?}`.
  - `master` and each of `tracks` (with `number`, `id`, `name`, `muted`): the levels of `measure_mix` and `bands: [{band, dbfs, share_percent}]`.
  - `overlap`: null, or `{how_to_read, window_s, top_band, pairs: [{a, a_id, b, b_id, top_band, bands: [{band, shared_dbfs, score}]}]}`; `score` is 0 to 1.
  - `sections` (only when asked): `[{from, to, marker, seconds, master, tracks: [{number, id, …}]}]` with peak, RMS, LUFS and the loudest 3 s of each.
- Text: a head line, a line of bands for the mix and each track, "Tracks a and b share most in …" for each pair, then the JSON.

## 8: key names

### `get_key_names` (new; read only)

- Arguments: `track` (a keys track: number or id).
- Answer: `{track, track_id, track_name, sound, kit, source, certain, keys: [{key, note ("C2"), name, source, id? (built-in kits), sounds? (when known)}], range: {low, high, low_note, high_note} | null, notes: [English sentences]}`. `source`: `builtin`, `sf2_sample_name`, `lv2_midnam`, `general_midi_guess` (only the usual layout; `certain: false`) or `none`.
- Errors: no such track; a vocal track; a plugin that does not answer in 5 s.

## 9: track ids

No new tool. Every track argument takes `"id:7"` too, and these answers gained ids: `song_overview` (`tracks[].id`, `clips[].track_id`), `get_notes` (`track_id`), `get_mixer` (`tracks[].id`), `get_automation` (`track` is now the resolved number, plus `track_id`), `add_track` (`track_id`), `measure_mix` and `analyze_bands` (`id`, `a_id`, `b_id`), `export_midi` (`tracks[].id`), the note writes and `create_clip` (`clip_place.track_id`, `created_clip.track_id`), `get_key_names` (`track_id`), the mute set tools (`*_ids`).

## 12: finding song files

### `list_songs` (new; read only)

- Arguments: `folder` (absolute; the song files in it and in the folders right under it); `limit` (1–500, default 100) and `offset` for the songs of a folder.
- A song: `{path, name, modified (ISO 8601 UTC), size, open_now}`. Only `<name>.meloet`, newest first; no `.prev`, temporary or hidden files; symbolic links are not followed.
- Without `folder`: `{song_open_now: song | null, recent: [song] (opened or saved lately, up to 20), songs_folder: {folder, total, offset, songs, complete} | null}` (with `exists: false` when the songs folder is not there). With `folder`: `{folder, total, offset, songs, complete}`; `complete: false` after 500 songs or 20 000 entries.

## 13: mute sets

| Tool | Arguments | Answer |
|---|---|---|
| `save_mute_set` (new) | `name` (1–64), `overwrite` (default false) | `{changed, set, step, undo_steps}`; the same contents again: `changed: false`, no step |
| `list_mute_sets` (new; read only) | `name` (only that set) | `{mute_sets: [{name, tracks, track_ids, muted, muted_ids, soloed, soloed_ids, muted_returns: ["return_a" …], deleted_track_ids, not_in_set, not_in_set_ids, as_now}]}` (tracks as numbers now) |
| `recall_mute_set` (new) | `name` | `{changed, set, step, undo_steps, solo_changed, muted, muted_ids, muted_returns, soloed, soloed_ids, deleted_track_ids, not_in_set, not_in_set_ids, rested_lanes, rested_lanes_ids}`; as now: `changed: false` |
| `delete_mute_set` (new; destructive) | `name` | `{deleted, step, undo_steps}` |

One step each. Solo comes with a set but is not part of the step. `deleted_track_ids` are plain integers.

## 2: plugin state

### `get_plugin_state` (new; read only)

- Arguments: the plugin place; `inline` (also the state as base64 when that is at most 256 KiB of text); `readable_inline` (Surge XT: the patch XML in the answer when at most 256 KiB).
- Answer: `{plugin: {name, vendor, key, format}, bytes, sha256, file (a 0600 file in the private temporary folder, deleted after an hour; null when inline), base64 (with inline), readable: {kind, bytes, file} | {kind, bytes, text} | null}`.

### `set_plugin_state` (new; destructive, not idempotent)

- Arguments: the plugin place; `base64` or `file` (absolute path), one of them; at most 64 MiB; an empty state is an error.
- Answer: the write fields (`step` is null) plus `given: {bytes, sha256}` and `now: {bytes, sha256}` (asked of the plugin again). The same state as the song's: "no change". A state the plugin refuses or that stops it: error, the song keeps its state. Only the same plugin in the same format (`vst3` or `lv2`) takes a state.

### `set_plugin_state_text` (new; destructive, not idempotent)

- Arguments: the plugin place; `text` or `file` (UTF-8): Surge XT's patch XML as `get_plugin_state` gives it in `readable`, edited. Other plugins are refused.
- Answer: the write fields plus `given: {bytes, sha256}`, `now: {bytes, sha256, readable_bytes}`. A broken patch changes nothing. Do not change effect types (`fxN_type`) here.

## 1: presets

### `list_presets` (new; read only)

- Arguments: the plugin place; `name`, `category` (parts, any case); `kind`: `"factory"`, `"user"` or `"all"` (default); `limit` (1–500, default 50), `offset`.
- Answer: `{plugin: {name, vendor, key, format}, total, offset, count, next_offset, categories: [{name, count}], categories_total, presets: [{id, name, category, factory, source}], sources: [{source, found, note}]}`. ids: `surge:factory/…`, `surge:3rdparty/…`, `surge:user/…`, `vstpreset:<absolute path>`, `lv2:<URI>`.

### `load_preset` (new; destructive)

- Arguments: the plugin place; `id` (from `list_presets`).
- Answer: the write fields plus `now: {bytes, sha256}` and `preset`. The same preset again (same id and the song's state unchanged since): `{changed: false, step: null, undo_steps, preset}`.

### `save_preset` (new; destructive)

- Arguments: the plugin place; `name` (1–64, no `/` or `\` or control characters); `category` (Surge XT and VST3 only); `overwrite` (default false).
- Answer: `{preset, written (path), overwrote, replaced_characters, notes}`. No step. VST3 into `~/.vst3/presets/<vendor>/<plugin>/`, LV2 into `~/.lv2/<Plugin>_<Name>.preset.lv2/`, Surge XT into its user `Patches` folder.

## 2 and 1: Surge XT

All take the plugin place and refuse a plugin that is not Surge XT. Writes read the state now, change the patch XML, put it as one step, and read back; nothing changed → `changed: false`.

| Tool | Arguments | Answer |
|---|---|---|
| `surge_list_modulations` (new; read only) | `target` (part of a patch name, such as "pitch"), `source` (number, short name "vlfo_1" or name "LFO 1"), `limit` (default 100, at most 500), `offset` | `{plugin: {name, format}, patch: {name, category, revision}, routings: [{target, target_name, source: {id, short, name}, depth, muted, source_index, source_scene}], page: {total, offset, next_offset}}` |
| `surge_get_patch_params` (new; read only) | `name` (part of a patch name, such as "a_lfo0_"), `limit` (default 50, at most 500), `offset` | `{patch, params: [{name, type (int / float), value, natural: {value, unit}, attributes, routings, param_id, param_name}], page}` |
| `surge_set_modulation` (new; idempotent) | `target` (patch name, such as "a_pitch"), `source`, `depth` (the target's own units: semitones for `a_pitch`, ×12 with `extend_range`), `muted`, `scene` ("a" / "b", for a target of the whole patch), `params` (patch parameter changes in the same step) | the write fields plus `target`, `source`, `routings_now`, `params_now`, and `note` when Surge XT did not keep the routing |
| `surge_remove_modulation` (new; destructive, idempotent) | `target`, `source` (leave out for every routing of the target) | the write fields plus `routings_now` |
| `surge_set_patch_params` (new) | `params`: 1–64 of `{name, value | hz | seconds, deactivated, temposync, extend_range}` (`fxN_type` is refused: use `set_param` with `text`) | the write fields plus `params_now` |

## 4: plugin parameters

### `list_effects` (changed; read only)

- Arguments (new, all optional): `name` (name or vendor), `kind` (eq, dynamics, reverb, delay, modulation, distortion, filter, pitch, spatial, analysis, other), `format` (`vst3` / `lv2`), `effect` (one effect), `brief`, `limit` (default 50, at most 500), `offset`.
- Answer: `{total, offset, count, next_offset, builtin: [...] (first page only), plugins: [...]}`; rows as before; with `brief` a plugin is `{key, name, vendor, kind}`. An old call without arguments now gets the first 50.

### `get_plugin_params` (changed; read only)

- Arguments: the plugin place, and new: `name`, `group` ("" for none), `changed_only`, `ids`, `midi_cc` (also the "MIDI CC" parameters; default left out), `brief`, `groups_only`, `choices` (also probe names, at most 20 parameters a call), `limit` (default 50, at most 500), `offset`.
- Answer: `{plugin: {key, name, format}, total, offset, count, next_offset, params: [...]}`. A row: `id, name, group, value, default, text, steps, labels, automatable` as before, plus `kind` (`smooth` / `steps` / `switch`), `choices: [{value, text}]`, `likely_choice`, `choices_note`. `brief`: `{id, name, value, text}`. `groups_only`: `{plugin, total, groups: [{name, count}]}`.

### `get_param_choices` (new; read only)

- Arguments: the plugin place; `param` (a plugin's id or exact name; a built-in effect's name or index); `probes` (2–4001, default 1001); `most` (2–256, default 64).
- Answer: `{id, name, plugin, list, source: "steps" | "scan" | "none", choices: [{value, text}] | null, note?}`. Sets nothing.

### `set_param` (changed)

- Arguments: the plugin place; `param` (a plugin's by id **or exact name**; a built-in effect's by name or index); `value` (now optional) **or** `text` (new: the name of a value, such as "Sine"; for built-in effects a step, "on" / "off" or a value with its unit such as "2 kHz").
- Answer: the old fields (`step`, `undo_steps`; for built-in effects also `clip`, `take`, `track` as null) plus, for a plugin, `id`, `name`, `asked`, `value`, `text`, `before: {value, text}`, `changed`, `in_song`; for a built-in effect `index`, `param`, `asked`, `value`, `text`, `before`, `changed`, `unit`. No change is a success (`changed: false`). A value the plugin shows but does not keep in its state is an error that says so (not in undo or the file).

### `set_params` (new)

- Arguments: the plugin place; `params`: 1–64 of `{param, value | text}` (as `set_param`). All are checked first; one bad one sets nothing.
- Answer for a plugin: `{plugin, changed, in_song, step, undo_steps, params: [{id, name, asked, value, text, before, changed}], unchanged: [names]}`; for a built-in effect: `{effect, changed, in_song: true, step, undo_steps, params}`. One step.

## Tools changed only by the common rules

Track arguments that take ids, `changed: true` on success, and "no change" as a success: `delete_track`, `rename_track`, `move_track`, `set_instrument`, `set_volume`, `set_pan`, `set_send`, `set_mute`, `set_solo`, `insert_effect`, `replace_effect`, `remove_effect`, `move_effect`, `bypass_effect`, `get_automation`, `add_lane`, `delete_lane`, `write_automation`, `move_automation`, `delete_automation`, `render_audio`, `render_note`, `import_audio`, `add_track`, `song_overview`, `get_notes`, `get_mixer`.

## Tools not changed in round 11

Apart from `changed: true` on a successful write: `list_sounds`, `delete_notes`, `rename_clip`, `set_tempo`, `add_marker`, `undo`, `add_vocal_track`, `delete_clips`, `move_clips`, `duplicate_clips`, `split_clip`, `join_clips`, `switch_take`, `set_clip`, `set_clip_stretch`, `move_notes`, `resize_notes`, `set_note_length`, `set_velocity`, `quantize_notes`, `unquantize_notes`, `set_meter`, `delete_meter_change`, `delete_tempo_change`, `set_tempo_ramp`, `move_marker`, `rename_marker`, `delete_marker`, `set_loop`, `set_loop_on`, `save_song`, `save_song_as`, `open_song`, `new_song`, `export_song`, `list_instruments`, `get_clip_audio`.
