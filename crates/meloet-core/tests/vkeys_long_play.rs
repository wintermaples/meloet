//! Not a check: a virtual keyboard that plays for minutes, for `tools/measure/long-record.sh`.
//! About 3.5 keys a second (some overlapping), the sustain pedal now and then, a pitch bend
//! sweep now and then. Every key is let go before it ends.
//!
//! `MELOET_LONGPLAY_NAME` (client name, required), `MELOET_LONGPLAY_SECONDS` (600),
//! `MELOET_LONGPLAY_LEAD_MS` (1000: from the port appearing to the first key),
//! `MELOET_LONGPLAY_LOG` (JSON with what was sent). Run with `-- --ignored --nocapture`.

use std::time::{Duration, Instant};

use meloet_midi::MidiEvent;
use meloet_midi::vkeys::VirtualKeyboard;

/// A small fixed generator, so that every run plays the same thing.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
#[ignore = "plays a virtual keyboard for minutes"]
fn play_for_minutes() {
    let Ok(name) = std::env::var("MELOET_LONGPLAY_NAME") else {
        eprintln!("MELOET_LONGPLAY_NAME is not set; nothing played");
        return;
    };
    let seconds: f64 = env_or("MELOET_LONGPLAY_SECONDS", 600.0);
    let lead = Duration::from_millis(env_or("MELOET_LONGPLAY_LEAD_MS", 1000));
    let keys = VirtualKeyboard::open(&name).expect("open the virtual keyboard");
    std::thread::sleep(lead);

    // (ms from the start, event), made first, then sent in order.
    let mut plan: Vec<(u64, MidiEvent)> = Vec::new();
    let mut rng = Lcg(0x6d65_6c6f_6574);
    let total_ms = (seconds * 1000.0) as u64;
    let mut held_until = [0u64; 128];
    let mut t = 0;
    while t < total_ms - 1_000 {
        let note = loop {
            let n = 48 + rng.next(37) as u8;
            if held_until[n as usize] <= t {
                break n;
            }
        };
        let hold = 80 + rng.next(320);
        held_until[note as usize] = t + hold + 1;
        let velocity = 20 + rng.next(11) as u8;
        plan.push((t, MidiEvent::NoteOn { note, velocity }));
        plan.push((t + hold, MidiEvent::NoteOff { note, velocity: 0 }));
        t += 220 + rng.next(130);
    }
    let mut p = 4_000;
    while p + 3_000 < total_ms - 1_000 {
        plan.push((
            p,
            MidiEvent::Control {
                controller: 64,
                value: 127,
            },
        ));
        plan.push((
            p + 3_000,
            MidiEvent::Control {
                controller: 64,
                value: 0,
            },
        ));
        p += 9_000 + rng.next(4_000);
    }
    let mut b = 7_000;
    while b + 600 < total_ms - 1_000 {
        for step in 0..=12u64 {
            let shape = 1.0 - (step as f64 / 6.0 - 1.0).abs();
            plan.push((
                b + step * 50,
                MidiEvent::PitchBend {
                    value: (shape * 4_000.0) as i16,
                },
            ));
        }
        b += 13_000 + rng.next(5_000);
    }
    plan.sort_by_key(|(at, _)| *at);

    let start = Instant::now();
    let (mut notes, mut pedals, mut bends) = (0u32, 0u32, 0u32);
    let mut late_max_us = 0u128;
    for (at, event) in &plan {
        let when = start + Duration::from_millis(*at);
        std::thread::sleep(when.saturating_duration_since(Instant::now()));
        late_max_us = late_max_us.max(Instant::now().duration_since(when).as_micros());
        keys.send(*event).expect("send");
        match event {
            MidiEvent::NoteOn { .. } => notes += 1,
            MidiEvent::Control { .. } => pedals += 1,
            MidiEvent::PitchBend { .. } => bends += 1,
            MidiEvent::NoteOff { .. } => {}
        }
    }
    let log = format!(
        "{{\"client\":\"{name}\",\"seconds\":{seconds},\"notes\":{notes},\"pedalEvents\":{pedals},\"bendEvents\":{bends},\"sendLateMaxUs\":{late_max_us}}}\n"
    );
    eprint!("{log}");
    if let Ok(path) = std::env::var("MELOET_LONGPLAY_LOG") {
        std::fs::write(path, log).expect("write the log");
    }
}
