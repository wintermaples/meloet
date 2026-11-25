//! The mixer: faders, pans, sends, the effects in the insert slots and their parameters, and
//! the insert the parameters panel shows.
//!
//! A drag is heard while it goes on and is one step when it ends: until then its value is a
//! preview, kept here and not in the song. What the engine hears of it is `mixsync.rs`.
//! All of it works while counting in or recording too: nothing here moves where a take is
//! recorded.

use std::sync::Arc;

use meloet_model::effects::EffectKind as ModelEffect;
use meloet_model::{
    Bus, Gain, Insert, Inserts, InstrumentRef, Pan, PluginPlace, PluginRef, SoundKnob, StepKind,
    StripId, TrackId, knob_usable, knobs, mixer,
};
use meloet_protocol::{
    EffectKind, EffectName, InsertRef, InstrumentKind, MasterView, NamedInsert, ParamEnd, ParamRef,
    ParamsView, PluginSlotRef, PluginState, PluginWindowKind, ReturnView, SelectedInsert,
    SelectedParams, SelectedPlugin, SelectedSound, SendBus, SlotsView, SoundKnobsView,
    StatusMessage, StripRef,
};

use super::Play;
use super::automation::{LaneKey, LetGo};
use super::wave3::{knob_view, model_knob};
use crate::added::{slots_view, sound_name};
use crate::instruments::kind_of;
use crate::state::Effect;

/// A value that sounds in place of the song's while its control is dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Preview {
    Gain(StripId, Gain),
    Pan(StripId, Pan),
    Send(TrackId, Bus, Gain),
    Param {
        strip: StripId,
        slot: usize,
        kind: ModelEffect,
        param: u32,
        value: f32,
    },
    /// A knob of a track's sound (round 12, E3), in percent.
    Knob {
        track: TrackId,
        knob: SoundKnob,
        value: f32,
    },
}

impl Preview {
    fn same_control(&self, other: &Preview) -> bool {
        use Preview as P;
        match (*self, *other) {
            (P::Gain(a, _), P::Gain(b, _)) | (P::Pan(a, _), P::Pan(b, _)) => a == b,
            (P::Send(a, x, _), P::Send(b, y, _)) => (a, x) == (b, y),
            (
                P::Param {
                    strip: a,
                    slot: x,
                    param: m,
                    ..
                },
                P::Param {
                    strip: b,
                    slot: y,
                    param: n,
                    ..
                },
            ) => (a, x, m) == (b, y, n),
            (
                P::Knob {
                    track: a, knob: x, ..
                },
                P::Knob {
                    track: b, knob: y, ..
                },
            ) => (a, x) == (b, y),
            _ => false,
        }
    }
}

/// The knobs of a sound that comes with meloet as the parameters panel shows them; `None`
/// for an SF2 preset.
fn knobs_view(t: &meloet_model::Track, row: Option<u32>) -> Option<SoundKnobsView> {
    if !matches!(t.instrument, InstrumentRef::Builtin(_)) {
        return None;
    }
    Some(SoundKnobsView {
        values: t.knobs.values(),
        usable: SoundKnob::ALL.map(|k| knob_usable(&t.instrument, k)),
        row,
    })
}

/// What the parameters panel shows, as it was when it was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Picked {
    /// An effect that comes with meloet, in an insert slot.
    Effect(StripId, usize, ModelEffect),
    /// A plugin, by its key: in an insert slot, or the instrument of a track.
    Plugin(PluginPlace, Arc<str>),
    /// The instrument of a keys track that has no parameters: a sound that comes with meloet
    /// or a preset of an SF2 file (spec 3, 19-A).
    Sound(TrackId, InstrumentRef),
}

impl Picked {
    fn place(&self) -> PluginPlace {
        match self {
            Picked::Effect(strip, slot, _) => PluginPlace::Insert(*strip, *slot),
            Picked::Plugin(place, _) => *place,
            Picked::Sound(track, _) => PluginPlace::Instrument(*track),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Mixer {
    previews: Vec<Preview>,
    selected: Option<Picked>,
    /// The chosen row of the panel.
    row: Option<u32>,
}

/// What selecting an insert or an instrument brings with it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Selecting {
    /// The parameters panel is to be shown, in front.
    pub panel: bool,
    pub notice: Option<StatusMessage>,
    /// The window of this plugin is to open, or to come to the front.
    pub window: Option<PluginSlotRef>,
}

pub(crate) fn effect_name(insert: &Insert) -> EffectName {
    match (insert.kind(), insert.plugin_ref()) {
        (Some(kind), _) => EffectName::Builtin {
            effect: effect_kind(kind),
        },
        (None, Some(plugin)) => EffectName::Plugin {
            name: plugin.name.to_string(),
        },
        (None, None) => unreachable!("an insert is an effect of meloet or a plugin"),
    }
}

fn insert_ref(strip: StripId, slot: usize) -> InsertRef {
    InsertRef {
        strip: strip_ref(strip),
        slot: slot as u8,
    }
}

impl Mixer {
    pub(super) fn previews(&self) -> &[Preview] {
        &self.previews
    }
}

pub(super) fn strip_id(strip: StripRef) -> StripId {
    match strip {
        StripRef::Track { id } => StripId::Track(TrackId(id)),
        StripRef::Return { bus } => StripId::Return(bus_of(bus)),
        StripRef::Master => StripId::Master,
    }
}

pub(super) fn strip_ref(strip: StripId) -> StripRef {
    match strip {
        StripId::Track(id) => StripRef::Track { id: id.0 },
        StripId::Return(Bus::A) => StripRef::Return { bus: SendBus::A },
        StripId::Return(Bus::B) => StripRef::Return { bus: SendBus::B },
        StripId::Master => StripRef::Master,
    }
}

fn bus_of(bus: SendBus) -> Bus {
    match bus {
        SendBus::A => Bus::A,
        SendBus::B => Bus::B,
    }
}

pub(crate) fn model_effect(kind: EffectKind) -> ModelEffect {
    match kind {
        EffectKind::Eq4 => ModelEffect::Eq4,
        EffectKind::Compressor => ModelEffect::Compressor,
        EffectKind::Reverb => ModelEffect::Reverb,
        EffectKind::Delay => ModelEffect::Delay,
        EffectKind::Chorus => ModelEffect::Chorus,
        EffectKind::Limiter => ModelEffect::Limiter,
        EffectKind::Gate => ModelEffect::Gate,
        EffectKind::DeEsser => ModelEffect::DeEsser,
        EffectKind::Saturation => ModelEffect::Saturation,
    }
}

pub(crate) fn effect_kind(kind: ModelEffect) -> EffectKind {
    match kind {
        ModelEffect::Eq4 => EffectKind::Eq4,
        ModelEffect::Compressor => EffectKind::Compressor,
        ModelEffect::Reverb => EffectKind::Reverb,
        ModelEffect::Delay => EffectKind::Delay,
        ModelEffect::Chorus => EffectKind::Chorus,
        ModelEffect::Limiter => EffectKind::Limiter,
        ModelEffect::Gate => EffectKind::Gate,
        ModelEffect::DeEsser => EffectKind::DeEsser,
        ModelEffect::Saturation => EffectKind::Saturation,
    }
}

fn gain_of(db: Option<f32>) -> Gain {
    db.map_or(Gain::SILENT, |db| Gain::from_db(f64::from(db)))
}

/// `None` is −∞.
pub(crate) fn gain_view(gain: Gain) -> Option<f32> {
    gain.db().map(|db| db as f32)
}

impl Play {
    /// The values that sound in place of the song's, one for each control being dragged.
    pub fn mix_previews(&self) -> &[Preview] {
        &self.mixer.previews
    }

    /// Takes the control of `value` as previewed (`commit` false) or as part of the song.
    /// `edit`: the step that puts the value in the song. `typed`: the value was typed. A
    /// lane that moves the control rests or is written into (docs/architecture.md 21-11).
    fn mix(
        &mut self,
        value: Preview,
        commit: bool,
        typed: bool,
        edit: impl FnOnce(&Self) -> Result<meloet_model::Edit, meloet_model::EditError>,
        effects: &mut Vec<Effect>,
    ) {
        let lane = self.preview_lane(&value);
        if !commit {
            if let Some((key, place)) = lane {
                self.hand_moved(key, place, effects);
            }
            match (self.mixer.previews.iter_mut()).find(|p| p.same_control(&value)) {
                Some(p) => *p = value,
                None => self.mixer.previews.push(value),
            }
            return;
        }
        self.mixer.previews.retain(|p| !p.same_control(&value));
        let (let_go, held) = match lane {
            Some((key, place)) => self.hand_lifted(key, place, typed, effects),
            None => (LetGo::Free, false),
        };
        match let_go {
            LetGo::Free => {
                if let Ok(e) = edit(self) {
                    self.apply(e, effects);
                }
            }
            LetGo::Rest(key) => {
                let e = edit(self);
                self.rest_with(key, e, effects);
            }
            LetGo::Written | LetGo::Nothing => {}
        }
        if let Some((key, _)) = lane.filter(|_| held) {
            self.hold_lane(key.0, key.1, false, effects);
        }
    }

    /// The lane that would move the control of `value`, and the place of its knob.
    fn preview_lane(&self, value: &Preview) -> Option<(LaneKey, f32)> {
        match *value {
            Preview::Gain(strip, gain) => Self::gain_lane(strip, gain),
            Preview::Pan(strip, pan) => Self::pan_lane(strip, pan),
            Preview::Send(track, bus, gain) => Self::send_lane(track, bus, gain),
            Preview::Param {
                strip,
                slot,
                kind,
                param,
                value,
            } => self.param_lane(strip, slot, kind, param, value),
            Preview::Knob { track, knob, value } => Self::knob_lane(track, knob, value),
        }
    }

    pub fn set_gain(
        &mut self,
        strip: StripRef,
        db: Option<f32>,
        commit: bool,
        effects: &mut Vec<Effect>,
    ) {
        let (strip, gain) = (strip_id(strip), gain_of(db));
        if self.doc().channel(strip).is_none() {
            return;
        }
        let edit = |p: &Play| mixer::set_gain(p.doc(), strip, gain);
        self.mix(Preview::Gain(strip, gain), commit, false, edit, effects);
    }

    pub fn set_pan(&mut self, strip: StripRef, pan: i32, commit: bool, effects: &mut Vec<Effect>) {
        let (strip, pan) = (strip_id(strip), Pan::new(pan));
        if self.doc().channel(strip).is_none() {
            return;
        }
        let edit = |p: &Play| mixer::set_pan(p.doc(), strip, pan);
        self.mix(Preview::Pan(strip, pan), commit, false, edit, effects);
    }

    pub fn set_send(
        &mut self,
        track: u32,
        bus: SendBus,
        db: Option<f32>,
        commit: bool,
        effects: &mut Vec<Effect>,
    ) {
        let (track, bus, gain) = (TrackId(track), bus_of(bus), gain_of(db));
        if self.doc().track(track).is_none() {
            return;
        }
        let edit = |p: &Play| mixer::set_send(p.doc(), track, bus, gain);
        self.mix(
            Preview::Send(track, bus, gain),
            commit,
            false,
            edit,
            effects,
        );
    }

    pub fn set_param(
        &mut self,
        insert: InsertRef,
        effect: EffectKind,
        param: u32,
        value: f32,
        commit: bool,
        effects: &mut Vec<Effect>,
    ) {
        let (strip, slot, kind) = (
            strip_id(insert.strip),
            usize::from(insert.slot),
            model_effect(effect),
        );
        let there = self.doc().insert(strip, slot).and_then(|i| i.kind());
        let (Some(value), true) = (kind.clamp(param, value), there == Some(kind)) else {
            return;
        };
        let preview = Preview::Param {
            strip,
            slot,
            kind,
            param,
            value,
        };
        let edit = |p: &Play| mixer::set_param(p.doc(), strip, slot, kind, param, value);
        self.mix(preview, commit, false, edit, effects);
    }

    /// E3 `SetSoundKnob`, as the mixer's knobs. Dropped for a track whose sound is not
    /// `instrument` now, and for a knob that sound does not use.
    pub fn set_sound_knob(
        &mut self,
        track: u32,
        instrument: InstrumentKind,
        knob: meloet_protocol::SoundKnob,
        value: f32,
        commit: bool,
        effects: &mut Vec<Effect>,
    ) {
        let (track, knob) = (TrackId(track), model_knob(knob));
        let Some(t) = self.doc().track(track) else {
            return;
        };
        let InstrumentRef::Builtin(sound) = &t.instrument else {
            return;
        };
        if kind_of(*sound) != instrument || !knob_usable(&t.instrument, knob) || value.is_nan() {
            return;
        }
        let value = value.clamp(0.0, 100.0);
        let edit = |p: &Play| knobs::set_sound_knob(p.doc(), track, knob, value);
        self.mix(
            Preview::Knob { track, knob, value },
            commit,
            false,
            edit,
            effects,
        );
    }

    /// Esc during a drag: the sound is the song's again.
    pub fn cancel_mix_preview(&mut self, effects: &mut Vec<Effect>) {
        self.mixer.previews.clear();
        self.cancel_hands(effects);
        self.cancel_plugin_drags(effects);
    }

    /// M2 「全部のトラックを N dB 下げる」: one step (`mixer::lower_all_tracks`); nothing when no
    /// track changes or `db` is not above 0.
    pub fn lower_all_tracks(&mut self, db: f32, effects: &mut Vec<Effect>) {
        if let Ok(e) = mixer::lower_all_tracks(self.doc(), f64::from(db)) {
            self.apply(e, effects);
        }
    }

    pub fn toggle_return_mute(&mut self, bus: SendBus, effects: &mut Vec<Effect>) {
        let e = mixer::toggle_return_mute(self.doc(), bus_of(bus));
        self.apply(e, effects);
    }

    /// `RemoveEffect`. Returns what the status line says of it.
    pub fn remove_effect(
        &mut self,
        insert: InsertRef,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let (strip, slot) = (strip_id(insert.strip), usize::from(insert.slot));
        let effect = effect_name(self.doc().insert(strip, slot)?);
        let e = mixer::remove_effect(self.doc(), strip, slot).ok()?;
        let lanes = self.lane_count();
        self.apply(e, effects);
        Some(StatusMessage::EffectRemoved {
            insert: insert_ref(strip, slot),
            effect,
            lanes_removed: lanes.saturating_sub(self.lane_count()),
        })
    }

    /// `ToggleBypass`. A plugin that stopped stays as it is: the sound passes it by already.
    pub fn toggle_bypass(
        &mut self,
        insert: InsertRef,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let (strip, slot) = (strip_id(insert.strip), usize::from(insert.slot));
        let held = self.doc().insert(strip, slot)?;
        let place = PluginPlace::Insert(strip, slot);
        if held.plugin_ref().is_some() && self.plugin_state(place) == PluginState::Stopped {
            return None;
        }
        let effect = effect_name(held);
        let e = mixer::toggle_bypass(self.doc(), strip, slot).ok()?;
        let bypassed = matches!(e.step, StepKind::Bypass { bypassed: true, .. });
        self.apply(e, effects);
        Some(StatusMessage::EffectBypassed {
            insert: insert_ref(strip, slot),
            effect,
            bypassed,
        })
    }

    /// `MoveInsert`. What names a slot of the strip follows in [`Play::inserts_follow`].
    pub fn move_insert(
        &mut self,
        insert: InsertRef,
        to: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let (strip, slot) = (strip_id(insert.strip), usize::from(insert.slot));
        let e = mixer::move_insert(self.doc(), strip, slot, usize::from(to)).ok()?;
        self.apply(e, effects);
        let inserts = &self.doc().channel(strip)?.inserts;
        Some(StatusMessage::InsertsReordered {
            strip: strip_ref(strip),
            order: inserts.iter().flatten().map(effect_name).collect(),
        })
    }

    /// `TypeParam`. An effect that comes with meloet is read here; a plugin reads its own
    /// values, and what the status line says comes with its answer.
    pub fn type_param(
        &mut self,
        param: ParamRef,
        text: &str,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let (insert, effect, param) = match param {
            ParamRef::Builtin {
                insert,
                effect,
                param,
            } => (insert, effect, param),
            ParamRef::Plugin { slot, table, param } => {
                let text: String = (text.chars())
                    .map(meloet_model::effects::plain_number_char)
                    .collect();
                self.type_plugin_param(slot, table, param, &text, effects);
                return None;
            }
        };
        let (strip, slot, kind) = (
            strip_id(insert.strip),
            usize::from(insert.slot),
            model_effect(effect),
        );
        let held = (self.doc().insert(strip, slot)).filter(|i| i.kind() == Some(kind))?;
        let (spec, now) = (kind.param(param)?, held.value(param)?);
        let Some(typed) = spec.read(text) else {
            return Some(StatusMessage::ParamRejected {
                param: ParamRef::Builtin {
                    insert: insert_ref(strip, slot),
                    effect,
                    param,
                },
                kept: now.to_string(),
            });
        };
        let value = spec.clamp(typed);
        let preview = Preview::Param {
            strip,
            slot,
            kind,
            param,
            value,
        };
        let edit = |p: &Play| mixer::set_param(p.doc(), strip, slot, kind, param, value);
        self.mix(preview, true, true, edit, effects);
        (typed < spec.min || typed > spec.max).then_some(StatusMessage::ParamClamped {
            to: ParamEnd::Builtin {
                effect,
                param,
                value,
            },
        })
    }

    /// Puts `kind` into an empty slot and selects it. Returns whether it went in.
    pub(super) fn add_effect(
        &mut self,
        strip: StripId,
        slot: usize,
        kind: ModelEffect,
        effects: &mut Vec<Effect>,
    ) -> bool {
        let Ok(e) = mixer::add_effect(self.doc(), strip, slot, kind) else {
            return false;
        };
        self.apply(e, effects);
        self.pick(PluginPlace::Insert(strip, slot));
        true
    }

    /// Puts a plugin, as the shelf has it, into an empty slot and selects it. Returns
    /// whether it went in. It is loaded then, and the slot lets the sound through until it
    /// is.
    pub(super) fn add_plugin_effect(
        &mut self,
        strip: StripId,
        slot: usize,
        plugin: Arc<PluginRef>,
        effects: &mut Vec<Effect>,
    ) -> bool {
        let Ok(e) = mixer::add_plugin_effect(self.doc(), strip, slot, plugin) else {
            return false;
        };
        self.apply(e, effects);
        self.pick(PluginPlace::Insert(strip, slot));
        true
    }

    /// Puts `with` in place of what the slot holds, and selects it. Returns whether it did.
    pub(super) fn replace_effect(
        &mut self,
        strip: StripId,
        slot: usize,
        with: &super::shelf::Chosen,
        effects: &mut Vec<Effect>,
    ) -> bool {
        use super::shelf::Chosen;
        let edit = match with {
            Chosen::Builtin(kind) => mixer::replace_effect(self.doc(), strip, slot, *kind),
            Chosen::Plugin(plugin) => {
                mixer::replace_plugin_effect(self.doc(), strip, slot, plugin.clone())
            }
        };
        let Ok(e) = edit else {
            return false;
        };
        self.apply(e, effects);
        self.pick(PluginPlace::Insert(strip, slot));
        true
    }

    /// What is at `place` now, as the panel can show it.
    fn picked_at(&self, place: PluginPlace) -> Option<Picked> {
        if let PluginPlace::Insert(strip, slot) = place
            && let Some(kind) = self.doc().insert(strip, slot)?.kind()
        {
            return Some(Picked::Effect(strip, slot, kind));
        }
        if let Some(plugin) = self.doc().plugin(place) {
            return Some(Picked::Plugin(place, plugin.key.clone()));
        }
        let PluginPlace::Instrument(id) = place else {
            return None;
        };
        let track = self.doc().track(id)?;
        (!track.kind.is_vocal()).then(|| Picked::Sound(id, track.instrument.clone()))
    }

    /// Makes what is at `place` the selected one; nothing is selected when nothing is there.
    fn pick(&mut self, place: PluginPlace) {
        let picked = self.picked_at(place);
        if picked != self.mixer.selected {
            self.mixer.selected = picked;
            self.mixer.row = None;
        }
    }

    /// What comes with the selection as it is now (spec 3 「パラメータ」).
    fn selecting(&self) -> Selecting {
        match &self.mixer.selected {
            None => Selecting::default(),
            Some(Picked::Effect(..)) => Selecting {
                panel: true,
                ..Selecting::default()
            },
            Some(Picked::Sound(_, instrument)) => Selecting {
                panel: true,
                notice: (!matches!(instrument, InstrumentRef::Builtin(_))).then(|| {
                    StatusMessage::NoParams {
                        sound: sound_name(instrument),
                    }
                }),
                ..Selecting::default()
            },
            Some(Picked::Plugin(place, key)) => {
                if self.plugin_state(*place) != PluginState::Ready {
                    return Selecting::default();
                }
                match self.window_kind(*place, key) {
                    PluginWindowKind::Opens => Selecting {
                        window: Some(Self::slot_ref(*place)),
                        ..Selecting::default()
                    },
                    PluginWindowKind::None => Selecting {
                        panel: true,
                        ..Selecting::default()
                    },
                    PluginWindowKind::NotYet => Selecting {
                        panel: true,
                        notice: self.doc().plugin(*place).map(|p| {
                            StatusMessage::PluginWindowNotYet {
                                loop_bars: self.loop_on().map(|r| self.bars(r)),
                                name: p.name.to_string(),
                            }
                        }),
                        window: None,
                    },
                }
            }
        }
    }

    /// Whether the parameters panel is to come to the front for what is selected now.
    pub fn selection_wants_panel(&self) -> bool {
        self.selecting().panel
    }

    /// An insert moved inside its strip: what is selected stays with its effect.
    pub(super) fn selection_follows(&mut self, follow: impl Fn(PluginPlace) -> PluginPlace) {
        self.mixer.selected = match self.mixer.selected.take() {
            Some(Picked::Effect(strip, slot, kind)) => {
                match follow(PluginPlace::Insert(strip, slot)) {
                    PluginPlace::Insert(strip, slot) => Some(Picked::Effect(strip, slot, kind)),
                    PluginPlace::Instrument(_) => None,
                }
            }
            Some(Picked::Plugin(place, key)) => Some(Picked::Plugin(follow(place), key)),
            sound @ Some(Picked::Sound(..)) => sound,
            None => None,
        };
    }

    /// The insert slots of a strip, each plugin as it is now.
    pub fn slots_of(&self, strip: StripId, inserts: &Inserts) -> SlotsView {
        slots_view(inserts, |slot| {
            self.plugin_state(PluginPlace::Insert(strip, slot))
        })
    }

    /// `SelectInsert`: an effect of meloet or a plugin. `None`, or an empty slot: nothing
    /// is selected.
    pub fn select_insert(&mut self, insert: Option<InsertRef>) -> Selecting {
        match insert {
            Some(i) => self.pick(PluginPlace::Insert(strip_id(i.strip), usize::from(i.slot))),
            None => {
                self.mixer.selected = None;
                self.mixer.row = None;
            }
        }
        self.selecting()
    }

    /// `SelectInstrument`: a plugin, or a sound with no parameters, which the panel and the
    /// status line then say (spec 3, 19-A). What is selected stays for a track that is gone
    /// and for a vocal track.
    pub fn select_instrument(&mut self, track: u32) -> Selecting {
        let place = PluginPlace::Instrument(TrackId(track));
        if self.picked_at(place).is_none() {
            return Selecting::default();
        }
        self.pick(place);
        self.selecting()
    }

    /// `SelectParam`: a place in the table of an effect of meloet, or the plugin's own
    /// number for one of the parameters it lists.
    pub fn select_param(&mut self, param: Option<u32>) {
        self.mixer.row = match &self.mixer.selected {
            Some(Picked::Effect(_, _, kind)) => param.filter(|&p| kind.param(p).is_some()),
            Some(Picked::Plugin(..)) => param.filter(|&p| self.has_param(p)),
            Some(Picked::Sound(_, instrument)) => param
                .filter(|&p| SoundKnob::from_index(p).is_some_and(|k| knob_usable(instrument, k))),
            None => None,
        };
    }

    pub(super) fn selected_pick(&self) -> Option<&Picked> {
        self.mixer.selected.as_ref()
    }

    pub(super) fn mixer_row(&self) -> Option<u32> {
        self.mixer.row
    }

    /// The selected insert, as the screen names it; `None` also while an instrument is
    /// selected.
    pub fn selected_insert(&self) -> Option<InsertRef> {
        match self.mixer.selected.as_ref()?.place() {
            PluginPlace::Insert(strip, slot) => Some(insert_ref(strip, slot)),
            PluginPlace::Instrument(_) => None,
        }
    }

    /// The song changed, or another one took its place: previews are over, and what is
    /// selected stays only as far as it is still there.
    pub(super) fn mixer_changed(&mut self) {
        self.mixer.previews.clear();
        if let Some(picked) = self.mixer.selected.take() {
            let now = self.picked_at(picked.place());
            if now.as_ref() != Some(&picked) {
                self.mixer.row = None;
            }
            self.mixer.selected = now;
        }
    }

    pub(super) fn forget_mixer(&mut self) {
        self.mixer = Mixer::default();
    }

    pub fn params_view(&self) -> ParamsView {
        let selected = self
            .mixer
            .selected
            .as_ref()
            .and_then(|picked| match picked {
                Picked::Effect(strip, slot, _) => {
                    let insert = self.doc().insert(*strip, *slot)?;
                    let kind = insert.kind()?;
                    Some(SelectedParams::Builtin(SelectedInsert {
                        insert: insert_ref(*strip, *slot),
                        effect: effect_kind(kind),
                        values: insert.values().to_vec(),
                        row: self.mixer.row,
                        preset: self.preset_of(kind, insert.values()),
                    }))
                }
                Picked::Plugin(place, key) => {
                    let table = self.table_shown();
                    Some(SelectedParams::Plugin(SelectedPlugin {
                        slot: Self::slot_ref(*place),
                        window: self.window_kind(*place, key),
                        table: table.map(|(number, _)| number),
                        count: table.map_or(0, |(_, count)| count),
                        row: self.mixer.row.filter(|&row| self.has_param(row)),
                        automated: self.automated_rows(*place),
                    }))
                }
                Picked::Sound(track, instrument) => Some(SelectedParams::Sound(SelectedSound {
                    track: track.0,
                    sound: sound_name(instrument),
                    knobs: (self.doc().track(*track)).and_then(|t| knobs_view(t, self.mixer.row)),
                })),
            });
        ParamsView {
            selected,
            clips: None,
        }
    }

    pub fn returns_view(&self) -> [ReturnView; 2] {
        Bus::ALL.map(|bus| {
            let r = &self.doc().returns[bus.index()];
            ReturnView {
                gain_db: gain_view(r.channel.gain),
                pan: r.channel.pan.get(),
                muted: r.muted,
                slots: self.slots_of(StripId::Return(bus), &r.channel.inserts),
            }
        })
    }

    pub fn master_view(&self) -> MasterView {
        let master = &self.doc().master;
        MasterView {
            gain_db: gain_view(master.gain),
            pan: master.pan.get(),
            slots: self.slots_of(StripId::Master, &master.inserts),
            lanes: super::wave3::master_lanes_view(self.doc()),
        }
    }

    /// The status line while a row of the parameters panel is chosen. Not while counting in
    /// or recording, which have their own line.
    pub fn param_status(&self) -> Option<StatusMessage> {
        let param = self.mixer.row?;
        if self.rec.is_some() {
            return None;
        }
        let loop_bars = self.loop_on().map(|r| self.bars(r));
        Some(match self.mixer.selected.as_ref()? {
            Picked::Sound(track, _) => StatusMessage::SoundKnobSelected {
                loop_bars,
                track: self.track_ref(*track)?,
                knob: knob_view(SoundKnob::from_index(param)?),
            },
            Picked::Effect(strip, slot, kind) => StatusMessage::ParamSelected {
                loop_bars,
                insert: insert_ref(*strip, *slot),
                effect: effect_kind(*kind),
                param,
            },
            Picked::Plugin(place, _) => StatusMessage::PluginParamSelected {
                loop_bars,
                slot: Self::slot_ref(*place),
                plugin: self.doc().plugin(*place)?.name.to_string(),
                param: self.param_name(param)?.to_owned(),
            },
        })
    }

    /// What the mixer says once as it opens (docs/history/design-notes.ja.md A-17): the slots that are passed
    /// by and the ones whose plugin stopped, each in the mixer's order. `None` when there
    /// are none. A slot that is both counts as stopped.
    pub fn mixer_passes_by(&self) -> Option<StatusMessage> {
        let doc = self.doc();
        let strips = (doc.tracks.iter())
            .map(|t| (StripId::Track(t.id), &t.channel))
            .chain(Bus::ALL.map(|bus| (StripId::Return(bus), &doc.returns[bus.index()].channel)))
            .chain([(StripId::Master, &*doc.master)]);
        let (mut bypassed, mut stopped) = (Vec::new(), Vec::new());
        for (strip, channel) in strips {
            for (slot, insert) in channel.inserts.iter().enumerate() {
                let Some(insert) = insert else {
                    continue;
                };
                let named = NamedInsert {
                    insert: insert_ref(strip, slot),
                    effect: effect_name(insert),
                };
                let state = (insert.plugin_ref())
                    .map(|_| self.plugin_state(PluginPlace::Insert(strip, slot)));
                if state == Some(PluginState::Stopped) {
                    stopped.push(named);
                } else if insert.bypassed {
                    bypassed.push(named);
                }
            }
        }
        (!bypassed.is_empty() || !stopped.is_empty())
            .then_some(StatusMessage::MixerPassesBy { bypassed, stopped })
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod knob_tests;
