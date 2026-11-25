use std::fs;

use meloet_engine::{
    BlockTiming, Effect, EffectContext, Engine, EngineConfig, InputEnds, MixStrip, TrackSet,
    input_ends,
};

use meloet_model::{Meter, Tempo, Timeline};

use super::*;
use crate::audiofiles::SONG_AUDIO;
use crate::settings::temp_dir;

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const BLOCK_US: f64 = BLOCK as f64 * 1e6 / RATE as f64;

/// Stands for a plugin on the master that says it delays the sound by so many frames.
struct Says(u32);

impl Effect for Says {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn set_param(&mut self, _index: u32, _value: f32) {}
    fn process(&mut self, _left: &mut [f32], _right: &mut [f32], _ctx: &EffectContext) {}
    fn reset(&mut self) {}
    fn latency(&self) -> u32 {
        self.0
    }
}

/// When the input's frame `frame` (counted from its first) was sung: a source's time is its
/// block's first frame, a block before the cycle.
fn sung_us(frame: f64) -> f64 {
    1e6 - BLOCK_US + frame * 1e6 / f64::from(RATE)
}

/// An engine with a plugin on the master that says `lag`, and an input on its clock, cycle
/// after cycle; the song rolls at once. The frame of the input the mark of the roll points
/// at, and when the song of the roll came out of the master.
fn marked(lag: u32) -> (f64, f64) {
    let dir = std::env::temp_dir().join(format!("meloet-input-mark-{}-{lag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut parts = Engine::new(EngineConfig::default());
    let says: Box<dyn Effect> = Box::new(Says(lag));
    assert!(
        (parts.handle)
            .set_effect(MixStrip::Master, 0, Some(says))
            .is_ok()
    );
    let InputEnds {
        mut tap,
        record,
        meters,
        ..
    } = input_ends(1, RATE);
    let writer = DiskWriter::spawn();
    writer.set_input(Some((record, meters)));
    let path = dir.join("歌 1.wav");
    let job = RecordJob {
        files: vec![RecordFile {
            path: path.clone(),
            channels: vec![0],
        }],
        format: RecordFormat::Bits32,
        heard: Some(HeardLine {
            clock: parts.handle.clock().clone(),
            x_us: 0.0,
        }),
    };
    writer.start(job).unwrap();
    let mut job = Job {
        x_us: 0.0,
        from_said: false,
        trouble_said: false,
        faults_said: (0, 0),
    };
    let mut rolled = None;
    for n in 0..200u64 {
        if n == 60 {
            let record = EngineCommand::Record {
                from: 0,
                count_in_beats: 0,
                muted: TrackSet::default(),
            };
            assert!(parts.handle.send(record).is_ok());
        }
        let callback_us = (1e6 + n as f64 * BLOCK_US) as u64;
        let timing = BlockTiming {
            callback_us,
            playback_us: callback_us + 2 * BLOCK_US as u64,
        };
        parts.engine.process(&mut [0.0; BLOCK * 2], timing);
        // Each sample is the number of its frame.
        let samples: Vec<f32> = (0..BLOCK as u64)
            .map(|i| (n * BLOCK as u64 + i) as f32)
            .collect();
        let first_frame = n as f64 * BLOCK as f64;
        tap.block(&samples, sung_us(first_frame) as u64, callback_us);
        while let Some(r) = parts.handle.pop_recorded() {
            if matches!(r, Recorded::Rolling { .. }) {
                rolled = Some(r);
            }
        }
    }
    let rolled = rolled.expect("the song rolled");
    let Recorded::Rolling { frame, .. } = rolled else {
        unreachable!()
    };
    let file_frame = Worker::mark(&mut job, &writer, &parts.handle, &rolled).expect("a mark");
    assert!(writer.stop().is_some());
    let bytes = fs::read(&path).unwrap();
    let first = f32::from_le_bytes(bytes[44..48].try_into().unwrap());
    let _ = fs::remove_dir_all(&dir);
    let out = frame + u64::from(lag);
    let out_us = parts.handle.clock().time_of_frame(out as f64).unwrap();
    (f64::from(first) + file_frame, out_us)
}

/// The voice that lies at the roll's mark was sung when the song of the roll came out of the
/// master, after its plugins delayed it: as many frames later as they say.
#[test]
fn a_voice_is_marked_where_the_song_came_out_of_the_master() {
    let (plain, plain_us) = marked(0);
    let (late, late_us) = marked(1_000);
    for (frame, out_us) in [(plain, plain_us), (late, late_us)] {
        let off_us = sung_us(frame) - out_us;
        assert!(
            off_us.abs() < 1e6 / f64::from(RATE),
            "{frame}: {off_us} us off"
        );
    }
    assert!((late - plain - 1_000.0).abs() < 0.1, "{plain} {late}");
}

#[test]
fn the_files_begin_half_a_beat_of_the_meter_before_the_roll() {
    let at_120 = |num, den| {
        let meter = Meter::new(num, den).unwrap();
        TempoMap::new(&Timeline::new(Tempo::from_centi(12_000).unwrap(), meter))
    };
    // A quarter note is half a second at 120, whatever the meter.
    assert_eq!(half_beat_us(&at_120(4, 4), 0.0), 250_000.0);
    assert_eq!(half_beat_us(&at_120(2, 2), 0.0), 500_000.0);
    assert_eq!(half_beat_us(&at_120(6, 8), 0.0), 125_000.0);
    assert_eq!(half_beat_us(&at_120(6, 8), -300.0), 125_000.0);
}

#[test]
fn a_recording_never_takes_a_name_the_song_has() {
    let root = temp_dir("rec-names");
    let store = AudioStore::new(Some(root.join("state")));
    let song = root.join("曲");
    std::fs::create_dir_all(song.join(SONG_AUDIO)).unwrap();
    store.set_song_dir(Some(song.clone()));
    let work = vocalrec::work_audio_dir(&root.join("state"));
    std::fs::create_dir_all(&work).unwrap();
    let ask = |audio: u32| VocalFileAsk {
        audio,
        name: vocalrec::file_name("歌", audio),
        channels: vec![0],
    };
    let names =
        |taken: &[String], asks: &[VocalFileAsk]| free_recording_names(&store, &work, taken, asks);
    assert_eq!(names(&[], &[ask(40)]), ["歌 40.wav"]);

    // A file brought in and saved earlier has the name: its song folder's file would
    // play in place of the recording, and a save would keep that one.
    std::fs::write(song.join(SONG_AUDIO).join("歌 40.wav"), b"brought in").unwrap();
    assert_eq!(names(&[], &[ask(40)]), ["歌 40 (2).wav"]);
    // A recording that was undone keeps its file in the work place.
    std::fs::write(work.join("歌 40 (2).wav"), b"undone").unwrap();
    assert_eq!(names(&[], &[ask(40)]), ["歌 40 (3).wav"]);
    // A name of the audio table whose file is not found.
    let taken = ["歌 41.wav".to_owned()];
    assert_eq!(names(&taken, &[ask(41)]), ["歌 41 (2).wav"]);
    // Two files of one recording never get the same name.
    assert_eq!(
        names(&taken, &[ask(41), ask(41)]),
        ["歌 41 (2).wav", "歌 41 (3).wav"]
    );
}
