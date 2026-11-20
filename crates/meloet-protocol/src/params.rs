//! Plugin windows, the parameters of a plugin in the parameters panel, and the names the
//! status line gives effects and parameters (round 5, protocol 18).

use serde::{Deserialize, Serialize};

use crate::{EffectKind, InsertRef, PluginSlotRef, SelectedInsert, SelectedSound};

/// Whether a plugin has a window of its own, and whether meloet can open it. Known once the
/// plugin is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum PluginWindowKind {
    /// meloet opens it (a VST3 view embedded in X11, an LV2 X11UI). The panel's head has
    /// 「窓をひらく」, or 「窓を前に出す」 while the window is in `WindowsView::list`.
    Opens,
    /// It has a window of a kind meloet does not open yet: above the parameters "この子の窓は、
    /// まだひらけません。…" (spec 6), and no window button.
    NotYet,
    /// It has no window: "この子は窓を持ちません。パラメータはここで動かせます。", and no window
    /// button.
    None,
}

/// The plugin windows. A section of its own: it changes when a window opens or closes, which
/// is no step and touches nothing else.
///
/// The screen makes from it 「窓 N」 of the status line (not shown for an empty list), the
/// list above it, and the mark 「窓がひらいている」 on the insert slot, on the instrument slot of
/// the mixer and after the instrument's name in the arrange view: a slot has the mark when
/// an entry of `list` has that `slot`. Whether the list above the status line is unfolded is
/// the screen's own, as with every menu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WindowsView {
    /// Every window that is up, in the order they were opened. A window that was asked for
    /// and is not up yet is not here (`StatusMessage::PluginWindowOpening` says so when it
    /// takes long). meloet cannot know whether a window is behind another one: being here
    /// means only "open".
    pub list: Vec<PluginWindowView>,
}

/// A row of the list above 「窓 N」: the mark, `name`, and where the plugin sits, which the
/// screen names from `slot` ("1 ピアノ ・ 音源", "A リバーブ ・ インサート 1"). A click on the row
/// is `Command::OpenPluginWindow` with `slot`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PluginWindowView {
    /// Where the plugin sits now: Rust keeps it in step when inserts are moved or tracks
    /// deleted. `None`: the plugin the shelf auditions (`ShelfView::selected_sound`), which
    /// sits in no slot. At most one entry has any one `slot`.
    pub slot: Option<PluginSlotRef>,
    /// The plugin's name, as the song has it.
    pub name: String,
    /// The window's size in px of the PC's screen, as the plugin made it.
    pub width: u32,
    pub height: u32,
    /// The window does not fit the screen it is on
    /// (`StatusMessage::PluginWindowLarge`).
    pub larger_than_screen: bool,
}

/// What the parameters panel shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SelectedParams {
    /// An effect that comes with meloet, in an insert slot.
    Builtin(SelectedInsert),
    /// A plugin: in an insert slot, or the instrument of a track.
    Plugin(SelectedPlugin),
    /// The instrument of a track that has no parameters (spec 3, 19-A).
    Sound(SelectedSound),
}

/// A plugin in the parameters panel. Its name, format and state (`PluginView`) and whether
/// it is passed by (`InsertView::bypassed`) are in `song`, at `slot`; while its state is
/// `PluginState::Stopped` every row is unusable.
///
/// Its parameters are not in here: a plugin may have thousands (Surge XT: 2855), and the
/// values change while a knob of the plugin's window is turned. The names and groups are
/// fetched once (`plugin_params(table)`, a `ParamTable`), and the values that change come
/// on the messages Channel (`ParamValues`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SelectedPlugin {
    pub slot: PluginSlotRef,
    pub window: PluginWindowKind,
    /// Names the plugin's parameters as they are listed now; `plugin_params` takes it. A new
    /// number each time the plugin is selected, and when the list itself changes under the
    /// panel (the plugin says so, it was read again, an undo put its whole state back): the
    /// screen then fetches again and lets go of the table it had. `None`: there is no list
    /// (the plugin is being read, is not found, or stopped before it was read); the panel
    /// shows its head only.
    pub table: Option<u32>,
    /// How many parameters the table has ("Surge XT 774" on the folded panel), not counting
    /// `ParamTable::keyboard_knobs`; 0 with no table.
    pub count: u32,
    /// The chosen row, by `ParamRow::id` (`Command::SelectParam`).
    pub row: Option<u32>,
    /// The parameters an automation lane writes, by `ParamRow::id`, in the order of the
    /// table: the group 「オートメーションに書いている」 at the top of the list. Empty until
    /// automation is made.
    pub automated: Vec<u32>,
}

/// The reply of `plugin_params`: every parameter of a plugin, in the order and the groups
/// the plugin gives, with the values as of message `seq`.
///
/// The screen folds the groups (all folded at first when the table has more than 40
/// parameters, spec 3) and searches the names, on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamTable {
    /// `SelectedPlugin::table`.
    pub table: u32,
    /// The last `ParamValues::seq` already in the rows (0 before any).
    pub seq: u64,
    /// In the plugin's order. A plugin that names no groups has one group, with no name.
    /// Without the rows of `keyboard_knobs`.
    pub groups: Vec<ParamGroup>,
    /// Parameters that are there only to take a keyboard's knobs (MIDI CC), in the plugin's
    /// order: not in `groups`, not counted in `SelectedPlugin::count`. The screen shows them
    /// only while searching, in a group of their own (spec 3, 91-3).
    pub keyboard_knobs: Vec<ParamRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamGroup {
    /// As the plugin says it (the top level of a VST3 unit tree, an LV2 port group), not
    /// translated. `None`: the parameters the plugin put in no group; when other groups have
    /// names, the screen heads these 「そのほか」 (mock F-ParamMany).
    pub name: Option<String>,
    pub params: Vec<ParamRow>,
}

/// A parameter of a plugin: what a person can move. Parameters the plugin hides or only
/// reports (read-only) are not listed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamRow {
    /// The plugin's own number for it (a VST3 ParamID, the index of an LV2 port): what
    /// commands and `ParamValues` name it by. Once in a table.
    pub id: u32,
    /// As the plugin says it.
    pub name: String,
    pub kind: ParamKind,
    /// Where the plugin starts it, as `value`.
    pub default: f64,
    /// 0.0..=1.0 across the parameter's range, whatever its kind. `None`: the plugin does
    /// not tell (an LV2 plugin that does not answer `patch:Get`, as Surge XT's): the row
    /// has no position until a value comes.
    pub value: Option<f64>,
    /// The value as the plugin writes it, with its unit ("523.25 Hz", "Hall"). `None`: the
    /// plugin gives no text for it, or `value` is `None`.
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ParamKind {
    /// A bar.
    Smooth,
    /// ◂ ▸: `steps` values, 2 or more, evenly across 0.0..=1.0; step n (from 0) is
    /// n / (`steps` − 1).
    Steps {
        steps: u32,
        /// The text of each step, in order, for the list Enter opens. Empty when the plugin
        /// does not name its steps, or has more than 128.
        labels: Vec<String>,
    },
    /// On (1.0) and off (0.0).
    Toggle,
}

/// Values of the table the panel shows that changed, on the messages Channel: at most 30
/// messages a second, each with only the parameters that changed since the one before, and
/// only for the table of `ParamsView::selected`. Whatever moved them: the plugin's window,
/// `Command::SetPluginParam` (also with `commit: false`, so that the text of a dragged
/// value follows), `Command::TypeParam`, the plugin itself.
///
/// `seq` goes up by one with every message of a table, from `ParamTable::seq`. The screen
/// drops messages of another table and those not above the `seq` it has, and fetches the
/// table again when one is skipped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamValues {
    pub table: u32,
    pub seq: u64,
    pub values: Vec<ParamValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ParamValue {
    /// `ParamRow::id`.
    pub id: u32,
    /// As `ParamRow::value` and `ParamRow::text`.
    pub value: Option<f64>,
    pub text: Option<String>,
}

/// A parameter a value is typed into (`Command::TypeParam`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ParamRef {
    /// As `Command::SetParam` names it.
    Builtin {
        insert: InsertRef,
        effect: EffectKind,
        param: u32,
    },
    /// As `Command::SetPluginParam` names it.
    Plugin {
        slot: PluginSlotRef,
        table: u32,
        param: u32,
    },
}

/// The end of its range a typed value was put at (`StatusMessage::ParamClamped`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ParamEnd {
    /// The screen has the parameter's name and range, and writes `value` (in the
    /// parameter's own unit) as the panel does.
    Builtin {
        effect: EffectKind,
        param: u32,
        value: f32,
    },
    /// `value` is the plugin's text of the value set, and `range` its texts of both ends
    /// ("0.00 %〜100.00 %"); for a plugin that gives no texts, the numbers.
    Plugin {
        name: String,
        value: String,
        range: String,
    },
}

/// An insert slot and the effect in it, as the status line names them ("4 パッド の MVerb"):
/// the screen names the strip from `insert.strip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct NamedInsert {
    pub insert: InsertRef,
    pub effect: EffectName,
}

/// An effect by the name the status line gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EffectName {
    /// The screen has the name.
    Builtin { effect: EffectKind },
    /// "Dragonfly Reverb".
    Plugin { name: String },
}
