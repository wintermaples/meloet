//! The contract between Rust and the screen (docs/architecture.md 3).
//!
//! JSON shapes are camelCase. The TypeScript types in `ui/src/bridge/generated/` are written
//! from these types by `cargo test -p meloet-protocol`.
//! All times are CLOCK_MONOTONIC. JSON carries microseconds (`tUs`); the telemetry binary
//! carries nanoseconds. Song positions are ticks, 1920 to a quarter note, and a tempo is quarter
//! notes a minute whatever the meter (docs/architecture.md 11, 22-1, 22-2).

mod added;
mod audio;
mod automation;
mod export;
mod history;
mod pack;
mod params;
mod take;
mod telemetry;
mod timeline;
mod vocal;

use serde::{Deserialize, Serialize};

pub use added::{
    EffectRef, FileDropView, GmFamily, InsertEffectView, InsertView, KeyCount, LibraryView,
    MissingRef, MissingView, PluginEffectCategory, PluginFormatView, PluginInstrumentCategory,
    PluginKindView, PluginRole, PluginRowView, PluginState, PluginView, SamplePlaying, Sf2FileView,
    Sf2PresetView, SoundName, SoundRef, SoundSampleView, SoundView, StandIn,
};
pub use audio::{
    AUDIO_EXTENSIONS, AudioClipState, AudioClipView, AudioJob, AudioJobKind, AudioWorkView,
    CLIP_CENTS_MAX, CLIP_GAIN_MAX_DB, CLIP_GAIN_MIN_DB, CLIP_SEMITONES_MAX, ClipField, DropMode,
    MissingAudioView, PEAK_LEVELS, PEAKS_HEADER_LEN, PEAKS_KIND, PEAKS_MAX_COUNT, PEAKS_VERSION,
    Peaks, PeaksError, RecordingFaults, SOURCE_BPM_MAX, SOURCE_BPM_MIN, SelectedClip,
    SelectedClips, SourceTempoView, StretchChange, StretchView,
};
pub use automation::{
    AutoSelection, AutoTarget, AutomationView, LANE_HEADER_LEN, LANE_KIND, LANE_POINT_LEN,
    LANE_VERSION, LaneError, LaneName, LanePoint, LanePoints, LaneRef, LaneView, WritingLane,
};
pub use export::{
    EXPORT_TAIL_MAX_S, ExportChange, ExportCheck, ExportDialogView, ExportFailure, ExportFormat,
    ExportRange, ExportRunView, ExportSolo, ExportStoppedPlugin, ExportTail, ExportView,
};
pub use history::{HistoryRow, HistoryView, MeterValue, StepName, StepPlace, StepWhat, StripName};
pub use pack::{PackFailure, PackFetchView, SoundsView};
pub use params::{
    EffectName, NamedInsert, ParamEnd, ParamGroup, ParamKind, ParamRef, ParamRow, ParamTable,
    ParamValue, ParamValues, PluginWindowKind, PluginWindowView, SelectedParams, SelectedPlugin,
    WindowsView,
};
pub use take::{
    TAKE_CONTROL_LEN, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NO_FROM, TAKE_NOTE_LEN, TAKE_VERSION,
    TakeControl, TakeControlValue, TakeError, TakeNote, TakeNotes,
};
pub use telemetry::{
    MicMeter, TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_MIC_LEN, TELEMETRY_VERSION,
    Telemetry, TelemetryError, TransportReading,
};

pub use timeline::{MarkerView, MeterChangeView, TempoPointView, TimelineRef, TimelineView};
pub use vocal::{
    KeptVocals, LATENCY_CLICKS, LatencyDialogView, LatencyFailure, LatencyMeasure, LatencySource,
    LatencyView, RecordFormat, SelectedSound, TrackKindView, VocalInput, VocalRecordingView,
    VocalRef,
};

pub const PROTOCOL_VERSION: u32 = 27;

/// Screen → Rust. Intent only; Rust decides what the state becomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// CSS px inside the WebView.
    SetViewport {
        width: f32,
        height: f32,
    },
    /// 100 | 125 | 150
    SetScale {
        percent: u16,
    },
    /// With `Band::PianoRoll` while a vocal track is selected, the piano roll does not open:
    /// the status line says `StatusMessage::VocalHasNoPianoRoll`.
    ToggleBand {
        band: Band,
    },
    GoArrange,
    Back,
    /// Mod+, opens the front; "くわしく" in the status line opens it with the advanced part.
    OpenSettings {
        advanced: bool,
    },
    SettingsSelectOutput {
        output: String,
    },
    SettingsToggleAdvanced,
    SettingsApply,
    SettingsCancel,
    ChooserMove {
        delta: i32,
    },
    ChooserSelect {
        output: String,
    },
    ChooserConfirm,
    TestTone {
        output: String,
    },
    DismissHint,
    /// Space: play when stopped, stop otherwise.
    PlayStop,
    /// R and the record button: stopped → count-in then record; playing → record from here;
    /// counting in or recording → stop. The recording goes to the selected track
    /// (`SongView::play_target`); a song with no tracks says `StatusMessage::NoArmedTrack`.
    /// Nothing happens while the shelf auditions (`ShelfView::selected_sound`).
    Record,
    /// Home.
    GoToStart,
    /// A click on the ruler, and the position typed into "1. 1. 000" of the transport. Rust
    /// goes to `tick` as it is. The screen snaps first: in the arrange view to the lines the
    /// ruler shows, in the piano roll to its grid, not at all with Shift (B-61).
    Locate {
        tick: u64,
    },
    /// The tempo typed into the transport. Quarter notes a minute as typed; Rust keeps 0.01 steps.
    /// It changes the tempo in force where the play line is: the tempo point at or before it
    /// (inside a ramp, the point the ramp starts from), the head's tempo when there is none.
    /// With tempo points in the song the status line says `StatusMessage::TempoPointSet`.
    SetTempo {
        bpm: f64,
    },
    /// The meter typed into the transport: of the meter change in force where the play line
    /// is, the head's when there is none. With meter changes in the song the status line says
    /// `StatusMessage::MeterChangeSet`.
    SetMeter {
        num: u8,
        den: u8,
    },
    // Tempo points are named by their tick and meter changes by their bar (from 0); 0 is the
    // song's head (`timeline.rs`). A command that names a place with nothing at it does
    // nothing. Each command that changes the song is one step.
    //
    // The screen snaps what it sends (tempo points and markers to the heads of beats); core
    // snaps the same way again, to the nearest head of a beat, the later one on a tie.
    /// ▸ and ▾ of the row "テンポ ・ 拍子の変更" of the arrange view (`TimelineView::open`).
    /// No step.
    SetTimelineOpen {
        open: bool,
    },
    /// A double click on the tempo row. `bpm` is the tempo of the line there, which the
    /// screen reads from its map. A point already at `tick` gets `bpm`; tick 0 is
    /// the head's tempo. The new point is selected.
    AddTempoPoint {
        tick: u64,
        bpm: f64,
    },
    /// A drag of a tempo point, or a value typed on it: where it is and what it says when
    /// the drag ends; either may be as before. The head (`tick` 0) only takes `bpm`. A point
    /// that lands on another one takes its place (the other one goes). The point's ramp
    /// stays. Selected afterwards.
    MoveTempoPoint {
        tick: u64,
        to_tick: u64,
        bpm: f64,
    },
    /// "次の点まで、だんだん" of a tempo point's right-click menu. `tick` 0: from the head.
    SetTempoRamp {
        tick: u64,
        on: bool,
    },
    /// A double click on the meter row: a flag at the head of `bar`. A flag already there, and
    /// bar 0 (the head's meter), get the meter. `num` 1..=12 and `den` 2, 4, 8 or 16; other
    /// values do nothing. The new flag is selected.
    AddMeterChange {
        bar: u32,
        num: u8,
        den: u8,
    },
    /// A drag of a flag. Bar 0 does not move. A flag that lands on another one takes its place.
    MoveMeterChange {
        bar: u32,
        to_bar: u32,
    },
    /// A meter typed on a flag (`bar` 0: the head's).
    SetMeterChange {
        bar: u32,
        num: u8,
        den: u8,
    },
    /// A press on a tempo point, a flag or a marker's name tag in a way that does not go
    /// there (the right click), and a press on nothing (`None`). A thing that is not there
    /// selects nothing. For a tempo point and a flag the status line says
    /// `StatusMessage::TempoRampSelected`, `TempoPointSelected` or `MeterChangeSelected`.
    SelectTimeline {
        target: Option<TimelineRef>,
    },
    /// Delete on the selected thing, "消す" of the right-click menus. The head does not go.
    DeleteTimeline {
        target: TimelineRef,
    },
    /// K: a marker where the play line is (where it is heard, while playing), snapped to the
    /// nearest head of a beat. `name` is "マーカー 4" in the screen's words, the number being
    /// the song's markers with this one. The marker is selected and its name field is open
    /// (`TimelineView::renaming`); the status line says `StatusMessage::MarkerAdded`. Markers
    /// may share a tick.
    AddMarker {
        name: String,
    },
    /// A double click on a marker, F2 on the selected one, "名前を変える": opens its name
    /// field (`TimelineView::renaming`) and selects it. No step.
    OpenMarkerName {
        id: u32,
    },
    /// Enter in the name field. Spaces around the name are left out. An empty name, or one of
    /// spaces only, or the same name, keeps the old name and is no step. Closes the field
    /// when it is this marker's.
    RenameMarker {
        id: u32,
        name: String,
    },
    /// Esc in the name field, and the field losing the focus. The name stays as it was (for
    /// a marker just added: the name it was added with).
    CloseMarkerName,
    /// A drag of a marker.
    MoveMarker {
        id: u32,
        tick: u64,
    },
    /// A press on a marker: selects it and goes to its tick as `Locate` does. No step.
    GoToMarker {
        id: u32,
    },
    /// Alt → and Alt ←: to the first marker after the play line (`forward`), or the last one
    /// before it. Nothing when there is none. Selects the marker it goes to, as a press on it
    /// does. No step.
    StepMarker {
        forward: bool,
    },
    /// "ここから次のマーカーまでをループに": the loop's range becomes this marker's tick to the
    /// next marker's that is later (the song's end after the last marker, docs/architecture.md
    /// 21-12), which is one step, and the loop is turned on, which is none.
    LoopFromMarker {
        id: u32,
    },
    /// 0, 1 or 2.
    SetCountIn {
        bars: u8,
    },
    /// C.
    ToggleMetronome,
    /// L and the loop button. The range stays.
    ToggleLoop,
    /// A drag on the ruler. Bar lines, counting from 1, in either order: 5 and 9 are the heads
    /// of bars 5 and 9 ("ループ 5–9").
    SetLoop {
        from_bar: u32,
        to_bar: u32,
    },
    // Tracks are named by `TrackView::id` in every command, never by their number: the
    // numbers change when tracks are added, deleted or moved, and a command sent just before
    // that must not land on another track. A command for a track that is gone does nothing.
    /// A click on a track's header (B-70). The track becomes the selected one
    /// (`SongView::play_target`): the keys play it, and R records to it. There is no other
    /// "armed" keys track (spec 3, 12-A). Selecting a vocal track turns its ● on
    /// (`TrackKindView::Vocal::armed`), and the keys stay on the keys track selected last
    /// (`SongView::keys_target`, B-94-1).
    ///
    /// `from_arm`: sent by the ● of a track, the ● of the mixer and "録音待機" of the
    /// right-click menu, which only select the track. On the track already selected it does
    /// nothing (the ● cannot be turned off); on another track the status line says
    /// `StatusMessage::ArmMoved`.
    ///
    /// While counting in or recording the selected track stays as it is.
    SelectTrack {
        track: u32,
        from_arm: bool,
    },
    /// M, and Shift M on the selected track. Mute is part of the song: saved, and one step
    /// that undo takes back. A muted track is silent when the song plays (and while recording),
    /// but keys played on it still sound: mute is only about what the song plays. A muted
    /// track stays silent when it is soloed too.
    ToggleMute {
        track: u32,
    },
    /// S, and Shift S on the selected track. Solo is only for listening: not saved, not a
    /// step. Any number of tracks may be soloed; while one is, the song plays only the soloed
    /// tracks. Keys played sound on the selected track all the same.
    ToggleSolo {
        track: u32,
    },
    // The four below change the song: each is one step that undo takes back, and none does
    // anything while counting in or recording.
    /// "トラックを消す" of the right-click menu and the track menu. Does not ask. The track goes
    /// with its clips; the track below it becomes the selected one (the one above when it was
    /// the last; none when the song has no track left). The status line says
    /// `StatusMessage::TrackDeleted`.
    DeleteTrack {
        track: u32,
    },
    /// Enter in the name field (a double click on the name, F2, "名前を変える"). Spaces around
    /// the name are left out. An empty name, or one of spaces only, keeps the old name and is
    /// no step.
    RenameTrack {
        track: u32,
        name: String,
    },
    /// A drag of the header, "上へ動かす", "下へ動かす". `to`: the track's place among the
    /// tracks as they are afterwards, from 0; past the end is the end. The numbers follow the
    /// new order, the colors stay with their tracks. The selected track stays the same track.
    MoveTrack {
        track: u32,
        to: u32,
    },
    /// "色 ▸". `color` 1..=5 (`color.track.1` to `color.track.5`); other values do nothing.
    SetTrackColor {
        track: u32,
        color: u8,
    },
    // The shelf (round 4). Closing it stays `ToggleBand` and `Back`; closing it while it
    // auditions is `ShelfCancel` first.
    /// Opens the shelf (the right panel `Band::Shelf`, brought to the front) in `mode`.
    /// `NewTrack`: "＋ トラックを追加", "トラックを足す" of the menus, P. `Replace`: a click
    /// on the instrument's name in a track's header, or on the instrument slot of the mixer.
    /// `InsertEffect`: a click on an empty insert slot ("空き ＋"); the shelf then lists the
    /// effects. `ReplaceEffect`: 「入れかえる…」 of the right-click menu of a slot that holds
    /// an effect (spec 3, 86-3); the shelf lists the effects, and the row of the effect the
    /// slot holds is `ShelfView::current_effect`.
    /// When the shelf is open already, the mode changes and whatever was auditioned is put
    /// away as `ShelfCancel` does. `Replace` of a track that is gone does nothing, and so
    /// does `InsertEffect` for a strip that is gone or a slot that is not empty, and
    /// `ReplaceEffect` for a strip that is gone or a slot that is empty. Not while counting
    /// in or recording, but for `InsertEffect` and `ReplaceEffect`.
    OpenShelf {
        mode: ShelfMode,
    },
    /// A click on a row, ↑ ↓: the row becomes `ShelfView::selected_sound` and is auditioned.
    /// While a row is auditioned, the keys sound with that instrument and nothing can be
    /// recorded (`TransportView::can_record` is false). In `Replace` mode the clips of that
    /// track play with it too; the song itself is not changed. `None`, or an instrument that is not in
    /// `ShelfView::available`: as `ShelfCancel`.
    ShelfSelect {
        instrument: Option<InstrumentKind>,
    },
    /// Enter, "トラック N を作る", "入れかえる". Nothing without a selected row.
    ///
    /// `NewTrack`: a new track with the selected instrument right below the selected track
    /// (at the end when there is none), as one step, and that track becomes the selected one.
    /// Its name is the short name of the instrument's group (ピアノ, ベース, ドラム, パッド,
    /// シンセ); when a track has that name already, "ベース 2", with the smallest number from
    /// 2 that is free. Its color is the one of its place: colors 1 to 5 in the order of the
    /// tracks, again from 1 at the sixth. The status line says `StatusMessage::TrackAdded`.
    ///
    /// `Replace`: the track gets the selected instrument, as one step; its notes and its
    /// name stay. The status line says `StatusMessage::InstrumentReplaced`. Selecting the
    /// instrument the track has already is no step.
    ///
    /// `InsertEffect`: `ShelfView::chosen_effect` goes into the slot, with the parameters
    /// of a new one, as one step. It becomes the selected insert (as `SelectInsert`), so the
    /// parameters panel comes to the front, and the shelf's mode goes back to `NewTrack`.
    /// The status line says `StatusMessage::EffectInserted`.
    ///
    /// `ReplaceEffect`: `ShelfView::chosen_effect` takes the place of what the slot holds,
    /// with the parameters of a new one, as one step; undo brings the old effect back with
    /// its settings. What follows is as for `InsertEffect`, and the status line says
    /// `StatusMessage::InsertReplaced`. Choosing the effect the slot holds already
    /// (`ShelfView::current_effect`) is no step.
    ///
    /// Afterwards nothing is selected in the shelf, and the shelf stays open.
    ShelfConfirm,
    /// "やめる", and Esc while a row is auditioned: the audition ends and nothing is selected.
    /// In `Replace` mode the track sounds with its own instrument again. The shelf stays
    /// open; the next Esc closes it (`Back`).
    ShelfCancel,
    /// The star of a row, and F on the selected row. A meloet setting (settings.json), not
    /// part of the song.
    ToggleFavorite {
        instrument: InstrumentKind,
    },
    /// The ▶ of a row, and Space on a row of the shelf: plays the fixed phrase of that
    /// instrument's group (spec 3 「サンプル」), from its start. Another sample that is playing
    /// stops first. The sample is not recorded and is no part of the song; it goes through
    /// the master fader, whatever is muted or soloed. It does not select the row.
    PlaySample {
        instrument: InstrumentKind,
    },
    StopSample,
    // The mixer (round 4). A strip is named by `StripRef`, an insert slot by `InsertRef`.
    //
    // Fader, pan, send and parameter each come with `commit`, because a drag must be heard
    // while it goes on and still be one step:
    // - `commit: false` (every move of a drag): only the sound changes. The song does not,
    //   nothing is a step, and no state comes back; the screen shows the value it drags.
    //   Rust keeps the value as a "preview" of that one fader, pan, send or parameter.
    // - `commit: true` (the button let go, a value typed, a key step): the song gets the
    //   value as one step, and the preview of that control is over. A value the song has
    //   already is no step.
    // - `CancelMixPreview` (Esc during the drag): every preview is over and the sound is
    //   back at the song's values.
    // The keys (spec 3, 86-7: ↑ ↓ ← →, Home on a fader, pan or send that has the focus) use
    // the same two: every key step is sent with `commit: false`, and the screen sends the
    // value once more with `commit: true` when no key was pressed for 1 second after the last
    // one was let go, or the focus leaves. So the run of key steps is one step.
    // A preview is also over when the song changes under it (a step, undo, redo, another
    // song). All of this works while counting in or recording too, as do `ToggleReturnMute`,
    // `RemoveEffect` and the mute of a track: none of them moves where a take is recorded.
    // Values out of range are brought into range by Rust.
    /// The fader of a strip. `db`: `None` is −∞; below −60 becomes −∞, above +6 becomes +6
    /// (spec 2 「メーターとフェーダーの目盛」). Rust keeps 0.01 steps.
    SetGain {
        strip: StripRef,
        db: Option<f32>,
        commit: bool,
    },
    /// The pan knob: -100 (all left) to 100 (all right), 0 the middle ("L 20" is -20). The
    /// master has one too (mock F4-Mixer).
    SetPan {
        strip: StripRef,
        pan: i32,
        commit: bool,
    },
    /// How much of a track goes to return A or B, taken after its fader and pan. `db` as in
    /// `SetGain`.
    SetSend {
        track: u32,
        bus: SendBus,
        db: Option<f32>,
        commit: bool,
    },
    /// A parameter of an effect that comes with meloet, in an insert slot (a plugin's:
    /// `SetPluginParam`). `effect` is the effect the screen shows
    /// there and `param` the parameter's place in its table (`EFFECT_PARAMS`); when the slot
    /// holds another effect by now, or none, the command is dropped. `value` is in the
    /// parameter's own unit: a step is its number from 0, a switch 0 or 1.
    SetParam {
        insert: InsertRef,
        effect: EffectKind,
        param: u32,
        value: f32,
        commit: bool,
    },
    /// Esc while a fader, pan, send or parameter is dragged (see above).
    CancelMixPreview,
    /// The M of a return strip: the return is silent, or sounds again. Part of the song (one
    /// step, saved), as the mute of a track is. The S and ● of a return, and M, S and ● of
    /// the master, do nothing and have no command (design-notes A-11).
    ToggleReturnMute {
        bus: SendBus,
    },
    /// Takes the effect out of its slot, as one step. The slots below stay where they are.
    /// When it was the selected one, nothing is selected afterwards. 「外す」 of the slot's
    /// right-click menu, and Delete on the selected slot. A plugin goes the same way (its
    /// window closes). The status line says `StatusMessage::EffectRemoved`.
    RemoveEffect {
        insert: InsertRef,
    },
    /// A click on an insert slot that holds an effect or a plugin: it becomes the selected
    /// one (`ParamsView::selected`). `None`, or a slot that is empty: nothing is selected
    /// (the panel stays as it is). Not a step, and it works while recording too. An empty
    /// slot ("空き ＋") opens the shelf instead: `OpenShelf` with `ShelfMode::InsertEffect`.
    ///
    /// What comes with it (spec 3 「パラメータ」): for an effect that comes with meloet, and
    /// for a plugin whose `PluginWindowKind` is `NotYet` or `None`, the parameters panel
    /// (`Band::Params`) opens, in front; for `NotYet` the status line says
    /// `StatusMessage::PluginWindowNotYet`. For a plugin whose window opens, the window opens
    /// as `OpenPluginWindow` does (it comes to the front when it is open already) and the
    /// panel stays as it is. A plugin that is not `PluginState::Ready` is selected and
    /// nothing else happens.
    SelectInsert {
        insert: Option<InsertRef>,
    },
    /// A click on the instrument slot of a track whose sound is a plugin: the plugin becomes
    /// the selected one (`SelectedParams::Plugin` with `PluginSlotRef::Instrument`), with
    /// its window or the panel as for `SelectInsert`. A click on the sound's name in a track's
    /// header sends it too. For a sound that comes with meloet or a preset of an SF2 file the
    /// panel opens, in front, and says that there are no parameters (`SelectedParams::Sound`),
    /// and so does the status line (`StatusMessage::NoParams`; spec 3, 19-A). Nothing for a
    /// track that is gone and for a vocal track.
    SelectInstrument {
        track: u32,
    },
    /// A row of the parameters panel is chosen (a click, ↑ ↓): `SelectedInsert::row` or
    /// `SelectedPlugin::row`. `param` is the parameter's place in the effect's table for an
    /// effect that comes with meloet, `ParamRow::id` for a plugin. The status line then says
    /// `StatusMessage::ParamSelected` or `StatusMessage::PluginParamSelected`. `None`, or a
    /// number the effect has no parameter for: no row.
    SelectParam {
        param: Option<u32>,
    },
    /// A parameter of the selected plugin, moved in the parameters panel. `table` is the
    /// table the screen shows (`SelectedPlugin::table`) and `param` a `ParamRow::id` of it;
    /// when the plugin at `slot` has another table by now, or `slot` holds no plugin that is
    /// `PluginState::Ready`, the command is dropped. `value` is 0.0..=1.0 (`ParamRow::value`;
    /// a step as n / (steps − 1), a switch 0 or 1). `commit` as for `SetParam`: with `false`
    /// only the plugin hears the value, with `true` the song gets it as one step, and
    /// `CancelMixPreview` puts the plugin back at the song's value. Either way the value and
    /// its text come back in a `ParamValues` message.
    ///
    /// While the plugin's window is open, a `commit: true` first makes a step of what was
    /// changed in the window until then (spec 3, Y15), then its own.
    SetPluginParam {
        slot: PluginSlotRef,
        table: u32,
        param: u32,
        value: f64,
        commit: bool,
    },
    /// Enter in the value field of a row of the parameters panel (spec 3, 86-6): `text` as
    /// typed, with or without the unit. For an effect that comes with meloet Rust reads the
    /// number in it ("4" is 4 : 1); for a plugin the plugin reads it. The value is set as one
    /// step. A value past an end of the range is put at that end, and the status line says
    /// `StatusMessage::ParamClamped`. A text that does not read as a value changes nothing,
    /// and the status line says `StatusMessage::ParamRejected`. A stale `param` is dropped as
    /// for `SetParam` and `SetPluginParam`. For a plugin the value is set, and the status
    /// line speaks, once the plugin has read the text: a moment after the command.
    TypeParam {
        param: ParamRef,
        text: String,
    },
    /// 「とばす」 and 「もどす」 (spec 3, 13-A, 86-1): the mark at the left of a slot, B on the
    /// selected slot, 「とばす」 in the head of the parameters panel and in the right-click
    /// menu. Flips `InsertView::bypassed`, as one step, saved with the song. Nothing for an
    /// empty slot, and nothing for a plugin that is `PluginState::Stopped`. It works while
    /// counting in or recording too. The status line says `StatusMessage::EffectBypassed`.
    ToggleBypass {
        insert: InsertRef,
    },
    /// A drag of a slot inside its strip, 「上へ動かす」 and 「下へ動かす」 (spec 3, 86-2), as
    /// one step. `to`: the place of the slot among the four as they are afterwards, from 0;
    /// past the end is the end. The slots between move by one, empty ones too, as tracks do
    /// for `MoveTrack` (「上へ動かす」 is `to: slot − 1`). The selected insert and the open
    /// windows stay with their effects. Nothing for an empty slot, or when `to` is where it
    /// is. The status line says `StatusMessage::InsertsReordered`.
    MoveInsert {
        insert: InsertRef,
        to: u8,
    },
    /// `ShelfMode::InsertEffect`: a click on a row of the shelf's effects, ↑ ↓. The row
    /// becomes `ShelfView::chosen_effect`. There is nothing to audition: the effect is in
    /// the song only after `ShelfConfirm`. `None`: no row.
    ShelfSelectEffect {
        effect: Option<EffectKind>,
    },
    /// A plain click on a clip: it alone is selected and shown in the piano roll, and its track
    /// becomes the selected track (as `SelectTrack`). `None`: a click where there is no clip,
    /// which unselects every clip (the selected track stays).
    SelectClip {
        clip: Option<u32>,
    },
    /// More than one clip (B-71). Shift + click on a clip is `Add` with that clip (it is then
    /// `selected_clip`, shown in the piano roll, and its track selected, as with `SelectClip`).
    /// A box drawn from an empty place is `Replace` with the clips it touches, Shift + box
    /// `Add`; a box changes neither the piano roll nor the selected track. Clips that are not
    /// there are ignored.
    SelectClips {
        clips: Vec<u32>,
        mode: SelectMode,
    },
    /// Mod A while the arrange view has the keys.
    SelectAllClips,
    /// Delete: every selected clip (`ArrangeView::selected_clips`), as one step.
    DeleteSelected,
    /// ◂ ▸, T, Shift T on the selected clip. -1 is the take before, 1 the take after.
    StepTake {
        delta: i32,
    },
    /// "テイクを消す" (the right-click menu of a clip, the track menu): the take `clip` shows,
    /// as one step. The clip then shows the take before it (the next one when it was the first),
    /// and the takes after it move up a number. Nothing happens to a clip with one take, while
    /// recording or counting in, or to a clip that is gone.
    DeleteTake {
        clip: u32,
    },
    Undo,
    Redo,
    /// Mod N and the song menu. Asks first when there are unsaved changes.
    NewSong,
    /// Mod O and the song menu. Asks first when there are unsaved changes, then Rust shows the
    /// PC's open window.
    OpenSong,
    /// Mod S and the song menu. The first save goes to the song folder without asking.
    SaveSong,
    /// A button of the song dialog. `dialog` is `SongDialogView::id`; an answer to a dialog that
    /// is no longer up, or that the dialog has no button for, is dropped. Esc stays `Back`,
    /// which is "やめる" while a song dialog is up.
    AnswerSongDialog {
        dialog: u32,
        answer: SongAnswer,
    },
    // The piano roll and the clips (round 3). A note is named by `take`, the id of the take the
    // piano roll shows (`PianoRollView::take`), and its index in that take's notes (in order of
    // start, from 0). A command whose `take` is not the take shown now is dropped: it was aimed
    // at notes that have moved since. Fields named `start`, `at` or `tick` are song positions;
    // fields named `ticks` are how far to move.
    //
    // Every command that changes notes makes a new take (a new id) and puts it in place of the
    // shown one in its clip, so the number of takes stays the same; undo brings the old id back.
    /// A click, a box drawn with the select tool, Shift + box (`Add`), and Mod + click (`Toggle`).
    SelectNotes {
        take: u32,
        notes: Vec<u32>,
        mode: SelectMode,
    },
    /// Mod A in the piano roll.
    SelectAllNotes,
    ClearNoteSelection,
    /// The write tool: one command when the mouse button is let go. `take: None` when the piano
    /// roll shows no clip: Rust makes a clip on the track the keys play, from the bar of `start`,
    /// 4 bars long or up to the next clip, named after the track; making it and writing the note
    /// are one step (the user's decision, 2026-11-15).
    AddNote {
        take: Option<u32>,
        start: u64,
        length: u64,
        key: u8,
        velocity: u8,
    },
    /// A drag, ← → (a grid step), ↑ ↓ (1 or 12 keys). `copy`: Alt + drag and Mod D (the copies
    /// are selected afterwards).
    MoveNotes {
        take: u32,
        notes: Vec<u32>,
        ticks: i64,
        keys: i32,
        copy: bool,
    },
    /// A drag on an edge of the notes, every note by the same amount. `End`: the end moves and
    /// the start stays. `Start` (B-69): the start moves and the end stays, so `ticks` > 0 makes
    /// the notes shorter. A note never gets shorter than 1 tick.
    ResizeNotes {
        take: u32,
        notes: Vec<u32>,
        edge: ClipEdge,
        ticks: i64,
    },
    /// The erase tool, and Delete while the piano roll has the keys. Deleting clips is
    /// `DeleteSelected`.
    DeleteNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// The velocity lane. The result stays in 1..=127.
    ChangeVelocity {
        take: u32,
        notes: Vec<u32>,
        delta: i32,
    },
    /// Mod C.
    CopyNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// Mod X.
    CutNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// Mod V: into the clip of the piano roll, the earliest note at `at`.
    PasteNotes {
        at: u64,
    },
    /// `division`: how many to a whole note (4 is a quarter, 16 a sixteenth). `triplet` makes
    /// each step 2/3 as long.
    SetGrid {
        division: u16,
        triplet: bool,
    },
    /// "スケール固定" and "音階". `root` 0..=11 is C..B.
    SetMusicalScale {
        root: u8,
        mode: ScaleMode,
        lock: bool,
    },
    /// The piano roll sounds keys on the selected track (B-62): the write tool and the keyboard
    /// column while pressed (mock R3-PRMouse), a note while it is pressed, a drag at each change
    /// of key, ↑ ↓ on selected notes as a chord. Whatever was sounding stops first: this
    /// replaces it. Empty `keys` only stops. The screen times how long it sounds (↑ ↓: about
    /// 0.3 s) and sends `AuditionOff`, and sends the fixed velocity while
    /// `KeyboardsView::velocity_fixed` is on. Keys outside the locked scale are left out, and
    /// keys past the first 16.
    ///
    /// Not recorded, and no `Event::Note` comes back for it. The keys lit in the piano roll's
    /// keyboard column are the keys played (`Event::Note`) and the keys the screen sounds
    /// (`sounding` in `ui/src/components/pianoroll/Roll.svelte`), so the screen lights what it
    /// sends, on its own.
    Audition {
        keys: Vec<u8>,
        velocity: u8,
    },
    AuditionOff,
    /// The quantize panel: the settings the next `Quantize` uses. `strength` and `swing` are
    /// percent, 0..=100.
    SetQuantize {
        division: u16,
        triplet: bool,
        strength: u8,
        swing: u8,
        align: QuantizeAlign,
    },
    /// Q and "かけなおす". Empty `notes`: every note of the take. The new take points to the
    /// recorded take it came from (`TakeNotes::origin`).
    Quantize {
        take: u32,
        notes: Vec<u32>,
    },
    /// "もとの位置へ戻す": the notes go back to where they are in the origin take.
    Unquantize {
        take: u32,
        notes: Vec<u32>,
    },
    /// The note list and the "選んだノート" panel. `Length` and `Velocity` set every note to
    /// `value`. `Start` puts the earliest note at `value` (a song position) and keeps the
    /// distances of the others; `Key` puts the lowest note on `value` and keeps the intervals.
    SetNotesField {
        take: u32,
        notes: Vec<u32>,
        field: NoteField,
        value: i64,
    },
    /// Control points are named by their index in the take's controls (in order of tick).
    SelectControls {
        take: u32,
        points: Vec<u32>,
        mode: SelectMode,
    },
    /// CC 0..=127 (an on/off CC such as sustain only 0 or 127), pitch bend -8192..=8191.
    AddControl {
        take: u32,
        tick: u64,
        control: ControlKind,
        value: i32,
    },
    /// `value` is how much the values change.
    MoveControls {
        take: u32,
        points: Vec<u32>,
        ticks: i64,
        value: i32,
    },
    DeleteControls {
        take: u32,
        points: Vec<u32>,
    },
    /// A drag in the arrange view, of the selected clips together (B-71). Each clip moves by
    /// `ticks`, and by `tracks` rows of `SongView::tracks` (+ down, - up, 0 stays on its track).
    /// `copy`: Alt + drag. The group stops at the song's start, as one clip does. One step, or
    /// nothing at all: nothing happens when a clip would leave the tracks or overlap a clip not
    /// in `clips` (docs/architecture.md 17). Afterwards the moved clips (or the copies) are selected;
    /// `selected_clip` (or its copy) stays that, and the piano roll shows it.
    MoveClips {
        clips: Vec<u32>,
        ticks: i64,
        tracks: i32,
        copy: bool,
    },
    /// Mod D in the arrange view: copies of `clips` right after the group (the latest end among
    /// them), on their own tracks, keeping their distances. Otherwise as `MoveClips` with `copy`.
    DuplicateClips {
        clips: Vec<u32>,
    },
    /// Mod C in the arrange view. Rust keeps the clips (their takes, rows and distances) until
    /// the next copy or cut; `ArrangeView::clipboard_clips` says how many.
    CopyClips {
        clips: Vec<u32>,
    },
    /// Mod X in the arrange view: a copy, and the clips deleted, as one step.
    CutClips {
        clips: Vec<u32>,
    },
    /// Mod V and "貼りつけ" in the arrange view, as one step. The earliest clip kept goes to
    /// `to`; the others keep their distances and their rows from it. `to: None` (Mod V): on the
    /// selected track, at the head of the bar the play position is in. Rust snaps, as it knows
    /// the bars and where the play line is while playing. `Some` (the right-click menu): where
    /// it was clicked, as it is. Nothing happens when a clip would leave the tracks or overlap
    /// (as `MoveClips`). The pasted clips are selected.
    PasteClips {
        to: Option<ClipSpot>,
    },
    /// A drag on an edge. What is trimmed off stays in the take and comes back when the edge
    /// is pulled out again.
    TrimClip {
        clip: u32,
        edge: ClipEdge,
        ticks: i64,
    },
    /// Enter on a number field that holds what does not read as a number ("あ", nothing).
    /// `kept` is the value it still has, as shown. Rust only says so in the status line; the
    /// field stays "broken". `range`, as shown, for the fields whose range depends on the clip
    /// or the meter (`Position`, `Length`, `PlayPosition`); `None` for the fixed ones, whose
    /// range is a sentence of the screen.
    RejectValue {
        field: ValueField,
        kept: String,
        #[serde(default)]
        range: Option<String>,
    },
    /// A number out of range was typed (B-68). The screen puts it at the nearest end (decimals
    /// rounded; the meter's lower number to the nearest of 2 4 8 16, the larger one on a tie; a
    /// position's bar, beat and tick each by itself), sends the command that sets that value,
    /// and then this one, so that the status line says "ベロシティは 1〜127 なので、127 にしま
    /// した。". `value` is the value set, as shown; `range` as in `RejectValue`. Rust only says
    /// so; it is not a step.
    ClampValue {
        field: ValueField,
        value: String,
        #[serde(default)]
        range: Option<String>,
    },
    /// "そろえる" of the velocity panel, and Enter in its field (B-67): every note to `value`
    /// (1..=127), as one step. The status line says "3 ノートのベロシティを 100 にそろえました。"
    /// (`StatusMessage::VelocityMatched`). A clamped value: `ClampValue` after this one, whose
    /// sentence then replaces that one.
    MatchVelocity {
        take: u32,
        notes: Vec<u32>,
        value: u8,
    },
    /// "そろえる" of the length panel, and Enter in its field (design-notes C-6 51): every note
    /// to `value` ticks, as one step, each kept inside its clip as `SetNotesField` does. The
    /// status line says "3 ノートの長さを 0.240 にそろえました。" (`StatusMessage::LengthMatched`).
    /// A clamped value: `ClampValue` after this one, as for `MatchVelocity`.
    MatchLength {
        take: u32,
        notes: Vec<u32>,
        value: u64,
    },
    /// "固定" next to the velocity reading of the input meter, and a value typed on that reading
    /// (B-66). `value` 1..=127. A meloet setting (settings.json), not part of the song. While on,
    /// every key played sounds and is recorded at `value`, and `Event::Note` and the telemetry's
    /// velocity carry `value`, so the input meter reads it.
    SetVelocityFixed {
        on: bool,
        value: u8,
    },
    /// Mod Shift S and the file menu (B-59). Rust shows the PC's save window, titled
    /// "名前をつけて保存", with the song's name filled in. The folder `<name>/<name>.meloet` is made
    /// where the user chose, and the song takes that name. Cancelling the window changes
    /// nothing.
    SaveSongAs,
    /// Mod Q and "meloet を終わる": the same as closing the window (asks first when there are
    /// unsaved changes).
    Quit,
    /// "戻さない" on the recovery hint: back to the song as last saved, as one step.
    DiscardRecovered,
    /// "ファイルの場所をひらく" on the hint about a recovery file that did not read.
    ShowRecoveryFile,
    /// "もう一度スキャン" (right of the shelf's heading 「あとから入れたもの」, and under the plugin
    /// places): every bundle of every place is looked at again. Plugins that stopped a scan
    /// before are passed over (spec 3 Y13); `RetryStoppedPlugin` tries one of them again.
    /// While it runs, `PluginsView::scan` counts; does nothing while a counted scan runs.
    RescanPlugins,
    /// "もう一度ためす" on a row of the dialog 「スキャンで止まったプラグイン」: that one plugin
    /// (or that whole bundle) is scanned again. `path` and `id` are `StoppedPluginView::path`
    /// and `StoppedPluginView::id`. The row is `retrying` until it is done; then it is gone
    /// (the plugin is in the shelf) or back as it was, and the status line says
    /// `ScanRetryAdded` or `ScanRetryStopped`.
    RetryStoppedPlugin {
        path: String,
        id: Option<String>,
    },
    /// "場所を足す…": Rust shows the PC's window for choosing a folder. The folder joins
    /// `PluginsView::places` and it alone is scanned. Cancelling the window, or choosing a
    /// folder that is in the list already, changes nothing.
    AddPluginPlace,
    /// "外す" on a place the person added (`PluginPlaceView::added`); `path` is
    /// `PluginPlaceView::path`. Its plugins leave the shelf. A usual place stays.
    RemovePluginPlace {
        path: String,
    },
    /// "止まった 4 個を見る": puts up the dialog 「スキャンで止まったプラグイン」
    /// (`PluginsView::stopped_open`).
    OpenScanStopped,
    /// "閉じる" and Esc on that dialog.
    CloseScanStopped,
    /// "プラグインの場所…" on that dialog: closes it and opens the sound settings with the
    /// advanced part and 「プラグインの場所」 unfolded.
    OpenPluginPlaces,
    /// The ▾ of 「プラグインの場所」 in the advanced part of the sound settings.
    TogglePluginPlaces,
    /// "もう一度読みこむ" for a plugin that stopped (spec 6 「プラグインが止まった」): its host
    /// process is started again and the plugin gets back the state it had. Not used before
    /// plugins can be put on a track.
    ReloadPlugin {
        slot: PluginSlotRef,
    },
    /// "外す" for a plugin that stopped: takes it out of its slot, as one step. Not used before
    /// plugins can be put on a track.
    RemovePlugin {
        slot: PluginSlotRef,
    },
    // 「あとから入れたもの」 (round 5): SF2 presets and plugins in the shelf. The four below
    // do for any sound or effect what `ShelfSelect`, `PlaySample`, `ToggleFavorite` and
    // `ShelfSelectEffect` do for the ones that come with meloet; with a `Builtin` they are
    // the same command.
    /// A click on a row, ↑ ↓: as `ShelfSelect`, the row becomes `ShelfView::selected_sound`
    /// and is auditioned. A plugin, and an SF2 preset, is read first: until then it is
    /// `ShelfView::loading_sound` and the keys are silent. A plugin that does not load
    /// becomes `ShelfView::unreadable` and nothing is selected; its window is not opened.
    /// `None`, or a sound the shelf does not have: as `ShelfCancel`.
    ShelfSelectSound {
        sound: Option<SoundRef>,
    },
    /// The ▶ of a row: as `PlaySample`, with the phrase of the sound's group. An SF2 preset
    /// plays the phrase of the nearest group; a plugin the drum phrase when it says it is a
    /// drum, the piano phrase when it says piano or keys, else the synth phrase, with the
    /// sound it has when loaded (spec 3 Y14). While the sound is read, `ShelfView::sound_sample`
    /// has no `playing`.
    PlaySampleOf {
        sound: SoundRef,
    },
    /// The star of a row, and F on the selected row: `ShelfView::favorite_sounds`. A meloet
    /// setting.
    ToggleFavoriteSound {
        sound: SoundRef,
    },
    /// The star of a row of the shelf's effects, and F (spec 3, 86-4):
    /// `ShelfView::favorite_effects`. A meloet setting; it starts with the reverb and the
    /// delay.
    ToggleFavoriteEffect {
        effect: EffectRef,
    },
    /// `ShelfMode::InsertEffect`: as `ShelfSelectEffect`, the row becomes
    /// `ShelfView::chosen_effect`. `ShelfConfirm` then puts it into the slot as one step: a
    /// plugin with the state it starts with. `None`, or an effect the shelf does not have: no
    /// row.
    ShelfChooseEffect {
        effect: Option<EffectRef>,
    },
    /// "＋1" on a plugin's row, then one of its formats: the row uses `key`
    /// (`PluginFormatView::key`) from now on. A meloet setting. What the song already holds
    /// stays in the format it was put in with. A key no row has does nothing.
    UsePluginFormat {
        key: String,
    },
    /// 「窓をひらく」, 「窓を前に出す」, and a row of the list above 「窓 N」: the plugin's own
    /// window opens, or comes to the front when it is open already (`WindowsView::list`).
    /// `None`: of the plugin the shelf auditions (`ShelfView::selected_sound`), which sits in
    /// no slot of the song yet; nothing while it is still read or when the row is no plugin.
    /// `Some`: of a plugin of the song; nothing for a slot that holds no plugin, one that is
    /// not `PluginState::Ready`, or one whose `PluginWindowKind` is not `Opens`.
    ///
    /// When the window is not up 0.2 s later, the status line says
    /// `StatusMessage::PluginWindowOpening` until it is; when it is up and does not fit the
    /// screen, `StatusMessage::PluginWindowLarge`.
    ///
    /// What was changed in the window becomes one step when the person comes back to meloet
    /// (the window closed, or another window got the keys), if the plugin's state differs
    /// from the one it had when the window got the keys (spec 3, Y15). Nothing of that is
    /// in the contract: `SongView::undo_count` goes up by one.
    OpenPluginWindow {
        slot: Option<PluginSlotRef>,
    },
    /// Closes the window of the plugin at `slot` (`PluginWindowView::slot`), as its own close
    /// button does: what was changed in it becomes a step first. Nothing when it has no
    /// window open. No part of the screen sends this (user:2026-11-19: a window is closed
    /// with its own button, and when its plugin leaves the song).
    ClosePluginWindow {
        slot: Option<PluginSlotRef>,
    },
    /// 「ぜんぶ前に出す」 at the bottom of the list above 「窓 N」: every window of
    /// `WindowsView::list` comes in front of meloet's window, in the order of the list.
    RaisePluginWindows,
    /// 「SF2 を足す…」 (the heading 「あとから入れたもの」, and the notice of an SF2 file that is
    /// not found): Rust shows the PC's window for choosing a file. The file is copied into
    /// meloet, joins `LibraryView::sf2` as `opened_sf2`, and the shelf opens on its
    /// instruments; the status line says `StatusMessage::Sf2Added`. A file of a name that is
    /// there already takes that one's place. A file that does not read is not added
    /// (`StatusMessage::Sf2Unreadable`). Dropping a file on the window does the same with no
    /// command. Not while counting in or recording.
    AddSf2,
    /// Esc while files are held over the window: `WorkspaceView::file_drop` is `None` again,
    /// and letting go of them adds nothing.
    CancelFileDrop,
    /// 「曲から外す」 on a notice of `ShelfView::missing`, as one step: every track that uses
    /// it gets the sine wave (its notes and its name stay), and every insert slot that holds
    /// it is emptied. Not while counting in or recording.
    RemoveMissing {
        missing: MissingRef,
    },
    // Automation (round 6; spec 3 Y16, docs/architecture.md 21-5 to 21-11). A lane is named
    // by its track and target; points by `LaneView::lane_id` and their places in the lane,
    // and a command with a `lane` that is no lane's id now does nothing. Moving a fader or a
    // parameter by hand stays what it was (`SetGain`, `SetParam`, …): Rust makes it rest the
    // lane, or write into it while 「● 書く」 is on and the song plays.
    /// 「▸ パン」 of the row under the track's header, and the mark at the right of a row of
    /// the parameters panel: makes the lane, empty and unfolded, as one step. When the track
    /// has the lane already it is unfolded, which is no step. Nothing for a target the track
    /// does not have (no such insert, an instrument that is no plugin).
    AddLane {
        track: u32,
        target: AutoTarget,
    },
    /// 「帯を消す」: the lane goes with its points, as one step.
    DeleteLane {
        track: u32,
        target: AutoTarget,
    },
    /// ▾ and ▸ of a lane's header. No step.
    FoldLane {
        track: u32,
        target: AutoTarget,
        folded: bool,
    },
    /// A press on an empty place of the lane. Rust brings `tick` to the grid. `value`:
    /// 0.0..=1.0; without it the value of the line at `tick`. A point that is at that tick
    /// already gets the value. One step; the new point is selected.
    AddAutoPoint {
        lane: u32,
        tick: u64,
        value: Option<f32>,
    },
    /// One drag of points, as one step: each goes `d_tick` ticks along and `d_value` up, the
    /// group stopping together at the song's start and at 0 and 1. A point that was not
    /// dragged and has the tick of a dragged one afterwards is gone. The moved points are
    /// selected.
    MoveAutoPoints {
        lane: u32,
        points: Vec<u32>,
        d_tick: i64,
        d_value: f32,
    },
    /// Two presses on a point, or Delete with points selected. One step.
    DeleteAutoPoints {
        lane: u32,
        points: Vec<u32>,
    },
    /// A press on a point, a drag from an empty place around points; empty: nothing is
    /// selected. No step.
    SelectAutoPoints {
        lane: u32,
        points: Vec<u32>,
    },
    /// 「● 書く」 of a lane's header (spec 3, 16-B). No step, not saved; nothing for a lane
    /// that rests.
    SetLaneWrite {
        track: u32,
        target: AutoTarget,
        on: bool,
    },
    /// 「もどす」 of a resting lane, in its header and under the fader: the lane moves its
    /// target again. One step.
    RestoreLane {
        track: u32,
        target: AutoTarget,
    },
    /// 「ここまで戻す」 and Enter on a row of the history band (spec 3, Y19): row `index`
    /// becomes 「いまここ」, by undoing or redoing as many steps as it takes. `step` is the
    /// row's `HistoryRow::step`; when row `index` is another step by now, nothing happens.
    /// Nothing either when row `index` is 「いまここ」 already. Not while counting in or
    /// recording, as undo.
    HistoryJump {
        index: u32,
        step: u64,
    },
    // Vocal tracks, the input and the latency (protocol 23; spec 3, 23-A, 24-C, 25-B, 33-A).
    /// 「＋ 歌のトラック」 under the tracks and 「歌のトラックを足す」 of the menu: a vocal track
    /// named 「歌」 (「歌 2」 when there is one) below the last track, with 「入口 1 ・ モノ」,
    /// selected and with its ● on. One step. The status line says
    /// `StatusMessage::VocalTrackAdded`, or `StatusMessage::NoInput` when the PC has no input
    /// (the track is made all the same).
    AddVocalTrack,
    /// A row of the list under 「入口 1 ・ モノ ▾」 of a vocal track's header and of its
    /// instrument slot in the mixer. One step; nothing for a keys track, the input the track
    /// has already, and while counting in or recording.
    SetVocalInput {
        track: u32,
        input: VocalInput,
    },
    /// The ● of a vocal track (header, mixer, 「録音待機」 of the right-click menu): on or
    /// off, whichever track is selected (spec 3, 24-C). No step. Nothing for a keys track
    /// (its ● is `SelectTrack` with `from_arm`), and while counting in or recording.
    ToggleVocalArm {
        track: u32,
    },
    /// 「モニター」 of a vocal track (header, the right half of the input meter). No step, and
    /// not saved. The first time it is turned on the status line says
    /// `StatusMessage::MonitorFirstOn`.
    ToggleMonitor {
        track: u32,
    },
    // The advanced sound settings (mock F7-SettingsAdv). Each takes effect at once and is
    // kept with the PC's settings, not with the song; none is a step. What runs is in
    // `AudioView`. Nothing while counting in or recording.
    /// 「入口」: the PC's input the vocal tracks record from, by `EndpointView::id`. `None`:
    /// the one the PC uses.
    SettingsSelectInput {
        input: Option<String>,
    },
    /// 「バッファ」: the frames meloet asks for (one of `AudioView::buffer_choices`).
    SettingsSetBuffer {
        frames: u32,
    },
    /// 「サンプルレート」: the rate meloet asks for (one of `AudioView::sample_rate_choices`).
    SettingsSetSampleRate {
        hz: u32,
    },
    /// 「録る形」.
    SettingsSetRecordFormat {
        format: RecordFormat,
    },
    /// 「手で決める…」: the round trip typed in ms (`LatencyView::manual_ms`), 0.0..=500.0;
    /// a number outside is put at the nearer end. `None`: no typed number any more.
    SettingsSetManualLatency {
        ms: Option<f32>,
    },
    /// 「遅れを測る…」: the dialog opens over the sound settings
    /// (`WorkspaceView::latency_dialog`). Not while playing, counting in or recording.
    OpenLatencyMeasure,
    /// 「測る」 and 「もう一度」: plays the clicks and listens for them. The keys stay
    /// playable; play and record are unusable until it is done.
    StartLatencyMeasure,
    /// 「やめる（Esc）」 while measuring: stops, back to `LatencyMeasure::Idle`. The dialog
    /// stays.
    StopLatencyMeasure,
    /// 「この数を使う」: the measured number becomes `LatencyView::measured_ms` for this
    /// output, input and sample rate, and the dialog closes. Only with
    /// `LatencyMeasure::Done`.
    UseMeasuredLatency,
    /// 「閉じる」 and Esc while not measuring: the dialog closes; a measuring going on stops.
    CloseLatencyMeasure,
    // Vocal clips and audio files (protocol 24; spec 3, 26-A, 27-A, 29-A, 30-C;
    // docs/architecture.md 22-17).
    /// S: cuts clips in two at the play line, as one step however many
    /// (`StatusMessage::ClipSplit`): the selected clips the line is inside of, or with no
    /// clip selected the clip of the selected track under the line. Keys clips too (a note
    /// over the cut ends at it in the left clip). A vocal clip keeps its fade-in on the left
    /// half and its fade-out on the right one. Afterwards the right halves are selected.
    /// Nothing happens when the line is inside no such clip (on an edge is not inside).
    /// While playing the line is where it is heard when Rust takes the command. Not while
    /// counting in or recording.
    SplitClips,
    /// A value of a vocal clip, as one step: the end of a drag of the volume line or of a
    /// fade handle, a double click on the volume line (`GainDb` 0), and a value typed into
    /// the panel 「選んだクリップ」. `value` is ticks, or dB for `GainDb`. Rust puts it into
    /// its range (the volume `CLIP_GAIN_MIN_DB..=CLIP_GAIN_MAX_DB`; a fade no longer than the
    /// clip less the other fade; a length of at least a tick). `Start` and `Length` also work
    /// on a keys clip; a `Start` or `Length` that would overlap another clip does nothing
    /// (`StatusMessage::ClipsNotMoved`). Nothing happens for a `clip` that is no clip's id,
    /// or a value that is not a number.
    SetClipField {
        clip: u32,
        field: ClipField,
        value: f64,
    },
    /// A double click on a vocal clip: the row of its track is 160 high and shows the
    /// waveform large (`ArrangeView::tall_track`). `None`, or the track that is tall
    /// already: every row is 64 again. Not a step, not saved.
    SetTallTrack {
        track: Option<u32>,
    },
    /// 「音のファイルを入れる…」 of the menu 「ファイル」 (`track: None`) and of the right click
    /// on a vocal track (its id): Rust shows the PC's window for choosing files (wav, mp3,
    /// flac, ogg; more than one may be chosen). They go in at the play line: on `track`, or
    /// for `None` on the selected track when that is a vocal track, else on a new vocal
    /// track. More than one go one after the other in the order of their names
    /// (`DropMode::InRow`). One step for all of them (`StatusMessage::AudioImported`); a
    /// file that does not read is left out (`StatusMessage::AudioUnreadable`). A file that
    /// would overlap a clip of the track goes to a new vocal track. Not while counting in
    /// or recording.
    ImportAudio {
        track: Option<u32>,
    },
    /// The screen's answer to `Event::FileDrop`: where the audio files that were let go of
    /// go. `drop` is that event's; an answer to another drop does nothing. `track`: the
    /// vocal track they were let go of over, `None` over a keys track or below the tracks
    /// (a new vocal track). `tick`: where, already on the beat. `mode` for two or more
    /// files (spec 3, 30-C): `Separate` makes a new vocal track for each of them, whatever
    /// `track` is; `InRow` puts them on `track`. One file alone goes on `track`. Otherwise
    /// as `ImportAudio`.
    DropAudio {
        drop: u32,
        track: Option<u32>,
        tick: u64,
        mode: DropMode,
    },
    /// 「さがす…」 on the notice of an audio file that is not found
    /// (`SelectedClip::missing`): Rust shows the PC's window for choosing a file. The chosen
    /// file is copied to where the song looks for its audio, under the missing file's name,
    /// and every clip that plays it sounds again. Not a step: the song does not change.
    /// 「曲から外す」 is `RemoveMissing` with `MissingRef::Audio`.
    FindAudio {
        audio: u32,
    },
    // Stretching and pitch (protocol 25; spec 3, 28-A and Y22; docs/architecture.md 22-19).
    /// A row of the group 「のび縮み ・ 音の高さ」 of the panel 「選んだクリップ」, as one step:
    /// a switch that was pressed, or a number that was typed (`StretchChange`). The clip's
    /// place and its length in ticks stay whatever changes; what changes is which audio of
    /// the file sounds where, and how. When the audio no longer comes out as the file is, it
    /// is made anew (`AudioJobKind::Rendering`), and until that is done the clip sounds
    /// unstretched and unpitched. Nothing happens for a `clip` that is no vocal clip's id, a
    /// value the clip has already, a number that is not one, or while counting in or
    /// recording.
    SetClipStretch {
        clip: u32,
        change: StretchChange,
    },
    /// 「さがす」 beside 「この音のテンポ」: Rust takes the length of the clip's audio as 1, 2,
    /// 4 or 8 bars of the meter at the clip's start; of the tempos that makes, those from 60
    /// to 180; of those, the one nearest to the song's tempo at the clip's start (as a
    /// ratio; of two as near, the slower). It becomes the tempo of the audio, as
    /// `StretchChange::SourceBpm` (one step, a new take;
    /// `StatusMessage::SourceTempoFound`). Following stays as it is: the person turns it on.
    /// When no tempo fits, nothing changes (`StatusMessage::SourceTempoNotFound`). Nothing
    /// happens for a `clip` that is no vocal clip's id, or while counting in or recording.
    FindSourceTempo {
        clip: u32,
    },
    // Writing the song out (protocol 26; spec 3, 34-B; docs/architecture.md 22-20). None of
    // these is a step: the song does not change.
    /// 「書き出し…」 of the menu 「ファイル」 and Mod Shift E: the dialog opens
    /// (`ExportView::dialog`) with what was chosen last in this run, and Rust starts on
    /// 「書き出す前のたしかめ」. Not while counting in or recording, and not while an export
    /// runs.
    OpenExport,
    /// 「閉じる」, 「やめる」 and Esc in the dialog: it closes, and what the check wrote so
    /// far is thrown away.
    CloseExport,
    /// A choice of the dialog.
    SetExport {
        change: ExportChange,
    },
    /// 「えらぶ…」: Rust shows the PC's window for saving a file (for 「トラックごと」 the one
    /// for choosing a folder). `ExportDialogView::place` follows.
    ChooseExportPlace,
    /// 「書き出す」, 「このまま書き出す」 and Enter in the dialog: it closes and the export
    /// runs (`ExportView::run`); what the check wrote already is kept and used. Only with
    /// `ExportDialogView::can_export`.
    StartExport,
    /// 「やめる」 of the status line: the export stops and what it wrote is taken away
    /// (`ExportRunView::Cancelled`).
    CancelExport,
    /// 「フォルダをひらく」: the PC's file manager shows what the last export wrote. Only
    /// with `ExportRunView::Done`.
    ShowExported,
    // Protocol 27 (round 8; spec 3, 39-B, 40-A, Y26, 43-B, 44-A, 45-A; docs/architecture.md
    // 23-7).
    /// The end of a drag of the volume line of one of two or more selected vocal clips
    /// (39-B): every clip of `clips` moves by the same `by_db`, each put into
    /// `CLIP_GAIN_MIN_DB..=CLIP_GAIN_MAX_DB` on its own. One step (`StepName::SetClipGain`,
    /// `StepWhat::Clips`). Ids that are no vocal clip's are passed by; nothing happens when
    /// none is left, for 0, or a value that is not a number.
    ShiftClipGains {
        clips: Vec<u32>,
        by_db: f64,
    },
    /// 「音量」 typed into 「まとめて変える」 (40-A): every clip of `clips` gets `db`, put into
    /// its range. One step, as `ShiftClipGains`.
    SetClipGains {
        clips: Vec<u32>,
        db: f64,
    },
    /// 「曲のテンポについていく」, 「音の高さ」 or 「声の感じを保つ」 of 「まとめて変える」: the
    /// change of `SetClipStretch` for every clip of `clips`, as one step. `SourceBpm` does
    /// nothing here (it is only in the panel of one clip). Clips that have the value
    /// already are left as they are; nothing happens when every one has it.
    SetClipsStretch {
        clips: Vec<u32>,
        change: StretchChange,
    },
    /// 「取りにいく」 (the band's question, the row 「音の素材」 of the advanced settings, the
    /// shelf's notice, `StatusMessage::NewPack`), 「もう一度」 and 「続きから」: the answer is
    /// kept in the settings, and what of the pack is missing or newer is fetched, going on
    /// from what is on the disk (`SoundsView::fetch`). Nothing while fetching or with
    /// nothing to fetch. Not a step.
    FetchPack,
    /// 「あとで」 of the band, asking or after a failure: the band goes (`ask` false, `fetch`
    /// `Idle`) and nothing is fetched; the next start asks again. Not a step.
    PackLater,
    /// 「やめる」 while fetching: the fetch stops (`Idle`); what came is kept and the next
    /// `FetchPack` goes on from there. Not a step.
    StopPackFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SelectMode {
    /// The selection becomes these.
    Replace,
    /// These are added to the selection.
    Add,
    /// Each of these flips between selected and not.
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ScaleMode {
    Major,
    Minor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum QuantizeAlign {
    /// "はじまり".
    Start,
    /// Both the start and the end of each note.
    StartAndEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum NoteField {
    Start,
    Key,
    Length,
    Velocity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ControlKind {
    Cc { number: u8 },
    PitchBend,
}

/// Where pasted clips go: `track` is `TrackView::id`, `tick` a song position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipSpot {
    pub track: u32,
    pub tick: u64,
}

/// An edge of a clip (`TrimClip`) or of notes (`ResizeNotes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ClipEdge {
    Start,
    End,
}

/// The number fields whose range the status line names (spec 6 「使えない値を打った」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ValueField {
    Tempo,
    MeterNum,
    MeterDen,
    Velocity,
    /// The note list and the "選んだノート" panel (round 3): where a note starts, its key, its length.
    Position,
    Key,
    Length,
    /// "1. 1. 000" of the transport (B-61).
    PlayPosition,
    /// One part of a position ("1. 1. 000" of the transport, the position of a note), for
    /// `ClampValue` (B-68): when the bar, the beat or the tick was put in its range, the screen
    /// sends one `ClampValue` naming the part that was (the first of bar, beat, tick when more
    /// than one was), with `value` the whole position as set ("3. 4. 000"). `Bar` has no
    /// `range` ("小節は 1 からなので、1. 1. 000 にしました。"); `Beat` and `Tick` send theirs, as
    /// they depend on the meter ("1〜4", "0〜479"). A note's position out of its clip is
    /// `Position` with the clip's range, as before.
    Bar,
    Beat,
    Tick,
    /// The value of a point in the CC list (design-notes C-6 50), with its `range`
    /// ("0〜127", "−8192〜+8191").
    ControlValue,
}

/// A strip of the mixer. The returns are always these two, and there is one master.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StripRef {
    /// `id` is `TrackView::id`.
    Track {
        id: u32,
    },
    Return {
        bus: SendBus,
    },
    Master,
}

/// Send A and send B, and the returns they go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SendBus {
    A,
    B,
}

/// An insert slot of a strip. `slot` counts from 0, top first ("インサート 1" is 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InsertRef {
    pub strip: StripRef,
    pub slot: u8,
}

/// The insert slots of every strip (mock F4-Mixer).
pub const INSERT_SLOTS: usize = 4;

/// Where a plugin sits: as the instrument of a track (`track` is `TrackView::id`) or in an
/// insert slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PluginSlotRef {
    Instrument { track: u32 },
    Insert { insert: InsertRef },
}

/// The formats of plugins meloet reads; the mark on a row ("VST3", "LV2").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PluginFormat {
    Vst3,
    Lv2,
}

/// Why the scan of a bundle stopped (spec 3, Y13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ScanStopReason {
    /// "読みこむと止まりました"
    Crashed,
    /// "30 秒たっても終わりませんでした"
    TimedOut,
    /// "作りがこわれています"
    Broken,
}

/// The scan for plugins, what it could not read, and where it looks. A section of its own:
/// while a scan runs its count changes with every bundle, and neither the shelf nor the
/// workspace should be sent again for that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PluginsView {
    pub scan: ScanView,
    /// The bundles whose scan stopped, in the order of `places`. They are not in the shelf,
    /// and a scan passes them over until `RetryStoppedPlugin` or `RescanPlugins`.
    pub stopped: Vec<StoppedPluginView>,
    /// The usual places first, then the ones the person added, each in the order it has in
    /// the settings.
    pub places: Vec<PluginPlaceView>,
    /// The dialog 「スキャンで止まったプラグイン」 is up (`OpenScanStopped`).
    pub stopped_open: bool,
    /// 「プラグインの場所」 in the advanced part of the sound settings is unfolded.
    pub places_open: bool,
}

/// The strip under the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScanView {
    /// Nothing to say.
    Idle,
    /// "スキャン中 37 / 112 ・ 調べ終えた分から、もう使えます". Bundles looked at, and bundles
    /// this scan looks at in all.
    Running { done: u32, total: u32 },
    /// "スキャンが終わりました ・ 667 個を棚に入れました。4 個は…": the last scan that showed
    /// its count (the first one, and `RescanPlugins`) is over. `added` plugins are in the
    /// shelf; `stopped` is the length of `PluginsView::stopped` at that time.
    Finished { added: u32, stopped: u32 },
}

/// A row of the dialog 「スキャンで止まったプラグイン」.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StoppedPluginView {
    /// The plugin's name as the scan that stopped read it, or where a scan before this one
    /// read it; else the bundle's file name without its ending.
    pub name: String,
    pub format: PluginFormat,
    pub reason: ScanStopReason,
    /// The bundle; with `id`, what `RetryStoppedPlugin` names.
    pub path: String,
    /// The plugin in the bundle (an LV2 URI, a VST3 class id); `None` is the whole bundle
    /// (it could not be read at all).
    pub id: Option<String>,
    /// `path` as shown, the home folder written as "~".
    pub shown_path: String,
    /// `RetryStoppedPlugin` is at work on it: the button says "ためしています…".
    pub retrying: bool,
}

/// A row of 「プラグインの場所」.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PluginPlaceView {
    /// What `RemovePluginPlace` names.
    pub path: String,
    /// `path` as shown, the home folder written as "~".
    pub shown_path: String,
    /// The person added it ("外す"). False: a usual place of Linux ("ふつうの場所"), which
    /// cannot be removed.
    pub added: bool,
}

/// The effects that come with meloet (spec 3 Y8), in the order of the shelf. Their
/// parameters (number, range, unit, steps) are the table `EFFECT_PARAMS`
/// (`ui/src/bridge/generated/effects.ts`), written from `meloet_model::effects`; the names
/// shown are the screen's (`ja.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum EffectKind {
    Eq4,
    Compressor,
    Reverb,
    Delay,
    Chorus,
    Limiter,
}

impl EffectKind {
    pub const ALL: [EffectKind; 6] = [
        EffectKind::Eq4,
        EffectKind::Compressor,
        EffectKind::Reverb,
        EffectKind::Delay,
        EffectKind::Chorus,
        EffectKind::Limiter,
    ];
}

/// What each insert slot of a strip holds, top first; `None` is "空き ＋".
pub type SlotsView = [Option<InsertView>; INSERT_SLOTS];

/// The buttons of the song dialogs, one for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SongAnswer {
    /// "保存する" (`Unsaved`).
    Save,
    /// "保存しない" (`Unsaved`).
    DontSave,
    /// "場所をえらぶ" (`CannotSave`): Rust shows the PC's folder window.
    ChooseLocation,
    /// "1 つ前を開く" (`Broken` with `previous`).
    OpenPrevious,
    /// "やめる" (every dialog).
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Band {
    PianoRoll,
    Mixer,
    History,
    Shelf,
    Params,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub rev: u64,
    pub state: AppState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub song: SongView,
    pub timeline: TimelineView,
    pub automation: AutomationView,
    pub history: HistoryView,
    pub transport: TransportView,
    pub arrange: ArrangeView,
    pub piano_roll: PianoRollView,
    pub workspace: WorkspaceView,
    pub shelf: ShelfView,
    pub library: LibraryView,
    pub params: ParamsView,
    pub plugins: PluginsView,
    pub windows: WindowsView,
    pub keyboards: KeyboardsView,
    pub outputs: OutputsView,
    pub inputs: InputsView,
    pub audio: AudioView,
    pub audio_work: AudioWorkView,
    pub export: ExportView,
    pub sounds: SoundsView,
    pub status: StatusView,
}

impl AppState {
    /// The sections that differ between `old` and `new`, each one whole.
    pub fn changes_from(old: &AppState, new: &AppState) -> Vec<Change> {
        let mut out = Vec::new();
        if old.song != new.song {
            out.push(Change::Song(Box::new(new.song.clone())));
        }
        if old.timeline != new.timeline {
            out.push(Change::Timeline(new.timeline.clone()));
        }
        if old.automation != new.automation {
            out.push(Change::Automation(new.automation.clone()));
        }
        if old.history != new.history {
            out.push(Change::History(new.history));
        }
        if old.transport != new.transport {
            out.push(Change::Transport(new.transport.clone()));
        }
        if old.arrange != new.arrange {
            out.push(Change::Arrange(new.arrange.clone()));
        }
        if old.piano_roll != new.piano_roll {
            out.push(Change::PianoRoll(new.piano_roll.clone()));
        }
        if old.workspace != new.workspace {
            out.push(Change::Workspace(new.workspace.clone()));
        }
        if old.shelf != new.shelf {
            out.push(Change::Shelf(new.shelf.clone()));
        }
        if old.library != new.library {
            out.push(Change::Library(new.library.clone()));
        }
        if old.params != new.params {
            out.push(Change::Params(new.params.clone()));
        }
        if old.plugins != new.plugins {
            out.push(Change::Plugins(new.plugins.clone()));
        }
        if old.windows != new.windows {
            out.push(Change::Windows(new.windows.clone()));
        }
        if old.keyboards != new.keyboards {
            out.push(Change::Keyboards(new.keyboards.clone()));
        }
        if old.outputs != new.outputs {
            out.push(Change::Outputs(new.outputs.clone()));
        }
        if old.inputs != new.inputs {
            out.push(Change::Inputs(new.inputs.clone()));
        }
        if old.audio != new.audio {
            out.push(Change::Audio(new.audio.clone()));
        }
        if old.audio_work != new.audio_work {
            out.push(Change::AudioWork(new.audio_work.clone()));
        }
        if old.export != new.export {
            out.push(Change::Export(new.export.clone()));
        }
        if old.sounds != new.sounds {
            out.push(Change::Sounds(new.sounds.clone()));
        }
        if old.status != new.status {
            out.push(Change::Status(new.status.clone()));
        }
        out
    }

    /// Replaces the section a change names.
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Song(v) => self.song = *v,
            Change::Timeline(v) => self.timeline = v,
            Change::Automation(v) => self.automation = v,
            Change::History(v) => self.history = v,
            Change::Transport(v) => self.transport = v,
            Change::Arrange(v) => self.arrange = v,
            Change::PianoRoll(v) => self.piano_roll = v,
            Change::Workspace(v) => self.workspace = v,
            Change::Shelf(v) => self.shelf = v,
            Change::Library(v) => self.library = v,
            Change::Params(v) => self.params = v,
            Change::Plugins(v) => self.plugins = v,
            Change::Windows(v) => self.windows = v,
            Change::Keyboards(v) => self.keyboards = v,
            Change::Outputs(v) => self.outputs = v,
            Change::Inputs(v) => self.inputs = v,
            Change::Audio(v) => self.audio = v,
            Change::AudioWork(v) => self.audio_work = v,
            Change::Export(v) => self.export = v,
            Change::Sounds(v) => self.sounds = v,
            Change::Status(v) => self.status = v,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SongView {
    /// Goes up each time core puts another song in place (new, open, recovery, "戻さない"),
    /// not on undo or redo. Take ids are per song: keep takes by (serial, id).
    pub serial: u32,
    pub name: String,
    pub undo_count: u32,
    pub redo_count: u32,
    /// For the piano roll band label: the notes of the shown take of `ArrangeView::piano_roll_clip`.
    pub note_count: u32,
    pub tracks: Vec<TrackView>,
    /// Return A, then return B.
    pub returns: [ReturnView; 2],
    pub master: MasterView,
    /// The selected track, by `TrackView::id` (B-70: its header is gray.4). `SelectTrack`,
    /// and a click on a clip, change it. `None` only when the song has no tracks.
    pub play_target: Option<u32>,
    /// The keys track the keys play and R records the keys to, so the keys track whose ● is
    /// lit, and 「弾くと鳴る先」: `play_target` when that is a keys track, else the keys track
    /// that was selected last (B-94-1). `None` when that leaves none: the keys are silent and
    /// 「弾くと鳴る先」 is 「—」.
    pub keys_target: Option<u32>,
    /// Kept while the loop is off.
    pub loop_range: Option<TickSpan>,
    pub loop_on: bool,
    pub file: SongFile,
}

/// The song against its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SongFile {
    /// Never saved, and nothing to save: a new song as it was made.
    NeverSaved,
    /// The file holds the song as it is.
    Saved,
    /// There are unsaved changes (also a new song with steps, and a song opened from the copy
    /// kept before the last save).
    Changed,
}

/// `[start, end)` in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TickSpan {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    /// Stays with the track for as long as the song is open, and is never given to another
    /// track. Commands name tracks by it.
    pub id: u32,
    /// What is shown: the track's place in `SongView::tracks`, from 1. Not part of the name.
    pub number: u32,
    pub name: String,
    /// 1..=5. Belongs to the track, not to its place: moving a track keeps it.
    pub color: u8,
    pub kind: TrackKindView,
    /// What the track sounds with, as the song has it. Of a vocal track the sine wave, which
    /// nothing shows or plays.
    pub sound: SoundView,
    /// The track sounds as the sine wave for now: its header, the mixer and the input meter
    /// say 「サイン波で代わり」 in place of the sound's name (spec 3, 85). The song keeps
    /// `sound`.
    pub stand_in: Option<StandIn>,
    /// A drum kit of an SF2 file (`GmFamily::DrumKit`): the keys the kit has a sound on, as
    /// the rows of the drum piano roll, top row first (spec 3 「SF2 のドラムのキット」). The
    /// screen names a row by its key. `None` for every other sound, and while the kit is not
    /// read.
    pub drum_keys: Option<Vec<u8>>,
    /// The fader: `None` is −∞, otherwise −60.0..=6.0.
    pub gain_db: Option<f32>,
    /// -100 (all left) to 100 (all right), 0 the middle.
    pub pan: i8,
    /// How much goes to return A and to return B, as `gain_db`.
    pub sends: [Option<f32>; 2],
    /// What Home puts the sends at (spec 3, 86-7): the amounts a new track with this sound
    /// starts with.
    pub send_defaults: [Option<f32>; 2],
    pub slots: SlotsView,
    /// Saved with the song (`ToggleMute`).
    pub muted: bool,
    /// Not saved (`ToggleSolo`).
    pub soloed: bool,
    /// The automation lanes, in the order they were added. The points are not in here
    /// (`LaneView::lane_id`). The fader, the pan, the sends, the mute and the values of the
    /// inserts above are where the hand put them: what sounds while a lane with points
    /// does not rest is the lane's value at the play line, which the screen works out from
    /// the points (docs/architecture.md 21-8).
    pub lanes: Vec<LaneView>,
    /// In order of start, never overlapping.
    pub clips: Vec<ClipView>,
}

/// A return strip. It has no sends, no instrument and no clips; its S and ● are unusable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ReturnView {
    pub gain_db: Option<f32>,
    pub pan: i8,
    /// Saved with the song (`ToggleReturnMute`).
    pub muted: bool,
    pub slots: SlotsView,
}

/// The master strip: a fader, a pan and inserts. Its M, S and ● are unusable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MasterView {
    pub gain_db: Option<f32>,
    /// −100 to 100, as a track's (mock F4-Mixer).
    pub pan: i8,
    pub slots: SlotsView,
}

/// The parameters panel: the selected insert or instrument, and for an effect that comes
/// with meloet its values. A section of its own because selecting an insert or a row is no
/// step: `song` is not sent again for it, as with the selection of the piano roll. The
/// mixer's own values (fader, pan, sends, which effect sits where, what is passed by) are in
/// `song`: they change only by steps, and every step sends `song` anyway (`undo_count`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamsView {
    /// `None`: nothing is selected (also after the effect is taken out, its track deleted,
    /// or an undo took it away): the panel says "ミキサーでインサートの枠をえらぶと、ここに
    /// パラメータが出ます。" (spec 3, 86-5), which needs nothing from Rust. The selection stays
    /// with its effect when inserts are moved (`Command::MoveInsert`).
    pub selected: Option<SelectedParams>,
    /// 「まとめて変える」 (spec 3, 40-A): two or more clips are selected and every one is a
    /// vocal clip. `selected` is `None` then. An insert or an instrument selected afterwards
    /// takes its place in the panel as `selected` does; selecting clips again brings it back.
    pub clips: Option<SelectedClips>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SelectedInsert {
    pub insert: InsertRef,
    pub effect: EffectKind,
    /// One for each parameter of the effect's table (`EFFECT_PARAMS`), in its order, as the
    /// song has them. A preview (`commit: false`) is not in here.
    pub values: Vec<f32>,
    /// The chosen row (`Command::SelectParam`).
    pub row: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipView {
    pub id: u32,
    pub name: String,
    pub start: u64,
    pub end: u64,
    /// The take shown and played, counting from 1 ("2/3").
    pub take: u32,
    pub takes: u32,
    /// Fetch its notes with `take_notes`; they never change, so keep them by this id.
    pub take_id: u32,
    /// The take's tick 0 is at `start + take_offset` in the song; below 0 when the start of
    /// the clip has been trimmed.
    pub take_offset: i64,
    pub note_count: u32,
    /// A clip of a vocal track: `take_id` names a take of audio, which `take_notes` does not
    /// have, and `note_count` is 0. `None` for a clip of notes.
    pub audio: Option<AudioClipView>,
}

/// The sounds that come with meloet (spec 3 「最初から入っている音色をふやす」（84）), in the
/// order of the shelf. `Sine` is what used to be the placeholder sound; `Organ` is the pipe
/// organ, which songs from before 84 call 「オルガン」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum InstrumentKind {
    GrandPiano,
    ElectricPiano,
    OrganDrawbar,
    OrganPercussive,
    OrganRock,
    Organ,
    OrganQuiet,
    OrganChurch,
    OrganReed,
    Vibraphone,
    Marimba,
    GuitarNylon,
    GuitarSteel,
    GuitarElectric,
    GuitarDistortion,
    FingerBass,
    PickBass,
    WoodBass,
    SynthBass,
    StudioKit,
    ElectroKit,
    BrushKit,
    Strings,
    Pizzicato,
    WarmPad,
    Trumpet,
    BrassSection,
    AltoSax,
    Flute,
    SynthLead,
    Sine,
}

/// The headings of the shelf, in its order. `Drums`: the piano roll shows the kit's
/// instruments and the scale lock cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum InstrumentGroup {
    PianoKeys,
    Guitar,
    Bass,
    Drums,
    PadStrings,
    Wind,
    Synth,
}

/// The sounds of a drum kit (the studio kit, the electro kit and the brush kit), top row of the drum piano roll
/// first, with the General MIDI key each sits on (spec 3 「ドラムのピアノロール」). The screen has
/// the same table as `DRUM_ROWS` (`ui/src/bridge/generated/drums.ts`); the names shown are the
/// screen's (`ja.ts`). A note on any other key stays in the song but does not sound, and the
/// drum piano roll does not show it (it is in the note list, by its note name).
pub const DRUM_ROWS: [(&str, u8); 12] = [
    ("crash", 49),
    ("ride", 51),
    ("openHiHat", 46),
    ("pedalHiHat", 44),
    ("closedHiHat", 42),
    ("highTom", 50),
    ("midTom", 47),
    ("lowTom", 45),
    ("clap", 39),
    ("snare", 38),
    ("sideStick", 37),
    ("kick", 36),
];

impl InstrumentKind {
    /// In the order of the shelf. The screen has the same list, with the groups, as
    /// `INSTRUMENT_GROUPS` (`ui/src/bridge/generated/instruments.ts`).
    pub const ALL: [InstrumentKind; 31] = [
        InstrumentKind::GrandPiano,
        InstrumentKind::ElectricPiano,
        InstrumentKind::OrganDrawbar,
        InstrumentKind::OrganPercussive,
        InstrumentKind::OrganRock,
        InstrumentKind::Organ,
        InstrumentKind::OrganQuiet,
        InstrumentKind::OrganChurch,
        InstrumentKind::OrganReed,
        InstrumentKind::Vibraphone,
        InstrumentKind::Marimba,
        InstrumentKind::GuitarNylon,
        InstrumentKind::GuitarSteel,
        InstrumentKind::GuitarElectric,
        InstrumentKind::GuitarDistortion,
        InstrumentKind::FingerBass,
        InstrumentKind::PickBass,
        InstrumentKind::WoodBass,
        InstrumentKind::SynthBass,
        InstrumentKind::StudioKit,
        InstrumentKind::ElectroKit,
        InstrumentKind::BrushKit,
        InstrumentKind::Strings,
        InstrumentKind::Pizzicato,
        InstrumentKind::WarmPad,
        InstrumentKind::Trumpet,
        InstrumentKind::BrassSection,
        InstrumentKind::AltoSax,
        InstrumentKind::Flute,
        InstrumentKind::SynthLead,
        InstrumentKind::Sine,
    ];

    pub fn group(self) -> InstrumentGroup {
        use InstrumentKind as I;
        match self {
            I::GrandPiano
            | I::ElectricPiano
            | I::OrganDrawbar
            | I::OrganPercussive
            | I::OrganRock
            | I::Organ
            | I::OrganQuiet
            | I::OrganChurch
            | I::OrganReed
            | I::Vibraphone
            | I::Marimba => InstrumentGroup::PianoKeys,
            I::GuitarNylon | I::GuitarSteel | I::GuitarElectric | I::GuitarDistortion => {
                InstrumentGroup::Guitar
            }
            I::FingerBass | I::PickBass | I::WoodBass | I::SynthBass => InstrumentGroup::Bass,
            I::StudioKit | I::ElectroKit | I::BrushKit => InstrumentGroup::Drums,
            I::Strings | I::Pizzicato | I::WarmPad => InstrumentGroup::PadStrings,
            I::Trumpet | I::BrassSection | I::AltoSax | I::Flute => InstrumentGroup::Wind,
            I::SynthLead | I::Sine => InstrumentGroup::Synth,
        }
    }
}

impl InstrumentGroup {
    pub const ALL: [InstrumentGroup; 7] = [
        InstrumentGroup::PianoKeys,
        InstrumentGroup::Guitar,
        InstrumentGroup::Bass,
        InstrumentGroup::Drums,
        InstrumentGroup::PadStrings,
        InstrumentGroup::Wind,
        InstrumentGroup::Synth,
    ];
}

/// The shelf of instruments. A section of its own: `workspace` is sent again at every step
/// of a window resize, and the song at every edit; the shelf changes only with what the
/// user does in it.
///
/// Whether the shelf is open is `LayoutView::right_open`. When it is closed, `mode` is
/// `NewTrack` and `selected_sound`, `current_sound`, `loading_sound`, `sound_sample`,
/// `chosen_effect` and `current_effect` are `None`.
///
/// The shelf shows its instruments in `NewTrack` and `Replace` mode and its effects
/// (`EffectKind::ALL`, all of them always there, and the plugins of `LibraryView`) in
/// `InsertEffect` and `ReplaceEffect` mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ShelfView {
    pub mode: ShelfMode,
    /// The rows of the shelf, in its order: the instruments whose sounds are installed. One
    /// that is not here is not shown at all, not even as unusable.
    pub available: Vec<InstrumentKind>,
    /// The row that is auditioned: the keys sound with it (`Command::ShelfSelectSound`).
    pub selected_sound: Option<SoundRef>,
    /// `Replace` mode: the sound the track has now ("いまの音源").
    pub current_sound: Option<SoundRef>,
    /// The selected sound is still being read: the keys do not sound with it yet
    /// ("（名前）を読みこんでいます…", for a plugin with "やめる", `ShelfCancel`). `None` again
    /// once it sounds.
    pub loading_sound: Option<SoundRef>,
    /// The sample that is playing, or is about to once its sound is read
    /// (`Command::PlaySampleOf`). `None` again when it ends.
    pub sound_sample: Option<SoundSampleView>,
    /// `InsertEffect` and `ReplaceEffect` mode: the chosen row
    /// (`Command::ShelfChooseEffect`).
    pub chosen_effect: Option<EffectRef>,
    /// `ReplaceEffect` mode: the effect the slot holds now ("いまのエフェクト"). With
    /// `chosen_effect` the screen writes under the chosen row "（トラック）のインサート（番号）
    /// の（いま）を、（新しい物）に入れかえます。…" (spec 6) and 「入れかえる（Enter）」.
    pub current_effect: Option<EffectRef>,
    /// The plugin selected last did not load: under its row "Surge XT を読みこめません
    /// でした。" (spec 3 Y14). Nothing is selected then. Until another row is selected or the
    /// shelf is closed.
    pub unreadable: Option<SoundRef>,
    /// Every starred sound, in the order of the shelf. A plugin is named by the key its row
    /// uses now. May hold sounds the shelf does not have; the screen shows only the ones it
    /// has.
    pub favorite_sounds: Vec<SoundRef>,
    /// The starred effects (spec 3, 86-4).
    pub favorite_effects: Vec<EffectRef>,
    /// What the song uses and this PC does not have, SF2 files first, each once. Also while
    /// the shelf is closed.
    pub missing: Vec<MissingView>,
    /// Instruments that come with meloet whose sound is not fetched yet (spec 3, 45-A), in
    /// the order of the shelf: a row with the name in gray.8 and 「まだ」, or
    /// 「取ってきています」 for `PackFetchView::Fetching::instrument`; no star, no ▶, no
    /// audition. They are not in `available`; the shelf shows both in its order. Empty when
    /// the pack is all there.
    pub not_yet: Vec<InstrumentKind>,
}

/// What Enter does in the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ShelfMode {
    /// "新しいトラックの音源をえらぶ".
    NewTrack,
    /// "（トラック）の音源を入れかえる". `track` is `TrackView::id`.
    Replace { track: u32 },
    /// From "空き ＋": Enter puts the chosen effect into this slot. When the slot is no longer
    /// empty or its strip is gone (undo, a deleted track), the mode is `NewTrack` again.
    InsertEffect { strip: StripRef, slot: u8 },
    /// "（トラック）のインサート（番号）を入れかえる" (spec 3, 86-3), from 「入れかえる…」: Enter
    /// puts the chosen effect in place of the one this slot holds. When the slot is empty by
    /// now or its strip is gone, the mode is `NewTrack` again; when its effect is moved
    /// (`Command::MoveInsert`), `slot` follows it.
    ReplaceEffect { strip: StripRef, slot: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum TransportState {
    #[default]
    Stopped,
    CountIn,
    Playing,
    Recording,
}

/// Where it moves between patches comes from the telemetry (position, count-in beats left, lap).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TransportView {
    pub state: TransportState,
    /// Where play and record start from; while moving, where it started.
    pub position: u64,
    pub metronome: bool,
    /// 0, 1 or 2.
    pub count_in_bars: u8,
    /// Clicks of the metronome in the count-in (6/8 has 2 to a bar): of the one running, or
    /// else of the next one.
    pub count_in_beats: u32,
    /// False while the output is not playing.
    pub can_play: bool,
    /// Also false with nothing to record to: no keys target and no vocal track with its ● on
    /// (the record button still takes a click then, to say why), while the shelf auditions,
    /// and while the latency is measured.
    pub can_record: bool,
    /// The keys of the recording going on, from the record command until the take is in the
    /// song (also during the count-in). `None` also while only vocal tracks are recorded.
    pub recording: Option<RecordingView>,
    /// The vocal tracks of the recording going on, in the order of the tracks, for as long
    /// as `recording` would be there. Each draws a clip in `color.rec` from `from` to the
    /// play line (mocks F7-VocalRec, F7-TwoRec).
    pub recording_vocals: Vec<VocalRecordingView>,
    /// `None`: every block of the vocal recording going on, or of the last one, is what
    /// came in.
    pub recording_faults: Option<RecordingFaults>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecordingView {
    /// Counts up with each recording; `LiveNotes::recording` names it.
    pub id: u32,
    /// `TrackView::id`.
    pub track: u32,
    /// Where recording starts in the song.
    pub from: u64,
    /// The number the take of the current lap gets in its clip, counting from 1. A loop lap
    /// with no notes makes no take, so it goes up only after a lap with notes.
    pub take: u32,
    /// The clip the takes go into, when the recording overlaps one already.
    pub clip: Option<u32>,
    /// Pass of the loop, 0 for the first; 0 without a loop.
    pub lap: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArrangeView {
    /// Every selected clip, in the order of the tracks, then of start.
    pub selected_clips: Vec<u32>,
    /// The clip clicked last (`SelectClip`, Shift + click), while it is still selected: the one
    /// the take buttons (`StepTake`) and the take label act on. `None` after a box or Mod A
    /// unless that leaves exactly one clip selected, which is then this one.
    pub selected_clip: Option<u32>,
    /// The clip the piano roll shows: the one clicked last, kept after it is unselected.
    pub piano_roll_clip: Option<u32>,
    /// How many clips `PasteClips` would put in; 0 when there is nothing to paste.
    pub clipboard_clips: u32,
    /// The vocal track whose row is 160 high (`Command::SetTallTrack`; spec 3 「歌のトラックで
    /// ピアノロールをひらこうとしたとき」), by `TrackView::id`. `None` again when the track is
    /// deleted or another song comes. Not saved, and no step.
    pub tall_track: Option<u32>,
}

/// The piano roll's own state, kept apart from `song` so that a new selection does not send
/// the song again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PianoRollView {
    /// The shown take of `ArrangeView::piano_roll_clip`. The selections are indexes into it.
    pub take: Option<u32>,
    /// Indexes of the take's notes, in order, ranges that touch joined into one.
    pub selected_notes: Vec<IndexRange>,
    /// Indexes of the take's controls, the same way.
    pub selected_controls: Vec<IndexRange>,
    pub grid: Grid,
    pub scale: MusicalScale,
    /// What the quantize panel shows: the settings the next `Quantize` uses.
    pub quantize: QuantizeSettings,
    /// How many notes `PasteNotes` would put in; 0 when there is nothing to paste.
    pub clipboard_notes: u32,
}

/// `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct IndexRange {
    pub start: u32,
    pub end: u32,
}

/// `division`: how many to a whole note (16 is "1/16"); 0 is one bar ("1 小節", as long as the
/// meter makes it). `triplet` makes each step 2/3 as long.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub division: u16,
    pub triplet: bool,
}

/// `root` 0..=11 is C..B. `lock`: "スケール固定 ・ オン", only keys of the scale can be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MusicalScale {
    pub root: u8,
    pub mode: ScaleMode,
    pub lock: bool,
}

/// `strength` and `swing` are percent, 0..=100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct QuantizeSettings {
    pub division: u16,
    pub triplet: bool,
    pub strength: u8,
    pub swing: u8,
    pub align: QuantizeAlign,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub scale: u16,
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// Zooms whose smallest window does not fit the screen the window is on. Empty until the
    /// screen is known; never holds 100.
    pub unfit_scales: Vec<u16>,
    pub layout: LayoutView,
    pub settings: Option<SettingsDialogView>,
    pub chooser: Option<ChooserView>,
    pub hint: Option<Hint>,
    pub song_dialog: Option<SongDialogView>,
    /// The hint about the recovery file. While it is up, `hint` is `None`. It stays until the
    /// song is saved, "戻さない" or "閉じる" (`DismissHint`), or the song is let go.
    pub recovery: Option<RecoveryView>,
    /// Files are held over the window. `None` again when they are let go, taken out of the
    /// window, or Esc (`Command::CancelFileDrop`).
    pub file_drop: Option<FileDropView>,
    /// The dialog 「遅れを測る」, over the sound settings.
    pub latency_dialog: Option<LatencyDialogView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RecoveryView {
    /// The song was brought back: "保存する / 戻さない".
    Recovered {
        /// When the recovery file was written, Unix ms. The screen shows it in local time.
        written_ms: u64,
        cause: RecoveryCause,
        /// The song's name; `None` for a song never saved.
        song: Option<String>,
        /// The takes of every clip, all together.
        takes: u32,
        /// Vocals that were being recorded when meloet ended, kept as far as they were
        /// written: one line more in the hint. When only they were kept (the song had no
        /// other unsaved change), the rest is as for a recovery file written then.
        vocals: Option<KeptVocals>,
    },
    /// The recovery file did not read and was kept: "ファイルの場所をひらく / 閉じる".
    Unreadable {
        /// The last saved song, opened instead; `None`: a new song.
        song: Option<String>,
    },
}

/// When the recovery file was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum RecoveryCause {
    /// The PC was ending (a signal, or logind's PrepareForShutdown).
    PcEnding,
    /// Every now and then, a little after the song changed: meloet ended without warning.
    Sudden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SongDialogView {
    /// New for every dialog put up; `Command::AnswerSongDialog` names it.
    pub id: u32,
    pub kind: SongDialogKind,
    /// The heading: the song's name, or for `Broken` the name of the file that could not be
    /// read. `Unsaved` puts the same name in its sentence ("{song}").
    pub title: String,
    /// `Broken` only: the copy kept before the last save can be read. Without it the dialog
    /// has only the first sentence and "やめる".
    pub previous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SongDialogKind {
    /// "保存する / 保存しない / やめる". Before closing, a new song, or opening another.
    Unsaved,
    /// "場所をえらぶ / やめる".
    CannotSave,
    /// "1 つ前を開く / やめる", or "やめる" alone.
    Broken,
}

/// Heights are px at 100 %; the screen multiplies by the scale. `lower` is top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutView {
    pub meter: MeterSize,
    pub arrange_height: f32,
    pub arrange_overview: bool,
    pub lower: Vec<LowerBandView>,
    pub right_open: Vec<Band>,
    pub right_front: Option<Band>,
    pub show_back_hint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum MeterSize {
    Full,
    Slim,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LowerBandView {
    pub band: Band,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SettingsDialogView {
    pub draft_output: Option<String>,
    pub advanced_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ChooserView {
    pub highlighted: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Hint {
    SoundCameOut,
    EmptyArrange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyboardsView {
    pub list: Vec<KeyboardView>,
    /// The keyboard that was played last; before any key, the one plugged in last.
    pub active: Option<String>,
    pub lost: Option<String>,
    /// The keyboard input itself stopped (the PC's MIDI system, not one keyboard), and meloet
    /// is starting it again. The list is empty meanwhile.
    pub input_stopped: bool,
    /// "ベロシティ固定" (B-66), next to the velocity reading of the input meter.
    pub velocity_fixed: VelocityFixed,
}

/// `value` 1..=127, kept while off; 100 until the user changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VelocityFixed {
    pub on: bool,
    pub value: u8,
}

/// The fixed velocity before the user sets one.
pub const DEFAULT_FIXED_VELOCITY: u8 = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyboardView {
    pub id: String,
    pub name: String,
    pub port_name: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutputsView {
    pub list: Vec<EndpointView>,
    pub chosen: Option<String>,
    pub chosen_state: Option<ChosenState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InputsView {
    pub list: Vec<EndpointView>,
    /// The input the vocal tracks record from, by `EndpointView::id`: the one of
    /// `Command::SettingsSelectInput`, else the one the PC uses. `None`: the PC has no input
    /// meloet can use; every vocal track says 「入口なし ▾」.
    pub chosen: Option<String>,
    /// The input is open: a vocal track is selected or has its ● on, so the mic meter of the
    /// telemetry runs (and the PC shows that the microphone is in use).
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EndpointView {
    pub id: String,
    pub name: String,
    pub friendly: Option<Friendly>,
    pub availability: Availability,
    pub link: LinkKind,
    pub node_name: Option<String>,
    /// The PC's name of the device this endpoint is on, as it is.
    pub device: Option<String>,
    /// `None`: meloet can play through it without changing any PC-wide setting. Others are
    /// listed but cannot be chosen.
    pub unusable: Option<Unusable>,
    /// Of an input: how many channels it has ("入口 1 ・ 2（USB マイク）" has 2). `None` for
    /// an output, and for an input whose channels are not known.
    pub channels: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Unusable {
    /// A hole on a device that plays, but the PC has the device on another hole now.
    SwitchedByPc,
    /// Nothing plays through it: a PC setting has it off.
    StoppedByPc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Friendly {
    Headphones,
    Hdmi,
    LineOut,
    RearMic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    Jack,
    Plug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ChosenState {
    Ready,
    NotPlugged,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioView {
    pub running: bool,
    pub system: String,
    pub sample_rate: Option<u32>,
    pub requested_frames: u32,
    pub actual_frames: Option<u32>,
    pub forced_quantum: Option<u32>,
    pub latency_ms: Option<f32>,
    pub latency_ms_at_requested: Option<f32>,
    pub output_device: Option<String>,
    pub midi_input: Option<String>,
    // The advanced sound settings (spec 3, 33-A; mock F7-SettingsAdv). 「頼む数」 are
    // `requested_frames` and `requested_sample_rate`, 「動いている」 `actual_frames` and
    // `sample_rate`; where they differ the dialog says "頼んだ 128 ではなく、256 で動いて
    // います…".
    /// What 「バッファ ▾」 lists.
    pub buffer_choices: Vec<u32>,
    pub requested_sample_rate: u32,
    /// What 「サンプルレート ▾」 lists.
    pub sample_rate_choices: Vec<u32>,
    pub record_format: RecordFormat,
    pub latency: LatencyView,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StatusView {
    pub message: Option<StatusMessage>,
}

/// Which sentence, and the values to put in it. The wording lives in the screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "id", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StatusMessage {
    SearchingKeyboard,
    KeyboardFound {
        keyboard: String,
    },
    KeyboardLost {
        keyboard: String,
    },
    /// "出口を{output}にしました。{track} が録音待機になっています。" `track` is the selected one.
    OutputChosen {
        output: String,
        track: Option<TrackRef>,
    },
    OutputNotPlugged {
        output: String,
    },
    OutputMissing {
        output: String,
    },
    BufferFixed {
        actual: u32,
        requested: u32,
        latency_ms: f32,
        latency_ms_at_requested: f32,
    },
    SettingsOpen,
    /// "新しい曲 ・ テンポ {tempo} ・ {signature} ・ {track} が録音待機"
    NewSong {
        tempo_bpm: f64,
        meter_num: u8,
        meter_den: u8,
        track: Option<TrackRef>,
    },
    BandFoldedWindowLow {
        band: Band,
    },
    BandFoldedToOpen {
        opened: Band,
        folded: Band,
    },
    /// A third bottom band was opened; the one opened first was folded (two at most).
    BandFoldedThird {
        opened: Band,
        folded: Band,
    },
    /// The zoom went up to `scale` and the window, in px at 100 %, got too low for `band`.
    BandFoldedScale {
        scale: u16,
        band: Band,
    },
    ScaleDoesNotFit {
        requested: u16,
        kept: u16,
    },
    ScaleWindowGrown {
        scale: u16,
    },
    ScaleLoweredForScreen {
        from: u16,
        to: u16,
    },
    /// PipeWire cannot be reached. meloet connects again by itself when it is back.
    AudioSystemStopped,
    /// A recording was stopped because PipeWire stopped; what was played is in `take`.
    RecordingStoppedAudioSystem {
        take: u32,
    },
    /// A recording was stopped because the chosen output went away.
    RecordingStoppedOutputMissing {
        output: String,
        take: u32,
    },
    /// The keyboard input stopped; meloet is starting it again.
    KeyboardInputStopped,
    /// `beats` is the whole count-in, in clicks of the metronome; how many are left comes
    /// from the telemetry (`countInBeatsLeft`), in step with the clicks heard.
    CountingIn {
        beats: u32,
    },
    Recording {
        track: TrackRef,
        take: u32,
    },
    /// `lap` counts from 1 ("2 周目").
    LoopRecording {
        track: TrackRef,
        from_bar: u32,
        to_bar: u32,
        lap: u32,
    },
    TakeSwitched {
        track: TrackRef,
        clip: String,
        take: u32,
        takes: u32,
    },
    /// The record button was pressed in a song with no tracks.
    NoArmedTrack,
    /// `SelectTrack` with `from_arm` selected another track: "R を押すと 2 ベース に録ります"
    /// (mock R7-Arrange6).
    ArmMoved {
        track: TrackRef,
    },
    /// `ShelfConfirm` made a track: "5 シンセ を足しました。"
    TrackAdded {
        track: TrackRef,
    },
    /// `DeleteTrack`: "3 ドラム を消しました。取り消せます（Mod Z）". The number it had.
    TrackDeleted {
        track: TrackRef,
    },
    /// `ShelfConfirm` in `Replace` mode: "1 ピアノ の音源を エレクトリックピアノ にしました。
    /// 取り消せます（Mod Z）".
    /// `lanes_removed`: the automation lanes of the old instrument's parameters, which went
    /// with it; above 0 the sentence ends "（そのパラメータの帯 2 本も消えました）" (spec 6
    /// 「帯もいっしょに消えた」, 92-19). The same in `InstrumentReplacedSound`, `EffectRemoved`
    /// and `InsertReplaced`.
    InstrumentReplaced {
        track: TrackRef,
        instrument: InstrumentKind,
        lanes_removed: u32,
    },
    /// While a row is auditioned in `NewTrack` mode, in place of the summary: "シンセベース ・
    /// まだトラックには入っていません ・ 入れるには Enter" (mock F4-ShelfAudition).
    AuditionNewTrack {
        instrument: InstrumentKind,
    },
    /// The same in `Replace` mode: "エレクトリックピアノ ・ 1 ピアノ の音源は、まだグランドピアノ
    /// です ・ 入れかえるには Enter". `current`: the instrument the track still has.
    AuditionReplace {
        instrument: InstrumentKind,
        track: TrackRef,
        current: InstrumentKind,
    },
    /// While a sample plays and no row is auditioned: "サンプルを再生中 ・ グランドピアノ。行を
    /// えらぶと、鍵盤でためし弾きできます。"
    SamplePlaying {
        instrument: InstrumentKind,
    },
    /// A key was played on a track whose instrument is still being read, and so was silent:
    /// "グランドピアノを読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。" Until it is read.
    InstrumentLoading {
        instrument: InstrumentKind,
    },
    UndoPending {
        count: u32,
    },
    /// A number field refused what was typed: "{field}は {range} です。もとの {kept} のままです。"
    ValueRejected {
        field: ValueField,
        kept: String,
        range: Option<String>,
    },
    /// A number out of range was put at the nearest end (B-68, `Command::ClampValue`):
    /// "ベロシティは 1〜127 なので、127 にしました。" `value` as shown.
    ValueClamped {
        field: ValueField,
        value: String,
        range: Option<String>,
    },
    /// `Command::MatchVelocity` (B-67): "3 ノートのベロシティを 100 にそろえました。"
    VelocityMatched {
        count: u32,
        value: u8,
    },
    /// `Command::MatchLength`: "3 ノートの長さを 0.240 にそろえました。" `value` in ticks; the
    /// screen writes it as the note list does.
    LengthMatched {
        count: u32,
        value: u64,
    },
    /// `MoveClips` without `copy` did nothing, as a clip would overlap one outside the group
    /// (B-71): "ほかのクリップと重なるので、動かしませんでした。"
    ClipsNotMoved,
    /// `PasteClips` did nothing for the same reason: "ほかのクリップと重なるので、貼りつけませんでした。"
    ClipsNotPasted,
    /// `DuplicateClips`, or `MoveClips` with `copy` (Alt + drag), did nothing for the same
    /// reason: "ほかのクリップと重なるので、複製できませんでした。"
    ClipsNotDuplicated,
    /// The write tool outside the clip, where the clip cannot grow as it would overlap the clip
    /// next to it (B-82): "ほかのクリップと重なるので、書けませんでした。"
    NotesNotWritten,
    /// A note's edge dragged outside the clip, where the clip cannot grow for the same reason:
    /// "ほかのクリップと重なるので、長さを変えられませんでした。"
    NotesNotResized,
    /// A row of the parameters panel is chosen (`SelectedInsert::row`), in place of the
    /// summary and also while playing: "ループ 5–9 ・ A リバーブの「響きの長さ」をえらんでいます"
    /// (mock F4-Mixer). The screen names the strip, the effect and the parameter.
    ParamSelected {
        loop_bars: Option<BarSpan>,
        insert: InsertRef,
        effect: EffectKind,
        param: u32,
    },
    /// The same for a row of a plugin (`SelectedPlugin::row`): "1 ピアノ ・ Surge XT の
    /// 「Macro 1」をえらんでいます". `plugin` and `param` are the names the plugin gives; the
    /// screen names the strip from `slot`.
    PluginParamSelected {
        loop_bars: Option<BarSpan>,
        slot: PluginSlotRef,
        plugin: String,
        param: String,
    },
    /// `Command::TypeParam` put the value at an end of its range (spec 6 「打ちこんだ値が
    /// はみ出した」): "レシオは 1〜20 なので、20 : 1 にしました。"
    ParamClamped {
        to: ParamEnd,
    },
    // The inserts (spec 6, 86-9). The screen names the strip from `insert.strip` or `strip`
    // ("2 ベース", "A リバーブ", "マスター") and writes "インサート 2" from `insert.slot` + 1.
    /// `ShelfConfirm` in `InsertEffect` mode: "2 ベース のインサート 2 に リバーブ を入れました。"
    EffectInserted {
        insert: InsertRef,
        effect: EffectName,
    },
    /// `RemoveEffect`: "2 ベース のインサート 1 の コンプレッサー を外しました。取り消せます
    /// （Mod Z）". The slot and the effect it held.
    EffectRemoved {
        insert: InsertRef,
        effect: EffectName,
        lanes_removed: u32,
    },
    /// `MoveInsert`: "2 ベース のインサートを並べかえました（EQ 4 バンド → コンプレッサー）。"
    /// `order`: the effects of the strip as they are now, top first, empty slots left out.
    InsertsReordered {
        strip: StripRef,
        order: Vec<EffectName>,
    },
    /// `ShelfConfirm` in `ReplaceEffect` mode: "2 ベース のインサート 1 を EQ 4 バンド に入れかえ
    /// ました。取り消せます（Mod Z）". `effect`: the new one.
    InsertReplaced {
        insert: InsertRef,
        effect: EffectName,
        lanes_removed: u32,
    },
    /// `ToggleBypass`: "2 ベース の コンプレッサー をとばしました。", with `bypassed` false
    /// "…をもどしました。"
    EffectBypassed {
        insert: InsertRef,
        effect: EffectName,
        bypassed: bool,
    },
    // The plugin windows (spec 6). In the two sentences about one window 「（名前）」 is the
    // place and the plugin, "1 ピアノ ・ Surge XT" (mock F-ParamMany): the screen names the
    // strip from `slot`, and writes `name` alone for the plugin the shelf auditions (`None`).
    /// A window was asked for (`OpenPluginWindow`, `SelectInsert`, `SelectInstrument`) and is
    /// not up after 0.2 s: "1 ピアノ ・ Surge XT の窓をひらいています…". Until it is up, or the
    /// plugin gives up.
    PluginWindowOpening {
        name: String,
        slot: Option<PluginSlotRef>,
    },
    /// A window came up that does not fit the screen: "1 ピアノ ・ Surge XT の窓がひらいて
    /// います（1597×995 ・ この画面より大きい窓です）。パラメータは右でも動かせます。"
    PluginWindowLarge {
        name: String,
        slot: Option<PluginSlotRef>,
        width: u32,
        height: u32,
    },
    /// A plugin window came up and is the only one open (spec 6, 91-2): "Surge XT の窓が
    /// ひらいています". `name` is the plugin's alone.
    PluginWindowOpen {
        name: String,
    },
    /// A plugin window came up and others are open too (user:2026-11-19): "プラグインの窓が
    /// 2 つひらいています". `count`: the windows open now, this one with them; 2 or more.
    PluginWindowsOpen {
        count: u32,
    },
    /// A window was asked for and the plugin could not put it up (spec 6, 91-1): "Surge XT
    /// の窓をひらけませんでした。パラメータの欄では動かせます。" `name` is the plugin's alone.
    PluginWindowFailed {
        name: String,
    },
    /// The mixer was opened while insert slots are passed by (user:2026-11-19, mock
    /// F5-MixerBypass): "4 パッド の MVerb をとばしています ・ 3 ドラム の Brick EQ は止まって
    /// います（音は、その枠をとばして流しています）". Said once as the mixer opens, and only when
    /// one of the two lists is not empty. The slots in the mixer's order (tracks, returns,
    /// master; top first), the bypassed ones first.
    MixerPassesBy {
        bypassed: Vec<NamedInsert>,
        stopped: Vec<NamedInsert>,
    },
    /// `Command::TypeParam` with a text that does not read as a value (spec 6 「数字として
    /// 読めない」): "しきい値は数字で打ちます。もとの −24.0 dB のままです。" The screen names the
    /// parameter from `param` (its table for an effect that comes with meloet, the plugin's
    /// `ParamTable` row otherwise). `kept`: for an effect that comes with meloet the value
    /// as a plain number in the parameter's own unit, which the screen writes as the row
    /// does; for a plugin the plugin's own text of the value (`ParamRow::text`; the number
    /// with two decimals when the plugin gives no text, empty when it tells no value).
    ParamRejected {
        param: ParamRef,
        kept: String,
    },
    /// A plugin whose window meloet cannot open yet was selected (`PluginWindowKind::NotYet`):
    /// "ループ 5–9 ・ Mossy Verb の窓はまだひらけません。パラメータは右で動かせます" (mock
    /// F-ParamNoWindow). `name` is the plugin's alone.
    PluginWindowNotYet {
        loop_bars: Option<BarSpan>,
        name: String,
    },
    /// The left of the status line when nothing else is to be said: "ループ 5–9" when the loop
    /// is on, "選択なし" when nothing is selected, joined by " ・ ". `notes` while notes of the
    /// piano roll are selected ("選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み…", mock
    /// A-PianoRoll); `nothing_selected` is false then.
    Summary {
        loop_bars: Option<BarSpan>,
        nothing_selected: bool,
        notes: Option<NoteSelection>,
    },
    /// The write tool on a key outside the scale while the scale is locked: "D# は C メジャー
    /// の外の音です（スケール固定 ・ オン）" (spec 6). The screen names the key without its
    /// octave.
    OutOfScale {
        key: u8,
        root: u8,
        mode: ScaleMode,
    },
    /// Spec 6 「クオンタイズをかけた」: "クオンタイズ 1/16（75 %）を 3 ノートに。うすい枠が、そろえる前の位置。"
    /// `division` and `triplet` as in `Grid`.
    Quantized {
        division: u16,
        triplet: bool,
        strength: u8,
        count: u32,
    },
    /// "なおすためのファイルから戻しました ・ まだ保存していません".
    Recovered,
    /// "戻した変更をすてて、最後に保存した形にしました。取り消せます（Mod Z）".
    RecoveryDiscarded,
    /// "なおすためのファイルが読めませんでした ・ 最後に保存した曲をひらいています", or with
    /// `saved_song` false "… ・ 新しい曲をひらいています".
    RecoveryUnreadable {
        saved_song: bool,
    },
    /// A scan that shows no count (from the second start on, a place added) found new
    /// plugins: "新しいプラグインが 3 個見つかりました。棚の「あとから入れたもの」に入れました。"
    PluginsFound {
        count: u32,
    },
    /// The first scan, or `RescanPlugins`, is over: "スキャンが終わりました ・ 667 個を棚に入れ
    /// ました。4 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。" with
    /// the button "止まった 4 個を見る" (`Command::OpenScanStopped`). The numbers of
    /// `ScanView::Finished`.
    ScanFinished {
        added: u32,
        stopped: u32,
    },
    /// `RetryStoppedPlugin` and the bundle read: "Granular Fog を、もう一度ためしました。今度は
    /// 入りました。" `name` is `StoppedPluginView::name`.
    ScanRetryAdded {
        name: String,
    },
    /// `RetryStoppedPlugin` and it stopped again: "Granular Fog は、また止まりました。棚には
    /// 入れません。"
    ScanRetryStopped {
        name: String,
    },
    /// The dialog is up (mock F5-ScanStopped): "スキャンで止まったプラグイン をひらいています".
    ScanStoppedOpen,
    /// A plugin's host process answered no more (spec 6 「プラグインが止まった」): "（名前）が
    /// 止まりました ・ （トラック）。曲のほかの部分は動いています。音はこの枠をとばして流して
    /// います。パラメータの値は残っています。" with "もう一度読みこむ" (`Command::ReloadPlugin`)
    /// and "外す" (`Command::RemovePlugin`). The screen names the strip from `slot`.
    PluginStopped {
        name: String,
        slot: PluginSlotRef,
    },
    // 「あとから入れたもの」 (round 5). The five below say for an SF2 preset or a plugin what
    // `AuditionNewTrack`, `AuditionReplace`, `SamplePlaying`, `InstrumentReplaced` and
    // `InstrumentLoading` say for a sound that comes with meloet; with `SoundName::Builtin`
    // they are the same sentence.
    /// "Surge XT ・ まだトラックには入っていません ・ 入れるには Enter" (mock
    /// F5-PluginAudition), "Clean Guitar ・ FluidR3_GM.sf2 ・ まだトラックには…" (mock
    /// F5-SF2Shelf).
    AuditionNewTrackSound {
        sound: SoundName,
    },
    AuditionReplaceSound {
        sound: SoundName,
        track: TrackRef,
        current: SoundName,
    },
    SamplePlayingSound {
        sound: SoundName,
    },
    InstrumentReplacedSound {
        track: TrackRef,
        sound: SoundName,
        lanes_removed: u32,
    },
    /// A key was played on a track whose sound is still being read.
    SoundLoading {
        sound: SoundName,
    },
    /// The selected plugin is being read (`ShelfView::loading_sound`): "Surge XT を読みこんで
    /// います… ・ やめるには Esc" (mock F5-PluginLoading).
    PluginLoading {
        name: String,
    },
    /// Files are held over the window (`WorkspaceView::file_drop`): "SF2 のファイルを持って
    /// きています".
    Sf2Dragging,
    /// Files are held over the window and none of them is an SF2 file (spec 6): "SF2 では
    /// ないファイルを持ってきています（足せません）".
    NotSf2Dragging,
    /// "FluidR3_GM.sf2 を足しました。音色 128 ・ ドラムのキット 9": the presets of the file that
    /// are no drum kits, and its drum kits. `skipped`: of the files let go of with it, those
    /// not added (no SF2 file, or one that did not read): "1 つは SF2 ではないので、足して
    /// いません。" follows when it is not 0 (spec 6 「SF2 をいくつも足した」).
    Sf2Added {
        file: String,
        presets: u32,
        kits: u32,
        skipped: u32,
    },
    /// More than one SF2 file of the files let go of over the window were added, in the order
    /// they were held (spec 3, 89-10): "SF2 を 2 つ足しました（FluidR3_GM.sf2 ほか 1 つ）。1 つ
    /// は SF2 ではないので、足していません。" `first`: the first one added. `skipped` as for
    /// `Sf2Added`.
    Sf2AddedMany {
        added: u32,
        first: String,
        skipped: u32,
    },
    /// "FluidR3_GM.sf2 は読めませんでした。ファイルがこわれているか、SF2 ではないかもしれません。"
    Sf2Unreadable {
        file: String,
    },
    /// A song was opened that uses an SF2 file this PC does not have: "曲で使っている
    /// FluidR3_GM.sf2 が見つかりません。2 本のトラックはサイン波で鳴らしています。" The first
    /// such file; the shelf has a notice for each (`ShelfView::missing`).
    Sf2Missing {
        file: String,
        tracks: u32,
    },
    /// A song was opened whose tracks use sounds of the pack this PC does not have: "音の素材
    /// が見つからないので、4 本のトラックをサイン波で鳴らしています。"
    SoundsMissing {
        tracks: u32,
    },
    /// A song was opened that uses a plugin this PC does not have as the instrument of
    /// `tracks` tracks: "曲で使っている Surge XT が見つかりません。1 本のトラックはサイン波で
    /// 鳴らしています。" The first such plugin.
    PluginMissing {
        name: String,
        tracks: u32,
    },
    /// A plugin this PC has did not load as the instrument of `tracks` tracks of the song,
    /// which sound as the sine wave (spec 3, 89-12): "曲で使っている Surge XT を読みこめません
    /// でした。1 本のトラックはサイン波で鳴らしています。"
    PluginUnreadable {
        name: String,
        tracks: u32,
    },
    /// 「外す」 on the stopped plugin that was a track's instrument (spec 3, 89-5): "5 Surge XT
    /// の音源を外しました。サイン波で鳴らします。取り消せます（Mod Z）".
    InstrumentPluginRemoved {
        track: TrackRef,
    },
    // In the six below, `bar` and `beat` are as shown (from 1), and bar 1 beat 1 is the
    // song's head.
    /// A tempo point with a ramp to the next point is selected (spec 6): "5 小節目の変更をえら
    /// んでいます ・ 104.00 から 9 小節目の 120.00 へ、だんだん速く". On the last point, whose
    /// ramp does nothing, `TempoPointSelected` is said.
    TempoRampSelected {
        bar: u32,
        beat: u32,
        tempo_bpm: f64,
        to_bar: u32,
        to_beat: u32,
        to_tempo_bpm: f64,
    },
    /// A tempo point without a ramp is selected (B-92): "5 小節目の変更をえらんでいます ・
    /// 120.00".
    TempoPointSelected {
        bar: u32,
        beat: u32,
        tempo_bpm: f64,
    },
    /// A meter change is selected (B-92): "9 小節目の変更をえらんでいます ・ 3/4".
    MeterChangeSelected {
        bar: u32,
        meter_num: u8,
        meter_den: u8,
    },
    /// `AddMarker` (spec 6): "マーカー 4 を足しました。名前を打って Enter".
    MarkerAdded {
        name: String,
    },
    /// `SetTempo` in a song with tempo points changed this one (B-92): "5 小節目の変更を
    /// 120.00 にしました。"
    TempoPointSet {
        bar: u32,
        beat: u32,
        tempo_bpm: f64,
    },
    /// `SetMeter` in a song with meter changes changed this one (B-92): "9 小節目の変更を 3/4
    /// にしました。"
    MeterChangeSet {
        bar: u32,
        meter_num: u8,
        meter_den: u8,
    },
    /// Mod Z (spec 6): "「ノートを動かす」を取り消しました（3 ノート）。やりなおすには Mod
    /// Shift Z". With `StepWhat::None` the brackets are left out.
    Undone {
        name: StepName,
        what: StepWhat,
    },
    /// Mod Shift Z (spec 6): "「ノートを動かす」をやりなおしました（3 ノート）。"
    Redone {
        name: StepName,
        what: StepWhat,
    },
    /// `HistoryJump` (spec 6): "11 手目「マーカーを足す」まで戻しました（5 手）。" `index` and
    /// `name`: the row that is 「いまここ」 now; `steps`: how many steps were undone, or
    /// with `forward` redone.
    HistoryJumped {
        index: u32,
        name: StepName,
        steps: u32,
        forward: bool,
    },
    /// Mod Z with no step to undo (spec 6): "取り消せる手がありません。"
    NothingToUndo,
    /// 「● 書く」 is on and the song plays (spec 6): "1 ピアノ ・ 音量を書いています（● 書く ・
    /// 止めるとオフに戻ります）".
    AutomationWriting {
        track: TrackRef,
        target: LaneName,
    },
    /// A hand moved what a lane moves (spec 6): "4 パッド の音量を手で動かしたので、オート
    /// メーションを休ませました。「もどす」で線のとおりに戻ります".
    AutomationRested {
        track: TrackRef,
        target: LaneName,
    },
    /// 「もどす」 (spec 6): "4 パッド の音量を、オートメーションのとおりに戻しました。"
    AutomationRestored {
        track: TrackRef,
        target: LaneName,
    },
    /// The mixer was opened and the faders of these tracks are moved by lanes that do not
    /// rest (mock F6-MixerAuto): "1 ピアノ と 4 パッド の音量は、オートメーションで動いて
    /// います". Not said when `MixerPassesBy` is.
    AutomationMoving {
        tracks: Vec<TrackRef>,
    },
    /// Mod Shift Z with no step to redo (not in spec 6; after `NothingToUndo`):
    /// "やりなおせる手がありません。"
    NothingToRedo,
    // Protocol 23 (spec 6, the sentences of the tenth edition).
    /// `SelectInstrument` on a sound with no parameters: "グランドピアノには、動かせる
    /// パラメータがありません" (spec 6 「動かせるパラメータがない」, 19-A).
    NoParams {
        sound: SoundName,
    },
    /// `AddVocalTrack`: "6 歌 を足しました ・ 入口 1 から録ります ・ 歌のトラックの ● は、
    /// 選んでいなくても自分でつけます".
    VocalTrackAdded {
        track: TrackRef,
        input: VocalInput,
    },
    /// "音の入口がありません。マイクをつなぐか、音の設定で入口をえらんでください。" with
    /// 「音の設定…」: a vocal track was added, selected or armed, or R was pressed with one
    /// armed, and there is no input.
    NoInput,
    /// "自分の声が少し遅れて聞こえます（いまは 13 ms）。スピーカーだと…": the first time a
    /// monitor is turned on. `latency_ms`: how late the voice comes out, `None` while the
    /// audio is not running (the screen leaves the bracket out).
    MonitorFirstOn {
        latency_ms: Option<f32>,
    },
    /// "マイクの音が大きすぎます（0 dB に届きました）。…": the input of an armed or selected
    /// vocal track reached 0 dB.
    MicTooLoud,
    /// E, the menu, or `ToggleBand` for the piano roll with a vocal track selected.
    VocalHasNoPianoRoll,
    /// Recording with vocal tracks, in place of `Recording` and `LoopRecording`. With `keys`
    /// "● 録音中 ・ 1 ピアノ（鍵盤）と 6 歌（入口 1）に録っています ・ 止めるには Space",
    /// without "● 録音中 ・ 6 歌（入口 1）・ モニター オン ・ 止めるには Space". `vocals` is
    /// never empty, in the order of the tracks. `monitor`: one of them is heard through
    /// meloet.
    RecordingVocals {
        keys: Option<TrackRef>,
        vocals: Vec<VocalRef>,
        monitor: bool,
    },
    /// While the dialog 「遅れを測る」 is up: "「遅れを測る」のダイアログをひらいています"
    /// (mock F7-Latency).
    LatencyMeasureOpen,
    /// A song was opened and `count` of its audio files are not found: "曲で使っている音の
    /// ファイルが 2 つ見つかりません。そのクリップは鳴りません。" (spec 6).
    AudioMissing {
        count: u32,
    },
    /// `Command::SplitClips`: "歌 テイク 1 を 7. 1. 0000 で切りました。" `name`: the clip, the
    /// first in the order of the tracks when `count` clips were cut.
    ClipSplit {
        name: String,
        tick: u64,
        count: u32,
    },
    /// Audio files were brought in: "ドラム.wav を入れました（6 歌 ・ 5. 1. 0000）。" `file`,
    /// `track` and `tick`: the first of the `count` clips made. `skipped`: files that were
    /// with them and are not audio that reads.
    AudioImported {
        file: String,
        track: TrackRef,
        tick: u64,
        count: u32,
        skipped: u32,
    },
    /// "「声.aac」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。": nothing was brought in.
    /// `file`: the first that did not read.
    AudioUnreadable {
        file: String,
    },
    /// Files with audio among them are held over the window: "音のファイルを 8 つ持ってきて
    /// います ・ 左に落とすと別々のトラック、右に落とすと 1 本にならべる" (spec 6); the second
    /// half only for two or more. `sf2`: SF2 files among them, which go to the shelf.
    AudioDragging {
        audio: u32,
        sf2: u32,
    },
    /// `Command::FindSourceTempo` found a tempo and set it (spec 6 「この音のテンポを
    /// さがした」): "この音のテンポは 120.00 くらいです（さがした数を入れました）。", with
    /// `same` "…くらいです（いまの数と同じです）。": the audio had that tempo already. `name`:
    /// the clip. `bars`: how many bars the audio was taken to be (1, 2, 4 or 8).
    SourceTempoFound {
        name: String,
        bpm: f64,
        bars: u8,
        same: bool,
    },
    /// `Command::FindSourceTempo` found none: the audio is no 1, 2, 4 or 8 bars at a tempo
    /// from 60 to 180. "この音のテンポは見つかりませんでした。数を打ちこんでください。"
    SourceTempoNotFound {
        name: String,
    },
    /// The dialog is up (mock F7-Export): "「書き出し」のダイアログをひらいています".
    ExportDialogOpen,
    // Protocol 27 (spec 6, the sentences of the twelfth edition).
    /// The input of the vocal recording went away (spec 6 「入口が抜けた」): "入口 1（USB マイク）
    /// が抜けました。録音を止めました。録れた所までは残しています。" `input`: the channels the
    /// first vocal track that was recording took; `device`: the PC's name of the input.
    InputLost {
        input: VocalInput,
        device: String,
    },
    /// Said as the recording stops, when the input skipped (spec 6 「録っている間に音が
    /// とぎれた」): "録っている間に音が 3 回とぎれました（PC がいそがしかったため）。…"
    /// `count`: `RecordingFaults::gaps` + `dropped` of the recording.
    RecordingGaps {
        count: u32,
    },
    /// `Command::FindAudio` took the chosen file (spec 6 「さがして見つけた」): "歌 テイク
    /// 1.wav を見つけました。2 つのクリップがまた鳴ります。" `file`: the name the song looks
    /// for; `clips`: the clips that play it.
    AudioFound {
        file: String,
        clips: u32,
    },
    /// `Command::FindAudio` and the chosen file has another sample rate or count of channels
    /// (spec 6 「さがしたファイルが合わない」): "えらんだファイルは使えません（サンプルレートか
    /// チャンネルの数がちがいます）。" Nothing was copied.
    AudioFoundMismatch,
    /// A file brought in is audio of a kind meloet reads and does not read (spec 6 「こわれた
    /// ファイル ・ 3 チャンネル以上」): "「声.wav」はこわれているので読めません。" In place of
    /// `AudioUnreadable` for such a file.
    AudioBroken {
        file: String,
    },
    /// The same for a file of three channels or more: "「8ch.wav」は 3 チャンネル以上なので
    /// 読めません（読めるのはモノとステレオ）。"
    AudioTooManyChannels {
        file: String,
    },
    /// While saving copies audio files into the song's folder (spec 6 「保存で音を写して
    /// いる」): "保存しています ・ 音のファイルを写しています 3 / 12（80 / 240 MB）". `done` of
    /// `total` files, `done_bytes` of `total_bytes`. Sent at most 4 times a second; the next
    /// message takes its place when the save is over.
    SavingAudio {
        done: u32,
        total: u32,
        done_bytes: u64,
        total_bytes: u64,
    },
    /// Audio of a vocal clip did not come from the disk in time and was not heard (spec 6
    /// 「ディスクが間に合わない」): "音のファイルを読むのが間に合わず、歌が少し切れました
    /// （ディスクがいそがしかったため）。" Said once as it happens, then not again until the song
    /// has stopped.
    DiskTooSlow,
    /// The band asks for the pack (mock F8-PackAsk): "音の素材がまだありません ・ いまはサイン波
    /// で鳴ります". While `SoundsView::ask`.
    PackAsking,
    /// An instrument's sound came and sounds now, and more are coming (spec 3, 45-A):
    /// "グランドピアノ が鳴るようになりました ・ のこりの音色を取っています".
    SoundReady {
        instrument: InstrumentKind,
    },
    /// The last file came (spec 6 「音の素材が取れた」): "音の素材をぜんぶ取りました。31 の
    /// 音色が鳴ります。" `count`: `SoundsView::have`.
    PackFetched {
        count: u32,
    },
    /// The fetch stopped (spec 6 「音の素材 ・ つながっていない」 「… ・ とちゅうで切れた」
    /// 「… ・ ディスクがいっぱい」 「… ・ こわれていた」), with the button of the row (「もう一度」
    /// or 「続きから」, `Command::FetchPack`). The numbers of `PackFetchView::Failed`.
    PackFailed {
        failure: PackFailure,
        done_bytes: u64,
        total_bytes: u64,
    },
    /// Once, at the start that found it (spec 6 「音の素材 ・ 新しい包み」): "音の素材の新しい
    /// 包みがあります（12 MB）。" with 「取りにいく」 (`Command::FetchPack`).
    NewPack {
        bytes: u64,
    },
}

/// The selected notes of the piano roll, for the status line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct NoteSelection {
    pub count: u32,
    /// The keys of the first `NOTE_SELECTION_KEYS` selected notes, in the order of the take.
    /// The mocks show at most 4 note names in the status line ("D4 E4 F4" in A-PianoRoll,
    /// "C3 E3 G3 C4" in F-PRPlay and F-PRRec); none shows more selected notes than names.
    pub keys: Vec<u8>,
    /// The take is quantized (it has an origin take), so it can go back ("もとの位置へ戻せます").
    pub quantized: bool,
    /// The keys of all the selected notes, each once with how many notes are on it, lowest
    /// first ("キック ×4 スネア ×2", spec 3, 85). At most `NOTE_SELECTION_KEYS` + 1: a fifth
    /// only says that there are more ("…").
    pub names: Vec<KeyCount>,
}

/// How many keys `NoteSelection::keys` holds at most.
pub const NOTE_SELECTION_KEYS: usize = 4;

/// "{number} {name}" (`track.label`). `number` is `TrackView::number` at the time of the
/// message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrackRef {
    pub number: u32,
    pub name: String,
}

/// Bar lines counting from 1: `{from: 5, to: 9}` is "5–9".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BarSpan {
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub rev: u64,
    pub changes: Vec<Change>,
}

/// One whole section. `section` is the key of that section in `AppState`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "section", content = "value", rename_all = "camelCase")]
pub enum Change {
    /// Boxed: with its strips it is far larger than the other sections.
    Song(Box<SongView>),
    Timeline(TimelineView),
    Automation(AutomationView),
    History(HistoryView),
    Transport(TransportView),
    Arrange(ArrangeView),
    PianoRoll(PianoRollView),
    Workspace(WorkspaceView),
    Shelf(ShelfView),
    Library(LibraryView),
    Params(ParamsView),
    Plugins(PluginsView),
    Windows(WindowsView),
    Keyboards(KeyboardsView),
    Outputs(OutputsView),
    Inputs(InputsView),
    Audio(AudioView),
    AudioWork(AudioWorkView),
    Export(ExportView),
    Sounds(SoundsView),
    Status(StatusView),
}

/// Things sent the moment they arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    Note {
        on: bool,
        note: u8,
        velocity: u8,
        t_us: u64,
    },
    Control {
        controller: u8,
        value: u8,
        t_us: u64,
    },
    Bend {
        value: i16,
        t_us: u64,
    },
    /// Files with audio among them are held over the window (`WorkspaceView::file_drop`
    /// with `audio` above 0) and the pointer came in or moved: where it is, in px of the
    /// page from the top left corner of the window's inside. The screen works out the track
    /// and the tick under it and draws the gray.7 frame there. Not sent for files with no
    /// audio among them.
    FileDrag {
        x: i32,
        y: i32,
    },
    /// The files of `FileDrag` were let go of at (`x`, `y`). Rust keeps their paths and
    /// waits for `Command::DropAudio` with this `drop`, which says where that is in the
    /// song; SF2 files among them are added at once, as before. A drop that gets no answer
    /// is let go when the next files come over the window.
    FileDrop {
        drop: u32,
        x: i32,
        y: i32,
    },
}

/// What the messages Channel carries, in arrival order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ServerMessage {
    Patch(Patch),
    Event(Event),
    LiveNotes(LiveNotes),
    ParamValues(ParamValues),
}

/// Notes of the recording going on, added a little at a time. `seq` goes up by one with every
/// message for the whole run of the app; the screen fetches `live_notes` when it skips one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LiveNotes {
    pub seq: u64,
    /// `RecordingView::id`.
    pub recording: u32,
    /// New notes, and notes seen before that now have their length. The same `(lap, index)`
    /// replaces what the screen holds.
    pub notes: Vec<LiveNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LiveNote {
    /// Pass of the loop, 0 for the first.
    pub lap: u32,
    /// Order of the key press within the lap, from 0.
    pub index: u32,
    pub key: u8,
    pub velocity: u8,
    /// Song position.
    pub start: u64,
    /// `None` while the key is down: draw it up to the play line.
    pub length: Option<u64>,
}

/// The reply of `live_notes`: every note of the recording going on, as of message `seq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LiveNotesSnapshot {
    /// The last `LiveNotes::seq` already in `notes` (0 before any).
    pub seq: u64,
    /// `None` when nothing is being recorded.
    pub recording: Option<u32>,
    pub notes: Vec<LiveNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HelloReply {
    pub protocol_version: u32,
    pub ok: bool,
    pub mono_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SendReply {
    pub rev: u64,
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests23;
#[cfg(test)]
mod tests24;
#[cfg(test)]
mod tests25;
#[cfg(test)]
mod tests26;
#[cfg(test)]
mod tests27;
