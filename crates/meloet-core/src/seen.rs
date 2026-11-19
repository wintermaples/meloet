//! What the tests read off the views by kind: the sounds and the effects that come with
//! meloet, which the views name as any sound (`SoundRef`) and any effect (`InsertView`).

use meloet_protocol::{
    EffectKind, EffectRef, INSERT_SLOTS, InsertEffectView, InstrumentKind, MasterView, ReturnView,
    SelectedInsert, SelectedParams, ShelfView, SlotsView, SoundRef, SoundView, TrackView,
};

fn own(sound: &Option<SoundRef>) -> Option<InstrumentKind> {
    match sound {
        Some(SoundRef::Builtin { instrument }) => Some(*instrument),
        _ => None,
    }
}

/// A sample of a sound of meloet that plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
    pub instrument: InstrumentKind,
    pub started_us: u64,
    pub length_us: u64,
}

pub(crate) trait ShelfSeen {
    fn selected(&self) -> Option<InstrumentKind>;
    fn current(&self) -> Option<InstrumentKind>;
    fn loading(&self) -> Option<InstrumentKind>;
    fn sample(&self) -> Option<Sample>;
    fn selected_effect(&self) -> Option<EffectKind>;
    fn favorites(&self) -> Vec<InstrumentKind>;
}

impl ShelfSeen for ShelfView {
    fn selected(&self) -> Option<InstrumentKind> {
        own(&self.selected_sound)
    }

    fn current(&self) -> Option<InstrumentKind> {
        own(&self.current_sound)
    }

    fn loading(&self) -> Option<InstrumentKind> {
        own(&self.loading_sound)
    }

    fn sample(&self) -> Option<Sample> {
        let sample = self.sound_sample.as_ref()?;
        let playing = sample.playing?;
        Some(Sample {
            instrument: own(&Some(sample.sound.clone()))?,
            started_us: playing.started_us,
            length_us: playing.length_us,
        })
    }

    fn selected_effect(&self) -> Option<EffectKind> {
        match self.chosen_effect {
            Some(EffectRef::Builtin { effect }) => Some(effect),
            _ => None,
        }
    }

    fn favorites(&self) -> Vec<InstrumentKind> {
        (self.favorite_sounds.iter())
            .filter_map(|s| own(&Some(s.clone())))
            .collect()
    }
}

fn own_effects(slots: &SlotsView) -> [Option<EffectKind>; INSERT_SLOTS] {
    let mut out = [None; INSERT_SLOTS];
    for (to, from) in out.iter_mut().zip(slots) {
        if let Some(InsertEffectView::Builtin { effect }) = from.as_ref().map(|i| &i.effect) {
            *to = Some(*effect);
        }
    }
    out
}

pub(crate) trait StripSeen {
    /// `None` for a slot that is empty and for one that holds a plugin.
    fn inserts(&self) -> [Option<EffectKind>; INSERT_SLOTS];
}

impl StripSeen for TrackView {
    fn inserts(&self) -> [Option<EffectKind>; INSERT_SLOTS] {
        own_effects(&self.slots)
    }
}

impl StripSeen for ReturnView {
    fn inserts(&self) -> [Option<EffectKind>; INSERT_SLOTS] {
        own_effects(&self.slots)
    }
}

impl StripSeen for MasterView {
    fn inserts(&self) -> [Option<EffectKind>; INSERT_SLOTS] {
        own_effects(&self.slots)
    }
}

pub(crate) trait TrackSeen {
    /// `Sine` for an SF2 preset and for a plugin.
    fn instrument(&self) -> InstrumentKind;
}

impl TrackSeen for TrackView {
    fn instrument(&self) -> InstrumentKind {
        match self.sound {
            SoundView::Builtin { instrument } => instrument,
            _ => InstrumentKind::Sine,
        }
    }
}

pub(crate) trait ParamsSeen {
    /// The selected insert, which holds an effect that comes with meloet.
    fn builtin(self) -> SelectedInsert;
}

impl ParamsSeen for Option<SelectedParams> {
    fn builtin(self) -> SelectedInsert {
        match self {
            Some(SelectedParams::Builtin(insert)) => insert,
            other => panic!("no effect of meloet is selected: {other:?}"),
        }
    }
}
