//! A virtual keyboard for tests and measurements. meloet sees it when started with
//! `MELOET_ACCEPT_VIRTUAL=1`; with `MELOET_VKEYS_NAME` set too, only when `--name` is that name.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::process::ExitCode;
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant};

use meloet_midi::vkeys::VirtualKeyboard;
use meloet_midi::{MidiEvent, VKEYS_CLIENT_NAME};

const USAGE: &str = "\
meloet-vkeys play   [--notes 60,64,67] [--count 8] [--interval-ms 250] [--jitter-ms 0] [--hold-ms 120] [--velocity 90] [--log FILE]
meloet-vkeys cc     --controller N --value N
meloet-vkeys bend   --value N            (-8192..8191)
meloet-vkeys appear [--seconds 5]

Every command also takes --lead-ms N (default 500): the time between the port appearing and
the first event, so that a listener can subscribe, and --name NAME (default meloet-vkeys): the
client name, starting with meloet-vkeys.";

struct Args(HashMap<String, String>);

impl Args {
    fn parse(rest: &[String]) -> Result<Self, String> {
        let mut map = HashMap::new();
        for pair in rest.chunks(2) {
            match pair {
                [key, value] if key.starts_with("--") => {
                    map.insert(key[2..].to_string(), value.clone())
                }
                _ => return Err(format!("expected `--name value`, got `{}`", pair.join(" "))),
            };
        }
        Ok(Self(map))
    }

    fn get<T: FromStr>(&mut self, name: &str, default: Option<T>) -> Result<T, String> {
        match self.0.remove(name) {
            Some(text) => text
                .parse()
                .map_err(|_| format!("--{name}: cannot read `{text}`")),
            None => default.ok_or_else(|| format!("--{name} is required")),
        }
    }

    fn done(self) -> Result<(), String> {
        match self.0.keys().next() {
            Some(name) => Err(format!("unknown option --{name}")),
            None => Ok(()),
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("meloet-vkeys: {message}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

fn run(argv: &[String]) -> Result<(), String> {
    let (command, rest) = argv.split_first().ok_or("no command")?;
    let mut args = Args::parse(rest)?;
    let lead = Duration::from_millis(args.get("lead-ms", Some(500))?);
    let name: String = args.get("name", Some(VKEYS_CLIENT_NAME.to_string()))?;
    if !name.starts_with(VKEYS_CLIENT_NAME) {
        return Err(format!("--name must start with {VKEYS_CLIENT_NAME}"));
    }
    let open = || {
        let keys = VirtualKeyboard::open(&name).map_err(|e| e.to_string())?;
        sleep(lead);
        Ok::<_, String>(keys)
    };
    let send = |keys: &VirtualKeyboard, event| keys.send(event).map_err(|e| e.to_string());
    match command.as_str() {
        "play" => {
            let notes: Vec<u8> = args
                .get("notes", Some("60,64,67".to_string()))?
                .split(',')
                .map(|n| {
                    n.trim()
                        .parse()
                        .ok()
                        .filter(|n| *n <= 127)
                        .ok_or(format!("--notes: cannot read `{n}`"))
                })
                .collect::<Result<_, _>>()?;
            let count: usize = args.get("count", Some(8))?;
            let interval = Duration::from_millis(args.get("interval-ms", Some(250))?);
            let jitter_ms: u64 = args.get("jitter-ms", Some(0))?;
            let hold = Duration::from_millis(args.get("hold-ms", Some(120))?);
            let velocity: u8 = args.get("velocity", Some(90))?;
            let log: Option<String> = args.0.remove("log");
            args.done()?;
            if velocity == 0 || velocity > 127 {
                return Err("--velocity must be 1..127".into());
            }
            let keys = open()?;
            let start = Instant::now();
            // xorshift64: keys at random phases against the screen's frames, not locked to them.
            let mut seed = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64))
                | 1;
            let mut json = format!(
                "{{\"client\":\"{name}\",\"address\":\"{}\",\"notes\":[",
                keys.address()
            );
            for (index, note) in notes.iter().copied().cycle().take(count).enumerate() {
                // Sleep to fixed points in time, so that a slow send does not shift later notes.
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let jitter = Duration::from_micros(seed % (jitter_ms * 1000 + 1));
                let at = start + interval * index as u32 + jitter;
                sleep(at.saturating_duration_since(Instant::now()));
                let on_t_us = send(&keys, MidiEvent::NoteOn { note, velocity })?;
                sleep((at + hold).saturating_duration_since(Instant::now()));
                let off_t_us = send(&keys, MidiEvent::NoteOff { note, velocity: 0 })?;
                let comma = if index == 0 { "" } else { "," };
                let _ = write!(
                    json,
                    "{comma}\n{{\"index\":{index},\"note\":{note},\"velocity\":{velocity},\"on_t_us\":{on_t_us},\"off_t_us\":{off_t_us}}}"
                );
            }
            json.push_str("\n]}\n");
            if let Some(path) = log {
                std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?;
            }
        }
        "cc" => {
            let controller: u8 = args.get("controller", None)?;
            let value: u8 = args.get("value", None)?;
            args.done()?;
            if controller > 127 || value > 127 {
                return Err("--controller and --value must be 0..127".into());
            }
            send(&open()?, MidiEvent::Control { controller, value })?;
        }
        "bend" => {
            let value: i16 = args.get("value", None)?;
            args.done()?;
            if !(-8192..=8191).contains(&value) {
                return Err("--value must be -8192..8191".into());
            }
            send(&open()?, MidiEvent::PitchBend { value })?;
        }
        "appear" => {
            let seconds: f64 = args.get("seconds", Some(5.0))?;
            args.done()?;
            let _keys = VirtualKeyboard::open(&name).map_err(|e| e.to_string())?;
            sleep(Duration::try_from_secs_f64(seconds).map_err(|e| format!("--seconds: {e}"))?);
        }
        other => return Err(format!("unknown command `{other}`")),
    }
    Ok(())
}
