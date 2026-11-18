//! The contract between Rust and the screen (docs/architecture.md 3).
//!
//! JSON shapes are camelCase. The TypeScript types in `ui/src/bridge/generated/` are written
//! from these types by `cargo test -p meloet-protocol`.
//! All times are CLOCK_MONOTONIC. JSON carries microseconds (`tUs`); the telemetry binary
//! carries nanoseconds. Song positions are ticks, 480 to a quarter note (docs/architecture.md 11).

mod take;
mod telemetry;

use serde::{Deserialize, Serialize};

pub use take::{
    TAKE_CONTROL_LEN, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NO_FROM, TAKE_NOTE_LEN, TAKE_VERSION,
    TakeControl, TakeControlValue, TakeError, TakeNote, TakeNotes,
};
pub use telemetry::{
    TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_VERSION, Telemetry, TelemetryError,
    TransportReading,
};

pub const PROTOCOL_VERSION: u32 = 14;

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
    /// Nothing happens while the shelf auditions (`ShelfView::selected`).
    Record,
    /// Home.
    GoToStart,
    /// A click on the ruler, and the position typed into "1. 1. 000" of the transport. Rust
    /// goes to `tick` as it is. The screen snaps first: in the arrange view to the lines the
    /// ruler shows, in the piano roll to its grid, not at all with Shift (B-61).
    Locate {
        tick: u64,
    },
    /// Beats per minute as typed; Rust keeps 0.01 steps.
    SetTempo {
        bpm: f64,
    },
    SetMeter {
        num: u8,
        den: u8,
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
    /// "armed" track (spec 3, 12-A).
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
    /// effects.
    /// When the shelf is open already, the mode changes and whatever was auditioned is put
    /// away as `ShelfCancel` does. `Replace` of a track that is gone does nothing, and so
    /// does `InsertEffect` for a strip that is gone or a slot that is not empty. Not while
    /// counting in or recording.
    OpenShelf {
        mode: ShelfMode,
    },
    /// A click on a row, ↑ ↓: the row becomes `ShelfView::selected` and is auditioned. While
    /// a row is auditioned, the keys sound with that instrument and nothing can be recorded
    /// (`TransportView::can_record` is false). In `Replace` mode the clips of that track play
    /// with it too; the song itself is not changed. `None`, or an instrument that is not in
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
    /// `InsertEffect`: `ShelfView::selected_effect` goes into the slot, with the parameters
    /// of a new one, as one step. It becomes the selected insert (as `SelectInsert`), so the
    /// parameters panel comes to the front, and the shelf's mode goes back to `NewTrack`.
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
    // A preview is also over when the song changes under it (a step, undo, redo, another
    // song). While counting in or recording the song is not to be changed: `commit: true`
    // then only ends the preview (the sound goes back), and `commit: false` does nothing.
    // Values out of range are brought into range by Rust.
    /// The fader of a strip. `db`: `None` is −∞; below −60 becomes −∞, above +6 becomes +6
    /// (spec 2 「メーターとフェーダーの目盛」). Rust keeps 0.01 steps.
    SetGain {
        strip: StripRef,
        db: Option<f32>,
        commit: bool,
    },
    /// The pan knob: -100 (all left) to 100 (all right), 0 the middle ("L 20" is -20). The
    /// master has none; for it nothing happens.
    SetPan {
        strip: StripRef,
        pan: i32,
        commit: bool,
    },
    /// How much of a track goes to return A or B, taken after its fader. `db` as in `SetGain`.
    SetSend {
        track: u32,
        bus: SendBus,
        db: Option<f32>,
        commit: bool,
    },
    /// A parameter of the effect in an insert slot. `effect` is the effect the screen shows
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
    /// When it was the selected one, nothing is selected afterwards. No part of the screen
    /// sends this yet: where it is asked for is not decided.
    RemoveEffect {
        insert: InsertRef,
    },
    /// A click on an insert slot that holds an effect: it becomes the selected one
    /// (`ParamsView::selected`), and the parameters panel (`Band::Params`) opens, in front
    /// (spec 3: an effect that comes with meloet has no window). `None`, or a slot that is
    /// empty: nothing is selected (the panel stays as it is). Not a step, and it works while
    /// recording too. An empty slot ("空き ＋") opens the shelf instead:
    /// `OpenShelf` with `ShelfMode::InsertEffect`.
    SelectInsert {
        insert: Option<InsertRef>,
    },
    /// A row of the parameters panel is chosen (a click, ↑ ↓): `SelectedInsert::row`. The
    /// status line then says `StatusMessage::ParamSelected`. `None`, or a number the effect
    /// has no parameter for: no row.
    SelectParam {
        param: Option<u32>,
    },
    /// `ShelfMode::InsertEffect`: a click on a row of the shelf's effects, ↑ ↓. The row
    /// becomes `ShelfView::selected_effect`. There is nothing to audition: the effect is in
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
pub type InsertsView = [Option<EffectKind>; INSERT_SLOTS];

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
    pub transport: TransportView,
    pub arrange: ArrangeView,
    pub piano_roll: PianoRollView,
    pub workspace: WorkspaceView,
    pub shelf: ShelfView,
    pub params: ParamsView,
    pub keyboards: KeyboardsView,
    pub outputs: OutputsView,
    pub inputs: InputsView,
    pub audio: AudioView,
    pub status: StatusView,
}

impl AppState {
    /// The sections that differ between `old` and `new`, each one whole.
    pub fn changes_from(old: &AppState, new: &AppState) -> Vec<Change> {
        let mut out = Vec::new();
        if old.song != new.song {
            out.push(Change::Song(new.song.clone()));
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
        if old.params != new.params {
            out.push(Change::Params(new.params.clone()));
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
        if old.status != new.status {
            out.push(Change::Status(new.status.clone()));
        }
        out
    }

    /// Replaces the section a change names.
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Song(v) => self.song = v,
            Change::Transport(v) => self.transport = v,
            Change::Arrange(v) => self.arrange = v,
            Change::PianoRoll(v) => self.piano_roll = v,
            Change::Workspace(v) => self.workspace = v,
            Change::Shelf(v) => self.shelf = v,
            Change::Params(v) => self.params = v,
            Change::Keyboards(v) => self.keyboards = v,
            Change::Outputs(v) => self.outputs = v,
            Change::Inputs(v) => self.inputs = v,
            Change::Audio(v) => self.audio = v,
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
    pub tempo_bpm: f64,
    pub meter_num: u8,
    pub meter_den: u8,
    pub tempo_changes: u32,
    pub undo_count: u32,
    pub redo_count: u32,
    /// For the piano roll band label: the notes of the shown take of `ArrangeView::piano_roll_clip`.
    pub note_count: u32,
    pub tracks: Vec<TrackView>,
    /// Return A, then return B.
    pub returns: [ReturnView; 2],
    pub master: MasterView,
    /// The selected track, by `TrackView::id` (B-70: its header is gray.4): the one the keys
    /// play and the one R records to, so also the one whose ● is lit. `SelectTrack`, and a
    /// click on a clip, change it. `None` only when the song has no tracks.
    pub play_target: Option<u32>,
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
    pub instrument: InstrumentKind,
    /// The fader: `None` is −∞, otherwise −60.0..=6.0.
    pub gain_db: Option<f32>,
    /// -100 (all left) to 100 (all right), 0 the middle.
    pub pan: i8,
    /// How much goes to return A and to return B, as `gain_db`.
    pub sends: [Option<f32>; 2],
    pub inserts: InsertsView,
    /// Saved with the song (`ToggleMute`).
    pub muted: bool,
    /// Not saved (`ToggleSolo`).
    pub soloed: bool,
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
    pub inserts: InsertsView,
}

/// The master strip: a fader and inserts. Its M, S and ● are unusable, and it has no pan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MasterView {
    pub gain_db: Option<f32>,
    pub inserts: InsertsView,
}

/// The parameters panel: the selected insert and the values of its effect. A section of its
/// own because selecting an insert or a row is no step: `song` is not sent again for it, as
/// with the selection of the piano roll. The mixer's own values (fader, pan, sends, which
/// effect sits where) are in `song`: they change only by steps, and every step sends `song`
/// anyway (`undo_count`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamsView {
    /// `None`: no insert is selected (also after its effect is taken out, its track deleted,
    /// or an undo took it away).
    pub selected: Option<SelectedInsert>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// The sounds that come with meloet (spec 3 「最初から入っている音色」), in the order of the
/// shelf. `Sine` is what used to be the placeholder sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum InstrumentKind {
    GrandPiano,
    ElectricPiano,
    Organ,
    FingerBass,
    SynthBass,
    StudioKit,
    ElectroKit,
    Strings,
    WarmPad,
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
    Bass,
    Drums,
    PadStrings,
    Synth,
}

/// The sounds of a drum kit (the studio kit and the electro kit), top row of the drum piano roll
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
    pub const ALL: [InstrumentKind; 11] = [
        InstrumentKind::GrandPiano,
        InstrumentKind::ElectricPiano,
        InstrumentKind::Organ,
        InstrumentKind::FingerBass,
        InstrumentKind::SynthBass,
        InstrumentKind::StudioKit,
        InstrumentKind::ElectroKit,
        InstrumentKind::Strings,
        InstrumentKind::WarmPad,
        InstrumentKind::SynthLead,
        InstrumentKind::Sine,
    ];

    pub fn group(self) -> InstrumentGroup {
        use InstrumentKind as I;
        match self {
            I::GrandPiano | I::ElectricPiano | I::Organ => InstrumentGroup::PianoKeys,
            I::FingerBass | I::SynthBass => InstrumentGroup::Bass,
            I::StudioKit | I::ElectroKit => InstrumentGroup::Drums,
            I::Strings | I::WarmPad => InstrumentGroup::PadStrings,
            I::SynthLead | I::Sine => InstrumentGroup::Synth,
        }
    }
}

impl InstrumentGroup {
    pub const ALL: [InstrumentGroup; 5] = [
        InstrumentGroup::PianoKeys,
        InstrumentGroup::Bass,
        InstrumentGroup::Drums,
        InstrumentGroup::PadStrings,
        InstrumentGroup::Synth,
    ];
}

/// The shelf of instruments. A section of its own: `workspace` is sent again at every step
/// of a window resize, and the song at every edit; the shelf changes only with what the
/// user does in it.
///
/// Whether the shelf is open is `LayoutView::right_open`. When it is closed, `mode` is
/// `NewTrack` and `selected`, `current`, `loading`, `sample` and `selected_effect` are `None`.
///
/// The shelf shows its instruments in `NewTrack` and `Replace` mode and its effects
/// (`EffectKind::ALL`, all of them always there) in `InsertEffect` mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ShelfView {
    pub mode: ShelfMode,
    /// The rows of the shelf, in its order: the instruments whose sounds are installed. One
    /// that is not here is not shown at all, not even as unusable.
    pub available: Vec<InstrumentKind>,
    /// The starred instruments, in the order of the shelf. May hold instruments that are not
    /// in `available`; the screen shows only the ones that are.
    pub favorites: Vec<InstrumentKind>,
    /// The row that is auditioned: the keys sound with it (`Command::ShelfSelect`).
    pub selected: Option<InstrumentKind>,
    /// `Replace` mode: the instrument the track has now ("いまの音源").
    pub current: Option<InstrumentKind>,
    /// The selected instrument is still being read: the keys do not sound with it yet
    /// ("（名前）を読みこんでいます…"). `None` again once it sounds.
    pub loading: Option<InstrumentKind>,
    /// The sample that is playing (`Command::PlaySample`). `None` again when it ends.
    pub sample: Option<SampleView>,
    /// `InsertEffect` mode: the chosen row (`Command::ShelfSelectEffect`).
    pub selected_effect: Option<EffectKind>,
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
}

/// A sample that is playing. The screen counts "0:02 / 0:04" from these on its own clock
/// (`HelloReply::mono_us`), with no message per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SampleView {
    pub instrument: InstrumentKind,
    /// When its first sound reached the ear, CLOCK_MONOTONIC µs.
    pub started_us: u64,
    pub length_us: u64,
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
    /// Beats of the count-in: of the one running, or else of the next one.
    pub count_in_beats: u32,
    /// False while the output is not playing.
    pub can_play: bool,
    /// Also false with no selected track (the record button still takes a click then, to say
    /// why), and while the shelf auditions.
    pub can_record: bool,
    /// From the record command until the take is in the song (also during the count-in).
    pub recording: Option<RecordingView>,
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
    pub chosen: Option<String>,
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
    /// `beats` is the whole count-in; how many are left comes from the telemetry
    /// (`countInBeatsLeft`), in step with the clicks heard.
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
    InstrumentReplaced {
        track: TrackRef,
        instrument: InstrumentKind,
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
    Song(SongView),
    Transport(TransportView),
    Arrange(ArrangeView),
    PianoRoll(PianoRollView),
    Workspace(WorkspaceView),
    Shelf(ShelfView),
    Params(ParamsView),
    Keyboards(KeyboardsView),
    Outputs(OutputsView),
    Inputs(InputsView),
    Audio(AudioView),
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
}

/// What the messages Channel carries, in arrival order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ServerMessage {
    Patch(Patch),
    Event(Event),
    LiveNotes(LiveNotes),
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
