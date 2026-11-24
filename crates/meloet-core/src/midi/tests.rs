//! The song as a MIDI file, read back with [`smf::read`].

use meloet_model::song::Note;
use meloet_model::{
    BuiltinInstrument, Doc, InstrumentRef, Language, Meter, NewTake, Tempo, TempoMap, TrackId,
    arrange, edit, timeline,
};

use super::smf::{self, SmfNote};
use super::{DRUM_CHANNEL, MidiTooLong, midi_file, write_beside};

pub(crate) const BAR: u64 = 7_680;

/// What [`song`] holds.
pub(crate) struct Parts {
    pub piano: TrackId,
    pub drums: TrackId,
    pub vocals: TrackId,
    pub bass: TrackId,
    pub empty: TrackId,
}

fn tempo(bpm: u32) -> Tempo {
    Tempo::from_centi(bpm * 100).unwrap()
}

fn note(start: u64, length: u64, key: u8, velocity: u8) -> Note {
    Note {
        start,
        length,
        key,
        velocity,
        from: None,
    }
}

fn recorded(doc: &Doc, track: TrackId, origin: u64, length: u64, notes: Vec<Note>) -> Doc {
    let take = NewTake {
        origin,
        length,
        notes,
        controls: Vec::new(),
    };
    edit::add_recording(doc, track, vec![take], 0)
        .unwrap()
        .0
        .doc
}

fn keys_track(doc: &Doc, instrument: BuiltinInstrument, name: &str) -> (Doc, TrackId) {
    let instrument = InstrumentRef::Builtin(instrument);
    let (e, id) = edit::add_track(doc, None, instrument, Language::Ja).unwrap();
    (
        edit::rename_track(&e.doc, id, name).map_or(e.doc, |r| r.doc),
        id,
    )
}

/// Four bars: 120 a minute, a ramp from bar 2 down to 60 at bar 3, 100 from bar 4, which is
/// in 3/4. In the order of the tracks: 「ピアノ」 (a note crosses bar 2, one rings past
/// the clip's end, one is past it: the take is longer than the clip), 「ドラム」 (an electronic kit, on bar 2), 「ボーカル」 (a vocal track),
/// 「ベース」 (muted), 「空」 (no clip).
pub(crate) fn song() -> (Doc, Parts) {
    let mut doc = Doc::new_default();
    doc.name = "テストの曲".to_owned();
    let piano = doc.tracks[0].id;
    let doc = timeline::set_tempo(&doc, tempo(120)).unwrap().doc;
    let doc = timeline::add_tempo_point(&doc, BAR, tempo(120))
        .unwrap()
        .doc;
    let doc = timeline::set_tempo_ramp(&doc, BAR, true).unwrap().doc;
    let doc = timeline::add_tempo_point(&doc, 2 * BAR, tempo(60))
        .unwrap()
        .doc;
    let doc = timeline::add_tempo_point(&doc, 3 * BAR, tempo(100))
        .unwrap()
        .doc;
    let doc = timeline::add_meter_change(&doc, 3, Meter::new(3, 4).unwrap())
        .unwrap()
        .doc;
    assert!(!doc.timeline.tempo_points[1].ramp);

    let doc = edit::rename_track(&doc, piano, "ピアノ").map_or(doc, |e| e.doc);
    let doc = recorded(
        &doc,
        piano,
        0,
        5 * BAR,
        vec![
            note(0, 960, 60, 100),
            note(BAR - 480, 960, 62, 90),
            note(2 * BAR, 1_920, 64, 80),
            note(4 * BAR - 480, 1_920, 65, 70),
            note(4 * BAR + 960, 480, 67, 70),
        ],
    );
    let clip = doc.tracks[0].clips[0].id;
    let doc = arrange::set_clip_length(&doc, clip, 4 * BAR).unwrap().doc;
    let (doc, drums) = keys_track(&doc, BuiltinInstrument::ElectroKit, "ドラム");
    let hits = (0..8).map(|i| note(i * 960, 400, [36, 38][i as usize % 2], 110));
    let doc = recorded(&doc, drums, BAR, BAR, hits.collect());
    let (e, vocals) = edit::add_vocal_track(&doc, None, Language::Ja).unwrap();
    let doc = edit::rename_track(&e.doc, vocals, "ボーカル").unwrap().doc;
    let (doc, bass) = keys_track(&doc, BuiltinInstrument::FingerBass, "ベース");
    let doc = recorded(&doc, bass, 0, 2 * BAR, vec![note(0, 3_840, 40, 64)]);
    let doc = edit::toggle_mute(&doc, bass).unwrap().doc;
    let (doc, empty) = keys_track(&doc, BuiltinInstrument::GrandPiano, "空");
    let parts = Parts {
        piano,
        drums,
        vocals,
        bass,
        empty,
    };
    (doc, parts)
}

fn place(doc: &Doc, id: TrackId) -> usize {
    doc.tracks.iter().position(|t| t.id == id).unwrap()
}

fn smf_note(start: u64, end: u64, channel: u8, key: u8, velocity: u8) -> SmfNote {
    SmfNote {
        start,
        end,
        channel,
        key,
        velocity,
    }
}

#[test]
fn the_whole_song_reads_back_with_its_tempos_meters_and_tracks() {
    let (doc, p) = song();
    assert!(doc.tracks[place(&doc, p.bass)].muted);
    let keys: Vec<usize> = [p.piano, p.drums, p.bass, p.empty]
        .map(|id| place(&doc, id))
        .to_vec();
    let made = midi_file(&doc, &keys, None, 0, 4 * BAR, 1920).unwrap();
    assert_eq!((made.notes, made.cut, made.ramps), (4 + 8 + 1, 0, true));
    let channels: Vec<(usize, usize, u8)> = (made.tracks.iter())
        .map(|t| (t.index, t.notes, t.channel))
        .collect();
    assert_eq!(
        channels,
        [
            (keys[0], 4, 0),
            (keys[1], 8, DRUM_CHANNEL),
            (keys[2], 1, 0),
            (keys[3], 0, 0)
        ]
    );

    let read = smf::read(&smf::write(&made.file)).unwrap();
    assert_eq!(
        (read.format, read.division, read.tracks.len()),
        (1, 1920, 5)
    );
    let head = &read.tracks[0];
    assert_eq!(head.name.as_deref(), Some("テストの曲"));
    assert!(head.notes.is_empty());
    assert_eq!(head.length, 4 * BAR);
    assert_eq!(head.meters, [(0, 4, 2), (3 * BAR, 3, 2)]);
    // 120 a minute; the ramp as a step each sixteenth note, slower and slower; 60; 100.
    let map = TempoMap::new(&doc.timeline);
    assert_eq!(head.tempos.len(), 1 + 16 + 1 + 1, "{:?}", head.tempos);
    assert_eq!(head.tempos[0], (0, 500_000));
    let ramp = &head.tempos[1..17];
    for (k, &(tick, us)) in ramp.iter().enumerate() {
        assert_eq!(tick, BAR + k as u64 * 480);
        let bpm = map.tempo_at((tick + 240) as f64);
        assert_eq!(us, (60_000_000.0 / bpm).round() as u32);
    }
    assert!(ramp.windows(2).all(|w| w[0].1 < w[1].1));
    assert!(ramp[0].1 > 500_000 && ramp[15].1 < 1_000_000);
    assert_eq!(
        head.tempos[17..],
        [(2 * BAR, 1_000_000), (3 * BAR, 600_000)]
    );

    let names: Vec<Option<&str>> = read.tracks[1..].iter().map(|t| t.name.as_deref()).collect();
    assert_eq!(
        names,
        [Some("ピアノ"), Some("ドラム"), Some("ベース"), Some("空")]
    );
    // The last note rings past the clip's end: it ends there.
    assert_eq!(
        read.tracks[1].notes,
        [
            smf_note(0, 960, 0, 60, 100),
            smf_note(BAR - 480, BAR + 480, 0, 62, 90),
            smf_note(2 * BAR, 2 * BAR + 1_920, 0, 64, 80),
            smf_note(4 * BAR - 480, 4 * BAR, 0, 65, 70),
        ]
    );
    let drums = &read.tracks[2].notes;
    assert_eq!(drums.len(), 8);
    assert!(drums.iter().all(|n| n.channel == DRUM_CHANNEL));
    assert_eq!(drums[1], smf_note(BAR + 960, BAR + 1_360, 9, 38, 110));
    // Muted, and still written.
    assert_eq!(read.tracks[3].notes, [smf_note(0, 3_840, 0, 40, 64)]);
    // No notes: an empty track all the same.
    assert!(read.tracks[4].notes.is_empty());
    assert!(
        read.tracks
            .iter()
            .all(|t| t.length == 4 * BAR && t.hanging == 0)
    );
}

#[test]
fn a_range_takes_the_notes_that_start_in_it_and_cuts_them_at_its_end() {
    let (doc, p) = song();
    let keys = [
        place(&doc, p.piano),
        place(&doc, p.drums),
        place(&doc, p.bass),
    ];
    let (from, to) = (BAR, 2 * BAR + 960);
    let made = midi_file(&doc, &keys, None, from, to, 1920).unwrap();
    // The piano's note across bar 2 started before the range; the one at bar 3 is cut.
    assert_eq!((made.notes, made.cut), (1 + 8, 1));
    let read = smf::read(&smf::write(&made.file)).unwrap();
    let head = &read.tracks[0];
    assert_eq!(head.length, to - from);
    assert_eq!(head.meters, [(0, 4, 2)]);
    assert_eq!(head.tempos.len(), 16 + 1);
    assert_eq!(head.tempos[0].0, 0);
    assert_eq!(head.tempos[16], (BAR, 1_000_000));
    assert_eq!(read.tracks[1].notes, [smf_note(BAR, BAR + 960, 0, 64, 80)]);
    assert_eq!(read.tracks[2].notes[0], smf_note(0, 400, 9, 36, 110));
    // The bass's one note started before the range: its track is empty.
    assert!(read.tracks[3].notes.is_empty());
    assert_eq!(read.tracks[3].name.as_deref(), Some("ベース"));

    // A range from the middle of 3/4 bars: the meter it starts in.
    let made = midi_file(&doc, &keys[..1], None, 3 * BAR + 100, 4 * BAR, 1920).unwrap();
    let read = smf::read(&smf::write(&made.file)).unwrap();
    assert_eq!(read.tracks[0].meters, [(0, 3, 2)]);
    assert_eq!(read.tracks[0].tempos, [(0, 600_000)]);
    assert_eq!(read.tracks[1].notes.len(), 1);
}

#[test]
fn coarser_ticks_one_clip_and_what_cannot_be_written() {
    let (doc, p) = song();
    let drums = place(&doc, p.drums);
    let clip = doc.tracks[drums].clips[0].id;
    let made = midi_file(&doc, &[drums], Some(clip), BAR, 2 * BAR, 480).unwrap();
    let read = smf::read(&smf::write(&made.file)).unwrap();
    assert_eq!(read.division, 480);
    assert_eq!(read.tracks[1].notes[1], smf_note(240, 340, 9, 38, 110));
    assert_eq!(read.tracks[0].length, BAR / 4);
    // Another track's clip is not that one: nothing of it.
    let piano = place(&doc, p.piano);
    let made = midi_file(&doc, &[piano], Some(clip), 0, 4 * BAR, 1920).unwrap();
    assert_eq!(made.notes, 0);

    // A vocal track holds no notes; a place with no track is passed by.
    let vocals = place(&doc, p.vocals);
    let made = midi_file(&doc, &[vocals, 99], None, 0, 4 * BAR, 1920).unwrap();
    assert_eq!(made.tracks.len(), 1);
    assert_eq!(made.file.tracks[0].notes, []);

    // An empty range still says its tempo and meter.
    let made = midi_file(&doc, &[piano], None, 2 * BAR, 2 * BAR, 1920).unwrap();
    assert_eq!(made.file.tempos, [(0, 1_000_000)]);
    assert_eq!(made.file.meters, [(0, 4, 4)]);
    assert_eq!((made.file.length, made.notes), (0, 0));

    let longest = smf::MAX_OUT_TICKS;
    assert!(midi_file(&doc, &[piano], None, 0, longest, 1920).is_ok());
    assert_eq!(
        midi_file(&doc, &[piano], None, 0, longest + 1, 1920),
        Err(MidiTooLong)
    );
    assert_eq!(
        midi_file(&doc, &[piano], None, 5, longest + 5, 1920).map(|m| m.file.length),
        Ok(longest)
    );
}

#[test]
fn a_file_is_written_beside_and_moved_over_the_old_one() {
    let dir = crate::settings::temp_dir("midi-beside");
    let path = dir.join("曲.mid");
    write_beside(&path, b"old").unwrap();
    write_beside(&path, b"new").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "{left:?}");
    // A folder that is not there: nothing is left behind.
    let nowhere = dir.join("nowhere").join("曲.mid");
    assert!(write_beside(&nowhere, b"x").is_err());
    assert!(!dir.join("nowhere").exists());
}
