//! The SoundFont reader on a real file, FluidR3_GM (MIT). The tests pass without doing
//! anything on a PC that does not have the file.

use std::path::PathBuf;
use std::process::Command;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_sound::sampler::Sampler;
use meloet_sound::sf2::Sf2File;

const RATE: f32 = 48_000.0;
const BLOCK: usize = 256;

fn fluid_r3() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    [
        format!("{home}/.cache/meloet/sounds-src/fluidr3/FluidR3_GM.sf2"),
        "/usr/share/sounds/sf2/FluidR3_GM.sf2".to_string(),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
    .or_else(|| {
        eprintln!("FluidR3_GM.sf2 is not on this PC: nothing checked");
        None
    })
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

fn play(sampler: &mut Sampler, seconds: f32) -> Vec<f32> {
    let mut out = Vec::new();
    let (mut left, mut right) = ([0.0_f32; BLOCK], [0.0_f32; BLOCK]);
    while out.len() < (seconds * RATE) as usize {
        sampler.render(&mut left, &mut right);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        out.extend(left.iter().zip(&right).map(|(l, r)| (l + r) * 0.5));
    }
    out
}

#[test]
fn the_presets_are_listed_and_only_one_presets_samples_are_read() {
    let Some(path) = fluid_r3() else { return };
    let sf = Sf2File::open(&path).unwrap();
    let presets = sf.presets();
    assert_eq!(presets.len(), 189);
    let name = |bank, program| {
        let found = presets
            .iter()
            .find(|p| (p.bank, p.program) == (bank, program));
        found.map(|p| p.name.as_str())
    };
    assert_eq!(name(0, 0), Some("Yamaha Grand Piano"));
    assert_eq!(name(0, 24), Some("Nylon String Guitar"));
    assert_eq!(name(128, 0), Some("Standard"));
    assert_eq!(name(128, 40), Some("Brush"));
    assert_eq!(presets.iter().filter(|p| p.bank == 128).count(), 31);

    let piano = sf.load(0, 0, RATE).unwrap();
    assert_eq!(piano.kind(), "Yamaha Grand Piano");
    // The file is 142 MiB; the piano is a small part of it.
    assert!(
        piano.sample_bytes() < 40 * 1024 * 1024,
        "{}",
        piano.sample_bytes()
    );
    let mut sampler = Sampler::new(piano.into());
    sampler.prepare(RATE, BLOCK);
    sampler.event(InstrumentEvent::NoteOn {
        note: 60,
        velocity: 100,
    });
    let out = play(&mut sampler, 4.0);
    let (early, late) = (rms(&out[4800..14_400]), rms(&out[168_000..]));
    // A held piano note dies away by itself.
    assert!(early > 0.01, "{early}");
    assert!(late < early * 0.5, "{early} {late}");
}

#[test]
fn every_preset_loads_and_plays() {
    let Some(path) = fluid_r3() else { return };
    let sf = Sf2File::open(&path).unwrap();
    for preset in sf.presets() {
        let bank = sf.load(preset.bank, preset.program, RATE).unwrap();
        assert!(bank.region_count() > 0);
        let mut sampler = Sampler::new(bank.into());
        sampler.prepare(RATE, BLOCK);
        let mut loudest = 0.0_f32;
        for note in [36, 38, 42, 60, 72] {
            sampler.event(InstrumentEvent::NoteOn {
                note,
                velocity: 100,
            });
            let out = play(&mut sampler, 0.3);
            loudest = out.iter().fold(loudest, |m, x| m.max(x.abs()));
            sampler.event(InstrumentEvent::NoteOff { note });
        }
        assert!(
            loudest > 1.0e-3,
            "{}:{} {}",
            preset.bank,
            preset.program,
            preset.name
        );
    }
}

/// The same zones as `tools/sounds/sf2.py` gives, for every preset: one line a zone.
const PYTHON: &str = r#"
import sys
sys.path.insert(0, sys.argv[1])
import sf2
sf = sf2.SoundFont(sys.argv[2])
for bank, prog in sorted({(bank, prog) for _, prog, bank, _ in sf.phdr[:-1]}):
    pname, regs, mods = sf.regions(bank, prog)
    print(f"== {bank}:{prog} {pname} {len(regs)}")
    for r in regs:
        s = sf.samples[r["sample"]]
        gens = ",".join(f"{k}={v}" for k, v in r["gen"].items())
        print("|".join(str(x) for x in (s["name"], s["type"], s["link"], r["inst"], r["keyLo"],
                                         r["keyHi"], r["velLo"], r["velHi"], gens)))
"#;

#[test]
fn the_zones_are_the_ones_the_python_reader_gives() {
    let Some(path) = fluid_r3() else { return };
    let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/sounds");
    let python = Command::new("python3")
        .args(["-I", "-c", PYTHON])
        .arg(&tools)
        .arg(&path)
        .output();
    let want = match python {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        other => {
            eprintln!("python3 with numpy did not run: nothing checked ({other:?})");
            return;
        }
    };

    let sf = Sf2File::open(&path).unwrap();
    let mut got = String::new();
    for preset in sf.presets() {
        let zones = sf.zones(preset.bank, preset.program).unwrap();
        let head = format!(
            "== {}:{} {} {}\n",
            preset.bank,
            preset.program,
            preset.name,
            zones.len()
        );
        got.push_str(&head);
        for z in zones {
            let gens: Vec<String> = z
                .generators
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            got.push_str(&format!(
                "{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
                z.sample,
                z.sample_type,
                z.sample_link,
                z.instrument,
                z.keys.0,
                z.keys.1,
                z.velocities.0,
                z.velocities.1,
                gens.join(",")
            ));
        }
    }
    let (got_lines, want_lines) = (got.lines().count(), want.lines().count());
    for (number, (g, w)) in got.lines().zip(want.lines()).enumerate() {
        assert_eq!(g, w, "line {number}");
    }
    assert_eq!(got_lines, want_lines);
    // 189 presets and their 16912 zones.
    assert_eq!(got_lines, 189 + 16_912);
}
