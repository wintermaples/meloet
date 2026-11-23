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

fn row(step: StepName, name: &str) -> (StepName, StepWhat, Option<StepPlace>) {
    (
        step,
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
    assert_eq!(named(&d.step), row(StepName::SaveMuteSet, "爽快"));
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
    assert_eq!(named(&d.step), row(StepName::RecallMuteSet, "爽快"));
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
    assert_eq!(named(&d.step), row(StepName::DeleteMuteSet, "A"));
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

fn screen_save(name: &str) -> Command {
    Command::SaveMuteSet {
        name: name.into(),
        overwrite: false,
    }
}

fn screen_rename(name: &str, to: &str) -> Command {
    Command::RenameMuteSet {
        name: name.into(),
        to: to.into(),
    }
}

/// `SongView::mute_sets`: the names and whether each is as the song is now.
fn sets(m: &Model) -> Vec<(String, bool)> {
    (m.view().song.mute_sets.iter())
        .map(|s| (s.name.clone(), s.as_now))
        .collect()
}

fn set(name: &str, as_now: bool) -> (String, bool) {
    (name.to_owned(), as_now)
}

fn names(m: &Model) -> Vec<String> {
    sets(m).into_iter().map(|(name, _)| name).collect()
}

fn current(m: &Model) -> u32 {
    m.view().history.current
}

fn last_row(m: &Model) -> (StepName, StepWhat, Option<StepPlace>) {
    let n = current(m);
    named(&m.history_rows(n, n).pop())
}

#[test]
fn the_screen_saves_recalls_renames_and_deletes_a_set_as_one_step_each() {
    let m = &mut ready();
    three_tracks(m);
    assert!(sets(m).is_empty());
    let start = current(m);

    cmd(m, Command::ToggleMute { track: 2 });
    cmd(m, Command::ToggleSolo { track: 3 });
    cmd(m, screen_save(" 爽快 "));
    assert_eq!(current(m), start + 2);
    assert_eq!(last_row(m), row(StepName::SaveMuteSet, "爽快"));
    assert_eq!(sets(m), [set("爽快", true)]);
    // The solo is part of the set.
    cmd(m, Command::ToggleSolo { track: 3 });
    assert_eq!(sets(m), [set("爽快", false)]);
    cmd(m, Command::ToggleMute { track: 2 });
    cmd(m, Command::ToggleMute { track: 1 });
    cmd(m, screen_save("クール"));
    assert_eq!(sets(m), [set("爽快", false), set("クール", true)]);
    let before = current(m);

    // A name taken (not to be written over), no name, the same set again: nothing.
    for command in [
        screen_save("爽快"),
        screen_save("  "),
        Command::SaveMuteSet {
            name: "クール".into(),
            overwrite: true,
        },
    ] {
        cmd(m, command.clone());
        assert_eq!(current(m), before, "{command:?}");
    }

    cmd(
        m,
        Command::RecallMuteSet {
            name: "爽快".into(),
        },
    );
    assert_eq!(current(m), before + 1);
    assert_eq!(last_row(m), row(StepName::RecallMuteSet, "爽快"));
    assert_eq!(flags(m), [(false, false), (true, false), (false, true)]);
    assert_eq!(sets(m), [set("爽快", true), set("クール", false)]);
    // As it is already: no step.
    cmd(
        m,
        Command::RecallMuteSet {
            name: "爽快".into(),
        },
    );
    assert_eq!(current(m), before + 1);

    cmd(m, screen_rename("爽快", " サビ "));
    assert_eq!(current(m), before + 2);
    assert_eq!(
        last_row(m),
        (
            StepName::RenameMuteSet,
            StepWhat::Renamed {
                from: "爽快".into(),
                to: "サビ".into(),
            },
            Some(StepPlace::Mixer),
        )
    );
    assert_eq!(sets(m), [set("サビ", true), set("クール", false)]);
    // An old name, the name of another set, the name it has, no name: nothing.
    for command in [
        screen_rename("爽快", "x"),
        screen_rename("サビ", "クール"),
        screen_rename("サビ", " サビ"),
        screen_rename("サビ", "\t"),
        Command::RecallMuteSet {
            name: "爽快".into(),
        },
        Command::DeleteMuteSet {
            name: "爽快".into(),
        },
    ] {
        cmd(m, command.clone());
        assert_eq!(current(m), before + 2, "{command:?}");
    }

    cmd(
        m,
        Command::DeleteMuteSet {
            name: "クール".into(),
        },
    );
    assert_eq!(current(m), before + 3);
    assert_eq!(last_row(m), row(StepName::DeleteMuteSet, "クール"));
    assert_eq!(sets(m), [set("サビ", true)]);

    // Undo brings the names back, redo takes them again.
    cmd(m, Command::Undo);
    assert_eq!(names(m), ["サビ", "クール"]);
    cmd(m, Command::Undo);
    assert_eq!(names(m), ["爽快", "クール"]);
    cmd(m, Command::Undo);
    assert_eq!(flags(m), [(true, false), (false, false), (false, true)]);
    cmd(m, Command::Redo);
    cmd(m, Command::Redo);
    cmd(m, Command::Redo);
    assert_eq!(names(m), ["サビ"]);
    assert_eq!(current(m), before + 3);
}

#[test]
fn the_screen_changes_no_set_while_recording() {
    let m = &mut ready();
    three_tracks(m);
    cmd(m, screen_save("A"));
    cmd(m, Command::ToggleMute { track: 1 });
    let before = (m.ai_song().song.doc.mute_sets.clone(), current(m));
    cmd(m, Command::Record);
    assert!(m.play().busy());
    for command in [
        screen_save("B"),
        Command::RecallMuteSet { name: "A".into() },
        screen_rename("A", "C"),
        Command::DeleteMuteSet { name: "A".into() },
    ] {
        cmd(m, command.clone());
        let now = (m.ai_song().song.doc.mute_sets.clone(), current(m));
        assert_eq!(now, before, "{command:?}");
    }
    assert!(m.ai_song().song.doc.tracks[0].muted, "not recalled");
}

fn rename(name: &str, to: &str) -> AiAsk {
    AiAsk::RenameMuteSet {
        name: name.into(),
        to: to.into(),
    }
}

#[test]
fn the_ai_renames_a_set_by_the_rule_of_a_save() {
    let m = &mut ready();
    three_tracks(m);
    done(m, save("A", false));
    cmd(m, Command::ToggleMute { track: 1 });
    done(m, save("B", false));
    let steps = m.ai_song().undo_count;

    let (d, _) = done(m, rename(" A ", "サビ"));
    assert_eq!(d.undo_count, steps + 1);
    assert_eq!(
        named(&d.step),
        (
            StepName::RenameMuteSet,
            StepWhat::Renamed {
                from: "A".into(),
                to: "サビ".into(),
            },
            Some(StepPlace::Mixer),
        )
    );
    assert_eq!(names(m), ["サビ", "B"]);
    // The name it has: done, no step.
    let (d, _) = done(m, rename("サビ", " サビ "));
    assert_eq!((d.undo_count, d.step), (steps + 1, None));
    assert_eq!(
        ask(m, rename("サビ", "B")).0,
        AiAnswer::Refused(AiRefusal::MuteSetExists("B".into()))
    );
    assert_eq!(
        ask(m, rename("A", "C")).0,
        AiAnswer::Refused(AiRefusal::NoSuchMuteSet("A".into()))
    );
    assert!(matches!(
        ask(m, rename("サビ", "")).0,
        AiAnswer::Refused(AiRefusal::Cannot(_))
    ));
    assert_eq!(m.ai_song().undo_count, steps + 1);
    cmd(m, Command::Undo);
    assert_eq!(names(m), ["A", "B"]);
}
