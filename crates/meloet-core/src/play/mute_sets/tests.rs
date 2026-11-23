use meloet_protocol::{Command, HistoryRow, SendBus, StepName, StepPlace, StepWhat};

use crate::ai::{AiAnswer, AiAsk, AiDone, AiRefusal};
use crate::play::EngineOp;
use crate::play::mix::tests::three_tracks;
use crate::play::tests::{cmd, ready};
use crate::state::{Effect, Input, Model};

fn ask(m: &mut Model, ask: AiAsk) -> (AiAnswer, Vec<Effect>) {
    let effects = m.handle(Input::Ai(ask));
    (m.take_ai_answer().expect("an answer"), effects)
}

fn done(m: &mut Model, a: AiAsk) -> (AiDone, Vec<Effect>) {
    match ask(m, a) {
        (AiAnswer::Done(d), effects) => (*d, effects),
        (other, _) => panic!("done: {other:?}"),
    }
}

fn save(name: &str, overwrite: bool) -> AiAsk {
    AiAsk::SaveMuteSet {
        name: name.into(),
        overwrite,
    }
}

fn recall(name: &str) -> AiAsk {
    AiAsk::RecallMuteSet { name: name.into() }
}

fn flags(m: &Model) -> Vec<(bool, bool)> {
    (m.view().song.tracks.iter())
        .map(|t| (t.muted, t.soloed))
        .collect()
}

fn silent(effects: &[Effect]) -> Option<Vec<usize>> {
    effects.iter().rev().find_map(|e| match e {
        Effect::Engine(EngineOp::SilentTracks { tracks }) => Some(tracks.clone()),
        _ => None,
    })
}

fn row(name: &str) -> (StepName, StepWhat, Option<StepPlace>) {
    (
        StepName::Mute,
        StepWhat::Name { name: name.into() },
        Some(StepPlace::Mixer),
    )
}

fn named(step: &Option<HistoryRow>) -> (StepName, StepWhat, Option<StepPlace>) {
    let step = step.as_ref().expect("a step");
    (step.name.clone(), step.what.clone(), step.place.clone())
}

#[test]
fn saving_and_switching_sets_are_one_step_each_and_undo_takes_them_back() {
    let m = &mut ready();
    three_tracks(m);
    let start = m.ai_song().song;
    let steps = m.ai_song().undo_count;

    // 「爽快」: track 2 muted, track 3 soloed. 「クール」: tracks 1 and 3 muted, return B muted.
    cmd(m, Command::ToggleMute { track: 2 });
    cmd(m, Command::ToggleSolo { track: 3 });
    let (d, _) = done(m, save("爽快", false));
    assert_eq!(d.undo_count, steps + 2);
    assert_eq!(named(&d.step), row("爽快"));
    cmd(m, Command::ToggleSolo { track: 3 });
    cmd(m, Command::ToggleMute { track: 2 });
    cmd(m, Command::ToggleMute { track: 1 });
    cmd(m, Command::ToggleMute { track: 3 });
    cmd(m, Command::ToggleReturnMute { bus: SendBus::B });
    let (d, _) = done(m, save("クール", false));
    assert_eq!(d.undo_count, steps + 7);
    let cool = m.ai_song().song;

    let (d, effects) = done(m, recall("爽快"));
    assert_eq!(d.undo_count, steps + 8, "one step for every mute");
    assert_eq!(named(&d.step), row("爽快"));
    assert_eq!(flags(m), [(false, false), (true, false), (false, true)]);
    assert!(!m.ai_song().song.doc.returns[1].muted);
    // Track 3 soloed: the others do not play.
    assert_eq!(silent(&effects), Some(vec![0, 1]));

    let (d, effects) = done(m, recall("クール"));
    assert_eq!(d.undo_count, steps + 9);
    assert_eq!(flags(m), [(true, false), (false, false), (true, false)]);
    assert!(m.ai_song().song.doc.returns[1].muted);
    assert_eq!(silent(&effects), Some(vec![0, 2]));
    assert_eq!(m.ai_song().song.doc, cool.doc);

    // As the set says already: done, no step.
    let (d, _) = done(m, recall("クール"));
    assert_eq!((d.undo_count, d.step), (steps + 9, None));
    let (d, _) = done(m, save("クール", true));
    assert_eq!((d.undo_count, d.step), (steps + 9, None));

    // Undo brings back the mutes, not the solo (it is no step).
    cmd(m, Command::Undo);
    assert_eq!(flags(m), [(false, false), (true, false), (false, false)]);
    cmd(m, Command::Redo);
    for _ in 0..9 {
        cmd(m, Command::Undo);
    }
    let back = m.ai_song();
    assert_eq!(back.undo_count, steps);
    assert_eq!(
        meloet_model::Doc {
            next_id: 0,
            ..back.song.doc
        },
        meloet_model::Doc {
            next_id: 0,
            ..start.doc
        }
    );
}

#[test]
fn a_name_taken_a_set_not_there_and_deleting() {
    let m = &mut ready();
    three_tracks(m);
    done(m, save("A", false));
    let steps = m.ai_song().undo_count;
    assert_eq!(
        ask(m, save(" A ", false)).0,
        AiAnswer::Refused(AiRefusal::MuteSetExists("A".into()))
    );
    assert_eq!(
        ask(m, recall("B")).0,
        AiAnswer::Refused(AiRefusal::NoSuchMuteSet("B".into()))
    );
    let delete = || AiAsk::DeleteMuteSet { name: "A".into() };
    let (d, _) = done(m, delete());
    assert_eq!(d.undo_count, steps + 1);
    assert_eq!(named(&d.step), row("A"));
    assert!(m.ai_song().song.doc.mute_sets.is_empty());
    assert_eq!(
        ask(m, delete()).0,
        AiAnswer::Refused(AiRefusal::NoSuchMuteSet("A".into()))
    );
    cmd(m, Command::Undo);
    assert_eq!(m.ai_song().song.doc.mute_sets.len(), 1);
    assert_eq!(m.ai_song().undo_count, steps);
}

#[test]
fn a_song_opened_or_saved_heads_the_recent_list() {
    let m = &mut ready();
    assert!(m.ai_song().recent.is_empty());
    let effects = three_tracks(m);
    let path = std::path::PathBuf::from("/nowhere/曲/曲.meloet");
    let kept = |effects: &[Effect]| {
        effects.iter().find_map(|e| match e {
            Effect::SaveRecentSongs(songs) => Some(songs.clone()),
            _ => None,
        })
    };
    assert_eq!(kept(&effects), Some(vec![path.clone()]));
    assert_eq!(m.ai_song().recent, std::slice::from_ref(&path));

    // Saved as another song: it comes first.
    cmd(m, Command::SaveSongAs);
    let other = std::path::PathBuf::from("/nowhere/別/別.meloet");
    m.handle(Input::SaveAsPicked(Some("/nowhere/別".into())));
    let effects = m.handle(Input::SongSaved {
        path: other.clone(),
        name: "別".into(),
    });
    assert_eq!(kept(&effects), Some(vec![other.clone(), path.clone()]));
    // Saved again over it: the list is as it was, and is not written.
    cmd(m, Command::SaveSong);
    let effects = m.handle(Input::SongSaved {
        path: other.clone(),
        name: "別".into(),
    });
    assert_eq!(kept(&effects), None);
    assert_eq!(m.ai_song().recent, [other, path]);
}
