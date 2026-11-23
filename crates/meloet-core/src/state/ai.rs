//! The switch 「AI とつなぐ」 (spec 3 「AI link」, A-27), what the screen shows of the socket,
//! and the AI link's asks (`crate::ai`).

use std::sync::Arc;

use meloet_model::{Doc, TrackId};
use meloet_protocol::{AiLinkView, AiPort};

use super::{Effect, Model};
use crate::ai::{AiAnswer, AiAsk, AiBusy, AiDone, AiMeasure, AiRefusal, AiSong};
use crate::export::ExportJob;
use meloet_protocol::ExportTail;

#[derive(Debug, Default)]
pub(super) struct AiSide {
    /// The switch, kept in the settings.
    pub(super) on: bool,
    /// What the socket's thread said last.
    port: AiPort,
    /// The answer to the last [`super::Input::Ai`], until the core thread takes it.
    answer: Option<AiAnswer>,
}

impl AiSide {
    pub(super) fn new(on: bool) -> AiSide {
        AiSide {
            on,
            ..AiSide::default()
        }
    }

    /// Returns whether it changed (the settings are then saved and the socket told).
    pub(super) fn set(&mut self, on: bool) -> bool {
        let changed = self.on != on;
        self.on = on;
        if changed {
            self.port = AiPort::Closed;
        }
        changed
    }

    pub(super) fn port_said(&mut self, port: AiPort) {
        self.port = port;
    }

    pub(super) fn view(&self) -> AiLinkView {
        AiLinkView {
            on: self.on,
            port: if self.on { self.port } else { AiPort::Closed },
        }
    }
}

impl Model {
    pub fn ai_on(&self) -> bool {
        self.ai.on
    }

    fn ai_busy(&self) -> Option<AiBusy> {
        if self.play.counting_in() {
            Some(AiBusy::CountingIn)
        } else if self.play.busy() {
            Some(AiBusy::Recording)
        } else if self.export.running() {
            Some(AiBusy::Exporting)
        } else {
            None
        }
    }

    pub fn ai_song(&self) -> AiSong {
        AiSong {
            song: self.play.song().clone(),
            undo_count: self.play.undo_count(),
            busy: self.ai_busy(),
        }
    }

    /// The answer to the last [`super::Input::Ai`].
    pub fn take_ai_answer(&mut self) -> Option<AiAnswer> {
        self.ai.answer.take()
    }

    pub(super) fn ai_ask(&mut self, ask: AiAsk, effects: &mut Vec<Effect>) {
        let answer = match self.ai_write(ask, effects) {
            Ok(done) => AiAnswer::Done(Box::new(done)),
            Err(refused) => AiAnswer::Refused(refused),
        };
        self.ai.answer = Some(answer);
    }

    fn ai_write(&mut self, ask: AiAsk, effects: &mut Vec<Effect>) -> Result<AiDone, AiRefusal> {
        if let Some(busy) = self.ai_busy() {
            return Err(AiRefusal::Busy(busy));
        }
        let before = self.play.undo_count();
        let play = &mut self.play;
        let wrote = match ask {
            AiAsk::WriteNotes { at, notes } => play.ai_write_notes(at, &notes, effects)?,
            AiAsk::DeleteNotes { take, notes } => play.ai_delete_notes(take, &notes, effects)?,
            AiAsk::CreateClip { track, tick } => play.ai_create_clip(track, tick, effects)?,
            AiAsk::RenameClip { clip, name } => play.ai_rename_clip(clip, &name, effects)?,
            AiAsk::SetTempo { tick, bpm } => play.ai_set_tempo(tick, bpm, effects)?,
            AiAsk::AddMarker { tick, name } => play.ai_add_marker(tick, &name, effects)?,
            AiAsk::AddTrack { instrument } => play.ai_add_track(instrument, effects)?,
            AiAsk::Undo => {
                let row = self.play.history_rows(before, before).pop();
                if row.is_none() {
                    return Err(AiRefusal::NothingToUndo);
                }
                self.notice = self.play.undo(effects);
                return Ok(AiDone {
                    step: row,
                    undo_count: self.play.undo_count(),
                    clip: None,
                    take: None,
                    track: None,
                });
            }
        };
        let undo_count = self.play.undo_count();
        if undo_count == before {
            return Err(AiRefusal::Unchanged);
        }
        let doc = &self.play.song().doc;
        let take = (wrote.clip)
            .and_then(|c| doc.clip(c))
            .filter(|(_, c)| c.is_keys())
            .map(|(_, c)| c.active_take_id().0);
        let track = (wrote.track)
            .and_then(|id| doc.tracks.iter().position(|t| t.id == id))
            .map(|i| i as u32 + 1);
        Ok(AiDone {
            step: self.play.history_rows(undo_count, undo_count).pop(),
            undo_count,
            clip: wrote.clip.map(|c| c.0),
            take,
            track,
        })
    }

    /// A job of the export that renders `ask` for its levels, with this id. Not while the
    /// song may not change, nor while the export's dialog or run is up (one second engine
    /// at a time).
    /// The tracks rendered alone come after the mix, as numbered and named in the song.
    pub fn ai_measure_job(
        &self,
        ask: &AiMeasure,
        id: u64,
    ) -> Result<(ExportJob, Vec<(u32, String)>), AiRefusal> {
        if let Some(busy) = self.ai_busy() {
            return Err(AiRefusal::Busy(busy));
        }
        if self.export.dialog_open() {
            return Err(AiRefusal::Busy(AiBusy::Exporting));
        }
        let doc = &self.play.song().doc;
        let tracks: Vec<TrackId> = match &ask.tracks {
            None => doc.tracks.iter().map(|t| t.id).collect(),
            Some(numbers) => {
                let mut ids = Vec::new();
                for &n in numbers {
                    let index = n.checked_sub(1).ok_or(AiRefusal::NoSuchTrack(n))? as usize;
                    let track = doc.tracks.get(index).ok_or(AiRefusal::NoSuchTrack(n))?;
                    if !ids.contains(&track.id) {
                        ids.push(track.id);
                    }
                }
                ids
            }
        };
        let named = (doc.tracks.iter().enumerate())
            .filter(|(_, t)| tracks.contains(&t.id))
            .map(|(i, t)| (i as u32 + 1, t.name.clone()))
            .collect();
        let doc = Doc {
            tracks: (doc.tracks.iter())
                .filter(|t| tracks.contains(&t.id))
                .cloned()
                .collect(),
            ..doc.clone()
        };
        let scene = self.export_scene();
        let job = ExportJob {
            id,
            doc: Arc::new(doc),
            span: ask.span,
            per_track: true,
            tail: ExportTail::Off,
            realtime: false,
            skip: scene.stopped_plugins.iter().map(|p| p.slot).collect(),
            sample_rate: scene.sample_rate,
            measure: true,
        };
        Ok((job, named))
    }
}

#[cfg(test)]
mod tests {
    use meloet_model::Note;
    use meloet_protocol::{AiLinkView, AiPort, Command};

    use crate::ai::{AiAnswer, AiAsk, AiBusy, AiPlace, AiRefusal};
    use crate::settings::Settings;
    use crate::state::{Effect, Input, Model};

    fn saved(effects: &[Effect]) -> Vec<bool> {
        (effects.iter())
            .filter_map(|e| match e {
                Effect::SaveSettings(s) => Some(s.ai_link),
                _ => None,
            })
            .collect()
    }

    fn told(effects: &[Effect]) -> Vec<bool> {
        (effects.iter())
            .filter_map(|e| match e {
                Effect::AiLink(on) => Some(*on),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_switch_is_a_setting_and_no_step() {
        let m = &mut Model::new(Settings::default());
        let off = AiLinkView {
            on: false,
            port: AiPort::Closed,
        };
        assert_eq!(m.view().ai_link, off);
        let steps = m.view().song.undo_count;

        let effects = m.handle(Input::Command(Command::SetAiLink { on: true }));
        assert_eq!(saved(&effects), [true]);
        assert_eq!(told(&effects), [true]);
        // Closed until the socket's thread says it listens.
        assert_eq!(m.view().ai_link.port, AiPort::Closed);
        m.handle(Input::AiPort(AiPort::Waiting));
        assert_eq!(
            m.view().ai_link,
            AiLinkView {
                on: true,
                port: AiPort::Waiting
            }
        );
        m.handle(Input::AiPort(AiPort::Connected));
        assert_eq!(m.view().ai_link.port, AiPort::Connected);
        assert_eq!(m.view().song.undo_count, steps);
        let again = m.handle(Input::Command(Command::SetAiLink { on: true }));
        assert!(saved(&again).is_empty(), "the same again saves nothing");
        assert!(told(&again).is_empty());

        let effects = m.handle(Input::Command(Command::SetAiLink { on: false }));
        assert_eq!(saved(&effects), [false]);
        assert_eq!(told(&effects), [false]);
        assert_eq!(m.view().ai_link, off);
        // A late word from the socket does not show while the switch is off.
        m.handle(Input::AiPort(AiPort::Connected));
        assert_eq!(m.view().ai_link, off);
    }

    #[test]
    fn the_switch_is_read_at_the_start() {
        let m = Model::new(Settings {
            ai_link: true,
            ..Settings::default()
        });
        assert!(m.ai_on());
        assert!(m.view().ai_link.on);
    }

    fn ask(m: &mut Model, ask: AiAsk) -> AiAnswer {
        m.handle(Input::Ai(ask));
        m.take_ai_answer().expect("an answer")
    }

    fn note(start: u64, key: u8) -> Note {
        Note {
            start,
            length: 480,
            key,
            velocity: 90,
            from: None,
        }
    }

    #[test]
    fn a_write_is_one_step_and_undo_takes_it_back() {
        let m = &mut Model::new(Settings::default());
        let before = m.ai_song().song.doc;
        let notes = vec![note(0, 60), note(480, 64), note(960, 67)];
        let AiAnswer::Done(done) = ask(
            m,
            AiAsk::WriteNotes {
                at: AiPlace::Track(1),
                notes,
            },
        ) else {
            panic!("written")
        };
        assert_eq!(done.undo_count, 1);
        assert!(done.clip.is_some() && done.take.is_some());
        let song = m.ai_song().song;
        let clip = &song.doc.tracks[0].clips[0];
        assert_eq!(clip.active_take().take.notes.len(), 3);

        let AiAnswer::Done(undone) = ask(m, AiAsk::Undo) else {
            panic!("undone")
        };
        assert_eq!(undone.undo_count, 0);
        assert_eq!(undone.step, done.step);
        let after = m.ai_song().song.doc;
        assert_eq!(after.tracks, before.tracks);
        assert_eq!(
            ask(m, AiAsk::Undo),
            AiAnswer::Refused(AiRefusal::NothingToUndo)
        );
    }

    #[test]
    fn an_old_take_does_nothing() {
        let m = &mut Model::new(Settings::default());
        let AiAnswer::Done(done) = ask(
            m,
            AiAsk::WriteNotes {
                at: AiPlace::Track(1),
                notes: vec![note(0, 60), note(480, 62)],
            },
        ) else {
            panic!("written")
        };
        let take = done.take.unwrap();
        let AiAnswer::Done(deleted) = ask(
            m,
            AiAsk::DeleteNotes {
                take,
                notes: vec![0],
            },
        ) else {
            panic!("deleted")
        };
        assert_ne!(deleted.take, Some(take));
        assert_eq!(
            ask(
                m,
                AiAsk::DeleteNotes {
                    take,
                    notes: vec![0]
                }
            ),
            AiAnswer::Refused(AiRefusal::NoSuchTake(take))
        );
        assert_eq!(m.ai_song().undo_count, 2);
    }

    #[test]
    fn no_write_while_counting_in_or_recording() {
        let m = &mut crate::state::tests::playing();
        m.handle(Input::Command(Command::Record));
        let busy = m.ai_song().busy;
        assert!(
            matches!(busy, Some(AiBusy::CountingIn | AiBusy::Recording)),
            "{busy:?}"
        );
        let answer = ask(
            m,
            AiAsk::AddMarker {
                tick: 0,
                name: "A".into(),
            },
        );
        assert_eq!(answer, AiAnswer::Refused(AiRefusal::Busy(busy.unwrap())));
        assert_eq!(m.ai_song().undo_count, 0);
    }
}
