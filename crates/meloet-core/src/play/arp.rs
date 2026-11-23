//! 「アルペジオ」 of a keys track (round 12, E2): the settings are part of the song, one step
//! for each change. The engine runs it (`meloet_engine`, `engine/arp.rs`) and is told with
//! the mixer (`EngineOp::Arp`, `mixsync.rs`); what it plays comes back as the notes played
//! (`RecordReport::Event`, `Played::Key`), so a recording and 「さっきのを残す」 hold its notes.

use meloet_model::{ARP_OCTAVES_MAX, TrackId, edit};
use meloet_protocol::Arp;

use super::{Play, wave3};
use crate::state::Effect;

impl Play {
    /// One step 「アルペジオを変える」. Nothing while counting in or recording, for a vocal
    /// track, for a track that is not there, and when nothing would change.
    pub fn set_arp(&mut self, track: u32, arp: Arp, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let mut arp = wave3::arp_of(&arp);
        arp.octaves = arp.octaves.clamp(1, ARP_OCTAVES_MAX);
        if let Ok(e) = edit::set_arp(self.doc(), TrackId(track), arp) {
            self.apply(e, effects);
        }
    }
}

#[cfg(test)]
mod tests {
    use meloet_model::record::{HeldControls, RecEventKind};
    use meloet_protocol::{
        Arp, ArpRate, ArpShape, Command, StepName, StepWhat, TakeNotes, TransportState,
    };

    use super::super::recent::{HeldNow, Played};
    use super::super::tests::{cmd, event, off, on, ready, report, started};
    use super::super::{EngineOp, Heard, RecordReport};
    use crate::state::{Effect, Input, Model};

    const S: u64 = 1_000_000;

    fn ops(effects: Vec<Effect>) -> Vec<EngineOp> {
        (effects.into_iter())
            .filter_map(|e| match e {
                Effect::Engine(op) => Some(op),
                _ => None,
            })
            .collect()
    }

    fn up_down() -> Arp {
        Arp {
            on: true,
            shape: ArpShape::UpDown,
            rate: ArpRate::Eighth,
            triplet: true,
            octaves: 2,
            hold: true,
        }
    }

    fn told(seat: usize, arp: Arp) -> EngineOp {
        EngineOp::Arp {
            seat,
            arp: super::wave3::arp_of(&arp),
        }
    }

    fn arp_of(m: &Model, track: usize) -> Arp {
        m.view().song.tracks[track].arp
    }

    fn last_row(m: &Model) -> (StepName, StepWhat) {
        let n = m.view().song.undo_count;
        let row = m.history_rows(n, n).pop().expect("a row");
        (row.name, row.what)
    }

    #[test]
    fn a_change_is_one_step_that_the_engine_is_told_and_undo_takes_back() {
        let mut m = ready();
        let track = m.view().song.tracks[0].id;
        let steps = m.view().song.undo_count;
        let arp = up_down();
        assert_eq!(
            ops(cmd(&mut m, Command::SetArp { track, arp })),
            [told(0, arp)]
        );
        assert_eq!(arp_of(&m, 0), arp);
        assert_eq!(m.view().song.undo_count, steps + 1);
        assert_eq!(last_row(&m).0, StepName::SetArp);
        assert!(matches!(last_row(&m).1, StepWhat::Track { .. }));

        // The same again is no step.
        assert_eq!(cmd(&mut m, Command::SetArp { track, arp }), []);
        assert_eq!(m.view().song.undo_count, steps + 1);

        let off = Arp { on: false, ..arp };
        assert_eq!(
            ops(cmd(&mut m, Command::SetArp { track, arp: off })),
            [told(0, off)]
        );
        assert_eq!(ops(cmd(&mut m, Command::Undo)), [told(0, arp)]);
        assert_eq!(ops(cmd(&mut m, Command::Undo)), [told(0, Arp::default())]);
        assert_eq!(arp_of(&m, 0), Arp::default());
        assert_eq!(ops(cmd(&mut m, Command::Redo)), [told(0, arp)]);
        assert_eq!(arp_of(&m, 0), arp);

        // An engine made anew is told again.
        assert!(m.engine_seats().contains(&told(0, arp)));
    }

    #[test]
    fn octaves_are_kept_in_range_and_what_names_nothing_does_nothing() {
        let mut m = ready();
        let track = m.view().song.tracks[0].id;
        let wide = Arp {
            octaves: 9,
            ..up_down()
        };
        cmd(&mut m, Command::SetArp { track, arp: wide });
        assert_eq!(arp_of(&m, 0).octaves, 3);
        let none = Arp {
            octaves: 0,
            ..up_down()
        };
        cmd(&mut m, Command::SetArp { track, arp: none });
        assert_eq!(arp_of(&m, 0).octaves, 1);

        let steps = m.view().song.undo_count;
        let arp = Arp::default();
        assert_eq!(cmd(&mut m, Command::SetArp { track: 999, arp }), []);
        // A vocal track has none.
        cmd(&mut m, Command::AddVocalTrack);
        let view = m.view();
        let vocal = view.song.tracks.last().unwrap();
        let steps_now = view.song.undo_count;
        assert_eq!(steps_now, steps + 1);
        let on = up_down();
        let effects = cmd(
            &mut m,
            Command::SetArp {
                track: vocal.id,
                arp: on,
            },
        );
        assert_eq!(ops(effects), []);
        assert_eq!(m.view().song.undo_count, steps_now);
        assert_eq!(m.view().song.tracks.last().unwrap().arp, Arp::default());
    }

    #[test]
    fn nothing_changes_while_counting_in_or_recording() {
        let mut m = ready();
        let track = m.view().song.tracks[0].id;
        let arp = up_down();
        cmd(&mut m, Command::SetCountIn { bars: 1 });
        cmd(&mut m, Command::Record);
        assert_eq!(m.view().transport.state, TransportState::CountIn);
        assert_eq!(cmd(&mut m, Command::SetArp { track, arp }), []);
        started(&mut m);
        m.set_now(
            S,
            Some(Heard {
                tick: 10.0,
                state: TransportState::Recording,
                lap: 0,
            }),
        );
        m.handle(Input::Tick);
        assert_eq!(m.view().transport.state, TransportState::Recording);
        assert_eq!(cmd(&mut m, Command::SetArp { track, arp }), []);
        assert_eq!(arp_of(&m, 0), Arp::default());
    }

    #[test]
    fn a_copy_of_the_track_and_a_track_on_a_seat_used_before_are_told_theirs() {
        let mut m = ready();
        let track = m.view().song.tracks[0].id;
        let arp = up_down();
        cmd(&mut m, Command::SetArp { track, arp });
        let effects = ops(cmd(&mut m, Command::DuplicateTrack { track }));
        assert!(effects.contains(&told(1, arp)), "{effects:?}");
        assert_eq!(arp_of(&m, 1), arp);

        // The copy goes and another track takes its seat: the engine emptied the seat, and
        // is told nothing of an arpeggio that is off.
        let copy = m.view().song.tracks[1].id;
        let effects = ops(cmd(&mut m, Command::DeleteTrack { track: copy }));
        assert!(effects.contains(&EngineOp::ClearSeat { seat: 1 }));
        let effects = ops(cmd(&mut m, Command::DuplicateTrack { track }));
        assert!(effects.contains(&told(1, arp)), "{effects:?}");
    }

    /// (key, start, length) of the shown take of the first clip, in the take's ticks.
    fn notes(m: &Model) -> (Vec<(u8, u32, u32)>, u64) {
        let clip = m.view().song.tracks[0].clips[0].clone();
        let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
        let notes = (take.notes.iter())
            .map(|n| (n.key, n.start, n.length))
            .collect();
        (notes, clip.end - clip.start)
    }

    #[test]
    fn the_notes_said_ahead_of_where_the_stop_was_heard_are_not_in_the_take() {
        let mut m = ready();
        let track = m.view().song.tracks[0].id;
        cmd(
            &mut m,
            Command::SetArp {
                track,
                arp: Arp {
                    on: true,
                    ..Arp::default()
                },
            },
        );
        cmd(&mut m, Command::SetCountIn { bars: 0 });
        cmd(&mut m, Command::Record);
        started(&mut m);
        // Sixteenths of an arpeggio, each said a little before it is heard.
        let said = [(60, 0.0), (64, 480.0), (67, 960.0), (60, 1440.0)];
        for (i, &(key, tick)) in said.iter().enumerate() {
            event(&mut m, on(key), 10 + i as u64, tick, 0);
            event(&mut m, off(key), 10 + i as u64, tick + 384.0, 0);
        }
        // The person stopped while the third note sounded.
        m.set_now(
            100,
            Some(Heard {
                tick: 1000.0,
                state: TransportState::Recording,
                lap: 0,
            }),
        );
        cmd(&mut m, Command::Record);
        report(
            &mut m,
            RecordReport::Stopped {
                t_us: 100,
                tick: 1000.0,
                lap: 0,
            },
        );
        let (notes, _) = notes(&m);
        assert_eq!(notes, [(60, 0, 384), (64, 480, 384), (67, 960, 40)]);
    }

    fn still(run: u32, t_us: u64, kind: RecEventKind) -> Played {
        Played::Key {
            kind,
            t_us,
            run,
            at: None,
        }
    }

    #[test]
    fn the_keys_held_under_an_arpeggio_do_not_count_as_played_keys_still_down() {
        // The chord is held on the keyboard all along; what is said as played is the
        // arpeggio's notes, each with its end.
        let chord = HeldNow {
            keys: 1 << 60 | 1 << 64,
            controls: HeldControls::default(),
        };
        for arp_on in [true, false] {
            let mut m = ready();
            cmd(&mut m, Command::SetTempo { bpm: 120.0 });
            let track = m.view().song.tracks[0].id;
            let arp = Arp {
                on: arp_on,
                ..Arp::default()
            };
            cmd(&mut m, Command::SetArp { track, arp });
            let items = vec![still(0, S, on(60)), still(0, S + 100_000, off(60))];
            assert_eq!(m.handle(Input::Played { items, held: chord }), []);
            // More than two bars later (4 s at 120).
            let items = vec![still(0, 9 * S, on(72)), still(0, 9 * S + 100_000, off(72))];
            assert_eq!(m.handle(Input::Played { items, held: chord }), []);
            m.set_now(10 * S, None);
            cmd(&mut m, Command::KeepPlayed);
            let keys: Vec<u8> = notes(&m).0.iter().map(|n| n.0).collect();
            if arp_on {
                // Nothing was down in between: the second note began a new phrase.
                assert_eq!(keys, [72]);
            } else {
                assert_eq!(keys, [60, 72]);
            }
        }
    }
}
