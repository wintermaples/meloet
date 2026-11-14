//! Tests against the real ALSA sequencer (/dev/snd/seq). Where it cannot be opened they pass
//! without checking anything and say so on stderr.

use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use alsa::seq::{Addr, ClientIter, PortSubscribeIter, QuerySubsType, Seq};
use meloet_midi::vkeys::VirtualKeyboard;
use meloet_midi::{
    Keyboard, MidiEvent, MidiInput, MidiMessage, MidiOptions, list_ports, monotonic_us,
};

const WAIT: Duration = Duration::from_secs(3);
const VIRTUAL: MidiOptions = MidiOptions {
    accept_virtual: true,
    only_virtual: false,
};

fn sequencer_missing() -> bool {
    let missing = Seq::open(None, None, false).is_err();
    if missing {
        eprintln!("skipped: the ALSA sequencer cannot be opened here");
    }
    missing
}

/// Other tests and the real keyboard share the sequencer, so every test uses its own client name.
fn unique(tag: &str) -> String {
    format!("meloet-vkeys-test-{tag}-{}", std::process::id())
}

fn start(options: MidiOptions) -> (MidiInput, Receiver<MidiMessage>) {
    let (tx, rx) = channel();
    let input = MidiInput::start(options, move |m| {
        let _ = tx.send(m);
    })
    .expect("start the MIDI input");
    (input, rx)
}

/// The next message that `pick` accepts. Messages it rejects are dropped.
fn wait_for<T>(rx: &Receiver<MidiMessage>, mut pick: impl FnMut(&MidiMessage) -> Option<T>) -> T {
    let deadline = Instant::now() + WAIT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let message = rx
            .recv_timeout(left)
            .expect("the expected message did not come in time");
        if let Some(found) = pick(&message) {
            return found;
        }
    }
}

fn added(name: &str) -> impl FnMut(&MidiMessage) -> Option<Keyboard> {
    move |m| match m {
        MidiMessage::Added(k) if k.name == name => Some(k.clone()),
        _ => None,
    }
}

fn until_ready(rx: &Receiver<MidiMessage>) -> Vec<Keyboard> {
    let mut seen = Vec::new();
    wait_for(rx, |m| match m {
        MidiMessage::Added(k) => {
            seen.push(k.clone());
            None
        }
        MidiMessage::Ready => Some(()),
        other => panic!("before Ready came {other:?}"),
    });
    seen
}

#[test]
fn keyboard_that_was_there_before_start_is_added_before_ready() {
    if sequencer_missing() {
        return;
    }
    let name = unique("before");
    let keys = VirtualKeyboard::open(&name).unwrap();
    let (input, rx) = start(VIRTUAL);
    let seen = until_ready(&rx);
    let mine: Vec<_> = seen.iter().filter(|k| k.name == name).collect();
    assert_eq!(mine.len(), 1, "added exactly once: {seen:?}");
    assert_eq!(mine[0].address(), keys.address());
    assert_eq!(mine[0].id, format!("{name}:{name}"));
    assert_eq!(mine[0].label(), format!("{name}:{name} {}", keys.address()));

    let listed = list_ports(VIRTUAL).unwrap();
    assert!(
        listed
            .iter()
            .any(|k| k.name == name && k.address() == keys.address()),
        "{listed:?}"
    );
    assert!(
        list_ports(MidiOptions::default())
            .unwrap()
            .iter()
            .all(|k| k.name != name)
    );
    input.stop();
}

#[test]
fn virtual_keyboard_is_ignored_unless_asked() {
    if sequencer_missing() {
        return;
    }
    let name = unique("ignored");
    let _keys = VirtualKeyboard::open(&name).unwrap();
    let (input, rx) = start(MidiOptions::default());
    assert!(until_ready(&rx).iter().all(|k| k.name != name));
    input.stop();
}

#[test]
fn plugging_and_unplugging_after_start_is_reported_with_the_same_id() {
    if sequencer_missing() {
        return;
    }
    let name = unique("plug");
    let (input, rx) = start(VIRTUAL);
    until_ready(&rx);
    let mut ids = Vec::new();
    for _ in 0..2 {
        let keys = VirtualKeyboard::open(&name).unwrap();
        let k = wait_for(&rx, added(&name));
        assert_eq!(k.address(), keys.address());
        drop(keys);
        let id = k.id.clone();
        wait_for(&rx, |m| {
            matches!(m, MidiMessage::Removed { id: gone } if *gone == id).then_some(())
        });
        ids.push(k.id);
    }
    assert_eq!(ids[0], ids[1], "the id survives plugging in again");
    input.stop();
    // Nothing about this keyboard may be left over (no second Added, no second Removed).
    assert!(rx.try_iter().all(|m| !format!("{m:?}").contains(&name)));
}

#[test]
fn notes_controllers_and_bend_arrive_in_order() {
    if sequencer_missing() {
        return;
    }
    let name = unique("events");
    let keys = VirtualKeyboard::open(&name).unwrap();
    let (input, rx) = start(VIRTUAL);
    let id = wait_for(&rx, added(&name)).id;
    wait_for(&rx, |m| (*m == MidiMessage::Ready).then_some(()));

    let sent = [
        MidiEvent::NoteOn {
            note: 60,
            velocity: 90,
        },
        MidiEvent::NoteOn {
            note: 60,
            velocity: 0,
        },
        MidiEvent::NoteOn {
            note: 64,
            velocity: 1,
        },
        MidiEvent::NoteOff {
            note: 64,
            velocity: 33,
        },
        MidiEvent::Control {
            controller: 64,
            value: 127,
        },
        MidiEvent::Control {
            controller: 1,
            value: 0,
        },
        MidiEvent::PitchBend { value: -8192 },
        MidiEvent::PitchBend { value: 8191 },
        MidiEvent::PitchBend { value: 0 },
    ];
    let mut expected = sent.to_vec();
    expected[1] = MidiEvent::NoteOff {
        note: 60,
        velocity: 0,
    };

    let before = monotonic_us();
    for event in sent {
        keys.send(event).unwrap();
    }
    for want in expected {
        let (got, t_us) = wait_for(&rx, |m| match m {
            MidiMessage::Event {
                keyboard,
                event,
                t_us,
            } if *keyboard == id => Some((*event, *t_us)),
            _ => None,
        });
        assert_eq!(got, want);
        assert!(
            t_us >= before && t_us <= monotonic_us(),
            "the time is on the monotonic clock"
        );
    }
    input.stop();
}

#[test]
fn stop_ends_the_thread_and_the_sink_is_not_called_again() {
    if sequencer_missing() {
        return;
    }
    let name = unique("stop");
    let keys = VirtualKeyboard::open(&name).unwrap();
    for _ in 0..20 {
        let (input, rx) = start(VIRTUAL);
        wait_for(&rx, |m| (*m == MidiMessage::Ready).then_some(()));
        let begun = Instant::now();
        input.stop();
        assert!(
            begun.elapsed() < Duration::from_secs(1),
            "stop took {:?}",
            begun.elapsed()
        );
        keys.send(MidiEvent::NoteOn {
            note: 60,
            velocity: 90,
        })
        .unwrap();
        // Other tests' keyboards may have been heard before the stop; this one never.
        let mine = format!("{name}:{name}");
        assert!(
            rx.try_iter()
                .all(|m| !matches!(m, MidiMessage::Event { keyboard, .. } if keyboard == mine))
        );
        // The sink owned the only sender, so a closed channel means the thread and sink are gone.
        assert_eq!(
            rx.recv_timeout(Duration::from_millis(1)),
            Err(RecvTimeoutError::Disconnected)
        );
    }
    // Stopping while the thread has never been woken by an event, and by plain drop.
    let (input, _rx) = start(VIRTUAL);
    drop(input);
}

#[test]
fn vkeys_tool_plays_and_logs_what_it_sent() {
    if sequencer_missing() {
        return;
    }
    // The tool always uses the plain client name, so this is the only test that may run it.
    let log = std::env::temp_dir().join(format!("meloet-vkeys-{}.json", std::process::id()));
    let (input, rx) = start(VIRTUAL);
    until_ready(&rx);
    let mut tool = Command::new(env!("CARGO_BIN_EXE_meloet-vkeys"))
        .args([
            "play",
            "--notes",
            "60,64,67",
            "--count",
            "6",
            "--interval-ms",
            "30",
            "--hold-ms",
            "10",
            "--velocity",
            "90",
        ])
        .args(["--lead-ms", "300", "--log"])
        .arg(&log)
        .spawn()
        .unwrap();
    let id = wait_for(&rx, added("meloet-vkeys")).id;
    let mut got = Vec::new();
    while got.len() < 12 {
        got.push(wait_for(&rx, |m| match m {
            MidiMessage::Event {
                keyboard,
                event,
                t_us,
            } if *keyboard == id => Some((*event, *t_us)),
            _ => None,
        }));
    }
    assert!(tool.wait().unwrap().success());
    wait_for(&rx, |m| {
        matches!(m, MidiMessage::Removed { id: gone } if *gone == id).then_some(())
    });
    input.stop();

    let text = std::fs::read_to_string(&log).unwrap();
    let _ = std::fs::remove_file(&log);
    let field = |line: &str, name: &str| -> u64 {
        let rest = line
            .split(&format!("\"{name}\":"))
            .nth(1)
            .unwrap_or_else(|| panic!("{name} in {line}"));
        rest.split([',', '}']).next().unwrap().parse().unwrap()
    };
    let lines: Vec<&str> = text.lines().filter(|l| l.contains("\"index\"")).collect();
    assert_eq!(lines.len(), 6, "{text}");
    for (i, line) in lines.iter().enumerate() {
        let note = [60, 64, 67][i % 3];
        assert_eq!(
            (
                field(line, "index"),
                field(line, "note"),
                field(line, "velocity")
            ),
            (i as u64, note as u64, 90)
        );
        let (on, on_t) = got[i * 2];
        let (off, off_t) = got[i * 2 + 1];
        assert_eq!(on, MidiEvent::NoteOn { note, velocity: 90 });
        assert_eq!(off, MidiEvent::NoteOff { note, velocity: 0 });
        // Sent and received on the same clock; a send can never be later than its arrival.
        let (sent_on, sent_off) = (field(line, "on_t_us"), field(line, "off_t_us"));
        assert!(
            sent_on <= on_t && on_t - sent_on < 50_000,
            "on: sent {sent_on}, got {on_t}"
        );
        assert!(
            sent_off <= off_t && off_t - sent_off < 50_000,
            "off: sent {sent_off}, got {off_t}"
        );
    }
}

#[test]
#[ignore = "needs the Keystation 49 MK3 plugged in"]
fn real_keystation_is_added_and_only_its_first_port_is_heard() {
    const NAME: &str = "Keystation 49 MK3";
    let (input, rx) = start(MidiOptions::default());
    let seen = until_ready(&rx);
    let mine: Vec<_> = seen.iter().filter(|k| k.name == NAME).collect();
    assert_eq!(mine.len(), 1, "{seen:?}");
    let k = mine[0];
    assert_eq!(k.port, 0);
    eprintln!("label: {}   id: {}", k.label(), k.id);
    assert!(
        seen.iter()
            .all(|k| k.name != "Midi Through" && !k.name.starts_with("PipeWire")),
        "{seen:?}"
    );
    assert!(list_ports(MidiOptions::default()).unwrap().contains(k));

    let seq = Seq::open(None, None, false).unwrap();
    let meloet: Vec<i32> = ClientIter::new(&seq)
        .filter(|c| c.get_name() == Ok("meloet"))
        .map(|c| c.get_client())
        .collect();
    assert!(!meloet.is_empty());
    let listeners = |port: i32| -> usize {
        PortSubscribeIter::new(
            &seq,
            Addr {
                client: k.client,
                port,
            },
            QuerySubsType::READ,
        )
        .filter(|s| meloet.contains(&s.get_dest().client))
        .count()
    };
    assert_eq!(listeners(0), meloet.len(), "port 0 is heard");
    assert_eq!(listeners(1), 0, "port 1 (Transport) is not heard");
    input.stop();
}
