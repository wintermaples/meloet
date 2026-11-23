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
                    [--pattern spaced|fast|mixed] [--chord 1] [--spread-ms 6] [--fast-ms 20-150]
meloet-vkeys cc     --controller N --value N
meloet-vkeys bend   --value N            (-8192..8191)
meloet-vkeys button --note N             (a Mackie Control button on the button port: 94 play, 93 stop, 95 record, ...)
meloet-vkeys appear [--seconds 5] [--buttons 0|1]

Every command also takes --lead-ms N (default 500): the time between the port appearing and
the first event, so that a listener can subscribe, and --name NAME (default meloet-vkeys): the
client name, starting with meloet-vkeys. `button` and `appear --buttons 1` add a second port,
\"NAME buttons\", which meloet hears as the keyboard's button port.

play --pattern: spaced (the default) presses --chord keys every --interval-ms, each step up to
--jitter-ms late and the keys of a chord 0..--spread-ms apart; fast presses every key --fast-ms
after the one before; mixed mixes single keys, chords of 2..4 and runs of 2..4 fast keys, with
--interval-ms (+ jitter) between them. --count counts keys, not steps. A note still held is
skipped, so chords and runs need more --notes. The log lists the keys in the order pressed.";

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
            let jitter = Duration::from_millis(args.get("jitter-ms", Some(0))?);
            let hold = Duration::from_millis(args.get("hold-ms", Some(120))?);
            let velocity: u8 = args.get("velocity", Some(90))?;
            let chord: usize = args.get("chord", Some(1))?;
            let spread = Duration::from_millis(args.get("spread-ms", Some(6))?);
            let fast = ms_range(&args.get("fast-ms", Some("20-150".to_string()))?)?;
            let pattern: String = args.get("pattern", Some("spaced".to_string()))?;
            let log: Option<String> = args.0.remove("log");
            args.done()?;
            if velocity == 0 || velocity > 127 {
                return Err("--velocity must be 1..127".into());
            }
            if chord == 0 {
                return Err("--chord must be 1 or more".into());
            }
            let style = match pattern.as_str() {
                "spaced" => Style::Spaced { chord, spread },
                "fast" => Style::Fast { fast },
                "mixed" => Style::Mixed { fast, spread },
                other => return Err(format!("--pattern: spaced, fast or mixed, not `{other}`")),
            };
            let mut rng = Rng::from_clock();
            let keys_at = plan(&style, count, interval, jitter, &mut rng);
            let keys_at = assign_notes(keys_at, &notes, hold)?;

            let keyboard = open()?;
            let start = Instant::now();
            // Sleep to fixed points in time, so that a slow send does not shift later events.
            let mut events: Vec<(Duration, bool, usize)> = keys_at
                .iter()
                .enumerate()
                .flat_map(|(i, k)| [(k.at, true, i), (k.at + hold, false, i)])
                .collect();
            // At the same moment, a release goes first, so that a note can be pressed again.
            events.sort_by_key(|&(at, on, _)| (at, on));
            let mut times = vec![(0u64, 0u64); keys_at.len()];
            for (at, on, i) in events {
                sleep((start + at).saturating_duration_since(Instant::now()));
                let note = keys_at[i].note;
                if on {
                    times[i].0 = send(&keyboard, MidiEvent::NoteOn { note, velocity })?;
                } else {
                    times[i].1 = send(&keyboard, MidiEvent::NoteOff { note, velocity: 0 })?;
                }
            }
            let mut json = format!(
                "{{\"client\":\"{name}\",\"address\":\"{}\",\"notes\":[",
                keyboard.address()
            );
            for (index, (key, (on_t_us, off_t_us))) in keys_at.iter().zip(times).enumerate() {
                let comma = if index == 0 { "" } else { "," };
                let _ = write!(
                    json,
                    "{comma}\n{{\"index\":{index},\"note\":{},\"velocity\":{velocity},\"on_t_us\":{on_t_us},\"off_t_us\":{off_t_us}}}",
                    key.note
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
        "button" => {
            let note: u8 = args.get("note", None)?;
            args.done()?;
            if note > 127 {
                return Err("--note must be 0..127".into());
            }
            let keys = VirtualKeyboard::open_with_buttons(&name).map_err(|e| e.to_string())?;
            sleep(lead);
            keys.press_button(note).map_err(|e| e.to_string())?;
        }
        "appear" => {
            let seconds: f64 = args.get("seconds", Some(5.0))?;
            let buttons: u8 = args.get("buttons", Some(0))?;
            args.done()?;
            let _keys = if buttons == 1 {
                VirtualKeyboard::open_with_buttons(&name)
            } else {
                VirtualKeyboard::open(&name)
            }
            .map_err(|e| e.to_string())?;
            sleep(Duration::try_from_secs_f64(seconds).map_err(|e| format!("--seconds: {e}"))?);
        }
        other => return Err(format!("unknown command `{other}`")),
    }
    Ok(())
}

/// How the press times of `play` are laid out.
enum Style {
    /// One step per `--interval-ms` (each up to `--jitter-ms` late), `chord` keys per step.
    Spaced { chord: usize, spread: Duration },
    /// Every key `fast` after the one before.
    Fast { fast: (Duration, Duration) },
    /// Single keys, chords of 2..4 and runs of 2..4 fast keys, `--interval-ms` (+ jitter) apart.
    Mixed {
        fast: (Duration, Duration),
        spread: Duration,
    },
}

struct KeyAt {
    at: Duration,
    note: u8,
}

/// xorshift64: keys at random phases against the screen's frames, not locked to them.
struct Rng(u64);

impl Rng {
    fn from_clock() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self(nanos | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// 0..=most, in microseconds.
    fn upto(&mut self, most: Duration) -> Duration {
        Duration::from_micros(self.below(most.as_micros() as u64 + 1))
    }

    fn within(&mut self, (least, most): (Duration, Duration)) -> Duration {
        least + self.upto(most - least)
    }
}

/// "A-B" or "A", in ms.
fn ms_range(text: &str) -> Result<(Duration, Duration), String> {
    let error = || format!("--fast-ms: cannot read `{text}` (A-B in ms)");
    let (a, b) = text.split_once('-').unwrap_or((text, text));
    let a: u64 = a.trim().parse().map_err(|_| error())?;
    let b: u64 = b.trim().parse().map_err(|_| error())?;
    if a > b {
        return Err(error());
    }
    Ok((Duration::from_millis(a), Duration::from_millis(b)))
}

/// Press times from the start, earliest first.
fn plan(
    style: &Style,
    count: usize,
    interval: Duration,
    jitter: Duration,
    rng: &mut Rng,
) -> Vec<Duration> {
    let mut times = Vec::with_capacity(count);
    match *style {
        Style::Spaced { chord, spread } => {
            for step in 0u32.. {
                if times.len() >= count {
                    break;
                }
                let mut at = interval * step + rng.upto(jitter);
                for k in 0..chord {
                    if k > 0 {
                        at += rng.upto(spread);
                    }
                    times.push(at);
                }
            }
        }
        Style::Fast { fast } => {
            let mut at = Duration::ZERO;
            while times.len() < count {
                times.push(at);
                at += rng.within(fast);
            }
        }
        Style::Mixed { fast, spread } => {
            let mut start = Duration::ZERO;
            while times.len() < count {
                let kind = rng.below(3);
                let size = if kind == 0 { 1 } else { 2 + rng.below(3) };
                let mut at = start;
                for k in 0..size {
                    if k > 0 {
                        at += if kind == 1 {
                            rng.upto(spread)
                        } else {
                            rng.within(fast)
                        };
                    }
                    times.push(at);
                }
                start = at + interval + rng.upto(jitter);
            }
        }
    }
    times.truncate(count);
    times.sort();
    times
}

/// Goes round `notes`, skipping a note that is still held at that time.
fn assign_notes(times: Vec<Duration>, notes: &[u8], hold: Duration) -> Result<Vec<KeyAt>, String> {
    let mut released = [Duration::ZERO; 128];
    let mut next = 0;
    times
        .into_iter()
        .enumerate()
        .map(|(index, at)| {
            let pick = (0..notes.len())
                .map(|k| (next + k) % notes.len())
                .find(|&j| released[notes[j] as usize] <= at)
                .ok_or(format!(
                    "--notes: at key {index} every note is still held; give more notes or a shorter --hold-ms"
                ))?;
            let note = notes[pick];
            released[note as usize] = at + hold;
            next = pick + 1;
            Ok(KeyAt { at, note })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn spaced_keys_without_jitter_keep_the_old_steps() {
        let style = Style::Spaced {
            chord: 1,
            spread: 6 * MS,
        };
        let times = plan(&style, 5, 250 * MS, Duration::ZERO, &mut Rng(7));
        assert_eq!(times, (0..5).map(|i| 250 * MS * i).collect::<Vec<_>>());
        let keys = assign_notes(times, &[60, 64, 67], 120 * MS).unwrap();
        let notes: Vec<u8> = keys.iter().map(|k| k.note).collect();
        assert_eq!(notes, [60, 64, 67, 60, 64]);
    }

    #[test]
    fn a_chord_is_pressed_within_its_spread() {
        let style = Style::Spaced {
            chord: 3,
            spread: 6 * MS,
        };
        let times = plan(&style, 7, 250 * MS, 30 * MS, &mut Rng(11));
        assert_eq!(times.len(), 7);
        for group in times.chunks(3) {
            let width = *group.last().unwrap() - group[0];
            assert!(width <= 12 * MS, "{group:?}");
        }
    }

    #[test]
    fn mixed_keys_cover_every_gap_and_never_press_a_held_note() {
        let style = Style::Mixed {
            fast: (20 * MS, 150 * MS),
            spread: 6 * MS,
        };
        let times = plan(&style, 400, 257 * MS, 130 * MS, &mut Rng(3));
        assert_eq!(times.len(), 400);
        assert!(times.windows(2).all(|w| w[0] <= w[1]));
        let gaps: Vec<Duration> = times.windows(2).map(|w| w[1] - w[0]).collect();
        let frame = Duration::from_micros(16_667);
        assert!(gaps.iter().any(|&g| g <= frame));
        assert!(gaps.iter().any(|&g| g > frame && g <= 2 * frame));
        assert!(gaps.iter().any(|&g| g > 2 * frame && g <= 150 * MS));
        assert!(gaps.iter().any(|&g| g > 150 * MS));

        let hold = 120 * MS;
        let keys = assign_notes(times, &[60, 62, 64, 65, 67, 69, 71, 72], hold).unwrap();
        for (i, key) in keys.iter().enumerate() {
            for later in &keys[i + 1..] {
                if later.at >= key.at + hold {
                    break;
                }
                assert_ne!(later.note, key.note, "pressed again at {:?}", later.at);
            }
        }
    }

    #[test]
    fn too_few_notes_for_a_chord_is_an_error() {
        let times = vec![Duration::ZERO; 4];
        assert!(assign_notes(times, &[60, 64, 67], 120 * MS).is_err());
    }

    #[test]
    fn fast_ranges_read_a_pair_or_one_number() {
        assert_eq!(ms_range("20-150").unwrap(), (20 * MS, 150 * MS));
        assert_eq!(ms_range("40").unwrap(), (40 * MS, 40 * MS));
        assert!(ms_range("150-20").is_err());
        assert!(ms_range("x").is_err());
    }
}
