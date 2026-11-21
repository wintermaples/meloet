//! The maker against a real store in temporary folders: files with clicks go in, and the
//! made files are read back.

use std::f64::consts::TAU;
use std::sync::mpsc::Receiver;

use meloet_engine::song_frame;
use meloet_model::{FileClock, Meter, SourceTempo, Tempo, TempoMap, TempoPoint, Timeline};

use super::*;
use crate::audiofiles::wav::WavPcm;
use crate::made::Place;
use crate::settings::temp_dir;

const RATE: u32 = 48_000;
const BAR: u64 = 7_680;
const WAIT: Duration = Duration::from_secs(60);

struct Rig {
    store: Arc<AudioStore>,
    maker: Maker,
    news: Receiver<MadeNews>,
    cache: PathBuf,
    sources: Vec<([u8; 16], String)>,
}

impl Rig {
    fn new(label: &str) -> Rig {
        let cache = temp_dir(&format!("{label}-cache"));
        let store = Arc::new(
            AudioStore::new(Some(temp_dir(&format!("{label}-state"))))
                .with_cache(Some(cache.clone())),
        );
        let (tx, news) = mpsc::channel();
        let maker = Maker::spawn(store.clone(), move |news| {
            let _ = tx.send(news);
        })
        .unwrap();
        Rig {
            store,
            maker,
            news,
            cache,
            sources: Vec::new(),
        }
    }

    /// A mono file of the song, in the work place.
    fn file(&mut self, n: u8, samples: &[f32]) -> [u8; 16] {
        let name = format!("{n}.wav");
        let work = self.store.work_dir().unwrap();
        let mut wav = CacheWav::create(&work.join(&name), RATE, 1).unwrap();
        wav.write(samples).unwrap();
        wav.finish().unwrap();
        self.sources.push(([n; 16], name));
        self.store.set_sources(self.sources.clone());
        [n; 16]
    }

    fn uids(&self) -> Vec<[u8; 16]> {
        self.sources.iter().map(|s| s.0).collect()
    }

    fn want(&self, plans: &[&MadePlan]) {
        let plans = plans.iter().map(|&p| p.clone()).collect();
        self.maker.want(plans, self.uids());
    }

    /// The news up to the first one `done` says yes to.
    fn until(&self, what: &str, mut done: impl FnMut(&MadeNews) -> bool) -> Vec<MadeNews> {
        let mut seen = Vec::new();
        loop {
            let news =
                (self.news.recv_timeout(WAIT)).unwrap_or_else(|_| panic!("{what}: {seen:?}"));
            seen.push(news);
            if done(seen.last().unwrap()) {
                return seen;
            }
        }
    }

    fn until_ready(&self, key: MadeKey) -> Vec<MadeNews> {
        self.until(
            "ready",
            |n| matches!(n, MadeNews::Ready(keys) if keys.contains(&key)),
        )
    }

    fn made(&self, key: &MadeKey) -> Vec<f32> {
        let mut file = self.store.open_made(key, RATE).expect("the made file");
        let mut out = vec![0.0; file.frames() as usize];
        assert_eq!(file.read(0, &mut out).unwrap(), out.len());
        out
    }

    fn files(&self, folder: &str) -> Vec<String> {
        let mut names: Vec<String> = files_of(&self.cache.join(folder))
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

fn steady(centi: u32) -> Timeline {
    Timeline::new(Tempo::from_centi(centi).unwrap(), Meter::FOUR_FOUR)
}

/// The plan of a clip from `start` to `end` of `song` whose take starts at its start, with
/// the file's frame 0. `sync`: the tempo the file was played to, for a clip that follows.
fn plan(
    uid: [u8; 16],
    key: u8,
    song: &Timeline,
    sync: Option<u32>,
    (start, end): (u64, u64),
    semitones: f32,
) -> MadePlan {
    let map = Arc::new(TempoMap::new(song));
    let rate = f64::from(RATE);
    let frames = song_frame(&map, end as f64, rate) - song_frame(&map, start as f64, rate);
    let place = match sync {
        Some(centi) => {
            let sync = SourceTempo::steady(Tempo::from_centi(centi).unwrap());
            Place::Follow {
                clock: FileClock::new(start as i64, 0.0, rate, Some(&sync)),
                start_sec: map.seconds_at(start as f64),
                map,
            }
        }
        None => Place::Straight { file_frame: 0 },
    };
    MadePlan {
        key: [key; 32],
        clip: u32::from(key),
        audio: 100 + u32::from(key),
        uid,
        sample_rate: RATE,
        start,
        end,
        frames,
        place,
        file_end: i64::MAX,
        semitones,
        keep_formant: semitones != 0.0,
    }
}

/// A click: a few milliseconds of a decaying 3 kHz burst starting at `at`.
fn add_click(buf: &mut [f32], at: usize) {
    for i in 0..96 {
        let t = i as f64 / f64::from(RATE);
        buf[at + i] += (0.9 * (-t * 2000.0).exp() * (TAU * 3000.0 * t).cos()) as f32;
    }
}

/// The frame where a short click sits: the centre of energy of `x` around its largest sample.
fn click_at(x: &[f32]) -> f64 {
    let top = (0..x.len())
        .max_by(|&a, &b| x[a].abs().total_cmp(&x[b].abs()))
        .unwrap();
    let (from, to) = (top.saturating_sub(480), (top + 480).min(x.len()));
    let (mut weight, mut moment) = (0.0f64, 0.0f64);
    for (i, &v) in x[from..to].iter().enumerate() {
        let e = f64::from(v) * f64::from(v);
        weight += e;
        moment += e * (from + i) as f64;
    }
    moment / weight
}

/// A file of `beats` quarter notes at 120 with a click on each but the first.
fn clicks(beats: usize) -> (Vec<f32>, Vec<usize>) {
    let mut file = vec![0.0; beats * 24_000 + 1_000];
    let at: Vec<usize> = (1..beats).map(|k| k * 24_000).collect();
    for &at in &at {
        add_click(&mut file, at);
    }
    (file, at)
}

/// How far each click of the file is from where the plan says it sounds, in frames.
fn click_offsets(plan: &MadePlan, file: &[f32], at: &[usize], made: &[f32]) -> Vec<f64> {
    (at.iter())
        .map(|&at| {
            let in_file = click_at(&file[at - 100..at + 400]) + (at - 100) as f64;
            // The frame of the clip the plan puts that frame of the file at.
            let (mut a, mut b) = (0u64, plan.frames);
            while b - a > 1 {
                let mid = (a + b) / 2;
                if plan.file_frame_at(mid) < in_file {
                    a = mid
                } else {
                    b = mid
                }
            }
            let (lo, hi) = (
                a.saturating_sub(2_000) as usize,
                (a as usize + 2_000).min(made.len()),
            );
            click_at(&made[lo..hi]) + lo as f64 - a as f64
        })
        .collect()
}

#[test]
fn a_stretched_clip_is_made_after_a_while_and_its_clicks_are_where_the_song_has_them() {
    let mut rig = Rig::new("stretch");
    let (file, at) = clicks(8);
    let uid = rig.file(1, &file);
    // Played to 120, in a song at 96: two bars of the song are 5 seconds.
    let slow = plan(uid, 1, &steady(9_600), Some(12_000), (BAR, 3 * BAR), 0.0);
    assert_eq!(slow.frames, 240_000);
    let asked = Instant::now();
    rig.want(&[&slow]);
    let news = rig.until_ready(slow.key);
    assert!(asked.elapsed() >= SETTLE, "it waits before it starts");
    let job = |percent| MakerJob {
        clip: 1,
        audio: 101,
        percent,
    };
    assert_eq!(news[0], MadeNews::Work(vec![job(0)]));
    assert_eq!(news[news.len() - 1], MadeNews::Ready(vec![slow.key]));
    assert_eq!(
        rig.until("no work", |n| matches!(n, MadeNews::Work(_)))
            .pop(),
        Some(MadeNews::Work(vec![]))
    );
    assert_eq!(rig.maker.waiting(), 0);
    assert_eq!(rig.files(CACHE_MADE), [format!("{}.wav", hex(&slow.key))]);

    let made = rig.made(&slow.key);
    assert_eq!(made.len(), 240_000);
    let off = click_offsets(&slow, &file, &at, &made);
    eprintln!("x0.8: clicks off by {off:.1?} frames");
    assert!(off.iter().all(|o| o.abs() < 48.0), "{off:?}");
    // The beats of the file are the beats of the song: 30000 frames apart at 96.
    let second = click_at(&made[59_000..61_000]) + 59_000.0;
    assert!((second - 60_000.0).abs() < 48.0, "{second}");

    // In a song that gets faster, from 96 to 144 over four bars, and a semitone up.
    let ramp = Timeline {
        head_ramp: true,
        tempo_points: vec![TempoPoint {
            tick: 4 * BAR,
            tempo: Tempo::from_centi(14_400).unwrap(),
            ramp: false,
        }],
        ..steady(9_600)
    };
    for (key, semitones) in [(2, 0.0), (3, 1.0)] {
        let faster = plan(uid, key, &ramp, Some(12_000), (BAR, 3 * BAR), semitones);
        rig.want(&[&faster]);
        rig.until_ready(faster.key);
        let made = rig.made(&faster.key);
        assert_eq!(made.len() as u64, faster.frames);
        let off = click_offsets(&faster, &file, &at, &made);
        eprintln!("a ramp, {semitones:+}: clicks off by {off:.1?} frames");
        assert!(off.iter().all(|o| o.abs() < 48.0), "{off:?}");
    }
}

#[test]
fn what_is_in_the_cache_is_not_made_again() {
    let mut rig = Rig::new("again");
    let uid = rig.file(1, &clicks(4).0);
    let up = plan(uid, 7, &steady(12_000), None, (0, BAR / 2), 2.0);
    rig.want(&[&up]);
    rig.until_ready(up.key);
    let first = fs::metadata(rig.store.made_path(&up.key).unwrap()).unwrap();
    // Not wanted (the value was changed back), then wanted again (undo).
    rig.want(&[]);
    rig.until("no work", |n| *n == MadeNews::Work(vec![]));
    let asked = Instant::now();
    rig.want(&[&up]);
    assert_eq!(
        rig.news.recv_timeout(WAIT),
        Ok(MadeNews::Ready(vec![up.key]))
    );
    assert!(asked.elapsed() < SETTLE, "said at once");
    assert!(
        rig.news.recv_timeout(SETTLE * 2).is_err(),
        "and nothing is made"
    );
    let second = fs::metadata(rig.store.made_path(&up.key).unwrap()).unwrap();
    assert_eq!(first.len(), second.len());
    assert_eq!(rig.files(CACHE_MADE).len(), 1);
}

#[test]
fn a_plan_that_is_no_longer_wanted_is_given_up_in_the_middle() {
    let mut rig = Rig::new("giveup");
    // Two minutes: long enough to be caught in the middle.
    let long = rig.file(1, &vec![0.1; 120 * RATE as usize]);
    let short = rig.file(2, &clicks(2).0);
    let old = plan(long, 1, &steady(12_000), None, (0, 60 * BAR), 3.0);
    let new = plan(short, 2, &steady(12_000), None, (0, BAR / 4), 3.0);
    rig.want(&[&old]);
    let began =
        |n: &MadeNews| matches!(n, MadeNews::Work(jobs) if jobs.iter().any(|j| j.percent > 0));
    let seen = rig.until("a percent", began);
    assert!(!seen.iter().any(|n| matches!(n, MadeNews::Ready(_))));
    assert!(
        rig.files(CACHE_MADE)
            .iter()
            .any(|name| name.ends_with(".part"))
    );
    assert_eq!(rig.maker.waiting(), 1);
    // The value changed: other audio is wanted, the old plan is dropped.
    rig.want(&[&new]);
    let seen = rig.until_ready(new.key);
    assert!(
        !seen
            .iter()
            .any(|n| matches!(n, MadeNews::Ready(keys) if keys.contains(&old.key)))
    );
    assert!(seen.contains(&MadeNews::Work(vec![MakerJob {
        clip: 2,
        audio: 102,
        percent: 0
    }])));
    assert_eq!(rig.files(CACHE_MADE), [format!("{}.wav", hex(&new.key))]);

    // Dropping the maker in the middle leaves no file behind either.
    rig.want(&[&old]);
    rig.until("a percent", began);
    let Rig { maker, cache, .. } = rig;
    drop(maker);
    let left: Vec<_> = files_of(&cache.join(CACHE_MADE)).collect();
    assert_eq!(left.len(), 1);
}

#[test]
fn the_clip_nearest_to_the_play_line_is_made_first() {
    let mut rig = Rig::new("near");
    let uid = rig.file(1, &clicks(4).0);
    let song = steady(12_000);
    let plans: Vec<MadePlan> = (0..4u8)
        .map(|k| {
            plan(
                uid,
                k + 1,
                &song,
                None,
                (u64::from(k) * 4 * BAR, u64::from(k) * 4 * BAR + BAR / 4),
                1.0,
            )
        })
        .collect();
    // In the third clip: it, the one ahead, then the ones behind, the nearer first.
    rig.maker.set_near(8 * BAR + 10);
    rig.want(&plans.iter().collect::<Vec<_>>());
    let mut order = Vec::new();
    while order.len() < 4 {
        if let MadeNews::Ready(keys) = rig.news.recv_timeout(WAIT).unwrap() {
            order.extend(keys.iter().map(|k| k[0]));
        }
    }
    assert_eq!(order, [3, 4, 2, 1]);
}

#[test]
fn a_file_that_is_not_there_yet_is_tried_again_when_the_store_has_more() {
    let mut rig = Rig::new("later");
    let gone = plan([9; 16], 1, &steady(12_000), None, (0, BAR / 4), 1.0);
    rig.want(&[&gone]);
    // It fails without a word, and is no work that anybody waits for.
    rig.until("no work", |n| *n == MadeNews::Work(vec![]));
    assert_eq!(rig.maker.waiting(), 0);
    rig.file(9, &clicks(2).0);
    rig.maker.retry();
    rig.until_ready(gone.key);
}

fn put(path: &Path, bytes: usize, age: Duration, now: SystemTime) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
    let file = fs::File::options().append(true).open(path).unwrap();
    file.set_modified(now - age).unwrap();
}

#[test]
fn the_cache_loses_its_oldest_files_over_the_limit_and_never_what_the_song_uses() {
    let cache = temp_dir("trim");
    let now = SystemTime::now();
    let hours = |h: u64| Duration::from_secs(3_600 * h);
    let used_uid = hex(&[1u8; 16]);
    let used_key = hex(&[2u8; 32]);
    let files = [
        (CACHE_AUDIO, format!("{used_uid}-48000.wav"), 4_000, 90),
        (CACHE_PEAKS, format!("{used_uid}.peaks"), 100, 90),
        (CACHE_MADE, format!("{used_key}.wav"), 4_000, 80),
        (CACHE_AUDIO, "aa-48000.wav".to_owned(), 3_000, 70),
        (CACHE_MADE, "bb.wav".to_owned(), 3_000, 50),
        (CACHE_PEAKS, "aa.peaks".to_owned(), 100, 60),
        (CACHE_MADE, "cc.wav".to_owned(), 3_000, 1),
    ];
    for (folder, name, bytes, age) in &files {
        put(&cache.join(folder).join(name), *bytes, hours(*age), now);
    }
    let there = |folder: &str, name: &str| cache.join(folder).join(name).is_file();
    let used = |folder: &str, name: &str| match folder {
        CACHE_MADE => name == format!("{used_key}.wav"),
        _ => name.starts_with(&used_uid),
    };
    // Under the limit nothing goes.
    assert_eq!(trim_cache(&cache, 17_200, &used, now), 0);
    // 17200 bytes are there; under 12000 the three oldest that are not used go.
    assert_eq!(trim_cache(&cache, 12_000, &used, now), 6_100);
    assert!(!there(CACHE_AUDIO, "aa-48000.wav") && !there(CACHE_MADE, "bb.wav"));
    assert!(!there(CACHE_PEAKS, "aa.peaks") && there(CACHE_MADE, "cc.wav"));
    // With no room at all only what the song uses stays, and it counts as used now.
    assert_eq!(trim_cache(&cache, 0, &used, now), 3_000);
    for (folder, name, _, _) in &files[..3] {
        assert!(there(folder, name), "{name}");
        let at = fs::metadata(cache.join(folder).join(name))
            .unwrap()
            .modified()
            .unwrap();
        assert!(now.duration_since(at).unwrap() < Duration::from_secs(1));
    }
    assert!(
        files[3..]
            .iter()
            .all(|(folder, name, _, _)| !there(folder, name))
    );

    // What was left half written long ago goes at the start; a fresh one may be another
    // meloet's.
    put(
        &cache.join(CACHE_MADE).join("old.wav.part"),
        10,
        hours(1),
        now,
    );
    put(
        &cache.join(CACHE_AUDIO).join("old.wav.part"),
        10,
        hours(1),
        now,
    );
    put(
        &cache.join(CACHE_MADE).join("new.wav.part"),
        10,
        Duration::from_secs(5),
        now,
    );
    remove_left_parts(&cache, now);
    assert!(!there(CACHE_MADE, "old.wav.part") && !there(CACHE_AUDIO, "old.wav.part"));
    assert!(there(CACHE_MADE, "new.wav.part"));
    assert!(there(CACHE_MADE, &format!("{used_key}.wav")));
}

#[test]
fn the_maker_keeps_the_cache_under_its_limit() {
    let mut rig = Rig::new("limit");
    let uid = rig.file(1, &clicks(4).0);
    let now = SystemTime::now();
    let day = Duration::from_secs(86_400);
    // Left from other songs, and the decoded audio of this one.
    put(
        &rig.cache.join(CACHE_MADE).join("other.wav"),
        50_000,
        day,
        now,
    );
    put(
        &rig.cache.join(CACHE_AUDIO).join("other-48000.wav"),
        50_000,
        day * 2,
        now,
    );
    let own = format!("{}-48000.wav", hex(&uid));
    put(
        &rig.cache.join(CACHE_AUDIO).join(&own),
        50_000,
        day * 3,
        now,
    );
    rig.maker.set_cache_limit(60_000);
    let first = plan(uid, 1, &steady(12_000), None, (0, BAR / 4), 1.0);
    rig.want(&[&first]);
    rig.until_ready(first.key);
    rig.until("no work", |n| *n == MadeNews::Work(vec![]));
    // The made file is 96 kB by itself: everything that is not this song's went.
    assert_eq!(rig.files(CACHE_MADE), [format!("{}.wav", hex(&first.key))]);
    assert_eq!(rig.files(CACHE_AUDIO), [own.clone()]);
    // Another value: what was made for the first is not used any more, and goes.
    let second = plan(uid, 2, &steady(12_000), None, (0, BAR / 4), 2.0);
    rig.want(&[&second]);
    rig.until_ready(second.key);
    rig.until("no work", |n| *n == MadeNews::Work(vec![]));
    assert_eq!(rig.files(CACHE_MADE), [format!("{}.wav", hex(&second.key))]);
    assert_eq!(rig.files(CACHE_AUDIO), [own]);
    assert!(WavPcm::open(&rig.store.made_path(&second.key).unwrap()).is_ok());
}
