use std::sync::Arc;

use meloet_model::effects::EffectKind;
use meloet_model::{AutoTarget, Bus, InsertId, Pan, StripId, automation, mixer};
use meloet_protocol::{ExportFormat, ExportLoudness, ExportTail, RecordFormat};

use super::*;
use crate::audiofiles::bounced::adopt;
use crate::export::fixture::{BAR, Place, RATE, Run, read_wav, song};
use crate::export::tags::SongInfo;
use crate::export::{ExportJob, ExportNews, ExportTarget};

fn render(place: &Place, id: u64, doc: &Doc, span: (u64, u64), tail: ExportTail) -> Run {
    let job = ExportJob {
        id,
        doc: Arc::new(doc.clone()),
        span: TickRange::new(span.0, span.1).unwrap(),
        per_track: false,
        tail,
        realtime: false,
        skip: Vec::new(),
        sample_rate: RATE,
        measure: None,
    };
    let run = place.start(place.setup(job));
    run.rendered();
    run
}

#[test]
fn the_track_alone_keeps_its_sound_and_nothing_of_the_mix() {
    let (doc, parts) = song();
    let lead = doc.track(parts.lead).unwrap();
    let strip = StripId::Track(parts.lead);
    let doc = mixer::set_pan(&doc, strip, Pan::new(40)).unwrap().doc;
    let doc = meloet_model::edit::toggle_mute(&doc, parts.lead)
        .unwrap()
        .doc;
    let delay: InsertId = lead.channel.inserts[0].as_ref().unwrap().id;
    let (e, lane) = automation::add_lane(
        &doc,
        parts.lead,
        AutoTarget::Insert {
            insert: delay,
            param: 0,
        },
        false,
    )
    .unwrap();
    let doc = automation::add_point(&e.doc, lane, 0, Some(0.5), 0.5)
        .unwrap()
        .0
        .doc;
    let (e, _) = automation::add_lane(&doc, parts.lead, AutoTarget::Send(Bus::A), false).unwrap();
    let doc = e.doc;

    let alone = bounce_doc(&doc, parts.lead).unwrap();
    let [t] = &alone.tracks[..] else {
        panic!("one track")
    };
    let lead = doc.track(parts.lead).unwrap();
    assert!(!t.muted);
    assert_eq!(
        t.channel,
        Channel {
            inserts: lead.channel.inserts.clone(),
            ..Channel::default()
        }
    );
    assert_eq!(t.sends, [Gain::SILENT; 2]);
    let targets: Vec<AutoTarget> = t.automation.iter().map(|l| l.target).collect();
    assert_eq!(
        targets,
        [AutoTarget::Insert {
            insert: delay,
            param: 0
        }],
        "not the lanes of the fader and the send"
    );
    assert_eq!((&t.clips, &t.instrument), (&lead.clips, &lead.instrument));
    assert_eq!(*alone.master, Channel::default());
    assert!(alone.master_lanes.is_empty());
    assert!(
        alone
            .returns
            .iter()
            .all(|r| r.muted && r.channel == Channel::default())
    );
    assert!(alone.audio.is_empty() && alone.loop_range.is_none());
    assert_eq!(alone.timeline, doc.timeline);
    assert!(bounce_doc(&doc, TrackId(999)).is_none());
}

/// The track rendered alone, written as the bounce writes it and taken into the work place,
/// then placed by the step: the song plays as it did before, and the track alone sample for
/// sample. The lead's delay does not follow the tempo here: an effect that does reads the
/// tempo once for each stretch between the notes of every track (`engine.rs`
/// `frames_to_next_point`), so inside a ramp it sounds a little otherwise with other tracks
/// than alone.
#[test]
fn the_audio_at_its_place_sounds_as_the_track_did() {
    let place = Place::new("bounce-same");
    let (doc, parts) = song();
    let strip = StripId::Track(parts.lead);
    let doc = mixer::set_param(&doc, strip, 0, EffectKind::Delay, 2, 0.0)
        .unwrap()
        .doc;
    place.files_of(&doc);
    let lead = doc.track(parts.lead).unwrap();
    let span = (lead.clips[0].start, lead.clips.last().unwrap().end);
    assert!(
        span.1 > 4 * BAR,
        "the range goes through the tempo's changes"
    );

    let (after, rendered) = bounced(&place, &doc, parts.lead, span);
    assert!(rendered.iter().any(|s| s.abs() > 0.01), "the track sounds");
    let was = render(&place, 2, &doc, span, ExportTail::Off).pass(0);
    let now = render(&place, 3, &after, span, ExportTail::Off).pass(0);
    assert_eq!(was.len(), now.len());
    let off = (was.iter().zip(&now)).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(off < 1e-4, "the song sounds as it did but for {off}");

    // The track alone, without its lane of the fader (it is read in steps that begin where
    // the notes are), is the new track alone, to the sample.
    let only = |doc: &Doc, keep: TrackId| {
        let mut doc = doc.clone();
        doc.tracks.retain(|t| t.id == keep);
        let t = Arc::make_mut(&mut doc.tracks[0]);
        t.muted = false;
        t.automation.clear();
        doc
    };
    let new_track = after.tracks[1].id;
    let was = render(&place, 4, &only(&doc, parts.lead), span, ExportTail::Off).pass(0);
    let now = render(&place, 5, &only(&after, new_track), span, ExportTail::Off).pass(0);
    assert!(was.iter().any(|s| s.abs() > 0.01));
    assert!(was == now, "the track sounds as it did, to the sample");
}

/// Renders `track` alone over `span` as a bounce does, writes it in 32-bit floats into the
/// work place, takes it in and makes the step. Returns the song after the step and the
/// rendered samples.
fn bounced(place: &Place, doc: &Doc, track: TrackId, span: (u64, u64)) -> (Doc, Vec<f32>) {
    let alone = bounce_doc(doc, track).unwrap();
    let run = render(place, 1, &alone, span, ExportTail::UntilSilent);
    let rendered = run.pass(0);
    let work = place.store.work_dir().unwrap();
    let temp = work.join(".bounce-test.wav");
    run.handle.finish(ExportTarget {
        path: temp.clone(),
        format: ExportFormat::Wav,
        sample_rate: RATE,
        bits: RecordFormat::Bits32,
        loudness: ExportLoudness::AsIs,
        info: SongInfo::default(),
    });
    run.until("finished", |n| matches!(n, ExportNews::Finished { .. }));
    assert_eq!(read_wav(&temp), rendered, "written as rendered");
    let audio = adopt(&place.store, &temp, "リード（音） 50.wav", &[]).unwrap();
    assert_eq!(audio.frames as usize, rendered.len() / 2);
    let after = bounce_track(doc, track, audio, span.0).unwrap().edit.doc;
    (place.store).set_sources(
        after
            .audio
            .iter()
            .map(|s| (s.uid, s.file.clone()))
            .collect(),
    );
    (after, rendered)
}

#[test]
fn a_tail_that_rings_on_is_in_the_audio_up_to_ten_seconds() {
    let place = Place::new("bounce-tail");
    let (doc, parts) = song();
    let strip = StripId::Track(parts.lead);
    let doc = mixer::set_param(&doc, strip, 0, EffectKind::Delay, 3, 80.0)
        .unwrap()
        .doc;
    let alone = bounce_doc(&doc, parts.lead).unwrap();
    let span = (0, BAR);
    let with = render(&place, 1, &alone, span, ExportTail::UntilSilent).pass(0);
    let without = render(&place, 2, &alone, span, ExportTail::Off).pass(0);
    assert!(with.len() > without.len(), "the delay rings on");
    assert!(with.len() / 2 <= without.len() / 2 + 10 * RATE as usize);
    assert_eq!(&with[..without.len()], &without[..]);
}
