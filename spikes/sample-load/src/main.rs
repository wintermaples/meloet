//! Spike: how big, how slow and how heavy is a sampled grand piano (Salamander V3)?
//! See README.md for how to run it.

mod decode;
mod sfz;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use sfz::{Kind, Region};

const WINDOW: usize = 480; // 10 ms at 48 kHz
const FADE: usize = 2400; // 50 ms at 48 kHz
const MIB: f64 = 1024.0 * 1024.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or_else(|| usage());
    match arg(0) {
        "sfz" => cmd_sfz(Path::new(arg(1))),
        "scan" => cmd_scan(
            Path::new(arg(1)),
            Path::new(arg(2)),
            arg(3).parse().unwrap(),
        ),
        "plan" => cmd_plan(Path::new(arg(1)), Path::new(arg(2))),
        "list" => cmd_list(
            Path::new(arg(1)),
            Path::new(arg(2)),
            arg(3),
            arg(4) == "extras",
            arg(5),
        ),
        "load" => cmd_load(
            Path::new(arg(1)),
            arg(2),
            Path::new(arg(3)),
            arg(4),
            arg(5).parse().unwrap(),
        ),
        "evict" => cmd_evict(Path::new(arg(1)), arg(2), Path::new(arg(3))),
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!(
        "sample-load sfz <file.sfz>\n\
         sample-load scan <file.sfz> <out.tsv> <threads>\n\
         sample-load plan <file.sfz> <scan.tsv>\n\
         sample-load list <file.sfz> <scan.tsv> <layers: 4 | 4,6 | 16> <extras|main> <trim: none|30|40|50|60|72>\n\
         sample-load load <dir> <ext> <list.tsv> <i16|f32> <threads>\n\
         sample-load evict <dir> <ext> <list.tsv>"
    );
    std::process::exit(2)
}

fn read_sfz(path: &Path) -> Vec<Region> {
    sfz::parse(&fs::read_to_string(path).expect("read sfz"))
}

fn sample_dir(sfz: &Path) -> PathBuf {
    sfz.parent().unwrap().join("samples")
}

// ---------------------------------------------------------------- sfz

fn cmd_sfz(path: &Path) {
    let regions = read_sfz(path);
    println!("| kind | regions | files | keys | velocities | keys per region | group opcodes |");
    println!("|---|---|---|---|---|---|---|");
    let mut groups: BTreeMap<(Kind, String), Vec<&Region>> = BTreeMap::new();
    for r in &regions {
        groups.entry((r.kind, r.group.clone())).or_default().push(r);
    }
    for ((kind, group), rs) in &groups {
        let files: BTreeSet<&str> = rs.iter().map(|r| r.name.as_str()).collect();
        let lo = rs.iter().map(|r| r.lokey).min().unwrap();
        let hi = rs.iter().map(|r| r.hikey).max().unwrap();
        let vels: BTreeSet<(i32, i32)> = rs.iter().map(|r| (r.lovel, r.hivel)).collect();
        let spans: BTreeSet<i32> = rs.iter().map(|r| r.hikey - r.lokey + 1).collect();
        let vel_text = if vels.len() > 3 {
            format!("{} ranges", vels.len())
        } else {
            vels.iter()
                .map(|(a, b)| format!("{a}-{b}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let key_text = if *kind == Kind::Pedal {
            "(CC 64)".to_string()
        } else {
            format!(
                "{}-{} ({}-{})",
                lo,
                hi,
                sfz::note_name(lo),
                sfz::note_name(hi)
            )
        };
        println!(
            "| {} | {} | {} | {} | {} | {:?} | `{}` |",
            kind.name(),
            rs.len(),
            files.len(),
            key_text,
            vel_text,
            spans,
            group
        );
    }
    println!();
    println!("| kind | regions | distinct files |");
    println!("|---|---|---|");
    for kind in [Kind::Main, Kind::Release, Kind::Resonance, Kind::Pedal] {
        let rs: Vec<&Region> = regions.iter().filter(|r| r.kind == kind).collect();
        let files: BTreeSet<&str> = rs.iter().map(|r| r.name.as_str()).collect();
        println!("| {} | {} | {} |", kind.name(), rs.len(), files.len());
    }
    let files: BTreeSet<&str> = regions.iter().map(|r| r.name.as_str()).collect();
    println!("| all | {} | {} |", regions.len(), files.len());

    println!();
    println!("Main layers (lovel-hivel), from the regions:");
    for (i, (lo, hi)) in layers(&regions).iter().enumerate() {
        print!(" v{}={lo}-{hi}", i + 1);
    }
    println!();
}

/// The distinct velocity ranges of the main regions, low to high.
fn layers(regions: &[Region]) -> Vec<(i32, i32)> {
    let set: BTreeSet<(i32, i32)> = regions
        .iter()
        .filter(|r| r.kind == Kind::Main)
        .map(|r| (r.lovel, r.hivel))
        .collect();
    set.into_iter().collect()
}

// ---------------------------------------------------------------- scan

#[derive(Clone, Debug)]
struct Scan {
    frames: usize,
    rate: u32,
    channels: usize,
    peak_db: f64,
    /// First frame at or above peak - 40 dB.
    lead: usize,
    /// Frames to keep when the tail is cut at peak - 20, 30, 40, 50, 60 dB (fade included).
    rel: [usize; 5],
    /// Frames to keep when the tail is cut at -72 dBFS (fade included).
    use72: usize,
    /// RMS of the last 100 ms.
    tail_db: f64,
    bytes: u64,
}

fn db(x: f64) -> f64 {
    if x <= 0.0 { -200.0 } else { 20.0 * x.log10() }
}

fn scan_one(path: &Path) -> Scan {
    let d = decode::decode::<f32>(path).unwrap_or_else(|e| panic!("{e}"));
    let ch = d.channels;
    let frames = d.frames();
    let frame_peak = |f: usize| {
        d.samples[f * ch..(f + 1) * ch]
            .iter()
            .fold(0f32, |m, s| m.max(s.abs()))
    };
    let peak = (0..frames).map(frame_peak).fold(0f32, f32::max);
    let lead = (0..frames)
        .find(|&f| frame_peak(f) >= peak * 0.01)
        .unwrap_or(0);
    let last_window_above = |threshold: f32| -> usize {
        let windows = frames.div_ceil(WINDOW);
        for w in (0..windows).rev() {
            let end = ((w + 1) * WINDOW).min(frames);
            if (w * WINDOW..end).any(|f| frame_peak(f) >= threshold) {
                return (end + FADE).min(frames);
            }
        }
        FADE.min(frames)
    };
    let tail_from = frames.saturating_sub(4800);
    let tail = &d.samples[tail_from * ch..];
    let tail_rms = (tail.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>()
        / tail.len().max(1) as f64)
        .sqrt();
    Scan {
        frames,
        rate: d.rate,
        channels: ch,
        peak_db: db(peak as f64),
        lead,
        rel: [20.0, 30.0, 40.0, 50.0, 60.0]
            .map(|d: f32| last_window_above(peak * 10f32.powf(-d / 20.0))),
        use72: last_window_above(10f32.powf(-72.0 / 20.0)),
        tail_db: db(tail_rms),
        bytes: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    }
}

fn cmd_scan(sfz_path: &Path, out: &Path, threads: usize) {
    let regions = read_sfz(sfz_path);
    let dir = sample_dir(sfz_path);
    let names: Vec<String> = regions
        .iter()
        .map(|r| r.name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let started = Instant::now();
    let results = parallel(&names, threads, |name| {
        scan_one(&dir.join(format!("{name}.flac")))
    });
    let mut text = String::from(
        "name\tframes\trate\tchannels\tpeak_db\tlead\tuse20\tuse30\tuse40\tuse50\tuse60\tuse72\ttail_db\tbytes\n",
    );
    for (name, s) in names.iter().zip(&results) {
        text.push_str(&format!(
            "{name}\t{}\t{}\t{}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\n",
            s.frames,
            s.rate,
            s.channels,
            s.peak_db,
            s.lead,
            s.rel[0],
            s.rel[1],
            s.rel[2],
            s.rel[3],
            s.rel[4],
            s.use72,
            s.tail_db,
            s.bytes
        ));
    }
    fs::write(out, text).expect("write scan");
    eprintln!(
        "scanned {} files in {:.1} s with {threads} threads",
        names.len(),
        started.elapsed().as_secs_f64()
    );
}

fn read_scan(path: &Path) -> BTreeMap<String, Scan> {
    let text = fs::read_to_string(path).expect("read scan");
    text.lines()
        .skip(1)
        .map(|line| {
            let c: Vec<&str> = line.split('\t').collect();
            (
                c[0].to_string(),
                Scan {
                    frames: c[1].parse().unwrap(),
                    rate: c[2].parse().unwrap(),
                    channels: c[3].parse().unwrap(),
                    peak_db: c[4].parse().unwrap(),
                    lead: c[5].parse().unwrap(),
                    rel: [6, 7, 8, 9, 10].map(|i| c[i].parse().unwrap()),
                    use72: c[11].parse().unwrap(),
                    tail_db: c[12].parse().unwrap(),
                    bytes: c[13].parse().unwrap(),
                },
            )
        })
        .collect()
}

/// Runs `work` over `items` on `threads` scoped threads; results keep the order of `items`.
fn parallel<T: Sync, R: Send>(
    items: &[T],
    threads: usize,
    work: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let mut out: Vec<(usize, R)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        mine.push((i, work(item)));
                    }
                    mine
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, r)| r).collect()
}

// ---------------------------------------------------------------- plan

/// Keeps `n` of the layers: for each of `n` equal slices of 1..=127, the layer whose top velocity
/// is nearest to the top of the slice. A kept layer then sounds from just above the kept layer
/// below it up to its own top, so a sample is never played louder than it was recorded.
fn pick_layers(all: &[(i32, i32)], n: usize) -> Vec<usize> {
    let mut picked = Vec::new();
    let mut from = 0;
    for i in 1..=n {
        let left = n - i;
        let choice = if i == n {
            all.len() - 1
        } else {
            let target = 127.0 * i as f64 / n as f64;
            (from..all.len() - left)
                .min_by(|&a, &b| {
                    let da = (all[a].1 as f64 - target).abs();
                    let db = (all[b].1 as f64 - target).abs();
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap()
        };
        picked.push(choice);
        from = choice + 1;
    }
    picked
}

fn layer_of(all: &[(i32, i32)], r: &Region) -> usize {
    all.iter().position(|l| *l == (r.lovel, r.hivel)).unwrap()
}

#[derive(Clone, Copy)]
enum Len {
    Full,
    /// Index into `Scan::rel`.
    Rel(usize),
    Use72,
}

impl Len {
    fn of(self, s: &Scan) -> usize {
        match self {
            Len::Full => s.frames,
            Len::Rel(i) => s.rel[i],
            Len::Use72 => s.use72,
        }
    }
}

fn frames_sum<'a>(
    scan: &BTreeMap<String, Scan>,
    names: impl Iterator<Item = &'a String>,
    len: Len,
) -> usize {
    names.map(|n| len.of(&scan[n])).sum()
}

fn mib(frames: usize, bytes_per_sample: usize) -> f64 {
    (frames * 2 * bytes_per_sample) as f64 / MIB
}

fn names_of(regions: &[Region], pred: impl Fn(&Region) -> bool) -> BTreeSet<String> {
    regions
        .iter()
        .filter(|r| pred(r))
        .map(|r| r.name.clone())
        .collect()
}

fn secs(frames: usize) -> f64 {
    frames as f64 / 48000.0
}

fn cmd_plan(sfz_path: &Path, scan_path: &Path) {
    let regions = read_sfz(sfz_path);
    let scan = read_scan(scan_path);
    let all = layers(&regions);
    assert!(
        scan.values().all(|s| s.rate == 48000 && s.channels == 2),
        "expected 48 kHz stereo"
    );

    println!("## Per layer (30 notes each)\n");
    println!(
        "| layer | vel | peak dBFS mean (min..max) | full s mean | s until peak -20 / -30 / -40 / -50 / -60 dB (mean) | -72 dBFS s mean | tail RMS dBFS mean | lead ms mean (max) | FLAC MiB |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for (i, (lo, hi)) in all.iter().enumerate() {
        let names = names_of(&regions, |r| r.kind == Kind::Main && layer_of(&all, r) == i);
        let ss: Vec<&Scan> = names.iter().map(|n| &scan[n]).collect();
        let n = ss.len() as f64;
        let mean = |f: &dyn Fn(&Scan) -> f64| ss.iter().map(|s| f(s)).sum::<f64>() / n;
        let min = |f: &dyn Fn(&Scan) -> f64| ss.iter().map(|s| f(s)).fold(f64::MAX, f64::min);
        let max = |f: &dyn Fn(&Scan) -> f64| ss.iter().map(|s| f(s)).fold(f64::MIN, f64::max);
        println!(
            "| v{} | {lo}-{hi} | {:.1} ({:.1}..{:.1}) | {:.1} | {:.1} / {:.1} / {:.1} / {:.1} / {:.1} | {:.1} | {:.1} | {:.1} ({:.1}) | {:.1} |",
            i + 1,
            mean(&|s| s.peak_db),
            min(&|s| s.peak_db),
            max(&|s| s.peak_db),
            mean(&|s| secs(s.frames)),
            mean(&|s| secs(s.rel[0])),
            mean(&|s| secs(s.rel[1])),
            mean(&|s| secs(s.rel[2])),
            mean(&|s| secs(s.rel[3])),
            mean(&|s| secs(s.rel[4])),
            mean(&|s| secs(s.use72)),
            mean(&|s| s.tail_db),
            mean(&|s| s.lead as f64 / 48.0),
            max(&|s| s.lead as f64 / 48.0),
            ss.iter().map(|s| s.bytes).sum::<u64>() as f64 / MIB,
        );
    }

    println!("\n## Per note (16 layers each)\n");
    println!(
        "| note | keys | full s (min..max) | -40 dB rel s: v1 / v8 / v16 | -60 dB rel s: v1 / v8 / v16 | -72 dBFS s: v1 / v8 / v16 |"
    );
    println!("|---|---|---|---|---|---|");
    let mut notes: BTreeMap<i32, Vec<&Region>> = BTreeMap::new();
    for r in regions.iter().filter(|r| r.kind == Kind::Main) {
        notes.entry(r.center).or_default().push(r);
    }
    for (center, rs) in &notes {
        let at =
            |layer: usize| &scan[&rs.iter().find(|r| layer_of(&all, r) == layer).unwrap().name];
        let full: Vec<f64> = rs.iter().map(|r| secs(scan[&r.name].frames)).collect();
        println!(
            "| {} | {}-{} | {:.1}..{:.1} | {:.1} / {:.1} / {:.1} | {:.1} / {:.1} / {:.1} | {:.1} / {:.1} / {:.1} |",
            sfz::note_name(*center),
            rs[0].lokey,
            rs[0].hikey,
            full.iter().cloned().fold(f64::MAX, f64::min),
            full.iter().cloned().fold(f64::MIN, f64::max),
            secs(at(0).rel[2]),
            secs(at(7).rel[2]),
            secs(at(15).rel[2]),
            secs(at(0).rel[4]),
            secs(at(7).rel[4]),
            secs(at(15).rel[4]),
            secs(at(0).use72),
            secs(at(7).use72),
            secs(at(15).use72),
        );
    }

    println!("\n## Extras\n");
    println!(
        "| kind | files | full s total | full s mean (max) | -40 dB rel s total | -60 dB rel s total | -72 dBFS s total | peak dBFS mean | FLAC MiB |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for kind in [Kind::Release, Kind::Resonance, Kind::Pedal] {
        let names = names_of(&regions, |r| r.kind == kind);
        let ss: Vec<&Scan> = names.iter().map(|n| &scan[n]).collect();
        println!(
            "| {} | {} | {:.1} | {:.2} ({:.2}) | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |",
            kind.name(),
            ss.len(),
            secs(ss.iter().map(|s| s.frames).sum()),
            secs(ss.iter().map(|s| s.frames).sum()) / ss.len() as f64,
            secs(ss.iter().map(|s| s.frames).max().unwrap()),
            secs(ss.iter().map(|s| s.rel[2]).sum()),
            secs(ss.iter().map(|s| s.rel[4]).sum()),
            secs(ss.iter().map(|s| s.use72).sum()),
            ss.iter().map(|s| s.peak_db).sum::<f64>() / ss.len() as f64,
            ss.iter().map(|s| s.bytes).sum::<u64>() as f64 / MIB,
        );
    }

    println!("\n## Thinning plans\n");
    println!("| layers | kept | new velocity ranges |");
    println!("|---|---|---|");
    let counts = [3usize, 4, 5, 6, 8, 16];
    for n in counts {
        let picked = pick_layers(&all, n);
        let mut lo = 1;
        let mut ranges = Vec::new();
        for &p in &picked {
            ranges.push(format!("{}-{}", lo, all[p].1));
            lo = all[p].1 + 1;
        }
        println!(
            "| {n} | {} | {} |",
            picked
                .iter()
                .map(|p| format!("v{}", p + 1))
                .collect::<Vec<_>>()
                .join(" "),
            ranges.join(", ")
        );
    }

    let lens = [
        Len::Full,
        Len::Use72,
        Len::Rel(4),
        Len::Rel(3),
        Len::Rel(2),
        Len::Rel(1),
    ];
    println!("\n## Memory when everything is loaded (MiB, stereo 48 kHz)\n");
    println!("Each cell: f32 / i16. Columns: where the tail is cut (50 ms fade included).\n");
    println!(
        "| what | files | full | -72 dBFS | peak -60 dB | peak -50 dB | peak -40 dB | peak -30 dB | source FLAC MiB |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    let mut rows: Vec<(String, BTreeSet<String>)> = Vec::new();
    for n in counts {
        let picked = pick_layers(&all, n);
        rows.push((
            format!("main, {n} layers"),
            names_of(&regions, |r| {
                r.kind == Kind::Main && picked.contains(&layer_of(&all, r))
            }),
        ));
    }
    rows.push((
        "release (hammer)".into(),
        names_of(&regions, |r| r.kind == Kind::Release),
    ));
    rows.push((
        "resonance".into(),
        names_of(&regions, |r| r.kind == Kind::Resonance),
    ));
    rows.push((
        "pedal".into(),
        names_of(&regions, |r| r.kind == Kind::Pedal),
    ));
    let extras = names_of(&regions, |r| r.kind != Kind::Main);
    for n in [4usize, 6] {
        let mut with = rows
            .iter()
            .find(|(l, _)| *l == format!("main, {n} layers"))
            .unwrap()
            .1
            .clone();
        with.extend(extras.iter().cloned());
        rows.push((format!("main {n} layers + all extras"), with));
    }
    for (label, names) in &rows {
        let cells: Vec<String> = lens
            .iter()
            .map(|l| {
                let f = frames_sum(&scan, names.iter(), *l);
                format!("{:.0} / {:.0}", mib(f, 4), mib(f, 2))
            })
            .collect();
        println!(
            "| {label} | {} | {} | {:.1} |",
            names.len(),
            cells.join(" | "),
            names.iter().map(|n| scan[n].bytes).sum::<u64>() as f64 / MIB,
        );
    }

    println!("\n## Streaming estimate: only the head of every sample in memory (MiB)\n");
    println!(
        "The head is cut from the -60 dB rel length (a sample shorter than the head is whole).\n"
    );
    println!("| what | files | head ms | f32 | i16 | rest on disk, seconds |");
    println!("|---|---|---|---|---|---|");
    for (label, names) in rows
        .iter()
        .filter(|(l, _)| l.contains("+ all extras") || l == "main, 16 layers")
    {
        for ms in [100usize, 250, 500] {
            let head = ms * 48;
            let kept: usize = names.iter().map(|n| scan[n].rel[4].min(head)).sum();
            let rest: usize = names
                .iter()
                .map(|n| scan[n].rel[4].saturating_sub(head))
                .sum();
            println!(
                "| {label} | {} | {ms} | {:.1} | {:.1} | {:.0} |",
                names.len(),
                mib(kept, 4),
                mib(kept, 2),
                secs(rest)
            );
        }
    }
}

// ---------------------------------------------------------------- list

fn cmd_list(sfz_path: &Path, scan_path: &Path, layer_counts: &str, extras: bool, trim: &str) {
    let regions = read_sfz(sfz_path);
    let scan = read_scan(scan_path);
    let all = layers(&regions);
    let mut picked = BTreeSet::new();
    for n in layer_counts.split(',') {
        picked.extend(pick_layers(&all, n.parse().expect("layer count")));
    }
    let names = names_of(&regions, |r| match r.kind {
        Kind::Main => picked.contains(&layer_of(&all, r)),
        _ => extras,
    });
    let len = match trim {
        "none" => Len::Full,
        "30" => Len::Rel(1),
        "40" => Len::Rel(2),
        "50" => Len::Rel(3),
        "60" => Len::Rel(4),
        "72" => Len::Use72,
        _ => usage(),
    };
    for n in &names {
        println!("{n}\t{}", len.of(&scan[n]));
    }
}

fn read_list(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .expect("read list")
        .lines()
        .filter_map(|l| l.split('\t').next())
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------- load

fn status_kb(key: &str) -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn cmd_load(dir: &Path, ext: &str, list: &Path, format: &str, threads: usize) {
    let paths: Vec<PathBuf> = read_list(list)
        .iter()
        .map(|n| dir.join(format!("{n}.{ext}")))
        .collect();
    let file_bytes: u64 = paths
        .iter()
        .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    let rss_before = status_kb("VmRSS:");
    let started = Instant::now();
    let out = match format {
        "i16" => {
            let loaded = parallel(&paths, threads, |p| {
                decode::decode::<i16>(p).unwrap_or_else(|e| panic!("{e}"))
            });
            (
                started.elapsed(),
                summary(&loaded, 2, |s| *s as i64),
                status_kb("VmRSS:"),
            )
        }
        "f32" => {
            let loaded = parallel(&paths, threads, |p| {
                decode::decode::<f32>(p).unwrap_or_else(|e| panic!("{e}"))
            });
            (
                started.elapsed(),
                summary(&loaded, 4, |s| (*s * 32768.0) as i64),
                status_kb("VmRSS:"),
            )
        }
        _ => usage(),
    };
    let (took, (frames, bytes, check), rss_after) = out;
    println!(
        "{}\t{ext}\t{format}\t{threads}\t{}\t{:.1}\t{:.0}\t{:.1}\t{:.0}\t{:.1}\t{:.1}\t{:.1}\t{check}",
        dir.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        paths.len(),
        file_bytes as f64 / MIB,
        secs(frames),
        bytes as f64 / MIB,
        took.as_secs_f64() * 1000.0,
        rss_before as f64 / 1024.0,
        rss_after as f64 / 1024.0,
        status_kb("VmHWM:") as f64 / 1024.0,
    );
}

/// Frames, bytes held, and a checksum that also makes sure every page was really written.
fn summary<S>(
    loaded: &[decode::Decoded<S>],
    size: usize,
    to: impl Fn(&S) -> i64,
) -> (usize, usize, i64) {
    let frames = loaded.iter().map(|d| d.frames()).sum();
    let bytes = loaded.iter().map(|d| d.samples.capacity() * size).sum();
    let check = loaded
        .iter()
        .flat_map(|d| d.samples.iter().step_by(1009))
        .map(to)
        .sum();
    (frames, bytes, check)
}

// ---------------------------------------------------------------- evict

/// Asks the kernel to forget the cached pages of these files only (no system-wide setting).
fn cmd_evict(dir: &Path, ext: &str, list: &Path) {
    use std::os::fd::AsRawFd;
    let mut failed = 0;
    for name in read_list(list) {
        let Ok(file) = fs::File::open(dir.join(format!("{name}.{ext}"))) else {
            failed += 1;
            continue;
        };
        let _ = file.sync_all();
        // SAFETY: the descriptor is open for the whole call; the call only gives advice.
        let rc = unsafe { libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED) };
        if rc != 0 {
            failed += 1;
        }
    }
    if failed > 0 {
        eprintln!("evict: {failed} files failed");
        std::process::exit(1);
    }
}
