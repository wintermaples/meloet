//! Telling the engine and the read-ahead thread about the vocal clips
//! (docs/architecture.md 22-8, 22-18 A1), the way the notes of a track are told.
//!
//! The rules send the clips of a seat as the song has them ([`EngineOp::Clips`]), whenever
//! they change. What the engine and the thread need is counted in frames, so it is made
//! where the stream's sample rate and the engine's map are known ([`clip_tables`], called
//! by the core's audio side): the table of the engine (where each clip is, its fades, its
//! volume) and the clips as the thread feeds them (which frames of which file).

use std::sync::Arc;

use meloet_engine::{ClipRegion, ClipTable};
use meloet_model::song::Clip;
use meloet_model::{Doc, TempoMap, Track};

use super::{EngineOp, Play};
use crate::pcm::{FeedClip, feed_clip};
use crate::state::Effect;

/// The vocal clips of a track.
fn audio_clips(track: &Track) -> Vec<Arc<Clip>> {
    (track.clips.iter())
        .filter(|c| c.active_audio().is_some())
        .cloned()
        .collect()
}

/// The table of the engine and the clips of the read-ahead thread for the vocal clips of
/// one track, at `sample_rate` on `map`. A clip that starts before the one before it ends
/// (the song never has one) is left out of both.
pub fn clip_tables(
    map: &TempoMap,
    sample_rate: u32,
    clips: &[Arc<Clip>],
) -> (ClipTable, Vec<FeedClip>) {
    let (table, feeds, _) = clip_tables_of(map, sample_rate, clips);
    (table, feeds)
}

/// As [`clip_tables`], with the clip each of the thread's clips was made of.
pub fn clip_tables_of(
    map: &TempoMap,
    sample_rate: u32,
    clips: &[Arc<Clip>],
) -> (ClipTable, Vec<FeedClip>, Vec<Arc<Clip>>) {
    let mut both: Vec<(ClipRegion, (FeedClip, Arc<Clip>))> = (clips.iter())
        .filter_map(|clip| {
            let feed = feed_clip(map, sample_rate, clip)?;
            let shape = clip.takes.shape()?;
            let region = ClipRegion::of(
                map,
                f64::from(sample_rate),
                clip.start,
                clip.end,
                shape.fade_in,
                shape.fade_out,
                shape.gain.db(),
            );
            Some((region, (feed, Arc::clone(clip))))
        })
        .collect();
    both.sort_by_key(|(region, _)| region.start);
    let mut end = 0;
    both.retain(|(region, _)| {
        let fits = region.start >= end && region.end > region.start;
        if fits {
            end = region.end;
        }
        fits
    });
    let (regions, fed): (Vec<ClipRegion>, Vec<(FeedClip, Arc<Clip>)>) = both.into_iter().unzip();
    let (feeds, kept) = fed.into_iter().unzip();
    (ClipTable::new(regions).unwrap_or_default(), feeds, kept)
}

impl Play {
    /// After the clips of the track on `seat` changed: its vocal clips go to the engine,
    /// if it has any or had any.
    pub(super) fn sync_seat_audio(
        &self,
        seat: usize,
        track: &Track,
        before: Option<&Arc<Track>>,
        effects: &mut Vec<Effect>,
    ) {
        let clips = audio_clips(track);
        let had = before.is_some_and(|b| b.clips.iter().any(|c| c.active_audio().is_some()));
        if !clips.is_empty() || had {
            effects.push(Effect::Engine(EngineOp::Clips { seat, clips }));
        }
    }

    /// The song's audio table, when it is another one than `before` had.
    pub(super) fn sync_audio_sources(&self, before: &Doc, effects: &mut Vec<Effect>) {
        if !Arc::ptr_eq(&self.song.doc.audio, &before.audio) {
            self.send_audio_sources(effects);
        }
    }

    fn send_audio_sources(&self, effects: &mut Vec<Effect>) {
        effects.push(Effect::Engine(EngineOp::AudioSources {
            sources: Arc::clone(&self.song.doc.audio),
        }));
    }

    /// Another song is on the seats: the audio table, and the vocal clips of every seat.
    /// `fresh`: for an engine that was told of no clips yet, which needs to hear only of
    /// the ones there are.
    pub(super) fn send_audio(&self, fresh: bool, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        if !(fresh && doc.audio.is_empty()) {
            self.send_audio_sources(effects);
        }
        for (seat, id, _) in self.seats.taken() {
            let clips = doc.track(id).map(|t| audio_clips(t)).unwrap_or_default();
            if !(fresh && clips.is_empty()) {
                effects.push(Effect::Engine(EngineOp::Clips { seat, clips }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use meloet_model::song::{ClipId, ClipTake, ClipTakes, TakeId};
    use meloet_model::{
        AudioId, AudioShape, AudioSource, AudioTake, ClipGain, Meter, Tempo, Timeline,
    };

    use super::*;

    fn clip(id: u32, start: u64, end: u64, shape: AudioShape) -> Arc<Clip> {
        let source = Arc::new(AudioSource {
            id: AudioId(id),
            uid: [id as u8; 16],
            file: format!("歌 {id}.wav"),
            frames: 480_000,
            sample_rate: 48_000,
            channels: 1,
            bits: Some(24),
            sha256: None,
        });
        Arc::new(Clip {
            id: ClipId(id),
            name: "歌".to_owned(),
            start,
            end,
            takes: ClipTakes::Vocal {
                takes: vec![ClipTake {
                    take: Arc::new(AudioTake {
                        id: TakeId(100 + id),
                        source,
                        first_frame: 1_000,
                        frames: 400_000,
                        sync: None,
                    }),
                    offset: 0,
                }],
                shape,
            },
            active: 0,
        })
    }

    #[test]
    fn the_table_and_the_feed_are_the_same_clips_in_frames() {
        // 120 a minute: a quarter note is 24000 frames at 48 kHz, 22050 at 44.1 kHz.
        let tempo = Tempo::from_centi(12_000).unwrap();
        let map = TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR));
        let quiet = AudioShape {
            fade_in: 960,
            fade_out: 1_920,
            gain: ClipGain::from_db(-6.0).unwrap(),
            ..AudioShape::RECORDED
        };
        let clips = vec![
            // Out of order, as a drag may leave them in the track.
            clip(2, 7_680, 15_360, AudioShape::RECORDED),
            clip(1, 0, 3_840, quiet),
            // Over the first one: left out.
            clip(3, 1_920, 5_760, AudioShape::RECORDED),
            Arc::new(Clip {
                takes: ClipTakes::Keys(Vec::new()),
                ..(*clip(4, 20_000, 30_000, AudioShape::RECORDED)).clone()
            }),
        ];
        let (table, feeds) = clip_tables(&map, 48_000, &clips);
        let regions = table.regions();
        assert_eq!(regions.len(), 2);
        assert_eq!((regions[0].start, regions[0].end), (0, 48_000));
        assert_eq!((regions[0].fade_in, regions[0].fade_out), (12_000, 24_000));
        assert!((regions[0].gain - 0.501_187).abs() < 1e-5);
        assert_eq!((regions[1].start, regions[1].end), (96_000, 192_000));
        assert_eq!(regions[1].gain, 1.0);
        assert_eq!(feeds.len(), 2);
        assert_eq!((feeds[0].start, feeds[0].end), (0, 48_000));
        assert_eq!((feeds[0].uid, feeds[0].file_frame), ([1; 16], 1_000));
        assert_eq!((feeds[1].uid, feeds[1].start), ([2; 16], 96_000));

        let (table, feeds) = clip_tables(&map, 44_100, &clips);
        assert_eq!(table.regions()[1].start, 88_200);
        assert_eq!(feeds[1].start, 88_200);
        assert_eq!(
            clip_tables(&map, 48_000, &[]),
            (ClipTable::default(), vec![])
        );
    }
}
