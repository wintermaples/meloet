//! The export against the playing side (docs/architecture.md 22-11): the same song is
//! played by the core's audio side, as it plays for a stream, and rendered by a job of the
//! export. Not one sample differs.
//!
//! The playing side here is all of the real one but the device: the rules open the song
//! and send their ops, `Audio::op` turns them into engine commands, the read-ahead thread
//! fills the rings, the maker's thread makes the audio of the stretched clips. The test
//! stands in for the stream and calls the engine's blocks.

use std::sync::mpsc::Receiver;

use meloet_engine::BlockTiming;
use meloet_model::song::Doc;
use meloet_model::{Song, StripId, edit, mixer};
use meloet_protocol::ExportTail;

use super::*;
use crate::export::fixture::{BAR, BLOCK, Place, RATE, loud, song};
use crate::play::Play;

struct Playing {
    play: Play,
    audio: Audio,
    news: Receiver<MadeNews>,
}

impl Playing {
    /// meloet as it starts, with a stream of [`RATE`] open.
    fn new(place: &Place) -> Playing {
        let mut play = Play::new(false, 1);
        let parts = Engine::new(EngineConfig {
            max_tracks: MAX_TRACKS,
            ..EngineConfig::default()
        });
        let (tx, news) = mpsc::channel();
        let maker = Maker::spawn(place.store.clone(), move |news| {
            let _ = tx.send(news);
        })
        .unwrap();
        let audio = Audio::new(
            parts.engine,
            parts.handle,
            false,
            Library::open(Vec::new(), Box::new(|_, _| {})),
            Sf2Store::open(None, Box::new(|_| {})),
            play.engine_seats(),
            (BLOCK as u32, RATE),
        )
        .with_store(place.store.clone())
        .with_maker(Some(maker));
        let mut playing = Playing { play, audio, news };
        playing.audio.open_by_hand(RATE);
        let mut effects = Vec::new();
        playing.play.stream_opened(&mut effects);
        playing.tell(effects);
        playing
    }

    fn tell(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            if let Effect::Engine(op) = effect {
                self.audio.op(op);
            }
        }
        self.audio.want_made();
    }

    fn block(&mut self, out: &mut [f32]) {
        let engine = self.audio.engine.as_mut().expect("the test is the stream");
        engine.process(out, BlockTiming::default());
        self.audio.handle.drop_retired();
    }

    /// The person opens the song, and waits until 「音を作っています」 is gone.
    fn open(&mut self, doc: &Doc, made: usize) {
        let mut session = Song::new_default().session;
        session.play_target = doc.tracks.first().map(|t| t.id);
        let song = Song {
            doc: doc.clone(),
            session,
        };
        let mut effects = Vec::new();
        self.play.replace_song(song, false, &mut effects);
        self.play.sync_silent(&mut effects);
        self.play.sync_target(&mut effects);
        self.play.sync_mix(&mut effects);
        self.play.sync_plugs(&mut effects);
        self.play.sync_automation(&mut effects);
        self.tell(effects);
        let until = Instant::now() + Duration::from_secs(120);
        let mut out = [0.0f32; 2 * BLOCK];
        while self.audio.made.len() < made {
            assert!(
                Instant::now() < until,
                "the audio of the clips was not made"
            );
            while let Ok(news) = self.news.try_recv() {
                if let MadeNews::Ready(keys) = news {
                    self.audio.made_ready(keys);
                }
            }
            self.block(&mut out);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// The person goes to `from`, waits a moment and plays: what comes out of the engine
    /// for `frames` frames, in blocks of [`BLOCK`].
    fn play(&mut self, from: u64, frames: usize) -> Vec<f32> {
        let mut out = [0.0f32; 2 * BLOCK];
        self.audio.op(EngineOp::Stop { t_us: 0 });
        self.audio.op(EngineOp::Locate { tick: from });
        for _ in 0..60 {
            self.block(&mut out);
            std::thread::sleep(Duration::from_millis(5));
        }
        self.audio.op(EngineOp::Play { from });
        let mut heard = Vec::with_capacity(2 * frames);
        let mut blocks = 0;
        while heard.len() < 2 * frames {
            let n = BLOCK.min(frames - heard.len() / 2);
            self.block(&mut out[..2 * n]);
            heard.extend_from_slice(&out[..2 * n]);
            // The stream gives the read-ahead thread time: a block is 5 ms long.
            blocks += 1;
            if blocks % 8 == 0 {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        assert_eq!(self.audio.handle.meters().stream_missing(), 0);
        heard
    }

    /// Stops and waits until the tails of the instruments and the effects are gone: two
    /// seconds of nothing at all.
    fn quiet(&mut self) {
        self.audio.op(EngineOp::Stop { t_us: 0 });
        let mut out = [0.0f32; 2 * BLOCK];
        let (mut silent, mut blocks) = (0, 0);
        while silent < 2 * RATE as usize {
            self.block(&mut out);
            silent = if out.iter().all(|&s| s == 0.0) {
                silent + BLOCK
            } else {
                0
            };
            blocks += 1;
            assert!(blocks * BLOCK < 120 * RATE as usize, "the tails do not end");
        }
    }
}

fn frames_of(doc: &Doc, start: u64, end: u64) -> usize {
    let map = TempoMap::new(&doc.timeline);
    map.frame_of(end as f64, map.seconds_at(start as f64), f64::from(RATE)) as usize
}

fn first_difference(a: &[f32], b: &[f32]) -> Option<(usize, f32, f32)> {
    (a.iter().zip(b))
        .position(|(a, b)| a.to_bits() != b.to_bits())
        .map(|at| (at / 2, a[at], b[at]))
}

#[test]
fn what_is_exported_is_what_plays_sample_for_sample() {
    let place = Place::new("export-same");
    let (doc, _) = song();
    place.files_of(&doc);
    // One engine plays each range twice; the second time is what is compared. The lead
    // starts its waves and the kit its noise where the place of each note says, not where
    // the last playing left off (docs/architecture.md 22-25).
    let mut playing = Playing::new(&place);
    playing.open(&doc, 2);
    let mut twice = |span: (u64, u64), frames: usize| {
        let first = playing.play(span.0, frames);
        playing.quiet();
        let second = playing.play(span.0, frames);
        assert_eq!(
            first_difference(&first, &second),
            None,
            "{span:?}: played twice"
        );
        playing.quiet();
        second
    };

    // The whole song; from the middle of a note, a clip and the ramp; the stretched clip.
    let spans = [
        (0, 6 * BAR),
        (2 * BAR + 1_234, 3 * BAR + 77),
        (5 * BAR, 6 * BAR),
    ];
    for (id, span) in spans.into_iter().enumerate() {
        let frames = frames_of(&doc, span.0, span.1);
        let heard = twice(span, frames);
        assert!(heard.iter().any(|s| s.abs() > 0.05), "the song sounds");
        let job = place.job(id as u64 + 1, &doc, span, ExportTail::Off);
        let run = place.start(place.setup(job));
        run.rendered();
        let written = run.pass(0);
        assert_eq!(written.len(), heard.len());
        assert_eq!(
            first_difference(&written, &heard),
            None,
            "{span:?}: the first frame that differs, exported and played"
        );
    }

    // With a tail the range is still the same audio.
    let span = (0, 2 * BAR);
    let frames = frames_of(&doc, span.0, span.1);
    let heard = twice(span, frames);
    let job = place.job(9, &doc, span, ExportTail::UntilSilent);
    let run = place.start(place.setup(job));
    run.rendered();
    assert_eq!(first_difference(&run.pass(0)[..2 * frames], &heard), None);
}

#[test]
fn what_goes_over_0_db_is_exported_as_it_is_and_cut_as_it_plays() {
    let place = Place::new("export-same-loud");
    let (doc, _) = song();
    let doc = loud(&doc, false);
    place.files_of(&doc);
    let mut playing = Playing::new(&place);
    playing.open(&doc, 2);
    let span = (0, 2 * BAR);
    let frames = frames_of(&doc, span.0, span.1);
    playing.play(span.0, frames);
    playing.quiet();
    let heard = playing.play(span.0, frames);
    // The output is cut at 0 dB: ears and speakers are spared.
    assert!(heard.iter().all(|s| s.abs() <= 1.0));
    assert!(heard.iter().any(|s| s.abs() == 1.0), "the song is not loud");

    let run = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    run.rendered();
    let written = run.pass(0);
    assert_eq!(written.len(), heard.len());
    assert!(written.iter().any(|s| s.abs() > 1.0));
    let cut: Vec<f32> = written.iter().map(|s| s.clamp(-1.0, 1.0)).collect();
    assert_eq!(
        first_difference(&cut, &heard),
        None,
        "the first frame that differs, exported and cut, and played"
    );
}

/// Round 12 E3: the knobs of the lead and of the kit turned, and a lane on the lead's
/// 「明るさ」. Played from the head twice, and exported: no frame differs. The master has no
/// limiter here: once a limiter has held the sound down, its gain comes back to just under 1
/// and stays there, so a second play would differ from the first in the last bits.
#[test]
fn turned_knobs_are_exported_as_they_play() {
    use meloet_model::SoundKnob as K;
    use meloet_model::automation::{AutoTarget, add_lane, add_point};
    use meloet_model::knobs::set_sound_knob;
    use meloet_model::mixer::remove_effect;

    let place = Place::new("export-knobs");
    let (doc, parts) = song();
    let doc = remove_effect(&doc, meloet_model::StripId::Master, 0)
        .unwrap()
        .doc;
    let turn = [
        (parts.lead, K::Attack, 70.0),
        (parts.lead, K::Release, 80.0),
        (parts.lead, K::Vibrato, 60.0),
        (parts.drums, K::Brightness, 80.0),
        (parts.drums, K::Release, 30.0),
    ];
    let doc = turn.into_iter().fold(doc, |doc, (track, knob, value)| {
        set_sound_knob(&doc, track, knob, value).unwrap().doc
    });
    let brightness = AutoTarget::Instrument { param: 0 };
    let (e, lane) = add_lane(&doc, parts.lead, brightness, false).unwrap();
    let (e, lane, _) = add_point(&e.doc, lane, 0, Some(0.2), 0.5).unwrap();
    let (e, _, _) = add_point(&e.doc, lane, 3 * BAR, Some(0.9), 0.5).unwrap();
    let doc = e.doc;
    place.files_of(&doc);
    let mut playing = Playing::new(&place);
    playing.open(&doc, 2);
    let span = (0, 6 * BAR);
    let frames = frames_of(&doc, span.0, span.1);
    let first = playing.play(span.0, frames);
    playing.quiet();
    let heard = playing.play(span.0, frames);
    assert_eq!(first_difference(&first, &heard), None, "played twice");
    assert!(heard.iter().any(|s| s.abs() > 0.05), "the song sounds");

    let run = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    run.rendered();
    let written = run.pass(0);
    assert_eq!(written.len(), heard.len());
    assert_eq!(
        first_difference(&written, &heard),
        None,
        "the first frame that differs, exported and played"
    );

    let (plain, _) = song();
    let plain = remove_effect(&plain, meloet_model::StripId::Master, 0)
        .unwrap()
        .doc;
    place.files_of(&plain);
    let run = place.start(place.setup(place.job(2, &plain, span, ExportTail::Off)));
    run.rendered();
    assert!(
        first_difference(&run.pass(0), &heard).is_some(),
        "the knobs are heard"
    );
}

/// A fade-out on the master (round 12, M5): the file is what plays, sample for sample; up to
/// the fade it is the song without one; the fader goes down its line; the song ends in
/// silence, and so the tail is over at once.
#[test]
fn a_fade_out_is_exported_as_it_plays_and_ends_in_silence() {
    let place = Place::new("export-fade-out");
    let (plain, _) = song();
    let doc = meloet_model::automation::add_fade_out(&plain, 2).unwrap();
    assert_eq!(
        doc.step,
        meloet_model::StepKind::AddFadeOut {
            range: meloet_model::TickRange::new(4 * BAR, 6 * BAR).unwrap()
        }
    );
    let doc = doc.doc;
    place.files_of(&doc);
    let mut playing = Playing::new(&place);
    playing.open(&doc, 2);
    let span = (0, 6 * BAR);
    let frames = frames_of(&doc, span.0, span.1);
    playing.play(span.0, frames);
    playing.quiet();
    let heard = playing.play(span.0, frames);

    let run = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    run.rendered();
    let faded = run.pass(0);
    assert_eq!(faded.len(), heard.len());
    assert_eq!(
        first_difference(&faded, &heard),
        None,
        "the first frame that differs, exported and played"
    );

    let run = place.start(place.setup(place.job(2, &plain, span, ExportTail::Off)));
    run.rendered();
    let whole = run.pass(0);
    assert_eq!(whole.len(), faded.len());
    // Before the fade the master's lane holds the fader where the hand put it.
    let start = frames_of(&doc, 0, 4 * BAR);
    let far = (faded[..2 * (start - BLOCK)].iter())
        .zip(&whole)
        .fold(0.0f32, |far, (a, b)| far.max((a - b).abs()));
    assert!(far < 1e-6, "{far} before the fade");
    // Half way down the line the fader is at the middle of its way.
    let level = |samples: &[f32], frame: usize| {
        let piece = &samples[2 * frame..2 * (frame + 2 * BLOCK)];
        (piece.iter().map(|s| s * s).sum::<f32>() / piece.len() as f32).sqrt()
    };
    let half = frames_of(&doc, 0, 5 * BAR);
    let fader = plain.master.gain.position();
    let want = meloet_model::Gain::from_position(fader / 2.0).linear();
    let got = level(&faded, half - BLOCK) / level(&whole, half - BLOCK);
    assert!((got / want - 1.0).abs() < 0.1, "{got}, not {want}");
    // The end is silent where the song without the fade still sounds.
    let last = |samples: &[f32]| {
        let tail = &samples[samples.len() - 2 * BLOCK / 4..];
        tail.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    };
    assert!(last(&faded) < 1e-3, "{}", last(&faded));
    assert!(
        level(&whole, frames - 2 * BLOCK) > 1e-3,
        "the song sounds to its end"
    );

    // With 「余韻」 the plain song rings on; the faded one has nothing left to ring.
    let tail = |id: u64, doc: &Doc| {
        let run = place.start(place.setup(place.job(id, doc, span, ExportTail::UntilSilent)));
        run.rendered();
        run.pass(0)
    };
    let ringing = tail(3, &plain);
    assert!(ringing.len() / 2 - frames > RATE as usize / 2);
    let ended = tail(4, &doc);
    let more = ended.len() / 2 - frames;
    assert!(more < RATE as usize / 4, "{more} frames of tail");
    assert_eq!(first_difference(&ended[..2 * frames], &faded), None);
    assert!(ended[2 * frames..].iter().all(|s| s.abs() < 1e-3));

    // A lane at rest plays nothing: the file is the plain song's, sample for sample.
    let rested = meloet_model::automation::rest_lane(
        &doc,
        meloet_model::LaneHome::Master,
        meloet_model::AutoTarget::Volume,
    );
    let rested = rested.unwrap().doc;
    let run = place.start(place.setup(place.job(5, &rested, span, ExportTail::Off)));
    run.rendered();
    assert_eq!(first_difference(&run.pass(0), &whole), None);
}

/// The last clip ends inside a bar: 「曲ぜんぶ」 ends there, and so does the fade-out, so the
/// file's last frames are at the fade's end, silent, and are what plays.
#[test]
fn a_fade_out_reaches_its_end_at_the_last_frame_of_the_whole_song() {
    let place = Place::new("export-fade-end");
    let (doc, parts) = song();
    let note = |start: u64| meloet_model::Note {
        start,
        length: 3 * BAR / 4 - start,
        key: 60 + (start / 480) as u8,
        velocity: 110,
        from: None,
    };
    let take = meloet_model::NewTake {
        origin: 6 * BAR,
        length: 3 * BAR / 4,
        notes: vec![note(0), note(480), note(960)],
        controls: Vec::new(),
    };
    let plain = (edit::add_recording(&doc, parts.lead, vec![take], 0).unwrap())
        .0
        .doc;
    let end = meloet_model::sections::last_clip_end(&plain);
    assert_eq!(end, 6 * BAR + 3 * BAR / 4, "inside bar 7");
    let faded = meloet_model::automation::add_fade_out(&plain, 2).unwrap();
    assert_eq!(
        faded.step,
        meloet_model::StepKind::AddFadeOut {
            range: meloet_model::TickRange::new(end - 2 * BAR, end).unwrap()
        }
    );
    let doc = faded.doc;
    place.files_of(&doc);
    let span = (0, end);
    let frames = frames_of(&doc, span.0, span.1);
    let pass = |id: u64, doc: &Doc| {
        let run = place.start(place.setup(place.job(id, doc, span, ExportTail::Off)));
        run.rendered();
        run.pass(0)
    };
    let written = pass(1, &doc);
    assert_eq!(written.len(), 2 * frames);
    let whole = pass(2, &plain);
    let last = |samples: &[f32]| {
        let tail = &samples[samples.len() - 2 * BLOCK / 4..];
        tail.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    };
    assert!(last(&written) < 1e-3, "{}", last(&written));
    assert!(last(&whole) > 1e-2, "the song sounds to its end");
    // Half a bar before the end the fade is not over yet.
    let near = frames_of(&doc, 0, end - BAR / 2);
    let near = &written[2 * near..2 * (near + BLOCK)];
    assert!(near.iter().any(|s| s.abs() > 1e-4), "silent before the end");

    let mut playing = Playing::new(&place);
    playing.open(&doc, 2);
    playing.play(span.0, frames);
    playing.quiet();
    let heard = playing.play(span.0, frames);
    assert_eq!(
        first_difference(&written, &heard),
        None,
        "the first frame that differs, exported and played"
    );
}

/// A song with one vocal clip over bars 2 and 3 with three takes, each of a file of its
/// own: the first two hold the same sine, the third another one, a quarter of a turn ahead.
/// The master has no limiter.
fn three_takes(place: &Place) -> (Doc, meloet_model::ClipId) {
    use meloet_model::{NewAudio, NewAudioTake, VocalRecording, add_vocal_recordings};

    let doc = Doc::new_default();
    let doc = (mixer::remove_effect(&doc, StripId::Master, 0).unwrap()).doc;
    let (e, vocals) = edit::add_vocal_track(&doc, None, meloet_model::Language::Ja).unwrap();
    let mut doc = e.doc;
    let mut clip = None;
    for n in 1..=3u8 {
        let recording = VocalRecording {
            track: vocals,
            audio: NewAudio {
                uid: [n; 16],
                file: format!("歌 {n}.wav"),
                frames: 6 * u64::from(RATE),
                sample_rate: RATE,
                channels: 1,
                bits: None,
            },
            takes: vec![NewAudioTake {
                origin: BAR,
                length: 2 * BAR,
                first_frame: 2_400,
                frames: 5 * u64::from(RATE),
            }],
        };
        let (e, added) = add_vocal_recordings(&doc, vec![recording], None).unwrap();
        doc = e.doc;
        clip = Some(added[0].clip);
    }
    let work = place.store.work_dir().unwrap();
    for (n, source) in doc.audio.iter().enumerate() {
        let samples: Vec<f32> = (0..source.frames)
            .map(|i| {
                let turns = |hz: f64| (i as f64 * hz / f64::from(RATE)).fract();
                let value = match n {
                    2 => (turns(330.0) * std::f64::consts::TAU).cos(),
                    _ => (turns(220.0) * std::f64::consts::TAU).sin(),
                };
                (value * 0.3) as f32
            })
            .collect();
        let path = work.join(&source.file);
        let mut wav = crate::audiofiles::wav::CacheWav::create(&path, RATE, 1).unwrap();
        wav.write(&samples).unwrap();
        wav.finish().unwrap();
    }
    (place.store).set_sources(doc.audio.iter().map(|s| (s.uid, s.file.clone())).collect());
    (doc, clip.unwrap())
}

/// The largest step from one frame to the next, on the left.
fn largest_step(audio: &[f32]) -> f32 {
    (audio.as_chunks::<2>().0.windows(2))
        .map(|w| (w[1][0] - w[0][0]).abs())
        .fold(0.0, f32::max)
}

/// How far apart two renderings are at most, between two places.
fn apart(a: &[f32], b: &[f32], from: usize, to: usize) -> f32 {
    (a[from..to].iter().zip(&b[from..to]))
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
}

#[test]
fn a_comp_has_no_click_at_its_seams_and_is_exported_as_it_plays() {
    use meloet_model::{CompPiece, set_comp};

    let place = Place::new("export-comp");
    let (doc, clip) = three_takes(&place);
    let export = |id: u64, doc: &Doc| {
        let job = place.job(id, doc, (0, 4 * BAR), ExportTail::Off);
        let run = place.start(place.setup(job));
        run.rendered();
        run.pass(0)
    };
    // The first take whole; then the first, the second (the same audio), the third (other
    // audio, far from the first at both of its seams) and the first again.
    let plain = edit::switch_take(&doc, clip, 0).unwrap().doc;
    let pieces: Vec<CompPiece> = [(0, 0), (BAR / 2, 1), (BAR, 2), (3 * BAR / 2, 0)]
        .map(|(from, take)| CompPiece { from, take })
        .to_vec();
    let comped = set_comp(&doc, clip, (BAR, 3 * BAR), &pieces).unwrap().doc;
    let ms = 2 * RATE as usize / 1_000;

    let whole = export(1, &plain);
    let heard = export(2, &comped);
    assert_eq!(whole.len(), heard.len());
    let at = |tick: u64| 2 * frames_of(&comped, 0, tick);
    let sine = largest_step(&whole);
    assert!(sine > 0.001, "the take sounds");
    // Where the first two takes sound, their seam in the middle, it is the take whole.
    let same = apart(&whole, &heard, at(BAR), at(2 * BAR) - 6 * ms);
    assert!(same < 1e-6, "the same audio over a seam: {same}");
    // The third take does sound, and is not the first.
    let other = apart(
        &whole,
        &heard,
        at(2 * BAR) + 6 * ms,
        at(5 * BAR / 2) - 6 * ms,
    );
    assert!(other > 0.05, "the third take: {other}");
    // A cut from one take to the other would be a step of ten times the sine's largest;
    // the seams add next to nothing to the steeper of the two waves.
    let step = largest_step(&heard);
    assert!(step < 1.8 * sine, "a click: {step} against {sine}");

    // What plays is what was exported, sample for sample.
    let mut playing = Playing::new(&place);
    playing.open(&comped, 0);
    let frames = frames_of(&comped, 0, 4 * BAR);
    playing.play(0, frames);
    playing.quiet();
    let played = playing.play(0, frames);
    assert_eq!(first_difference(&heard, &played), None);
    playing.quiet();

    // The song gets slower: every take that sounds is stretched by itself, for the whole
    // clip, and the pieces are still in their places.
    let slow = |doc: &Doc| {
        let tempo = meloet_model::Tempo::from_centi(10_000).unwrap();
        meloet_model::timeline::set_tempo(doc, tempo).unwrap().doc
    };
    let (plain, comped) = (slow(&plain), slow(&comped));
    let stretch = place.root.join("cache").join(crate::audiofiles::CACHE_MADE);
    let made = || std::fs::read_dir(&stretch).map_or(0, |d| d.count());
    let whole = export(3, &plain);
    assert_eq!(made(), 1);
    let heard = export(4, &comped);
    assert_eq!(made(), 3, "one for each take that sounds");
    let at = |tick: u64| 2 * frames_of(&comped, 0, tick);
    let same = apart(&whole, &heard, at(BAR), at(2 * BAR) - 6 * ms);
    assert!(same < 1e-6, "the same stretched audio over a seam: {same}");
    let other = apart(
        &whole,
        &heard,
        at(2 * BAR) + 6 * ms,
        at(5 * BAR / 2) - 6 * ms,
    );
    assert!(other > 0.05, "the third take, stretched: {other}");

    playing.open(&comped, 3);
    let frames = frames_of(&comped, 0, 4 * BAR);
    playing.play(0, frames);
    playing.quiet();
    let played = playing.play(0, frames);
    assert_eq!(first_difference(&heard, &played), None);
}

#[test]
fn audio_made_from_a_file_that_was_found_with_other_contents_is_made_again() {
    let place = Place::new("export-found-other");
    let (doc, _) = song();
    place.files_of(&doc);
    let mut playing = Playing::new(&place);
    playing.open(&doc, 2);
    let plans: Vec<MadePlan> = (playing.audio.clips.iter())
        .flat_map(|c| c.plans.iter().cloned())
        .collect();
    let plan = plans.first().expect("a stretched clip").clone();
    let path = place.store.made_path(&plan.key).unwrap();
    let source = (doc.audio.iter()).find(|s| s.uid == plan.uid).unwrap();
    // 「さがす…」 takes a file of the same name with other contents (`keeper::found`).
    let found = |value: f32| {
        let work = place.store.work_dir().unwrap();
        let mut wav =
            crate::audiofiles::wav::CacheWav::create(&work.join(&source.file), RATE, 1).unwrap();
        wav.write(&vec![value; source.frames as usize]).unwrap();
        wav.finish().unwrap();
        place.store.drop_cache(&plan.uid);
        place.store.file_arrived();
    };
    let made_again = |playing: &mut Playing| {
        let until = Instant::now() + Duration::from_secs(120);
        let mut out = [0.0f32; 2 * BLOCK];
        while !playing.audio.made.contains(&plan.key) {
            assert!(Instant::now() < until, "the audio was not made again");
            while let Ok(news) = playing.news.try_recv() {
                if let MadeNews::Ready(keys) = news {
                    playing.audio.made_ready(keys);
                    playing.audio.want_made();
                }
            }
            playing.block(&mut out);
            std::thread::sleep(Duration::from_millis(2));
        }
    };

    // Played: the read-ahead thread has the made file open.
    let span = (plan.start, plan.end);
    let frames = frames_of(&doc, span.0, span.1);
    playing.play(span.0, frames);
    playing.quiet();

    let before = std::fs::read(&path).unwrap();
    found(0.25);
    playing.audio.want_made();
    for p in plans.iter().filter(|p| p.uid == plan.uid) {
        assert!(!playing.audio.made.contains(&p.key));
    }
    assert!(
        !path.exists(),
        "the audio made from the old contents is gone"
    );
    made_again(&mut playing);
    assert_ne!(std::fs::read(&path).unwrap(), before);
    // What plays is the audio made again, as the export has it.
    playing.play(span.0, frames);
    playing.quiet();
    let heard = playing.play(span.0, frames);
    playing.quiet();
    let run = place.start(place.setup(place.job(1, &doc, span, ExportTail::Off)));
    run.rendered();
    assert_eq!(first_difference(&run.pass(0), &heard), None);

    // Found while no stream runs (`Audio::close` lets go of the plans): let go of when the
    // next stream has them, before the maker is asked.
    playing.audio.by_hand = None;
    for entry in playing.audio.clips.iter_mut() {
        entry.plans.clear();
    }
    playing.audio.plans_changed = true;
    playing.audio.want_made();
    let before = std::fs::read(&path).unwrap();
    found(-0.5);
    playing.audio.want_made();
    playing.audio.open_by_hand(RATE);
    playing.audio.resend_clips();
    playing.audio.want_made();
    assert!(
        !path.exists(),
        "the audio made from the old contents is gone"
    );
    made_again(&mut playing);
    assert_ne!(std::fs::read(&path).unwrap(), before);
}
