//! 「音のクリップにする」 and 「このトラックを音にする」 (round 12, A7), as far as they touch the
//! song: what is rendered, and the one step made of the audio once it is in the work place.

use std::sync::Arc;

use meloet_model::mixer::{Channel, Gain, Return};
use meloet_model::{AutoTarget, ClipId, Doc, NewAudio, TickRange, TrackId, bounce_track};
use meloet_protocol::StatusMessage;

use super::{MAX_TRACKS, Play};
use crate::state::Effect;
use crate::vocalrec;

/// What is turned into audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BounceRange {
    pub track: TrackId,
    pub span: TickRange,
}

impl Play {
    /// The track and the range of 「音のクリップにする」: the keys clips among `clips`, from the
    /// earliest start to the latest end. `None` when there is no keys clip, or they are on
    /// more than one track.
    pub fn bounce_clips_range(&self, clips: &[u32]) -> Option<BounceRange> {
        let doc = self.doc();
        let mut track = None;
        let (mut start, mut end) = (u64::MAX, 0);
        for &id in clips {
            let Some((t, c)) = doc.clip(ClipId(id)) else {
                continue;
            };
            if !c.is_keys() {
                continue;
            }
            if track.is_some_and(|had| had != t.id) {
                return None;
            }
            track = Some(t.id);
            start = start.min(c.start);
            end = end.max(c.end);
        }
        Some(BounceRange {
            track: track?,
            span: TickRange::new(start, end)?,
        })
    }

    /// The track and the range of 「このトラックを音にする」: from the start of its first clip
    /// to the end of its last. `None` for an id that is no keys track; `Some(None)` for a
    /// keys track with no clip.
    pub fn bounce_track_range(&self, track: u32) -> Option<Option<BounceRange>> {
        let t = self.doc().track(TrackId(track))?;
        if t.kind.is_vocal() {
            return None;
        }
        let (first, last) = (t.clips.first(), t.clips.last());
        Some(first.zip(last).and_then(|(first, last)| {
            Some(BounceRange {
                track: t.id,
                span: TickRange::new(first.start, last.end)?,
            })
        }))
    }

    /// The name the file of the audio is asked for: as a recording names its file, with the
    /// number the step will most likely give it.
    pub fn bounce_file_name(&self, track: TrackId) -> String {
        use meloet_model::{BOUNCED_SUFFIX, BOUNCED_SUFFIX_EN};
        let doc = self.doc();
        let name = doc.track(track).map_or("", |t| t.name.as_str());
        let suffix = self.names.pick(BOUNCED_SUFFIX, BOUNCED_SUFFIX_EN);
        let name = format!("{name}{suffix}");
        vocalrec::file_name(&name, doc.next_id)
    }

    /// The audio of the track is in the work place: the step 「音のクリップにする」, when the
    /// track is still there. Returns the notice to show.
    pub fn bounce_made(
        &mut self,
        from: TrackId,
        audio: NewAudio,
        start: u64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        if self.doc().track(from).is_none() {
            return Some(StatusMessage::BounceGone);
        }
        if self.doc().tracks.len() >= MAX_TRACKS {
            return Some(StatusMessage::TooManyTracks);
        }
        let made = bounce_track(self.doc(), from, audio, start, self.names).ok()?;
        // The solo goes with the sound: the keys track is muted now.
        let moved = self.song.session.soloed.remove(&from);
        if moved {
            self.song.session.soloed.insert(made.track);
        }
        self.apply(made.edit, effects);
        if moved {
            self.note_solo_step(from, made.track);
        }
        Some(StatusMessage::Bounced {
            track: self.track_ref(made.track)?,
        })
    }
}

/// The song as the track sounds alone through its instrument and inserts: the fader at
/// 0 dB, the pan in the middle, no sends (and nothing on the returns), a master with nothing
/// on it, the track not muted. Its lanes of the fader, the pan and the sends are left out
/// (the new track takes them over); the others move what they move while playing.
pub fn bounce_doc(song: &Doc, track: TrackId) -> Option<Doc> {
    let t = song.track(track)?;
    let strip = |target: AutoTarget| {
        matches!(
            target,
            AutoTarget::Volume | AutoTarget::Pan | AutoTarget::Send(_)
        )
    };
    let alone = meloet_model::song::Track {
        muted: false,
        channel: Channel {
            inserts: t.channel.inserts.clone(),
            ..Channel::default()
        },
        sends: [Gain::SILENT; 2],
        automation: (t.automation.iter())
            .filter(|l| !strip(l.target))
            .cloned()
            .collect(),
        ..meloet_model::song::Track::clone(t)
    };
    let quiet = || {
        Arc::new(Return {
            channel: Channel::default(),
            muted: true,
        })
    };
    Some(Doc {
        markers: Arc::default(),
        mute_sets: Arc::default(),
        loop_range: None,
        tracks: vec![Arc::new(alone)],
        returns: [quiet(), quiet()],
        master: Arc::new(Channel::default()),
        master_lanes: Vec::new(),
        audio: Arc::default(),
        ..song.clone()
    })
}

#[cfg(test)]
mod tests;
