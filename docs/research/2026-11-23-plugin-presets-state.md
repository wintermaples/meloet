# Research report: Surge XT 1.3.4 / VST3 / LV2 presets, state and drum key names

Read date: 2026-11-23. Surge: tag `release_xt_1.3.4`. JUCE: `surge-synthesizer/JUCE` commit `cf5754b1…` (Surge's JUCE fork, rebased on JUCE 7.0.12, July 2024). x42 avldrums: tag `v0.7.3`. lilv: tag `v0.26.2`.

Caveats on method:
- The fetch tool summarises pages and gives no line numbers. Everything below is from source files read this way, not from raw text.
- Layouts marked "(real file)" I checked against the real local factory `.fxp` that I read.
- Rank rule: all version-pinned facts are about frozen tags (Surge 1.3.4, JUCE 7.0.12 fork). I did not apply the "older than 2 years" downgrade because the code at a tag cannot change.
- I strongly suggest one runtime check of the VST3 state bytes. See the end.

## Answers in brief

| # | Conclusion | Rank |
|---|---|---|
| 1 | The `.fxp` is a 60-byte big-endian fxp header (fxID `'cjs3'`), then the Surge chunk. The chunk is a 32-byte little-endian `sub3` header, the XML, then wavetable blocks. | A |
| 2 | Plugin state = `saveRaw` = the same chunk without the fxp header. `loadRaw` accepts the `sub3` chunk or bare XML. It does not accept a full fxp. | A (full-fxp refusal is my inference) |
| 3a | VST3 for Surge 1.3.4: `JUCE_VST3_CAN_REPLACE_VST2=0`. The component state is therefore `[Surge chunk][31-byte JUCEPrivateData trailer]`, with no "VstW" or "VC2!" wrapper. | A |
| 3b | LV2: `<https://surge-synthesizer.github.io/lv2/surge-xt:StateString>`, an atom String holding JUCE's non-standard base64 of the chunk. | A |
| 4 | Modulation routings are `<modrouting>` children of the target parameter element. Depth is in the parameter's own units, not normalized. Streaming revision is 24. | A |
| 5 | Changing FX type through the host parameter path resets that slot's parameters to the new effect's defaults. | A (code path) |
| 6 | OSC is off by default. It cannot add or remove routings. A host can enable it only by writing `dawExtraState` into the state XML. | A/B |
| 7 | The VST3 exposes one empty program, and LV2 lists one empty preset. The 3008 patches are not reachable that way. | A |
| 8 | `.vstpreset` layout and preset folders confirmed. VST3 has `IUnitInfo::getProgramPitchName` for key names. | A |
| 9 | lilv call sequence confirmed from lilv.h and jalv source. | A |
| 10 | avldrums implements midnam. The note tables are listed below. SF2 has no per-key name field. GM map from Wikipedia. | A / A / B |

## Q1. `.fxp` byte layout

Sources: `src/common/PatchFileHeaderStructs.h`, `SurgeSynthesizerIO.cpp` (`savePatchToPath`, `loadPatchByPath`, `loadRaw`, `saveRaw`), `SurgePatch.cpp` (`load_patch`, `save_patch`). Confidence A, plus the real file `/usr/share/surge-xt/patches_factory/Leads/Sine Lead.fxp`.

**fxp header** (struct `fxChunkSetCustom`, packed, 60 bytes total; every integer is big-endian):

| Offset | Size | Field | Value written by Surge |
|---|---|---|---|
| 0 | 4 | chunkMagic | `'CcnK'` |
| 4 | 4 | byteSize | literal 0 (Surge never computes it; a real file shows four NULs) |
| 8 | 4 | fxMagic | `'FPCh'` |
| 12 | 4 | version | 1 |
| 16 | 4 | fxID | `'cjs3'` (not `'sub3'`) |
| 20 | 4 | fxVersion | 1 |
| 24 | 4 | numPrograms | 1 |
| 28 | 28 | prgName | patch name, via `strncpy`, NUL-padded |
| 56 | 4 | chunkSize | size of the Surge chunk that follows |

**Reader behaviour** (`loadPatchByPath`): it checks chunkMagic, fxMagic and fxID only. It does not check `version`, `fxVersion`, `numPrograms` or `byteSize`. It reads `chunkSize` bytes, passes them to `loadRaw`, and sets the category to the folder name.

**Surge chunk** (`save_patch` / `load_patch`):

- `patch_header` (packed), 32 bytes:
  - `char tag[4]` = `"sub3"`.
  - `uint32 xmlsize`, little-endian.
  - `uint32 wtsize[2][3]`, 24 bytes, little-endian, row-major [scene][osc], 0 when the oscillator has no embedded wavetable. A source comment says "TODO: FIX SCENE AND OSC COUNT ASSUMPTION".
- Then `xmlsize` bytes of XML (UTF-8, starting `<?xml …?><patch revision=…>`).
- Then, for each non-zero `wtsize[sc][osc]` in scene then osc order, one wavetable block of that size. It starts with a 12-byte header, little-endian:
  - `'vawt'` (4 bytes), `uint32` wave_size, `uint16` wave_count, `uint16` flags.
  - Surge writes flags with the int16 bit set (0x0004).
  - Local doc `/usr/share/surge-xt/wavetables/WT fileformat.txt`: flags 0001 = sample, 0002 = looped, 0004 = int16, 0008 = full 16-bit range.
  - The wave data follows. Int16 data is `2 * wave_size * wave_count` bytes.
- Load: if the first 4 bytes are not `"sub3"`, the whole buffer is parsed as bare XML. `load_patch` returns at once for `datasize <= 4`.
- Load: `load_patch` converts `xmlsize` in place, so it modifies the caller's buffer.
- Load: XML is capped at 4 MB.

## Q2. Plugin state vs `.fxp` chunk

Source: `src/surge-xt/SurgeSynthProcessor.cpp`, `SurgeSynthesizerIO.cpp`. Confidence A.

- `getStateInformation`:
  - It calls `populateDawExtraState()`, and the editor's `populateForStreaming` if an editor is open.
  - It then runs `surge->saveRaw(&data)` and copies the bytes.
  - `saveRaw` is exactly `save_patch(data)`, so the bytes are the `sub3` chunk with no fxp header. This is exactly what follows the fxp header in a `.fxp` file.
- `setStateInformation`:
  - It calls `enqueuePatchForLoad(data, size)`. The buffer is stored as-is, with no header parsing.
  - It then calls `processAudioThreadOpsWhenAudioEngineUnavailable()`.
  - The audio-thread handler calls `loadRaw(data, size)`, then `loadFromDawExtraState()`.
- `loadRaw`:
  - It stops the engine, calls `init_default_values()`, then `load_patch`.
  - It reloads the FX, clears `isDirty`, and sets `patch_loaded` and `refresh_editor`.
  - It finds `patchid` and category by matching name and category against `patch_list`.
- Accepted inputs: the `sub3` chunk, or bare XML (the non-`sub3` branch of `load_patch`).
- A full `.fxp` (starting `CcnK`) is not stripped by `loadRaw`. Only `loadPatchByPath` strips it. The `CcnK` bytes would be treated as XML and fail. This is my inference from the code, not tested.

## Q3. How JUCE wraps the state

### (a) VST3

Source: JUCE fork `modules/juce_audio_plugin_client/juce_audio_plugin_client_VST3.cpp`; Surge `src/CMakeLists.txt`. Confidence A.

- Surge sets `JUCE_VST3_CAN_REPLACE_VST2=0` in `target_compile_definitions(surge-juce INTERFACE …)`. This is the only mention of the macro, and `surge-juce` is the interface target the plugin links.
- Therefore `getState` writes only the output of the wrapper's `getStateInformation`, with no "VstW" header and no `CcnK`/`FBCh` bank.
- Consequence: the "VC2!" and VST2 wrapping applies only if that macro were 1. There, `getState` would write a 16-byte header (four big-endian int32: `'VstW'`, 8, 1, bypass), then a 160-byte `fxBank` (`'CcnK'`, byteSize = 152 + n, `'FBCh'`, version 2, fxID, fxVersion, size), then the state.
- Wrapper `getStateInformation` writes, in order:
  1. The plugin's bytes (the Surge chunk).
  2. An int64 zero (8 bytes).
  3. A ValueTree with property "Bypass", only if `getBypassParameter()==nullptr`. Surge returns its own `SurgeBypassParameter`, so nothing is written here.
  4. An int64 little-endian size of item 3 (here 0).
  5. The 15 ASCII bytes `JUCEPrivateData`, with no terminator.
- Trailer size is therefore 8 + 8 + 15 = 31 bytes. The local `Surge XT.so` contains the string `JUCEPrivateData`, so the code is present in the binary.
- `setState` reads in this order:
  1. It reads the whole stream. `ISizeableStream` is used when available, and it rejects streams over 100 MB.
  2. It checks whether the last 15 bytes equal the magic.
  3. If so, it reads the little-endian uint64 at `size-23` and strips `P + 23` bytes.
  4. It strips 8 more bytes (the leading zero int64).
  5. It passes the rest to Surge's `setStateInformation`.
- If the magic is absent, the whole buffer goes to Surge. A host that edits the XML can rebuild the `sub3` chunk and either keep the same 31-byte trailer or drop it.
- `IEditController::setComponentState` does not parse the stream. JUCE re-reads the parameter values from the processor and calls `restartComponent(kParamValuesChanged)`. A host that restores a state should call `IComponent::setState` first, then `setComponentState` with the same bytes.
- Not confirmed: that Ubuntu's `surge-xt` package was built with exactly these flags. It is a package build of the same tag, and the local `.lv2` evidence (Q7) is consistent with this flag set.

### (b) LV2

Source: JUCE fork `juce_audio_plugin_client_LV2.cpp`; `modules/juce_core/memory/juce_MemoryBlock.cpp`. Confidence A.

- State property: `<LV2URI>:StateString`, so `https://surge-synthesizer.github.io/lv2/surge-xt:StateString`. The separator is a colon, not `#`.
- Type is `atom:String`, with flags `LV2_STATE_IS_POD | LV2_STATE_IS_PORTABLE`. The stored size includes the terminating NUL.
- The value is `MemoryBlock::toBase64Encoding()` of `getStateInformation` bytes. There is no `JUCEPrivateData` in LV2.
- JUCE base64 is not RFC 4648. Format: `<decimal byte count>.<chars>`.
  - Alphabet (64 chars): `.ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+` (index 0 is `.`, 63 is `+`).
  - Bits are packed LSB-first. For each i in 0..numChars-1, the 6-bit group is `getBitRange(i*6, 6)`, where bit 0 is the least significant bit of byte 0.
  - `numChars = ((size<<3)+5)/6`. No `=` padding.
  - Example: byte `{0x01}` gives `1.A.`.
  - Decoder: finds the first `.`, parses the size, then maps each char with `c-43` through a table. Unknown chars are skipped.
- Second property: `<LV2URI>:Program` (`atom:Int`). `retrieve` checks it first. If present and an Int of 4 bytes, it calls `setCurrentProgram` and returns SUCCESS immediately without reading StateString. A host that writes state must therefore not include a Program property.
- `store` never writes Program.
- A state change is announced with `state:StateChanged` on the output sequence.

## Q4. Patch XML and modulation

Sources: `SurgePatch.cpp` (`save_xml` / `load_xml`), `src/common/ModulationSource.h` (`modsources`, `modsource_names_tag`, `ModulationRouting`), `Parameter.cpp`, `SurgeStorage.h`, plus the real file `Sine Lead.fxp`. Confidence A.

**Structure (`save_xml`):**
- `<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>`, then `<patch revision="N">`.
- Children in order: `<meta name category comment author [license]>` (with `<tags/>`), `<parameters>`, `<nonparamconfig>`, `<extraoscdata>`, `<stepsequences>`, `<msegs>`, `<formulae>`, `<extralfo>`, `<customcontroller>`, `<lfobanklabels>`, `<modwheel>`, `<compatability>`, optional `<patchTuning>`, `<tempoOnSave>`, `<dawExtraState populated="0|1">`.
- The category attribute is informational. In the real file the folder is "Leads" but `meta category="Monosynth (smooth)"`; the browser category comes from the folder.
- Streaming revision of 1.3.4: `ff_revision = 24` (`SurgeStorage.h`). Factory patches here are revision 20 (older revisions are converted on load).
- If `revision > ff_revision`, Surge warns through `reportError` but still loads.

**Parameter elements:**
- `<parameters>` has one element per parameter. The element name is the storage name, e.g. `a_pitch`, `a_lfo0_delay`, `fx1_type`. This is the underscore form seen in the real file, not the OSC form `a/vlfo/1/eg/delay`.
- Attributes: `type="0"` (int) or `"2"` (float), and `value` (natural units, 14 decimals for floats). Optional attributes: `temposync="1"`, `extend_range="0|1"`, `absolute="1"`, `deactivated="0|1"`, `deform_type`, and for portamento `porta_const_rate`, `porta_gliss`, `porta_retrigger`, `porta_curve`.
- Load order: elements are looked up in `param_ptr` order, falling back to `FirstChild`. Order is therefore not important.
- Missing element on load: `load_xml` leaves the parameter untouched. `loadRaw` calls `init_default_values()` first, so a missing parameter gets its default.
- Missing `value` attribute: falls back to `val_default`.
- `load_xml` also clears all modulation routings and sets every FX slot type to `fxt_off` before reading.
- Missing `deactivated`: a hard-coded list decides the default. Writing `deactivated="0"` explicitly is safest. The real file uses `deactivated="0"` on LFO rate, delay, filter and FX parameters.

**Modulation routing:**
- Stored as child elements `<modrouting …/>` nested inside the target parameter element, not in a separate section.

  Real file: `<a_pitch type="2" value="0.0" extend_range="0"><modrouting source="17" depth="0.281250" muted="0" source_index="0" /></a_pitch>`.

  The same patch has `<a_lfo0_magnitude …><modrouting source="4" depth="1.000000" …/>`, which is aftertouch controlling the vibrato amount.
- Attributes:
  - `source`: integer `modsources` id.
  - `depth`: float.
  - `muted`: 0 or 1.
  - `source_index`: int, default 0. It selects a sub-output for indexed modulators; I did not confirm which sources use it, so leave it 0.
  - `source_scene`: written only for global-level routings (targets in scene 0), default -1 in the struct and 0 on load. It is absent for scene parameters in the real file.
- Load: the routing is read only for float parameters. Compat: for revision < 9, source ids above `ms_ctrl7` shift by one; for revision < 21, routings targeting `volume` are skipped.

**`modsources` ids for 1.3.4** (`ModulationSource.h`; `n_modsources` = 41; tag names are the OSC names):

| id | name | OSC tag | id | name | OSC tag |
|---|---|---|---|---|---|
| 0 | original (off) | off | 21 | lfo5 | vlfo_5 |
| 1 | velocity | vel | 22 | lfo6 | vlfo_6 |
| 2 | keytrack | keytrk | 23–28 | slfo1–slfo6 | slfo_1 … slfo_6 |
| 3 | polyaftertouch | pat | 29 | timbre | timbre |
| 4 | aftertouch | at | 30 | releasevelocity | rel_vel |
| 5 | pitchbend | pb | 31 | random_bipolar | rand_bi |
| 6 | modwheel | mw | 32 | random_unipolar | rand_uni |
| 7–14 | ctrl1–ctrl8 (macros) | macro_1 … macro_8 | 33 | alternate_bipolar | alt_bi |
| 15 | ampeg | aeg | 34 | alternate_unipolar | alt_uni |
| 16 | filtereg | feg | 35 | breath | breath |
| 17 | lfo1 (voice LFO 1) | vlfo_1 | 36 | expression | expr |
| 18 | lfo2 | vlfo_2 | 37 | sustain | sus |
| 19 | lfo3 | vlfo_3 | 38 | lowest_key | lowest_key |
| 20 | lfo4 | vlfo_4 | 39 | highest_key | highest_key |
| | | | 40 | latest_key | latest_key |

**Depth units:**
- `Parameter::set_modulation_f01(v)` returns `v * (val_max - val_min)` for float parameters. `get_modulation_f01` divides by `(val_max - val_min)` and clamps to [-1, 1].
- So the stored `depth` is in the parameter's own natural units (semitones, dB, and so on), i.e. normalized times the parameter range. It is not normalized 0..1.
- Evidence: depth = -9.77 on `a_vca_level` in the real file cannot be normalized.
- Ranges I read in `Parameter.cpp`: `ct_pitch_semi7bp` is -7..7, `ct_pitch` is -60..60.
- Derived, not runtime-tested: for scene pitch `a_pitch` (`ct_pitch_semi7bp`), depth 0.28 means roughly ±0.28 semitone at LFO ±1. I did not verify how `extend_range` changes this scaling.
- How the UI displays depth: not confirmed from a primary source.
- OSC uses `getModDepth01`. By inference, OSC value = XML depth / (val_max - val_min).

**Element names for scene A:**
- Scene A pitch: `a_pitch`. Scene B uses the prefix `b_`.
- Voice LFO 1 is `a_lfo0_*`, voice LFO 6 is `a_lfo5_*`, scene LFO 1 is `a_lfo6_*`, scene LFO 6 is `a_lfo11_*`. This follows from the real file (a_lfo0..a_lfo11) and from the `lfo%i_` labels and `lt = vlfo / slfo` in the constructor.
- `a_lfo0_*` is the LFO that `source="17"` refers to (id 17 = `ms_lfo1` = `ms_lfo1 + l`, the constructor's modulation source for LFO l).
- LFO parameters (type attribute and the defaults seen in the real file):

| Element | type | Default seen |
|---|---|---|
| `_shape` | 0 | 0 |
| `_rate` | 2 (`ct_lforate_deactivatable`) | 0 |
| `_phase` | 2 | 0 |
| `_magnitude` | 2 (`ct_lfoamplitude`) | 1 |
| `_deform` | 2 | 0 |
| `_trigmode` | 0 | 1 |
| `_unipolar` | 0 | 0 |
| `_delay` | 2 (`ct_envtime_deactivatable`) | -8 |
| `_attack` | 2 | -8 |
| `_hold` | 2 | -8 |
| `_decay` | 2 | 0 |
| `_sustain` | 2 (`ct_percent`) | 1 |
| `_release` | 2 | 5 |

- Trigger-mode and shape enum names (e.g. 1 = keytrigger) were not confirmed from a primary source.

**Envelope-time encoding:**
- `ct_envtime`, `ct_envtime_deactivatable` and `ct_envtime_linkable_delay` use `displayType ATwoToTheBx` with unit "s", `val_min = -8`, `val_max = 5`, `val_default = 0`.
- `DisplayInfo` defaults `a = 1, b = 1`, and `Parameter.cpp` does not override them for envtime. So seconds = `a * 2^(b*value)` = `2^value`.
- That gives -8 = 2^-8 s ≈ 3.9 ms, treated as "no delay" (`minLabelValue = 0`), 0 = 1 s, 5 = 32 s.
- Rate: `ct_lforate` is `2^value` Hz, range -7..9. This is the same formula; my numeric check against the real file (value 2.284 ≈ 4.87 Hz) is arithmetic, not an observation of the UI.

**Vibrato with delayed onset:**
- Write `<a_pitch type="2" value="0.0000"><modrouting source="17" depth="D" muted="0" source_index="0"/></a_pitch>`.
- Keep the LFO as a sine: `a_lfo0_shape` 0, `a_lfo0_magnitude` 1.
- Set `a_lfo0_rate` ≈ 2.3 (about 5 Hz).
- Set `a_lfo0_delay` to e.g. -1 (0.5 s), with `deactivated="0"`.
- Optionally set `a_lfo0_attack` to e.g. 0 (1 s) for a smooth fade-in. I assumed the LFO's DAHDSR applies to all shapes, but I have not confirmed this in a primary source. The `deactivated` special-casing for LFO delay is also not confirmed.
- `D` is in semitones, e.g. 0.28.
- I did not confirm that an LFO with the `eg/delay` envelope works as expected for every `trigmode`. Check by listening.

## Q5. FX type change and FX storage

Source: `SurgeSynthesizer.cpp`. Confidence A for the code path; B/C for the GUI path.

- Host path: `setParameter01`, case `ct_fxtype`:
  - It writes the new type into `fxsync[cge].type` and restores the patch parameter's old value (fxsync is primary).
  - It then spawns a temporary effect on `&fxsync[cge]`, calls `init_ctrltypes()` and `init_default_values()`, and deletes it.
  - It sets `load_fx_needed` and `fx_reload[cge]`.
  - Result: the slot's parameters in `fxsync` are reset to the new effect's defaults.
- `loadFx(bool initp, bool force_reload_all)` is called with `(false, false)` from both `processControl` and `processAudioThreadOpsWhenAudioEngineUnavailable`. With `initp=false` it only clamps values and sets extend range. The reset therefore comes from the `setParameter01` code above.
- The GUI type-change path: I could not find it in the files I read (`SurgeGUIEditor.cpp`, `SurgeGUIEditorValueCallbacks.cpp`). Not confirmed whether it differs.
- XML storage: `n_fx_slots = 16`. In the real file: `fx1_type` (int), `fx1_p0` … `fx1_p11` (12 parameters; type 0 for int parameters, 2 for float), plus `volume_FX1..4`, `fx_disable`, `fx_bypass`.
- Extras: attributes such as `deactivated`, `extend_range`, `temposync`; routing children can sit on FX parameters; empty (off) slots are not written, and `load_xml` turns off all slots first.
- OSC short names for the first 8 slots: `fx/a/1, fx/a/2, fx/b/1, fx/b/2, fx/send/1, fx/send/2, fx/global/1, fx/global/2`. The full list of 16 and the exact mapping between that order and `fxN` numbering were not confirmed.

## Q6. OSC

Sources: `src/surge-xt/osc/OpenSoundControl.cpp` and `.h`; `SurgeStorage.h`; `globals.h`; `SurgeSynthProcessor.cpp`; manual-xt. Confidence A for code; the separate "OSC specification" page was not read.

- Introduced in 1.3 (manual: "Since version 1.3, Surge XT supports the Open Sound Control protocol"). 1.3.4 release notes mention a fix for parsing some modulation mappings over OSC (via the changelog; B).
- Default ports: in 53280, out 53281, out IP `127.0.0.1` (`globals.h`).
- Default is off: `oscStartIn{false}` and `oscStartOut{false}` in `DAWExtraStateStorage`.
- The settings (ports, IP, start flags) live in the patch/state XML under `dawExtraState` (written only when `populated="1"`), as:
  - `<oscPortIn v="INT"/>`
  - `<oscPortOut v="INT"/>`
  - `<oscIPAddrOut v="STR"/>`
  - `<oscStartIn v="1|0"/>`
  - `<oscStartOut v="1|0"/>`
- Without the UI, a host can enable OSC by putting these into the state XML: on `setStateInformation` Surge calls `tryLazyOscStartupFromStreamedState` (immediately if the audio engine is not active, otherwise on the next `processBlock`). It starts input if `oscStartIn` is true and `oscPortIn > 0`. This is a state edit too, so OSC is not a safer substitute for editing the XML.
- Port conflicts: input and output cannot share a port, and each instance in a host process binds its own UDP port (shared across instances, so you would need distinct ports).
- Modulation over OSC: only `/mod/[mute/][a|b/]<source>[/<index>]` with args (string target-param OSC name, float depth), and `/q/mod/...` to query. There is no add or remove message.
- Source names are the tag names above (`vlfo_1`, `mw`, …). Whether a missing routing is created by the depth message, and whether depth 0 clears it, are decided in the audio-thread handler, which I did not read (not confirmed).
- The code passes the depth through unchanged and queries use `getModDepth01`.
- Other addresses: `/param/<oscName>` (e.g. `a/osc/1/pitch`), `/param/macro/<n>`, `/param/<name>/<ext>` for `abs, enable, tempo_sync, extend, deform, const_rate, gliss, retrig, curve`, `/param/fx/<slot>/<n>/deactivate`, `/q/all_params`, `/q/all_mods`, `/patch/load` (path to an `.fxp`, no extension), `/patch/load_user`, `/patch/save`, `/patch/save_user`, `/patch/random|incr|decr`, `/wavetable/...`, `/tuning/...`, notes and controllers (`/mnote`, `/fnote`, `/cc`, `/pbend` …).
- Judgement: OSC is less safe than the XML edit for deterministic modulation changes, because of the open questions above, sockets, and extra state. It is useful as a read-back (`/q/all_mods`).

## Q7. Programs, presets, user data

Sources: `SurgeSynthProcessor.cpp`, `src/CMakeLists.txt`, local TTL, JUCE VST3 wrapper, `SurgeStorage.cpp`, manual-xt, local text file. Confidence A.

- `SurgeSynthProcessor::getNumPrograms()` returns `presetOrderToPatchList.size()+1` only if `SURGE_EXPOSE_PRESETS` is defined. Otherwise it returns 1, and `getProgramName` returns "".
- `option(SURGE_EXPOSE_PRESETS … OFF)` in `src/CMakeLists.txt`, so the default is OFF.
- JUCE 7.0.12 VST3 wrapper:
  - The program parameter ("juceProgramParameter", flags `kIsProgramChange | kCanAutomate`, an `AudioParameterInt` over `0..numPrograms-1`) is created only when there is more than one program. So Surge has none.
  - `IUnitInfo::getProgramListCount` returns 1 if `getNumPrograms() > 0`, with list name "Factory Presets" and the program count. With Surge that is 1 program named "".
  - `getProgramInfo`, `hasProgramPitchNames` and `getProgramPitchName` return `kNotImplemented`.
- Local `/usr/lib/lv2/Surge XT.lv2/manifest.ttl` has exactly one preset: `<https://surge-synthesizer.github.io/lv2/surge-xt:preset1> a pset:Preset ; lv2:appliesTo <…/surge-xt> ; rdfs:label "" ; state:state [ <…/surge-xt:Program> "0"^^xsd:int ]`. This matches `SURGE_EXPOSE_PRESETS` being off in the Ubuntu build.
- Net effect: the 3008 factory and 3rd-party patches must be loaded as files, by feeding the `.fxp` chunk into the state.
- Factory folders on this PC: `/usr/share/surge-xt/patches_factory`, `patches_3rdparty`, `fx_presets`, `modulator_presets`.
- User data on Linux:
  - Default `~/Documents/Surge XT` (manual: "The user patches are at ~/Documents/Surge XT"; created on first store). The Linux code uses `sst::plugininfra::paths::bestDocumentsFolderPathFor(...)`.
  - Subfolders in the user folder: `Patches`, `FX Presets`, `Modulator Presets`, `Wavetables`.
  - Overrides: the menu "Data Folders > Set custom user data folder…" (stored under user-defaults key `Surge::Storage::UserDataPath`). No environment variable overrides the user folder. `SURGE_DATA_HOME` and `PIPELINE_OVERRIDE_DATA_HOME` override the factory (shared) `datapath`.
  - The local `WHERE TO PLACE USER DATA.txt` only says to put user content in the user folder (open it via "Menu > Data Folders > Open User Data Folder").
- Category = folder: `refresh_patchlist` scans `patches_factory`, `patches_3rdparty` and the user `Patches`. Each folder (path relative to the scanned root, nested allowed) becomes a category, and only `.fxp` files count. In user `Patches`, loose files are category `_Unsorted`. In the factory folders, top-level `.fxp` files are not picked up. When a patch is loaded by path, its category is the folder name, not the `meta category` attribute.

## Q8. VST3 presets and programs

Sources: Steinberg "Preset Locations" page; SDK `vstpresetfile.h` / `.cpp`; `ivstunits.h`; `ivsteditcontroller.h`. Confidence A (some endianness points inferred).

**Preset locations on Linux**, in priority order:

1. User: `$HOME/.vst3/presets/$COMPANY/$PLUGIN-NAME/`
2. Shared: `/usr/share/vst3/presets/$COMPANY/$PLUGIN-NAME/`
3. Shared: `/usr/local/share/vst3/presets/$COMPANY/$PLUGIN-NAME/`
4. App-specific: `[$APPFOLDER]/vst3/presets/$COMPANY/$PLUGIN-NAME/`

- The folder names may only use characters allowed in file names; `\*?/:<>|` must be replaced by `_`.
- The page does not give the extension; the format page uses `.vstpreset`.

**File layout** (`.vstpreset`):
- Header, 48 bytes:
  - 4 bytes `'VST3'`.
  - 4 bytes int32 version (`kFormatVersion` = 1).
  - 32 bytes ASCII class id (`FUID::toString` of the class id, no terminator). The exact character form was not confirmed.
  - 8 bytes int64 offset to the chunk list (the last header field).
- Data area: the chunk data.
- Chunk list at the offset:
  - 4 bytes `'List'`.
  - 4 bytes int32 entry count.
  - Per entry 20 bytes: 4-byte chunk id, int64 offset, int64 size.
- Chunk ids in the SDK: `'Comp'` = component state (the same bytes as `IComponent::getState`), `'Cont'` = controller state, `'Prog'` = program data, `'Info'` = meta info (XML, opaque to the writer, must be last in the file).
- Endianness: the SDK writes in one byte order on every host and swaps only on big-endian builds, which implies little-endian. The Steinberg page does not state it. The JUCE reader also reads the list offset (byte 40) and entries little-endian.
- The JUCE loader (`loadVST3PresetFile`, which exists only in the `CAN_REPLACE_VST2` build) is not used here. For Surge a host loads the `Comp` chunk through `IComponent::setState`.

**Programs:**
- `IUnitInfo`: `getProgramListCount()`, `getProgramListInfo(listIndex, ProgramListInfo&)` (`id`, `name` (String128), `programCount`), `getProgramName(listId, programIndex, String128)`.
- `kRootUnitId` = 0, `kNoProgramListId` = -1.
- Select a program through the parameter with flag `kIsProgramChange` (1<<15; its `unitId` points at the unit that owns the program list). In JUCE that parameter is an integer with `stepCount = numPrograms - 1`, so normalized = index / stepCount. That last step is derived from the JUCE summary (B+).
- Useful for drums: `IUnitInfo::getProgramPitchName(listId, programIndex, int16 midiPitch, String128 name)` returns the key name for a program (the header's own example is "a drum program might name each key"). `hasProgramPitchNames` says if supported, and the plugin notifies `IUnitHandler::notifyProgramListChange` on changes. JUCE returns `kNotImplemented` for these, so JUCE-built plugins give nothing.

## Q9. LV2 presets with lilv 0.26.2

Sources: lilv `include/lilv/lilv.h` at v0.26.2; jalv `src/state.c`, `src/gtk/actions.c`, `src/gtk/menu.c`; local `/usr/lib/lv2/presets.lv2/presets.ttl`. Confidence A.

- List: `lilv_plugin_get_related(plugin, pset:Preset node)` returns presets linked by `lv2:appliesTo`. For each, call `lilv_world_load_resource(world, preset)` (loads `rdfs:seeAlso` data), then read `rdfs:label` with `lilv_world_get`. jalv warns when a label is missing. Unload with `lilv_world_unload_resource`.
- `pset:Preset` and `pset:Bank` must each have an `rdfs:label` (`presets.ttl`).
- Load: `lilv_state_new_from_world(world, map, preset_node)`, then `lilv_state_restore(state, instance, set_port_value, user_data, 0, features)`.
  - `set_port_value(port_symbol, user_data, value, size, type)` receives control port values.
  - jalv calls this at `state_features` (map, unmap, make-path, worker and so on).
  - Properties (state:interface) are restored when an instance is passed.
  - jalv pauses processing around the restore.
- Save the current state:
  - `lilv_state_new_from_instance(plugin, instance, map, scratch_dir, copy_dir, link_dir, save_dir, get_port_value, user_data, flags, features)`. jalv uses flags `LV2_STATE_IS_POD | LV2_STATE_IS_PORTABLE`, and `dir` for copy/link/save.
  - `get_port_value(port_symbol, user_data, &size, &type)` returns the value pointer.
  - `lilv_state_set_label(state, label)`.
  - `lilv_state_save(world, map, unmap, state, uri, dir, filename)` writes the bundle (the `.ttl` plus manifest) in `dir`. A NULL uri gives a relocatable subject.
- User preset convention (jalv GTK `actions.c`):
  - Bundle: `~/.lv2/<symbolify(PluginName_PresetName)>.preset.lv2/`. `symbolify` replaces every non-ASCII-alphanumeric character with `_`.
  - File: `<symbolify(label)>.ttl`.
  - The preset URI is built inside `jalv_save_preset`, which I did not read.
  - After saving, jalv unloads and reloads the bundle with `lilv_new_file_uri`.
  - Only presets whose URI starts with `file:` can be overwritten.
- Bank: `lilv_world_get(world, preset_node, pset:bank, NULL)`. jalv's menu uses the bank's `rdfs:label` (or its URI) as a submenu, which is the usual category. Presets with no bank go at the top.
- Not confirmed: Ardour's own user-preset location (I did not read it) and lilv's default `LV2_PATH` (I believe `~/.lv2` is searched, but did not verify it from a primary source: C).
- As a JUCE LV2 detail (Q3), the plugin's own Program property overrides StateString on restore.

## Q10. Drum key names

### (a) midnam

Sources: Ardour `libs/ardour/ardour/lv2_extensions.h`; avldrums `src/midnam_lv2.h`, `src/avldrums.c` at tag v0.7.3; local `avldrums.ttl`. Confidence A.

- URIs: extension data `http://ardour.org/lv2/midnam#interface` (`LV2_MIDNAM__interface`), host feature `http://ardour.org/lv2/midnam#update` (`LV2_MIDNAM__update`).
- Plugin interface `LV2_Midnam_Interface`, three function pointers:
  - `char* midnam(LV2_Handle)`: null-terminated XML, or NULL on error.
  - `char* model(LV2_Handle)`: the unique model id that matches the `<Model>` element, or NULL.
  - `void free(char*)`: the host must free every string from the first two with this.
  - The `<Model>` must be unique to the plugin instance.
- Host feature `LV2_Midnam {handle; void (*update)(handle)}`: the plugin calls it from `run()` to ask the host to re-read the midnam.
- Get the interface with `lilv_instance_get_extension_data(instance, uri)`.
- All 9 avldrums plugins in the local `avldrums.ttl` list the interface (as `lv2:extensionData`) and the update feature (as `lv2:optionalFeature`).
- avldrums 0.7.3 implements `midnam`/`model`/`free`:
  - `mn_file` builds the XML (a 4096-byte `snprintf` buffer) and calls `mn_buskman` when `kit_id == 4`, otherwise `mn_drums`.
  - `mn_model` returns `AVL-Drumkits-LV2:%p` (instance pointer).
  - It stores the `update` feature but never calls it. The names are hard-coded and do not come from the SF2 file.
- XML shape written by avldrums:
  - Declaration `<?xml version="1.0" encoding="UTF-8"?>` and the DOCTYPE `MIDINameDocument PUBLIC "-//MIDI Manufacturers Association//DTD MIDINameDocument 1.0//EN" "http://www.midi.org/dtds/MIDINameDocument10.dtd"`.
  - `<MIDINameDocument><Author>Glen MacArthur</Author><MasterDeviceNames><Manufacturer>AVL-Drumkits</Manufacturer><Model>AVL-Drumkits-LV2:%p</Model><CustomDeviceMode Name="Drumkit Keymap"><ChannelNameSetAssignments><ChannelNameSetAssign Channel="1" NameSet="Names"/>…` (channels 1 to 16).
  - `</CustomDeviceMode><ChannelNameSet Name="Names"><AvailableForChannels><AvailableChannel Channel="1" Available="true"/>…</AvailableForChannels><UsesNoteNameList Name="Notes"/></ChannelNameSet><NoteNameList Name="Notes"><Note Number="36" Name="Kick Drum"/>…`.
- Black Pearl (`kit_id` 0, also BlackPearlMulti) note map from `mn_drums` (identical at master and v0.7.3). Notes 35 is absent for kit 0; 61 is "Maracas" (it is "Shaker" and 35 "Stick Click" only for `kit_id > 2`, i.e. Blonde Bop HotRod):

| Note | Name | Note | Name |
|---|---|---|---|
| 36 | Kick Drum | 49 | Crash 1 |
| 37 | Snare Sidestick | 50 | Crash1 Choked |
| 38 | Snare Center | 51 | Ride Tip |
| 39 | Hand Clap | 52 | Ride Choked |
| 40 | Snare Edge | 53 | Ride Bell |
| 41 | Floor Tom Center | 54 | Tambourine |
| 42 | Closed Hat | 55 | Splash |
| 43 | Floor Tom Edge/Alt | 56 | Cowbell |
| 44 | Pedal Hat | 57 | Crash 2 |
| 45 | Tom Center | 58 | Crash 2 Choked |
| 46 | Semi Hat | 59 | Ride Shank |
| 47 | Tom Edge/Alt | 60 | Crash3 |
| 48 | Swish Hat | 61 | Maracas |

- Buskman's Holiday (`mn_buskman`): 35 Stick Click, 36 Cajon Thump, 37 Finger Snaps, 38 Cajon Slap Left, 39 Hand Clap, 40 Cajon Slap Right, 41 Large Conga Left, 42 Shakers, 43 Large Conga Right, 44 Shake Tambourine, 45 Small Conga Left, 46 Bump Tambourine, 47 Small Conga Right, 48 Claves, 49 Cymbal, 50 Cymbal Bell, 51 Cowbell, 52 Foot Stomp, 53 Bucket, 54 Bell Tree Down, 55 Bell Tree Up.

### (b) SF2: no per-key name field

Source: the SoundFont spec PDF at `https://polyphone.io/doc/files/sf_specifications_v2.04.pdf` (saved locally at `/home/meloet-agent/.claude/projects/-srv-meloet-agent-meloet/932e0754-efe9-47a1-96f6-46031fb55f1d/tool-results/webfetch-1795431934350-eubtyn.pdf`). Confidence A, with one caveat: the footer on every page says "SoundFont 2.01 Technical Specification", although the file is hosted as v2.04. I read pp. 2-3, 21-22, 28, 33-36 only.
- `sfPresetHeader` (7.2): `achPresetName[20]`, `wPreset`, `wBank`, `wPresetBagNdx`, `dwLibrary`, `dwGenre`, `dwMorphology`. A General MIDI percussion bank is conventionally `wBank = 128`.
- `sfSample` (7.10): `achSampleName[20]`, start/end/loop points, `dwSampleRate`, `byOriginalPitch`, `chPitchCorrection`, `wSampleLink`, `sfSampleType`.
- Generators (8.1.2): 43 `keyRange` (LS byte = highest key, MS byte = lowest; optional, must be first in the zone's list), 44 `velRange`, 41 `instrument`, 53 `sampleID`, 57 `exclusiveClass`, 46 `keynum`.
- Instrument names (`achInstName[20]`) were not read from the page text, but the instrument header carries a 20-char name by the same convention.
- No field in what I read names an individual key. A key name can only be derived from the zone's `keyRange` plus the name of the instrument or sample used by that zone (e.g. "Kick" from `achSampleName`). That is a heuristic; do not treat it as a defined mapping.

### (c) General MIDI Level 1 percussion map (35 to 81, channel 10)

Source: Wikipedia "General MIDI" (B). I could not read RP-003 itself: midi.org lists `RP-003_General_MIDI_System_Level_1_Specification_96-1-4_0.1.pdf` at https://midi.org/specifications/general-midi-level-1-2/download, but that URL returned 404 for me (the same page also offers a download that needs a login). Wikipedia does not cite RP-003 here. Wikipedia merges some original GM1 and GM2 names (e.g. "Acoustic Bass Drum or Low Bass Drum"), so original GM1 names for a few keys may differ slightly.

35 Acoustic Bass Drum, 36 Electric Bass Drum, 37 Side Stick, 38 Acoustic Snare, 39 Hand Clap, 40 Electric Snare (or Rimshot), 41 Low Floor Tom, 42 Closed Hi-hat, 43 High Floor Tom, 44 Pedal Hi-hat, 45 Low Tom, 46 Open Hi-hat, 47 Low-Mid Tom, 48 High-Mid Tom, 49 Crash Cymbal 1, 50 High Tom, 51 Ride Cymbal 1, 52 Chinese Cymbal, 53 Ride Bell, 54 Tambourine, 55 Splash Cymbal, 56 Cowbell, 57 Crash Cymbal 2, 58 Vibraslap, 59 Ride Cymbal 2, 60 High Bongo, 61 Low Bongo, 62 Mute High Conga, 63 Open High Conga, 64 Low Conga, 65 High Timbale, 66 Low Timbale, 67 High Agogo, 68 Low Agogo, 69 Cabasa, 70 Maracas, 71 Short Whistle, 72 Long Whistle, 73 Short Guiro, 74 Long Guiro, 75 Claves, 76 High Woodblock, 77 Low Woodblock, 78 Mute Cuica, 79 Open Cuica, 80 Mute Triangle, 81 Open Triangle.

The midi.org page https://midi.org/midi-ci-profile-for-default-drum-note-map says the note map used in that profile was established by many products in the 1980s and later standardized in General MIDI (B).

## Sources

| URL | What it gave | Rank |
|---|---|---|
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/PatchFileHeaderStructs.h | `fxChunkSetCustom`, `patch_header` layouts | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/SurgeSynthesizerIO.cpp | fxp read/write, `cjs3`, `loadRaw`, `saveRaw`, `enqueuePatchForLoad` | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/SurgePatch.cpp | `load_patch`, `save_patch`, `save_xml`, `load_xml`, LFO/FX declarations | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/ModulationSource.h | `modsources`, tag names, `ModulationRouting` | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/Parameter.cpp and Parameter.h | depth conversion, envtime/lforate ranges, `DisplayInfo` defaults | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/SurgeStorage.h and SurgeStorage.cpp | `ff_revision = 24`, OSC defaults, data paths, patch list | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/SurgeSynthesizer.cpp | FX type change path, `loadFx` | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/common/globals.h | OSC default ports and IP | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/surge-xt/SurgeSynthProcessor.cpp | get/setStateInformation, programs, OSC startup, bypass | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/surge-xt/osc/OpenSoundControl.cpp and .h | OSC address patterns | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/CMakeLists.txt | `JUCE_VST3_CAN_REPLACE_VST2=0`, `SURGE_EXPOSE_PRESETS` OFF | A |
| https://raw.githubusercontent.com/surge-synthesizer/surge/release_xt_1.3.4/src/surge-xt/CMakeLists.txt | plugin settings, preset-define use | A |
| https://api.github.com/repos/surge-synthesizer/surge/contents/libs/JUCE?ref=release_xt_1.3.4 | JUCE submodule sha `cf5754b1…` | A |
| https://api.github.com/repos/surge-synthesizer/JUCE/commits/cf5754b19c87ea63758802e3f4239c05a77f1412 | "Rebase to 7.0.12" | A |
| https://raw.githubusercontent.com/surge-synthesizer/JUCE/cf5754b19c87ea63758802e3f4239c05a77f1412/modules/juce_audio_plugin_client/juce_audio_plugin_client_VST3.cpp | VST3 `getState`/`setState`, trailer, program parameter, `IUnitInfo` | A |
| https://raw.githubusercontent.com/surge-synthesizer/JUCE/cf5754b19c87ea63758802e3f4239c05a77f1412/modules/juce_audio_plugin_client/juce_audio_plugin_client_LV2.cpp | LV2 state URIs, store/retrieve | A |
| https://raw.githubusercontent.com/surge-synthesizer/JUCE/cf5754b19c87ea63758802e3f4239c05a77f1412/modules/juce_core/memory/juce_MemoryBlock.cpp | JUCE base64 format | A |
| https://surge-synthesizer.github.io/manual-xt/ | OSC settings menu, user folder, "Since version 1.3" | A |
| https://surge-synthesizer.github.io/changelog (via search result) | OSC-related release notes | B |
| https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Preset+Locations.html and .../Preset+Format.html | Linux preset paths, header and chunk list layout | A |
| https://raw.githubusercontent.com/steinbergmedia/vst3_public_sdk/master/source/vst/vstpresetfile.h and .cpp | chunk ids, `kFormatVersion`, endianness handling | A |
| https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstunits.h and ivsteditcontroller.h | `IUnitInfo`, `kIsProgramChange` | A |
| https://raw.githubusercontent.com/lv2/lilv/v0.26.2/include/lilv/lilv.h | preset and state API | A |
| https://raw.githubusercontent.com/drobilla/jalv/master/src/state.c, src/gtk/actions.c, src/gtk/menu.c | jalv preset save, load and bank handling (master branch, not a 0.26-era tag; the jalv version was not matched to lilv 0.26.2) | A |
| https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/ardour/lv2_extensions.h | midnam interface and update feature | A |
| https://raw.githubusercontent.com/x42/avldrums.lv2/v0.7.3/src/avldrums.c, master `src/midnam_lv2.h` | `mn_drums`, `mn_buskman`, XML shape | A |
| https://polyphone.io/doc/files/sf_specifications_v2.04.pdf | SF2 structures and generators (footer says 2.01) | A (see caveat) |
| https://en.wikipedia.org/wiki/General_MIDI | GM percussion 35 to 81 | B |
| https://midi.org/general-midi-level-1, https://midi.org/midi-ci-profile-for-default-drum-note-map | RP-003 file name and GM drum map background | A (page text); RP-003 itself not read |
| `/usr/share/surge-xt/patches_factory/Leads/Sine Lead.fxp` (local) | real fxp header and XML (revision 20) | A (local file) |
| `/usr/share/surge-xt/wavetables/WT fileformat.txt` (local) | wavetable header layout | A (local) |
| `/usr/lib/lv2/Surge XT.lv2/manifest.ttl`, `dsp.ttl`; `/usr/lib/lv2/avldrums.lv2/avldrums.ttl`, `manifest.ttl`; `/usr/lib/lv2/presets.lv2/presets.ttl` (local) | LV2 preset listing, midnam declarations | A (local) |
| `/usr/lib/vst3/Surge XT.vst3/Contents/x86_64-linux/Surge XT.so` (local grep) | contains the string `JUCEPrivateData`; `VstW`/`FBCh` are character constants and are not greppable | A (local, partial) |

## What I could not confirm, and what to check next

1. Run one real check of the VST3 state bytes in the plughost: after `getState`, the bytes should end with 8 zero bytes, 8 zero bytes and `JUCEPrivateData`, and the start should be `sub3`. If it begins with `VstW`, the build flags differ from the tag.
2. The Surge GUI path for FX type change, and whether it resets parameters like the host path.
3. The OSC audio-thread handler for `/mod` (creation and depth 0 behaviour), and the OSC specification page itself.
4. The exact effect of `extend_range` on modulation depth scaling, and how the UI displays depth.
5. Names of the LFO shape and trigger-mode enums, and the LFO envelope behaviour for every shape.
6. RP-003 (GM1 original names), Ardour's user LV2 preset folder, lilv's default `LV2_PATH`, and the `FUID::toString` character form of the `.vstpreset` class id.
7. Possible test: build a small patch with the vibrato routing in section Q4, load it into Surge and measure pitch variation, rather than trusting the unit conclusions above.
8. No file was written in the repository by me. The saved PDF above is the only new file, and it lives under my tool-results folder.
