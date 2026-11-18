//! Just enough SFZ to read the Salamander file: headers, `key=value` opcodes, group inheritance.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Main,
    /// `trigger=release` with `pitch_keytrack=0`: the hammer falling back.
    Release,
    /// `trigger=release`, pitched: the strings ringing on after the key is let go.
    Resonance,
    /// Started by CC 64.
    Pedal,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Main => "main",
            Kind::Release => "release",
            Kind::Resonance => "resonance",
            Kind::Pedal => "pedal",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Region {
    pub kind: Kind,
    /// File name without directory and extension.
    pub name: String,
    pub lokey: i32,
    pub hikey: i32,
    pub lovel: i32,
    pub hivel: i32,
    pub center: i32,
    /// The group's own opcodes, as written, for the table.
    pub group: String,
}

pub fn parse(text: &str) -> Vec<Region> {
    let mut group: BTreeMap<String, String> = BTreeMap::new();
    let mut group_text = String::new();
    let mut region: Option<BTreeMap<String, String>> = None;
    let mut out = Vec::new();

    let mut flush = |region: &mut Option<BTreeMap<String, String>>,
                     group: &BTreeMap<String, String>,
                     group_text: &str| {
        let Some(own) = region.take() else { return };
        let get = |k: &str| own.get(k).or_else(|| group.get(k));
        let num = |k: &str, default: i32| get(k).and_then(|v| v.parse().ok()).unwrap_or(default);
        let Some(sample) = get("sample") else { return };
        let name = sample
            .rsplit('/')
            .next()
            .unwrap_or(sample)
            .trim_end_matches(".flac")
            .to_string();
        let kind = if get("on_locc64").is_some() {
            Kind::Pedal
        } else if get("trigger").map(String::as_str) == Some("release") {
            if get("pitch_keytrack").map(String::as_str) == Some("0") {
                Kind::Release
            } else {
                Kind::Resonance
            }
        } else {
            Kind::Main
        };
        out.push(Region {
            kind,
            name,
            lokey: num("lokey", 0),
            hikey: num("hikey", 127),
            lovel: num("lovel", 1),
            hivel: num("hivel", 127),
            center: num("pitch_keycenter", 60),
            group: group_text.to_string(),
        });
    };

    for line in text.lines() {
        let line = line.split("//").next().unwrap_or("");
        for token in line.split_whitespace() {
            match token {
                "<group>" => {
                    flush(&mut region, &group, &group_text);
                    group.clear();
                    group_text.clear();
                }
                "<region>" => {
                    flush(&mut region, &group, &group_text);
                    region = Some(BTreeMap::new());
                }
                _ => {
                    let Some((k, v)) = token.split_once('=') else {
                        continue;
                    };
                    match region.as_mut() {
                        Some(r) => {
                            r.insert(k.to_string(), v.to_string());
                        }
                        None => {
                            group.insert(k.to_string(), v.to_string());
                            if !group_text.is_empty() {
                                group_text.push(' ');
                            }
                            group_text.push_str(token);
                        }
                    }
                }
            }
        }
    }
    flush(&mut region, &group, &group_text);
    out
}

pub fn note_name(key: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[key.rem_euclid(12) as usize],
        key.div_euclid(12) - 1
    )
}
