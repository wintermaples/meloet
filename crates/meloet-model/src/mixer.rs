//! The mixer: what each strip holds (fader, pan, inserts, sends), the two returns and the
//! master, and the steps that change them (spec 3 「センド」, docs/architecture.md 19-3).

use std::sync::Arc;

use crate::edit::{Edit, EditError, track_index};
use crate::effects::EffectKind;
use crate::history::StepKind;
use crate::instrument::InstrumentGroup;
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

/// An effect in an insert slot with the values of its parameters: one for every parameter of
/// its table, each inside its range.
#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    kind: EffectKind,
    values: Vec<f32>,
}

impl Insert {
    /// With the defaults of the table.
    pub fn new(kind: EffectKind) -> Insert {
        Insert {
            kind,
            values: kind.params().iter().map(|p| p.default).collect(),
        }
    }

    pub fn kind(&self) -> EffectKind {
        self.kind
    }

    /// In the order of the table.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    pub fn value(&self, param: u32) -> Option<f32> {
        self.values.get(param as usize).copied()
    }

    /// Sets a parameter, brought into its range. `false` when the effect has no such
    /// parameter.
    pub fn set(&mut self, param: u32, value: f32) -> bool {
        match self.kind.clamp(param, value) {
            Some(v) => {
                self.values[param as usize] = v;
                true
            }
            None => false,
        }
    }
}

/// Sound goes through the slots from the first to the last (spec 5 「インサート」).
pub type Inserts = [Option<Insert>; INSERT_SLOTS];

/// What every strip has. The master's pan stays in the middle.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Channel {
    pub gain: Gain,
    pub pan: Pan,
    pub inserts: Inserts,
}

impl Channel {
    fn with_first(kind: EffectKind) -> Channel {
        let mut channel = Channel::default();
        channel.inserts[0] = Some(Insert::new(kind));
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

/// How much of a track goes to return A and to return B, taken after its fader.
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
/// synths send −15 dB to A, drums −24 dB, basses nothing. Nothing goes to B.
pub fn new_track_sends(instrument: &InstrumentRef) -> Sends {
    let InstrumentRef::Builtin(builtin) = instrument;
    let a = match builtin.group() {
        InstrumentGroup::PianoKeys | InstrumentGroup::PadStrings | InstrumentGroup::Synth => {
            Gain::from_db(-15.0)
        }
        InstrumentGroup::Drums => Gain::from_db(-24.0),
        InstrumentGroup::Bass => Gain::SILENT,
    };
    [a, Gain::SILENT]
}

/// The returns of a new song: a reverb on A and a delay on B.
pub fn new_song_returns() -> [Arc<Return>; 2] {
    [EffectKind::Reverb, EffectKind::Delay].map(|kind| {
        Arc::new(Return {
            channel: Channel::with_first(kind),
            muted: false,
        })
    })
}

/// The master of a new song: a limiter, whose default ceiling is −0.3 dB.
pub fn new_song_master() -> Arc<Channel> {
    Arc::new(Channel::with_first(EffectKind::Limiter))
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

/// The master has no pan (`EditError::NoPan`).
pub fn set_pan(doc: &Doc, strip: StripId, pan: Pan) -> Result<Edit, EditError> {
    if strip == StripId::Master {
        return Err(EditError::NoPan);
    }
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
            .filter(|i| i.kind() == kind)
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

/// Puts `kind`, with the defaults of its table, in an empty slot (`SlotTaken` otherwise).
pub fn add_effect(
    doc: &Doc,
    strip: StripId,
    slot: usize,
    kind: EffectKind,
) -> Result<Edit, EditError> {
    let new = with_channel(doc, strip, |c| {
        let place = c.inserts.get_mut(slot).ok_or(EditError::NoSuchInsert)?;
        if place.is_some() {
            return Err(EditError::SlotTaken);
        }
        *place = Some(Insert::new(kind));
        Ok(())
    })?;
    Ok(Edit {
        doc: new,
        step: StepKind::AddEffect { strip, slot, kind },
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
        step: StepKind::RemoveEffect {
            strip,
            slot,
            kind: kind.expect("set when the slot held an effect"),
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
