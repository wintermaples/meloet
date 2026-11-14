//! A stand-in keyboard on the ALSA sequencer, so latency can be sampled hundreds of times without hands.
//! vkeys <count> [min_gap_ms max_gap_ms]
use std::ffi::CString;
use std::time::Duration;

use alsa::seq::{EvNote, Event, EventType, PortCap, PortType, Seq};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let count: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(100);
    let lo: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(150);
    let hi: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(400);
    let seq = Seq::open(None, None, false).unwrap();
    seq.set_client_name(&CString::new("meloet-vkeys").unwrap()).unwrap();
    let port = seq.create_simple_port(&CString::new("meloet-vkeys").unwrap(), PortCap::READ | PortCap::SUBS_READ,
        PortType::MIDI_GENERIC | PortType::APPLICATION).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let mut r = 0x2545F491u32;
    for i in 0..count {
        let note = 48 + (i % 25) as u8;
        for (t, vel) in [(EventType::Noteon, 100u8), (EventType::Noteoff, 0)] {
            let mut ev = Event::new(t, &EvNote { channel: 0, note, velocity: vel, off_velocity: 0, duration: 0 });
            ev.set_source(port);
            ev.set_subs();
            ev.set_direct();
            seq.event_output_direct(&mut ev).unwrap();
            if t == EventType::Noteon { std::thread::sleep(Duration::from_millis(80)); }
        }
        r ^= r << 13; r ^= r >> 17; r ^= r << 5;
        std::thread::sleep(Duration::from_millis(lo + r as u64 % (hi - lo + 1)));
    }
    println!("sent {count}");
}
