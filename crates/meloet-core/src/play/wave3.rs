//! What the song holds of round 12, waves 3 and 4 (protocol 33), as the screen sees it: the
//! arpeggio (E2), the master's lanes (M5), sections (A2), comps and the takes of a vocal clip
//! (A8), 「音程を寄せる」 (M7). The features change these; until they are built nothing does
//! (`// round12 stub: <ID>`).

use meloet_model::sections::{self, Section};
use meloet_model::song::Clip;
use meloet_model::{AudioShape, Doc, TempoMap};
use meloet_protocol::{
    Arp, ArpRate, ArpShape, AudioClipView, AudioTakeView, AutoTarget, CompPiece, LaneName,
    LaneView, SectionUse, SoundKnob, SourceTempoView, TempoPointView, TuneView,
};

use super::{Play, scale_mode};

pub(crate) fn knob_view(knob: meloet_model::SoundKnob) -> SoundKnob {
    match knob {
        meloet_model::SoundKnob::Brightness => SoundKnob::Brightness,
        meloet_model::SoundKnob::Attack => SoundKnob::Attack,
        meloet_model::SoundKnob::Release => SoundKnob::Release,
        meloet_model::SoundKnob::Vibrato => SoundKnob::Vibrato,
    }
}

pub(crate) fn arp_view(arp: &meloet_model::Arp) -> Arp {
    Arp {
        on: arp.on,
        shape: match arp.shape {
            meloet_model::ArpShape::Up => ArpShape::Up,
            meloet_model::ArpShape::Down => ArpShape::Down,
            meloet_model::ArpShape::UpDown => ArpShape::UpDown,
            meloet_model::ArpShape::Random => ArpShape::Random,
        },
        rate: match arp.rate {
            meloet_model::ArpRate::Quarter => ArpRate::Quarter,
            meloet_model::ArpRate::Eighth => ArpRate::Eighth,
            meloet_model::ArpRate::Sixteenth => ArpRate::Sixteenth,
            meloet_model::ArpRate::ThirtySecond => ArpRate::ThirtySecond,
        },
        triplet: arp.triplet,
        octaves: arp.octaves,
        hold: arp.hold,
    }
}

pub(crate) fn arp_of(arp: &Arp) -> meloet_model::Arp {
    meloet_model::Arp {
        on: arp.on,
        shape: match arp.shape {
            ArpShape::Up => meloet_model::ArpShape::Up,
            ArpShape::Down => meloet_model::ArpShape::Down,
            ArpShape::UpDown => meloet_model::ArpShape::UpDown,
            ArpShape::Random => meloet_model::ArpShape::Random,
        },
        rate: match arp.rate {
            ArpRate::Quarter => meloet_model::ArpRate::Quarter,
            ArpRate::Eighth => meloet_model::ArpRate::Eighth,
            ArpRate::Sixteenth => meloet_model::ArpRate::Sixteenth,
            ArpRate::ThirtySecond => meloet_model::ArpRate::ThirtySecond,
        },
        triplet: arp.triplet,
        octaves: arp.octaves,
        hold: arp.hold,
    }
}

/// `ClipView::comp`: the pieces, each naming its take by id.
pub(crate) fn comp_view(clip: &Clip) -> Vec<CompPiece> {
    (clip.comp.iter())
        .filter_map(|p| {
            Some(CompPiece {
                from: p.from,
                take: clip.takes.id(p.take)?.0,
            })
        })
        .collect()
}

pub(crate) fn tune_view(shape: &AudioShape) -> Option<TuneView> {
    shape.tune.map(|t| TuneView {
        root: t.root,
        mode: scale_mode(t.mode),
        strength: t.strength,
    })
}

/// The master's lanes, of its volume (`MasterView::lanes`).
pub(crate) fn master_lanes_view(doc: &Doc) -> Vec<LaneView> {
    (doc.master_lanes.iter())
        .filter(|l| l.target == meloet_model::AutoTarget::Volume)
        .map(|l| LaneView {
            target: AutoTarget::Volume,
            name: LaneName::Volume,
            lane_id: l.id.0,
            points: u32::try_from(l.points.len()).unwrap_or(u32::MAX),
            step: l.step,
            resting: l.resting,
        })
        .collect()
}

/// `MarkerView::section` of each marker, in the order of `Doc::markers`: from the marker to
/// the next one that is later, after the last one to the latest clip end up to the head of
/// a bar.
pub(crate) fn sections(doc: &Doc) -> Vec<SectionUse> {
    let map = TempoMap::new(&doc.timeline);
    let end = sections::song_end(doc, &map);
    (doc.markers.iter())
        .map(|m| match sections::section_from(doc, &map, m.tick, end) {
            Section::Usable { .. } => SectionUse::Usable,
            Section::NotOnBars => SectionUse::NotOnBars,
            Section::Empty => SectionUse::Empty,
        })
        .collect()
}

impl Play {
    /// `AudioClipView::takes`: every take of the vocal clip, for the rows of 「いいとこ取り」.
    pub(super) fn with_takes(&self, mut view: AudioClipView, clip: &Clip) -> AudioClipView {
        view.takes = (clip.takes.vocal().iter().enumerate())
            .map(|(i, ct)| {
                let take = &ct.take;
                let follows = clip.takes.shape().is_some_and(|s| s.follow);
                AudioTakeView {
                    take_id: take.id.0,
                    number: i as u32 + 1,
                    audio: take.source.id.0,
                    first_frame: take.first_frame,
                    frames: take.frames,
                    take_offset: ct.offset,
                    source_tempo: (take.sync.as_ref().filter(|_| follows)).map(|s| {
                        SourceTempoView {
                            tempo_bpm: s.tempo.bpm(),
                            head_ramp: s.head_ramp,
                            tempo_points: (s.points.iter())
                                .map(|p| TempoPointView {
                                    tick: p.tick,
                                    tempo_bpm: p.tempo.bpm(),
                                    ramp: p.ramp,
                                })
                                .collect(),
                        }
                    }),
                    state: self.audio_state(&take.source),
                }
            })
            .collect();
        view
    }
}
