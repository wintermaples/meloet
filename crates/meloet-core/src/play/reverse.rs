//! 「ノーマライズ」 and 「逆にする」 of vocal clips (round 12, A9), as far as they change the
//! song: which frames the clips play when they are asked for, and the one step made of what
//! the thread that reads audio files in answers. That answer comes later; a clip that plays
//! other frames by then is left as it is.

use meloet_model::{
    AudioId, ClipId, NewAudio, PlayedFrames, Reversed, normalize_clips, normalize_gain,
    played_frames, reverse_clips,
};
use meloet_protocol::{AudioClipState, StatusMessage};

use super::Play;
use crate::state::Effect;
use crate::vocalrec;

impl Play {
    /// The frames each vocal clip of `clips` plays, in the order of `clips`. Ids that are
    /// no vocal clip's are passed over, and so are clips whose file is not found.
    pub fn played_frames_of(&self, clips: &[u32]) -> Vec<PlayedFrames> {
        let mut played: Vec<PlayedFrames> = Vec::new();
        for &id in clips {
            let Some(frames) = played_frames(self.doc(), ClipId(id)) else {
                continue;
            };
            if self.audio_state(&frames.source) != AudioClipState::Missing
                && !played.iter().any(|p| p.clip == frames.clip)
            {
                played.push(frames);
            }
        }
        played
    }

    /// The names the files of 「逆にする」 are asked for: as a recording names its file
    /// (`vocalrec::file_name`), with the numbers the step will give them if nothing comes
    /// between.
    pub fn backwards_names(&self, played: &[PlayedFrames]) -> Vec<String> {
        let doc = self.doc();
        (played.iter().enumerate())
            .map(|(i, p)| {
                let track = doc.clip(p.clip).map_or("", |(t, _)| t.name.as_str());
                let audio = AudioId(doc.next_id.saturating_add(2 * i as u32));
                vocalrec::file_name(track, audio.0)
            })
            .collect()
    }

    /// The loudest samples were measured: the clips that still play those frames get the
    /// volume that puts them at −1.0 dBFS, as one step. Returns the notice to show; a file
    /// that could not be read is said before anything else.
    pub fn normalize_measured(
        &mut self,
        measured: impl IntoIterator<Item = (PlayedFrames, Option<f32>)>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let (mut silent, mut limited, mut failed) = (0u32, 0u32, 0u32);
        let mut gains = Vec::new();
        for (played, peak) in measured {
            if played_frames(self.doc(), played.clip).as_ref() != Some(&played) {
                continue;
            }
            let Some(peak) = peak else {
                failed += 1;
                continue;
            };
            if peak == 0.0 {
                silent += 1;
            } else if let Some((gain, put_in)) = normalize_gain(peak) {
                limited += u32::from(put_in);
                gains.push((played, gain));
            }
        }
        if let Ok(e) = normalize_clips(self.doc(), &gains) {
            self.apply(e, effects);
        }
        match (failed, silent, limited) {
            (0, 0, 0) => None,
            (0, 0, count) => Some(StatusMessage::NormalizeLimited { count }),
            (0, count, _) => Some(StatusMessage::NormalizeSilent { count }),
            (count, _, _) => Some(StatusMessage::ClipWorkFailed { count }),
        }
    }

    /// The backwards files were made: each clip that still plays the frames they were made
    /// of shows them, as one step. Returns the notice to show; a file that could not be made
    /// is said before anything else.
    pub fn reverse_made(
        &mut self,
        made: impl IntoIterator<Item = (PlayedFrames, Option<NewAudio>)>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let mut failed = 0u32;
        let made: Vec<Reversed> = (made.into_iter())
            .filter_map(|(played, audio)| match audio {
                Some(audio) => Some(Reversed { played, audio }),
                None => {
                    if played_frames(self.doc(), played.clip).as_ref() == Some(&played) {
                        failed += 1;
                    }
                    None
                }
            })
            .collect();
        let failed = (failed > 0).then_some(StatusMessage::ClipWorkFailed { count: failed });
        let Ok(done) = reverse_clips(self.doc(), &made) else {
            return failed;
        };
        let count = done.follow_off;
        self.apply(done.edit, effects);
        failed.or((count > 0).then_some(StatusMessage::ReverseFollowOff { count }))
    }
}
