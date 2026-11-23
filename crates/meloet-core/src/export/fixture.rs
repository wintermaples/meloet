//! A song for the export's tests with something of everything the export has to get
//! right, and the audio files it plays in temporary folders.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_model::effects::EffectKind;
use meloet_model::song::{ClipId, Doc, Note};
use meloet_model::{
    AutoTarget, BuiltinInstrument, Bus, ClipGain, FadeEnd, Gain, InstrumentRef, NewAudio,
    NewAudioTake, NewTake, StripId, Tempo, TickRange, TrackId, VocalRecording,
    add_vocal_recordings, automation, edit, mixer, set_clip_fade, set_clip_gain,
    set_clip_semitones, timeline,
};
use meloet_protocol::ExportTail;

use super::job::{self, JobHandle, Setup};
use super::{ExportJob, ExportNews};
use crate::audiofiles::AudioStore;
use crate::audiofiles::wav::{CacheWav, WavPcm};
use crate::pcm::PcmFile;
use crate::settings::temp_dir;

pub const RATE: u32 = 48_000;
pub const BAR: u64 = 7_680;
pub const BLOCK: usize = 256;

/// The folders of a test and the place its audio files are opened from.
pub struct Place {
    pub root: PathBuf,
    pub store: Arc<AudioStore>,
}

impl Place {
    pub fn new(label: &str) -> Place {
        let root = temp_dir(label);
        let store = Arc::new(
            AudioStore::new(Some(root.join("state"))).with_cache(Some(root.join("cache"))),
        );
        Place { root, store }
    }

    /// The audio files of `doc`, each some seconds of a sound of its own, in the work
    /// place; and the store is told the song's table.
    pub fn files_of(&self, doc: &Doc) {
        let work = self.store.work_dir().unwrap();
        for (n, source) in doc.audio.iter().enumerate() {
            let hz = 180.0 + 70.0 * n as f32;
            let samples: Vec<f32> = (0..source.frames)
                .map(|i| {
                    let t = i as f32 / RATE as f32;
                    // A tone that wanders, with a click every quarter of a second.
                    let tone = (t * hz * std::f32::consts::TAU + (t * 3.0).sin() * 2.0).sin();
                    let click = if i % 12_000 < 40 { 0.5 } else { 0.0 };
                    tone * 0.3 + click
                })
                .collect();
            let mut wav = CacheWav::create(&work.join(&source.file), RATE, 1).unwrap();
            wav.write(&samples).unwrap();
            wav.finish().unwrap();
        }
        (self.store).set_sources(doc.audio.iter().map(|s| (s.uid, s.file.clone())).collect());
    }

    /// A job as the rules ask for it.
    pub fn job(&self, id: u64, doc: &Doc, span: (u64, u64), tail: ExportTail) -> ExportJob {
        ExportJob {
            id,
            doc: Arc::new(doc.clone()),
            span: TickRange::new(span.0, span.1).unwrap(),
            per_track: false,
            tail,
            realtime: false,
            skip: Vec::new(),
            sample_rate: RATE,
            measure: None,
        }
    }

    pub fn setup(&self, job: ExportJob) -> Setup {
        Setup {
            work: self.root.join(format!("export-{}", job.id)),
            job,
            block: BLOCK,
            store: self.store.clone(),
            samples: Vec::new(),
            known: Vec::new(),
            hosts: None,
            states: None,
            maker_waiting: None,
        }
    }

    pub fn start(&self, setup: Setup) -> Run {
        let (tx, news) = std::sync::mpsc::channel();
        let work = setup.work.clone();
        let handle = job::spawn(setup, move |news| {
            let _ = tx.send(news);
        })
        .unwrap();
        Run { handle, news, work }
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A job that runs, and what it says.
pub struct Run {
    pub handle: JobHandle,
    pub news: std::sync::mpsc::Receiver<ExportNews>,
    pub work: PathBuf,
}

impl Run {
    /// The news up to the first one `done` says yes to.
    pub fn until(&self, what: &str, done: impl Fn(&ExportNews) -> bool) -> Vec<ExportNews> {
        let until = Instant::now() + Duration::from_secs(120);
        let mut seen = Vec::new();
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let news =
                (self.news.recv_timeout(left)).unwrap_or_else(|_| panic!("{what}: {seen:?}"));
            seen.push(news);
            if done(seen.last().unwrap()) {
                return seen;
            }
        }
    }

    /// Waits until the job is rendered. Returns its last word: the peak and the length.
    pub fn rendered(&self) -> (Option<f32>, u32) {
        let seen = self.until("rendered", |n| {
            matches!(
                n,
                ExportNews::Counted { done: true, .. } | ExportNews::Failed { .. }
            )
        });
        match seen.last() {
            Some(ExportNews::Counted {
                peak_db, length_s, ..
            }) => (*peak_db, *length_s),
            other => panic!("the job failed: {other:?}"),
        }
    }

    /// What the job rendered, as it lies in the work place: interleaved stereo.
    pub fn pass(&self, n: usize) -> Vec<f32> {
        read_wav(&self.work.join(format!("pass-{n}.wav")))
    }
}

pub fn read_wav(path: &std::path::Path) -> Vec<f32> {
    let mut wav = WavPcm::open(path).unwrap();
    let mut out = vec![0.0; wav.frames() as usize * usize::from(wav.channels())];
    let frames = wav.read(0, &mut out).unwrap();
    assert_eq!(frames as u64, wav.frames());
    out
}

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn notes(keys: &[u8], step: u64, length: u64) -> NewTake {
    let notes: Vec<Note> = (keys.iter().enumerate())
        .map(|(i, &key)| Note {
            start: i as u64 * step,
            length,
            key,
            velocity: 70 + ((i * 7) % 50) as u8,
            from: None,
        })
        .collect();
    NewTake {
        origin: 0,
        length: keys.len() as u64 * step,
        notes,
        controls: Vec::new(),
    }
}

fn recorded(doc: &Doc, track: TrackId, n: u8, origin: u64, length: u64) -> (Doc, ClipId) {
    let recording = VocalRecording {
        track,
        audio: NewAudio {
            uid: [n; 16],
            file: format!("歌 {n}.wav"),
            frames: 6 * u64::from(RATE),
            sample_rate: RATE,
            channels: 1,
            bits: None,
        },
        takes: vec![NewAudioTake {
            origin,
            length,
            first_frame: 2_400,
            frames: 5 * u64::from(RATE),
        }],
    };
    let (e, added) = add_vocal_recordings(doc, vec![recording], None).unwrap();
    (e.doc, added[0].clip)
}

/// What [`song`] holds, for the tests that change it.
pub struct Parts {
    pub lead: TrackId,
    pub drums: TrackId,
    pub vocals: TrackId,
}

/// Six bars: 96 a minute, from bar 3 a ramp to 120 at bar 5, and another tempo in the
/// middle of bar 6. A lead with a lane on its fader, a delay and a send to the reverb; an
/// electronic drum kit; a vocal track with a clip that plays as it was sung, one at another
/// pitch and one that is stretched. The master has what a new song's master has.
pub fn song() -> (Doc, Parts) {
    let doc = Doc::new_default();
    let first = doc.tracks[0].id;
    let doc = timeline::set_tempo(&doc, tempo(9_600)).map_or(doc, |e| e.doc);
    let doc = (timeline::add_tempo_point(&doc, 2 * BAR, tempo(9_600)).unwrap()).doc;
    let doc = (timeline::add_tempo_point(&doc, 4 * BAR, tempo(12_000)).unwrap()).doc;
    let doc = timeline::set_tempo_ramp(&doc, 2 * BAR, true).unwrap().doc;

    let lead = InstrumentRef::Builtin(BuiltinInstrument::SynthLead);
    let doc = edit::set_instrument(&doc, first, lead).map_or(doc, |e| e.doc);
    let lead = first;
    let melody = [
        60, 64, 67, 72, 71, 67, 64, 62, 60, 65, 69, 72, 74, 72, 69, 65,
    ];
    let take = notes(&melody, 6 * BAR / 16, 2_000);
    let doc = edit::add_recording(&doc, lead, vec![take], 0)
        .unwrap()
        .0
        .doc;
    let (e, lane) = automation::add_lane(&doc, lead, AutoTarget::Volume, false).unwrap();
    let (e, lane, _) = automation::add_point(&e.doc, lane, 0, Some(0.8), 0.8).unwrap();
    let (e, lane, _) = automation::add_point(&e.doc, lane, 2 * BAR + 300, Some(0.35), 0.8).unwrap();
    let (e, _, _) = automation::add_point(&e.doc, lane, 4 * BAR, Some(0.9), 0.8).unwrap();
    let strip = StripId::Track(lead);
    let doc = mixer::add_effect(&e.doc, strip, 0, EffectKind::Delay)
        .unwrap()
        .doc;
    let doc = (mixer::set_send(&doc, lead, Bus::A, Gain::from_db(-9.0)).unwrap()).doc;

    let kit = InstrumentRef::Builtin(BuiltinInstrument::ElectroKit);
    let (e, drums) = edit::add_track(&doc, None, kit, meloet_model::Language::Ja).unwrap();
    let hits: Vec<u8> = (0..48).map(|i| [36, 42, 38, 42][i % 4]).collect();
    let take = notes(&hits, BAR / 8, 400);
    let doc = edit::add_recording(&e.doc, drums, vec![take], 0)
        .unwrap()
        .0
        .doc;
    let doc = (mixer::set_pan(&doc, StripId::Track(drums), meloet_model::Pan::new(-30)))
        .map_or(doc.clone(), |e| e.doc);

    let (e, vocals) = edit::add_vocal_track(&doc, None, meloet_model::Language::Ja).unwrap();
    let (doc, plain) = recorded(&e.doc, vocals, 1, BAR, 2 * BAR);
    let doc = set_clip_fade(&doc, plain, FadeEnd::In, 900).unwrap().doc;
    let doc = set_clip_fade(&doc, plain, FadeEnd::Out, 1_900).unwrap().doc;
    let doc = (set_clip_gain(&doc, plain, ClipGain::from_db(-4.0).unwrap()).unwrap()).doc;
    let (doc, pitched) = recorded(&doc, vocals, 2, 4 * BAR, BAR);
    let doc = set_clip_semitones(&doc, pitched, 3).unwrap().doc;
    let (doc, _) = recorded(&doc, vocals, 3, 5 * BAR, BAR);
    let doc = (timeline::add_tempo_point(&doc, 5 * BAR + BAR / 2, tempo(10_400)).unwrap()).doc;
    let parts = Parts {
        lead,
        drums,
        vocals,
    };
    (doc, parts)
}

/// [`song`] with every fader at +6 dB, the master's too: it goes over 0 dB. `limiter`:
/// the master keeps the limiter a new song has in its first slot.
pub fn loud(doc: &Doc, limiter: bool) -> Doc {
    let six = Gain::from_db(Gain::MAX_DB);
    let mut doc = doc.clone();
    if !limiter {
        doc = mixer::remove_effect(&doc, StripId::Master, 0).unwrap().doc;
    }
    let strips: Vec<StripId> = (doc.tracks.iter().map(|t| StripId::Track(t.id)))
        .chain([StripId::Master])
        .collect();
    for strip in strips {
        doc = mixer::set_gain(&doc, strip, six).unwrap().doc;
    }
    doc
}
