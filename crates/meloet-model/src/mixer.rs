//! The mixer: what each strip holds (fader, pan, inserts, sends), the two returns and the
//! master, and the steps that change them (spec 3 「センド」, docs/architecture.md 19-3).

use std::sync::Arc;

use crate::automation::drop_orphan_lanes;
use crate::edit::{Edit, EditError, Ids, track_index};
use crate::effects::{EffectKind, delay, reverb};
use crate::history::StepKind;
use crate::instrument::{InstrumentGroup, PluginRef};
use crate::song::{Doc, Track};
use crate::{InstrumentRef, TrackId};

/// The insert slots of a strip (mock F4-Mixer).
pub const INSERT_SLOTS: usize = 4;

/// A fader or send level: −∞, or −60.00 to +6.00 dB in steps of 0.01 (spec 2 「メーターと
/// フェーダーの目盛」).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Gain(f64);

impl Gain {
    pub const MAX_DB: f64 = 6.0;
    pub const MIN_DB: f64 = -60.0;
    pub const UNITY: Gain = Gain(0.0);
    pub const SILENT: Gain = Gain(f64::NEG_INFINITY);

    /// Below −60 dB (and what is not a number) is −∞; above +6 dB is +6 dB.
    pub fn from_db(db: f64) -> Gain {
        let db = (db * 100.0).round() / 100.0;
        if db.is_nan() || db < Self::MIN_DB {
            Self::SILENT
        } else {
            Gain(db.min(Self::MAX_DB))
        }
    }

    /// `None` is −∞.
    pub fn db(self) -> Option<f64> {
        self.0.is_finite().then_some(self.0)
    }

    /// What the samples are multiplied by.
    pub fn linear(self) -> f32 {
        self.db().map_or(0.0, |db| 10f64.powf(db / 20.0) as f32)
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self::UNITY
    }
}

/// −100 (all left) to 100 (all right); 0 is the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Pan(i8);

impl Pan {
    pub const CENTER: Pan = Pan(0);

    pub fn new(value: i32) -> Pan {
        Pan(value.clamp(-100, 100) as i8)
    }

    pub fn get(self) -> i8 {
        self.0
    }
}

/// The mix, in percent, a reverb or a delay starts with on a track or on the master.
pub const INSERT_MIX: f32 = 30.0;

/// Names what was put into an insert slot, from `Doc::next_id`. It stays with the effect when
/// the slots of its strip are put in another order; what replaces it has another one.
/// Automation names the insert by it (docs/architecture.md 21-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InsertId(pub u32);

/// What sits in an insert slot: an effect that comes with meloet, with one value for every
/// parameter of its table, each inside its range, or a plugin with its state.
#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    pub id: InsertId,
    effect: InsertEffect,
    /// 「とばす」 (spec 3, 13-A): the sound passes the slot by. The values stay.
    pub bypassed: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum InsertEffect {
    Builtin { kind: EffectKind, values: Vec<f32> },
    Plugin(Arc<PluginRef>),
}

impl Insert {
    /// With the defaults of the table.
    pub fn new(id: InsertId, kind: EffectKind) -> Insert {
        Insert {
            id,
            effect: InsertEffect::Builtin {
                kind,
                values: kind.params().iter().map(|p| p.default).collect(),
            },
            bypassed: false,
        }
    }

    pub fn plugin(id: InsertId, plugin: Arc<PluginRef>) -> Insert {
        Insert {
            id,
            effect: InsertEffect::Plugin(plugin),
            bypassed: false,
        }
    }

    /// As it starts in an empty slot of `strip`. A reverb or a delay on a track or on the
    /// master, where the sound itself goes through it, starts with its mix at
    /// [`INSERT_MIX`]; on a return, which carries only what was sent, it is all effect, as
    /// its table says.
    pub fn new_on(id: InsertId, kind: EffectKind, strip: StripId) -> Insert {
        let mut insert = Insert::new(id, kind);
        let mix = match kind {
            EffectKind::Reverb => Some(reverb::MIX),
            EffectKind::Delay => Some(delay::MIX),
            _ => None,
        };
        if let (Some(mix), StripId::Track(_) | StripId::Master) = (mix, strip) {
            insert.set(mix, INSERT_MIX);
        }
        insert
    }

    /// `None` for a plugin.
    pub fn kind(&self) -> Option<EffectKind> {
        match &self.effect {
            InsertEffect::Builtin { kind, .. } => Some(*kind),
            InsertEffect::Plugin(_) => None,
        }
    }

    /// `None` for an effect that comes with meloet.
    pub fn plugin_ref(&self) -> Option<&Arc<PluginRef>> {
        match &self.effect {
            InsertEffect::Builtin { .. } => None,
            InsertEffect::Plugin(plugin) => Some(plugin),
        }
    }

    /// In the order of the table. Empty for a plugin: its values are in its state.
    pub fn values(&self) -> &[f32] {
        match &self.effect {
            InsertEffect::Builtin { values, .. } => values,
            InsertEffect::Plugin(_) => &[],
        }
    }

    pub fn value(&self, param: u32) -> Option<f32> {
        self.values().get(param as usize).copied()
    }

    /// Sets a parameter, brought into its range. `false` when the effect has no such
    /// parameter, and for a plugin.
    pub fn set(&mut self, param: u32, value: f32) -> bool {
        let InsertEffect::Builtin { kind, values } = &mut self.effect else {
            return false;
        };
        match kind.clamp(param, value) {
            Some(v) => {
                values[param as usize] = v;
                true
            }
            None => false,
        }
    }
}

/// Where a plugin sits in the song: as the instrument of a track, or in an insert slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginPlace {
    Instrument(TrackId),
    Insert(StripId, usize),
}

/// Why the state of a plugin changed, for the step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginChange {
    /// Whatever was changed while the person was in the plugin's window (spec 3, 15-C).
    Window,
    /// One parameter, moved in the parameters panel; the plugin's own number for it.
    Param(u32),
}

/// Sound goes through the slots from the first to the last (spec 5 「インサート」).
pub type Inserts = [Option<Insert>; INSERT_SLOTS];

/// What every strip has.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Channel {
    pub gain: Gain,
    pub pan: Pan,
    pub inserts: Inserts,
}

impl Channel {
    fn with_first(id: InsertId, kind: EffectKind) -> Channel {
        let mut channel = Channel::default();
        channel.inserts[0] = Some(Insert::new(id, kind));
        channel
    }
}

/// The two sends and the returns they go to. There are always these two (spec 3 Y9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Bus {
    A,
    B,
}

impl Bus {
    pub const ALL: [Bus; 2] = [Bus::A, Bus::B];

    pub fn index(self) -> usize {
        self as usize
    }
}

/// How much of a track goes to return A and to return B, taken after its fader and pan.
pub type Sends = [Gain; 2];

#[derive(Debug, Clone, PartialEq)]
pub struct Return {
    pub channel: Channel,
    /// The return is silent. Saved with the song.
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StripId {
    Track(TrackId),
    Return(Bus),
    Master,
}

/// The sends of a track that is new (spec 3 Y9): piano and keys, pads and strings, and
/// synths send −15 dB to A, drums −24 dB, basses nothing. Nothing goes to B. Guitars and wind
/// instruments are not in Y9; they send as the piano does. An SF2 preset and a plugin send as
/// their group does ([`InstrumentRef::group`]).
pub fn new_track_sends(instrument: &InstrumentRef) -> Sends {
    let a = match instrument.group() {
        InstrumentGroup::PianoKeys
        | InstrumentGroup::Guitar
        | InstrumentGroup::PadStrings
        | InstrumentGroup::Wind
        | InstrumentGroup::Synth => Gain::from_db(-15.0),
        InstrumentGroup::Drums => Gain::from_db(-24.0),
        InstrumentGroup::Bass => Gain::SILENT,
    };
    [a, Gain::SILENT]
}

/// The `next_id` of a new song: its three inserts ([`new_song_returns`], [`new_song_master`])
/// have the ids 1, 2 and 3.
pub const NEW_SONG_NEXT_ID: u32 = 4;

/// The returns of a new song: a reverb on A and a delay on B.
pub fn new_song_returns() -> [Arc<Return>; 2] {
    [(1, EffectKind::Reverb), (2, EffectKind::Delay)].map(|(id, kind)| {
        Arc::new(Return {
            channel: Channel::with_first(InsertId(id), kind),
            muted: false,
        })
    })
}

/// The master of a new song: a limiter, whose default ceiling is −0.3 dB.
pub fn new_song_master() -> Arc<Channel> {
    Arc::new(Channel::with_first(InsertId(3), EffectKind::Limiter))
}

/// The id of an insert put in now, and the `next_id` after it.
fn new_insert_id(doc: &Doc) -> Result<(InsertId, u32), EditError> {
    let mut ids = Ids(doc.next_id);
    Ok((InsertId(ids.next()?), ids.0))
}

impl Doc {
    pub fn channel(&self, strip: StripId) -> Option<&Channel> {
        match strip {
            StripId::Track(id) => self.track(id).map(|t| &t.channel),
            StripId::Return(bus) => Some(&self.returns[bus.index()].channel),
            StripId::Master => Some(&self.master),
        }
    }

    pub fn insert(&self, strip: StripId, slot: usize) -> Option<&Insert> {
        self.channel(strip)?.inserts.get(slot)?.as_ref()
    }
}

/// A `Doc` whose strip `strip` is changed by `change`; the other strips stay shared.
fn with_channel(
    doc: &Doc,
    strip: StripId,
    change: impl FnOnce(&mut Channel) -> Result<(), EditError>,
) -> Result<Doc, EditError> {
    let mut new = doc.clone();
    match strip {
        StripId::Track(id) => {
            let ti = track_index(doc, id)?;
            let mut track = Track::clone(&doc.tracks[ti]);
            change(&mut track.channel)?;
            // What went out of a slot takes the lanes of its parameters along.
            drop_orphan_lanes(&mut track);
            new.tracks[ti] = Arc::new(track);
        }
        StripId::Return(bus) => {
            let mut ret = Return::clone(&doc.returns[bus.index()]);
            change(&mut ret.channel)?;
            new.returns[bus.index()] = Arc::new(ret);
        }
        StripId::Master => {
            let mut master = Channel::clone(&doc.master);
            change(&mut master)?;
            new.master = Arc::new(master);
        }
    }
    Ok(new)
}

fn unless_same<T: PartialEq>(place: &mut T, value: T) -> Result<(), EditError> {
    if *place == value {
        return Err(EditError::Unchanged);
    }
    *place = value;
    Ok(())
}

/// The fader.
pub fn set_gain(doc: &Doc, strip: StripId, gain: Gain) -> Result<Edit, EditError> {
    Ok(Edit {
        doc: with_channel(doc, strip, |c| unless_same(&mut c.gain, gain))?,
        step: StepKind::SetGain { strip },
    })
}

pub fn set_pan(doc: &Doc, strip: StripId, pan: Pan) -> Result<Edit, EditError> {
    Ok(Edit {
        doc: with_channel(doc, strip, |c| unless_same(&mut c.pan, pan))?,
        step: StepKind::SetPan { strip },
    })
}

pub fn set_send(doc: &Doc, track: TrackId, bus: Bus, gain: Gain) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let mut t = Track::clone(&doc.tracks[ti]);
    unless_same(&mut t.sends[bus.index()], gain)?;
    let mut new = doc.clone();
    new.tracks[ti] = Arc::new(t);
    Ok(Edit {
        doc: new,
        step: StepKind::SetSend { track, bus },
    })
}

/// Sets a parameter of the effect in `slot`, brought into its range. `kind` is the effect the
/// caller means: `NoSuchInsert` when the slot is empty or holds another effect, or the effect
/// has no parameter `param`.
pub fn set_param(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    kind: EffectKind,
    param: u32,
    value: f32,
) -> Result<Edit, EditError> {
    let new = with_channel(doc, strip, |c| {
        let insert = (c.inserts.get_mut(slot).and_then(Option::as_mut))
            .filter(|i| i.kind() == Some(kind))
            .ok_or(EditError::NoSuchInsert)?;
        let before = insert.value(param).ok_or(EditError::NoSuchInsert)?;
        insert.set(param, value);
        if insert.value(param) == Some(before) {
            return Err(EditError::Unchanged);
        }
        Ok(())
    })?;
    Ok(Edit {
        doc: new,
        step: StepKind::SetParam {
            strip,
            slot,
            kind,
            param,
        },
    })
}

/// Puts `kind`, as it starts on that strip ([`Insert::new_on`]), in an empty slot
/// (`SlotTaken` otherwise).
pub fn add_effect(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    kind: EffectKind,
) -> Result<Edit, EditError> {
    let (id, next_id) = new_insert_id(doc)?;
    let mut new = with_channel(doc, strip, |c| {
        let place = c.inserts.get_mut(slot).ok_or(EditError::NoSuchInsert)?;
        if place.is_some() {
            return Err(EditError::SlotTaken);
        }
        *place = Some(Insert::new_on(id, kind, strip));
        Ok(())
    })?;
    new.next_id = next_id;
    Ok(Edit {
        doc: new,
        step: StepKind::AddEffect { strip, slot, kind },
    })
}

/// Puts a plugin, as it is given, in an empty slot (`SlotTaken` otherwise).
pub fn add_plugin_effect(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    plugin: Arc<PluginRef>,
) -> Result<Edit, EditError> {
    let (id, next_id) = new_insert_id(doc)?;
    let mut new = with_channel(doc, strip, |c| {
        let place = c.inserts.get_mut(slot).ok_or(EditError::NoSuchInsert)?;
        if place.is_some() {
            return Err(EditError::SlotTaken);
        }
        *place = Some(Insert::plugin(id, plugin));
        Ok(())
    })?;
    new.next_id = next_id;
    Ok(Edit {
        doc: new,
        step: StepKind::AddPlugin {
            place: PluginPlace::Insert(strip, slot),
        },
    })
}

/// Empties the slot. The other slots stay where they are.
pub fn remove_effect(doc: &Doc, strip: StripId, slot: usize) -> Result<Edit, EditError> {
    let mut kind = None;
    let new = with_channel(doc, strip, |c| {
        let gone = c.inserts.get_mut(slot).and_then(Option::take);
        kind = Some(gone.ok_or(EditError::NoSuchInsert)?.kind());
        Ok(())
    })?;
    Ok(Edit {
        doc: new,
        step: match kind.expect("set when the slot held an effect") {
            Some(kind) => StepKind::RemoveEffect { strip, slot, kind },
            None => StepKind::RemovePlugin {
                place: PluginPlace::Insert(strip, slot),
            },
        },
    })
}

/// Flips 「とばす」 of the effect or plugin in `slot` (`NoSuchInsert` for an empty one).
pub fn toggle_bypass(doc: &Doc, strip: StripId, slot: usize) -> Result<Edit, EditError> {
    let mut bypassed = false;
    let new = with_channel(doc, strip, |c| {
        let insert =
            (c.inserts.get_mut(slot).and_then(Option::as_mut)).ok_or(EditError::NoSuchInsert)?;
        insert.bypassed = !insert.bypassed;
        bypassed = insert.bypassed;
        Ok(())
    })?;
    Ok(Edit {
        doc: new,
        step: StepKind::Bypass {
            strip,
            slot,
            bypassed,
        },
    })
}

/// Where the slot `slot` of a strip is once its slot `from` went to `to`: the slots between
/// the two move by one, towards where `from` was.
pub fn moved_slot(slot: usize, from: usize, to: usize) -> usize {
    if slot == from {
        to
    } else if from < to && (from + 1..=to).contains(&slot) {
        slot - 1
    } else if to < from && (to..from).contains(&slot) {
        slot + 1
    } else {
        slot
    }
}

/// Takes the slot `from` to `to`, moving the slots between by one. Nothing for a place
/// that is not there.
pub fn move_slot<T>(slots: &mut [T], from: usize, to: usize) {
    if from >= slots.len() || to >= slots.len() {
        return;
    }
    if from < to {
        slots[from..=to].rotate_left(1);
    } else {
        slots[to..=from].rotate_right(1);
    }
}

/// Moves what `from` holds to the slot `to` (past the last slot is the last slot); the slots
/// between move by one, empty ones too. `NoSuchInsert` for an empty slot, `Unchanged` when it
/// is there already.
pub fn move_insert(doc: &Doc, strip: StripId, from: usize, to: usize) -> Result<Edit, EditError> {
    let to = to.min(INSERT_SLOTS - 1);
    let new = with_channel(doc, strip, |c| {
        if c.inserts.get(from).is_none_or(Option::is_none) {
            return Err(EditError::NoSuchInsert);
        }
        if from == to {
            return Err(EditError::Unchanged);
        }
        move_slot(&mut c.inserts, from, to);
        Ok(())
    })?;
    Ok(Edit {
        doc: new,
        step: StepKind::MoveInsert { strip, from, to },
    })
}

/// `with`: what goes in, given its id; a new one, so that automation of what was there does
/// not move what replaces it.
fn replace(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    with: impl FnOnce(InsertId) -> Insert,
) -> Result<Edit, EditError> {
    let (id, next_id) = new_insert_id(doc)?;
    let with = with(id);
    let mut new = with_channel(doc, strip, |c| {
        let place =
            (c.inserts.get_mut(slot).and_then(Option::as_mut)).ok_or(EditError::NoSuchInsert)?;
        let same_plugin = match (place.plugin_ref(), with.plugin_ref()) {
            (Some(a), Some(b)) => a.key == b.key,
            _ => false,
        };
        if same_plugin || place.kind().is_some() && place.kind() == with.kind() {
            return Err(EditError::Unchanged);
        }
        *place = with;
        Ok(())
    })?;
    new.next_id = next_id;
    Ok(Edit {
        doc: new,
        step: StepKind::ReplaceEffect { strip, slot },
    })
}

/// Puts `kind`, as it starts on that strip ([`Insert::new_on`]), in place of what `slot`
/// holds, and heard. `NoSuchInsert` for an empty slot, `Unchanged` when it holds that kind
/// already.
pub fn replace_effect(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    kind: EffectKind,
) -> Result<Edit, EditError> {
    replace(doc, strip, slot, |id| Insert::new_on(id, kind, strip))
}

/// Puts a plugin, as it is given, in place of what `slot` holds, and heard. `NoSuchInsert`
/// for an empty slot, `Unchanged` when it holds the plugin of that key already.
pub fn replace_plugin_effect(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    plugin: Arc<PluginRef>,
) -> Result<Edit, EditError> {
    replace(doc, strip, slot, |id| Insert::plugin(id, plugin))
}

/// What 「曲から外す」 names: every preset of an SF2 file, or a plugin by its exact key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing<'a> {
    Sf2(&'a str),
    Plugin(&'a str),
}

/// Takes `missing` out of the song: every track that sounds with it gets the sine wave (its
/// name and its notes stay), and every insert slot that holds it is emptied. `Unchanged`
/// when the song does not use it.
pub fn remove_missing(doc: &Doc, missing: Missing) -> Result<Edit, EditError> {
    let uses = |instrument: &InstrumentRef| match (instrument, missing) {
        (InstrumentRef::Sf2(s), Missing::Sf2(file)) => &*s.file == file,
        (InstrumentRef::Plugin(p), Missing::Plugin(key)) => &*p.key == key,
        _ => false,
    };
    let held = |insert: &Option<Insert>| {
        let plugin = insert.as_ref().and_then(Insert::plugin_ref);
        matches!((plugin, missing), (Some(p), Missing::Plugin(key)) if &*p.key == key)
    };
    let (mut tracks, mut inserts) = (0, 0);
    let mut empty = |channel: &mut Channel| {
        for slot in channel.inserts.iter_mut().filter(|slot| held(slot)) {
            *slot = None;
            inserts += 1;
        }
    };
    let mut new = doc.clone();
    for slot in &mut new.tracks {
        if !uses(&slot.instrument) && !slot.channel.inserts.iter().any(held) {
            continue;
        }
        let mut track = Track::clone(slot);
        if uses(&track.instrument) {
            track.instrument = InstrumentRef::SINE;
            tracks += 1;
        }
        empty(&mut track.channel);
        drop_orphan_lanes(&mut track);
        *slot = Arc::new(track);
    }
    for slot in &mut new.returns {
        if slot.channel.inserts.iter().any(held) {
            let mut ret = Return::clone(slot);
            empty(&mut ret.channel);
            *slot = Arc::new(ret);
        }
    }
    if new.master.inserts.iter().any(held) {
        let mut master = Channel::clone(&new.master);
        empty(&mut master);
        new.master = Arc::new(master);
    }
    if tracks + inserts == 0 {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: new,
        step: StepKind::RemoveMissing { tracks, inserts },
    })
}

impl Doc {
    /// The plugin at `place`; `None` when something else is there, or nothing.
    pub fn plugin(&self, place: PluginPlace) -> Option<&Arc<PluginRef>> {
        match place {
            PluginPlace::Instrument(track) => match &self.track(track)?.instrument {
                InstrumentRef::Plugin(plugin) => Some(plugin),
                _ => None,
            },
            PluginPlace::Insert(strip, slot) => self.insert(strip, slot)?.plugin_ref(),
        }
    }
}

/// The plugin at `place` has another state: what it said of itself after the change. `key` is
/// the plugin the caller means: `NoSuchPlugin` when the place holds another one by now, or
/// none. The same state again is `Unchanged`.
pub fn set_plugin_state(
    doc: &Doc,
    place: PluginPlace,
    key: &str,
    state: Arc<[u8]>,
    change: PluginChange,
) -> Result<Edit, EditError> {
    let old = (doc.plugin(place))
        .filter(|p| &*p.key == key)
        .ok_or(EditError::NoSuchPlugin)?;
    if old.state == state {
        return Err(EditError::Unchanged);
    }
    let plugin = Arc::new(old.with_state(state));
    let new = match place {
        PluginPlace::Instrument(track) => {
            let ti = track_index(doc, track)?;
            let mut new = doc.clone();
            new.tracks[ti] = Arc::new(Track {
                instrument: InstrumentRef::Plugin(plugin),
                ..Track::clone(&doc.tracks[ti])
            });
            new
        }
        PluginPlace::Insert(strip, slot) => with_channel(doc, strip, |c| {
            let insert = (c.inserts.get_mut(slot).and_then(Option::as_mut))
                .ok_or(EditError::NoSuchPlugin)?;
            insert.effect = InsertEffect::Plugin(plugin);
            Ok(())
        })?,
    };
    Ok(Edit {
        doc: new,
        step: match change {
            PluginChange::Window => StepKind::PluginWindow { place },
            PluginChange::Param(param) => StepKind::SetPluginParam { place, param },
        },
    })
}

/// Mutes the return, or lets it sound again.
pub fn toggle_return_mute(doc: &Doc, bus: Bus) -> Edit {
    let old = &doc.returns[bus.index()];
    let muted = !old.muted;
    let mut new = doc.clone();
    new.returns[bus.index()] = Arc::new(Return {
        muted,
        ..Return::clone(old)
    });
    Edit {
        doc: new,
        step: StepKind::MuteReturn { bus, muted },
    }
}

#[cfg(test)]
mod tests;
