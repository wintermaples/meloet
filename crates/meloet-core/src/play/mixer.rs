//! The mixer: faders, pans, sends, the effects in the insert slots and their parameters, and
//! the insert the parameters panel shows.
//!
//! A drag is heard while it goes on and is one step when it ends: until then its value is a
//! preview, kept here and not in the song. What the engine hears of it is `mixsync.rs`.
//! All of it works while counting in or recording too: nothing here moves where a take is
//! recorded.

use meloet_model::effects::EffectKind as ModelEffect;
use meloet_model::{Bus, Gain, INSERT_SLOTS, Inserts, Pan, StripId, TrackId, mixer};
use meloet_protocol::{
    EffectKind, InsertRef, InsertsView, MasterView, ParamsView, PluginState, ReturnView,
    SelectedInsert, SendBus, StatusMessage, StripRef,
};

use super::Play;
use crate::added::slots_view;
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
            _ => false,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct Mixer {
    previews: Vec<Preview>,
    /// The insert the parameters panel shows, with the effect it held when it was selected.
    selected: Option<(StripId, usize, ModelEffect)>,
    /// The chosen row of the panel.
    row: Option<u32>,
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

pub(super) fn model_effect(kind: EffectKind) -> ModelEffect {
    match kind {
        EffectKind::Eq4 => ModelEffect::Eq4,
        EffectKind::Compressor => ModelEffect::Compressor,
        EffectKind::Reverb => ModelEffect::Reverb,
        EffectKind::Delay => ModelEffect::Delay,
        EffectKind::Chorus => ModelEffect::Chorus,
        EffectKind::Limiter => ModelEffect::Limiter,
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
    }
}

fn gain_of(db: Option<f32>) -> Gain {
    db.map_or(Gain::SILENT, |db| Gain::from_db(f64::from(db)))
}

/// `None` is −∞.
pub(crate) fn gain_view(gain: Gain) -> Option<f32> {
    gain.db().map(|db| db as f32)
}

pub(crate) fn inserts_view(inserts: &Inserts) -> InsertsView {
    let mut view = [None; INSERT_SLOTS];
    for (to, from) in view.iter_mut().zip(inserts) {
        *to = from.as_ref().and_then(|i| i.kind()).map(effect_kind);
    }
    view
}

impl Play {
    /// The values that sound in place of the song's, one for each control being dragged.
    pub fn mix_previews(&self) -> &[Preview] {
        &self.mixer.previews
    }

    /// Takes the control of `value` as previewed (`commit` false) or as part of the song.
    /// `edit`: the step that puts the value in the song.
    fn mix(
        &mut self,
        value: Preview,
        commit: bool,
        edit: impl FnOnce(&Self) -> Result<meloet_model::Edit, meloet_model::EditError>,
        effects: &mut Vec<Effect>,
    ) {
        if !commit {
            match (self.mixer.previews.iter_mut()).find(|p| p.same_control(&value)) {
                Some(p) => *p = value,
                None => self.mixer.previews.push(value),
            }
            return;
        }
        self.mixer.previews.retain(|p| !p.same_control(&value));
        if let Ok(e) = edit(self) {
            self.apply(e, effects);
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
        self.mix(Preview::Gain(strip, gain), commit, edit, effects);
    }

    pub fn set_pan(&mut self, strip: StripRef, pan: i32, commit: bool, effects: &mut Vec<Effect>) {
        let (strip, pan) = (strip_id(strip), Pan::new(pan));
        if self.doc().channel(strip).is_none() {
            return;
        }
        let edit = |p: &Play| mixer::set_pan(p.doc(), strip, pan);
        self.mix(Preview::Pan(strip, pan), commit, edit, effects);
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
        self.mix(Preview::Send(track, bus, gain), commit, edit, effects);
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
        self.mix(preview, commit, edit, effects);
    }

    /// Esc during a drag: the sound is the song's again.
    pub fn cancel_mix_preview(&mut self) {
        self.mixer.previews.clear();
    }

    pub fn toggle_return_mute(&mut self, bus: SendBus, effects: &mut Vec<Effect>) {
        let e = mixer::toggle_return_mute(self.doc(), bus_of(bus));
        self.apply(e, effects);
    }

    pub fn remove_effect(&mut self, insert: InsertRef, effects: &mut Vec<Effect>) {
        let (strip, slot) = (strip_id(insert.strip), usize::from(insert.slot));
        if let Ok(e) = mixer::remove_effect(self.doc(), strip, slot) {
            self.apply(e, effects);
        }
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
        self.mixer.selected = Some((strip, slot, kind));
        self.mixer.row = None;
        true
    }

    /// `SelectInsert`. Returns whether an insert is selected now: the parameters panel is
    /// then to be shown.
    pub fn select_insert(&mut self, insert: Option<InsertRef>) -> bool {
        let picked = insert.and_then(|i| {
            let (strip, slot) = (strip_id(i.strip), usize::from(i.slot));
            let kind = self.doc().insert(strip, slot)?.kind()?;
            Some((strip, slot, kind))
        });
        if picked != self.mixer.selected {
            self.mixer.selected = picked;
            self.mixer.row = None;
        }
        picked.is_some()
    }

    pub fn select_param(&mut self, param: Option<u32>) {
        let Some((_, _, kind)) = self.mixer.selected else {
            return;
        };
        self.mixer.row = param.filter(|&p| kind.param(p).is_some());
    }

    /// The selected insert, as the screen names it.
    pub fn selected_insert(&self) -> Option<InsertRef> {
        let (strip, slot, _) = self.mixer.selected?;
        Some(InsertRef {
            strip: strip_ref(strip),
            slot: slot as u8,
        })
    }

    /// The song changed, or another one took its place: previews are over, and what is
    /// selected stays only as far as it is still there.
    pub(super) fn mixer_changed(&mut self) {
        self.mixer.previews.clear();
        if let Some((strip, slot, kind)) = self.mixer.selected {
            let now = self.doc().insert(strip, slot).and_then(|i| i.kind());
            self.mixer.selected = now.map(|k| (strip, slot, k));
            if now != Some(kind) {
                self.mixer.row = None;
            }
        }
    }

    pub(super) fn forget_mixer(&mut self) {
        self.mixer = Mixer::default();
    }

    pub fn params_view(&self) -> ParamsView {
        let selected = self.mixer.selected.and_then(|(strip, slot, _)| {
            let insert = self.doc().insert(strip, slot)?;
            Some(SelectedInsert {
                insert: InsertRef {
                    strip: strip_ref(strip),
                    slot: slot as u8,
                },
                effect: effect_kind(insert.kind()?),
                values: insert.values().to_vec(),
                row: self.mixer.row,
            })
        });
        ParamsView { selected }
    }

    pub fn returns_view(&self) -> [ReturnView; 2] {
        Bus::ALL.map(|bus| {
            let r = &self.doc().returns[bus.index()];
            ReturnView {
                gain_db: gain_view(r.channel.gain),
                pan: r.channel.pan.get(),
                muted: r.muted,
                inserts: inserts_view(&r.channel.inserts),
                slots: slots_view(&r.channel.inserts, |_| PluginState::Missing),
            }
        })
    }

    pub fn master_view(&self) -> MasterView {
        let master = &self.doc().master;
        MasterView {
            gain_db: gain_view(master.gain),
            pan: master.pan.get(),
            inserts: inserts_view(&master.inserts),
            slots: slots_view(&master.inserts, |_| PluginState::Missing),
        }
    }

    /// The status line while a row of the parameters panel is chosen. Not while counting in
    /// or recording, which have their own line.
    pub fn param_status(&self) -> Option<StatusMessage> {
        let (strip, slot, kind) = self.mixer.selected?;
        let param = self.mixer.row?;
        if self.rec.is_some() {
            return None;
        }
        Some(StatusMessage::ParamSelected {
            loop_bars: self.loop_on().map(|r| self.bars(r)),
            insert: InsertRef {
                strip: strip_ref(strip),
                slot: slot as u8,
            },
            effect: effect_kind(kind),
            param,
        })
    }
}

#[cfg(test)]
mod tests;
