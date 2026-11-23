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
use meloet_model::Song;
use meloet_model::song::Doc;
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
