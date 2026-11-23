//! What each key of a track's sound is, for the AI link: meloet's own kits by the rows of the
//! drum piano roll, an SF2 preset by the samples its zones play, a plugin by what it says of
//! its keys (LV2 midnam), and, for something that looks like a drum kit and says nothing, the
//! usual General MIDI layout as a guess.
//!
//! The rules answer from the song, the catalog and what plugins said; the core thread then
//! adds what the samples in memory tell ([`heard`]); the SF2 file is read off the core thread
//! ([`read_sf2`]). Each of these steps leaves a whole answer.

use std::collections::HashMap;
use std::sync::Arc;

use meloet_model::drums::DrumPiece;
use meloet_model::{
    BuiltinInstrument, GmFamily, InstrumentGroup, InstrumentRef, PluginPlace, Sf2Ref,
};
use meloet_plugin::wire::KeyName;
use meloet_sound::sampler::SampleBank;
use meloet_sound::sf2::Sf2File;

use super::Play;
use crate::ai::{AiAnswer, AiKey, AiKeyMap, AiKeyNames, AiKeyNote, AiKeySource, AiRefusal};
use crate::catalog::Catalog;
use crate::instruments::{Samples, sampled};
use crate::sf2store::Sf2Store;
use crate::sounds::Library;
use crate::state::{Effect, PluginAsk};

/// How long a plugin asked for the names of its keys is waited for before it is asked again.
const ASK_AGAIN_US: u64 = 2_000_000;

/// The names of the rows of meloet's own kits as the drum piano roll shows them
/// (`ui/src/text/ja.ts`, `drum.*`), by the model's id of the drum.
const KIT_NAMES: [(&str, &str); DrumPiece::COUNT] = [
    ("crash", "クラッシュ"),
    ("ride", "ライド"),
    ("openHiHat", "オープンハイハット"),
    ("pedalHiHat", "ペダルハイハット"),
    ("closedHiHat", "クローズハイハット"),
    ("highTom", "ハイタム"),
    ("midTom", "ミッドタム 2"),
    ("lowTom", "ロータム"),
    ("clap", "クラップ"),
    ("snare", "スネア"),
    ("sideStick", "サイドスティック"),
    ("kick", "キック"),
];

/// The first key General MIDI puts a drum on.
const GM_FIRST: u8 = 35;
/// The names the drum piano roll shows for the keys of an SF2 kit, General MIDI's, from
/// [`GM_FIRST`] on (`ui/src/text/ja.ts`, `gmDrum.*`).
const GM_NAMES: [&str; 47] = [
    "キック 2",
    "キック",
    "サイドスティック",
    "スネア",
    "クラップ",
    "スネア 2",
    "フロアタム 2",
    "クローズハイハット",
    "フロアタム",
    "ペダルハイハット",
    "ロータム",
    "オープンハイハット",
    "ミッドタム 2",
    "ミッドタム",
    "クラッシュ",
    "ハイタム",
    "ライド",
    "チャイナ",
    "ライドのベル",
    "タンバリン",
    "スプラッシュ",
    "カウベル",
    "クラッシュ 2",
    "ビブラスラップ",
    "ライド 2",
    "ボンゴ（高）",
    "ボンゴ（低）",
    "コンガ（高 ・ ミュート）",
    "コンガ（高）",
    "コンガ（低）",
    "ティンバレス（高）",
    "ティンバレス（低）",
    "アゴゴ（高）",
    "アゴゴ（低）",
    "カバサ",
    "マラカス",
    "ホイッスル（短）",
    "ホイッスル（長）",
    "ギロ（短）",
    "ギロ（長）",
    "クラベス",
    "ウッドブロック（高）",
    "ウッドブロック（低）",
    "クイーカ（ミュート）",
    "クイーカ",
    "トライアングル（ミュート）",
    "トライアングル",
];

/// The names of their keys that plugins were asked for.
#[derive(Debug, Default)]
pub(super) struct KeyAsks {
    /// What they said, by the ticket of their load, until the AI link takes it.
    said: HashMap<u64, Option<Vec<KeyName>>>,
    /// The plugins asked, and when (`now_us`).
    asked: Vec<(u64, u64)>,
}

fn gm_name(key: u8) -> Option<&'static str> {
    GM_NAMES
        .get(usize::from(key.checked_sub(GM_FIRST)?))
        .copied()
}

fn kit_name(piece: DrumPiece) -> &'static str {
    (KIT_NAMES.iter())
        .find(|(id, _)| *id == piece.id())
        .map_or(piece.id(), |(_, name)| name)
}

/// The first and the last of keys that are in order.
fn range_of(keys: &[u8]) -> Option<(u8, u8)> {
    Some((*keys.first()?, *keys.last()?))
}

fn map_of(sound: InstrumentRef, kit: bool) -> AiKeyMap {
    AiKeyMap {
        sound,
        kit,
        source: AiKeySource::None,
        certain: true,
        keys: Vec::new(),
        range: None,
        notes: Vec::new(),
        sf2_path: None,
    }
}

/// A key named by General MIDI's layout, if it names it.
fn guessed(key: u8, sounds: Option<bool>) -> AiKey {
    let name = gm_name(key);
    AiKey {
        key,
        name: name.map(str::to_owned),
        id: None,
        sounds,
        source: match name {
            Some(_) => AiKeySource::GeneralMidiGuess,
            None => AiKeySource::None,
        },
    }
}

/// General MIDI's drum names on the keys that sound, or on every key it names.
fn guess(map: &mut AiKeyMap, sounding: Option<&[u8]>) {
    map.keys = match sounding {
        Some(keys) => keys.iter().map(|&key| guessed(key, Some(true))).collect(),
        None => (GM_FIRST..GM_FIRST + GM_NAMES.len() as u8)
            .map(|key| guessed(key, None))
            .collect(),
    };
    map.source = AiKeySource::GeneralMidiGuess;
    map.certain = false;
}

impl Play {
    /// What each key of the sound of track `track` (a number from 1) is. A plugin is asked
    /// first: until it answers, [`AiKeyNames::Waiting`].
    pub(crate) fn ai_key_names(
        &mut self,
        track: u32,
        catalog: &Catalog,
        effects: &mut Vec<Effect>,
    ) -> Result<AiKeyNames, AiRefusal> {
        let t = (track.checked_sub(1))
            .and_then(|i| self.doc().tracks.get(i as usize))
            .ok_or(AiRefusal::NoSuchTrack(track))?;
        if t.kind.is_vocal() {
            return Err(AiRefusal::NotKeys);
        }
        let (id, sound) = (t.id, t.instrument.clone());
        let map = match &sound {
            InstrumentRef::Builtin(b) => self.builtin_keys(*b),
            InstrumentRef::Sf2(preset) => sf2_keys(preset, catalog),
            InstrumentRef::Plugin(plugin) => {
                let ticket = self.key_plugin_ticket(PluginPlace::Instrument(id))?;
                let Some(names) = self.key_asks.said.remove(&ticket) else {
                    self.ask_key_names(ticket, effects);
                    return Ok(AiKeyNames::Waiting);
                };
                let drums = (catalog.plugin(&plugin.key))
                    .is_some_and(|found| found.phrase == InstrumentGroup::Drums);
                plugin_keys(sound.clone(), names, drums)
            }
        };
        Ok(AiKeyNames::Known(Box::new(map)))
    }

    /// The ticket of the plugin at `place`, when it is loaded and sounds.
    fn key_plugin_ticket(&self, place: PluginPlace) -> Result<u64, AiRefusal> {
        self.doc().plugin(place).ok_or(AiRefusal::NoSuchPlugin)?;
        (self.plugin_tickets().into_iter())
            .find(|(p, _)| *p == place)
            .map(|(_, ticket)| ticket)
            .ok_or(AiRefusal::PluginNotLoaded)
    }

    fn ask_key_names(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let now = self.now_us;
        let asked = &mut self.key_asks.asked;
        asked.retain(|(_, at)| now.saturating_sub(*at) < ASK_AGAIN_US);
        if !asked.iter().any(|(t, _)| *t == ticket) {
            asked.push((ticket, now));
            let ask = PluginAsk::KeyNames;
            effects.push(Effect::AskPlugin { ticket, ask });
        }
    }

    /// The answer to [`PluginAsk::KeyNames`]; kept only when it was asked for.
    pub(super) fn key_names_said(&mut self, ticket: u64, names: Option<Vec<KeyName>>) {
        let asks = &mut self.key_asks;
        if asks.asked.iter().any(|(t, _)| *t == ticket) {
            asks.asked.retain(|(t, _)| *t != ticket);
            asks.said.insert(ticket, names);
        }
    }

    fn builtin_keys(&self, b: BuiltinInstrument) -> AiKeyMap {
        let kit = b.group() == InstrumentGroup::Drums;
        let mut map = map_of(InstrumentRef::Builtin(b), kit);
        let computed = !sampled(b);
        if !computed && !self.sounds.installed.contains(&b) {
            map.notes.push(AiKeyNote::NotInstalled);
        } else if !computed && !self.sounds.loaded.contains(&b) {
            map.notes.push(AiKeyNote::NotRead);
        }
        if kit {
            let mut pieces = DrumPiece::ROWS;
            pieces.sort_by_key(|p| p.note());
            map.keys = (pieces.iter())
                .map(|&piece| AiKey {
                    key: piece.note(),
                    name: Some(kit_name(piece).to_owned()),
                    id: Some(piece.id()),
                    sounds: computed.then_some(true),
                    source: AiKeySource::Builtin,
                })
                .collect();
            map.source = AiKeySource::Builtin;
            if computed {
                map.range = range_of(&pieces.map(DrumPiece::note));
            }
        } else if computed {
            map.range = Some((0, 127));
        }
        map
    }
}

fn sf2_keys(preset: &Arc<Sf2Ref>, catalog: &Catalog) -> AiKeyMap {
    let kit = preset.family() == GmFamily::DrumKit;
    let mut map = map_of(InstrumentRef::Sf2(preset.clone()), kit);
    match catalog.sf2_preset(&preset.file, preset.bank, preset.program) {
        None => {
            map.notes.push(AiKeyNote::Sf2Missing);
            if kit {
                guess(&mut map, None);
            }
        }
        // Until the file is read, the keys its zones are on (which the catalog has for
        // kits) are named after General MIDI.
        Some((_, listed)) if kit => {
            guess(&mut map, Some(&listed.keys));
            map.range = range_of(&listed.keys);
        }
        Some(_) => {}
    }
    map
}

/// A plugin that names keys is taken as a kit (its keys are different sounds); one that
/// names none is a kit when it was scanned as a drum instrument, and is then guessed.
fn plugin_keys(sound: InstrumentRef, names: Option<Vec<KeyName>>, drums: bool) -> AiKeyMap {
    let mut names: Vec<KeyName> = (names.unwrap_or_default().into_iter())
        .filter(|n| n.key <= 127 && !n.name.trim().is_empty())
        .collect();
    names.sort_by_key(|n| n.key);
    names.dedup_by_key(|n| n.key);
    let mut map = map_of(sound, drums || !names.is_empty());
    if !names.is_empty() {
        map.keys = (names.into_iter())
            .map(|n| AiKey {
                key: n.key,
                name: Some(n.name.trim().to_owned()),
                id: None,
                sounds: None,
                source: AiKeySource::Lv2Midnam,
            })
            .collect();
        map.source = AiKeySource::Lv2Midnam;
    } else if drums {
        guess(&mut map, None);
    }
    map
}

/// On the core thread, after the rules answered: the keys the samples in memory sound on,
/// and where the SF2 file is whose zones name the keys. Other answers pass as they are.
pub(crate) fn heard(answer: AiAnswer, sounds: &Library, sf2: &Sf2Store) -> AiAnswer {
    let AiAnswer::KeyNames(AiKeyNames::Known(mut map)) = answer else {
        return answer;
    };
    match &map.sound {
        InstrumentRef::Builtin(b) => {
            if let Samples::Ready(bank) = sounds.samples(*b) {
                sounding(&mut map, &bank);
            }
        }
        InstrumentRef::Sf2(preset) => map.sf2_path = sf2.path_of(&preset.file),
        InstrumentRef::Plugin(_) => {}
    }
    AiAnswer::KeyNames(AiKeyNames::Known(map))
}

/// What a bank sounds on: its range, and for a kit which of its keys sound.
fn sounding(map: &mut AiKeyMap, bank: &SampleBank) {
    let keys = bank.keys();
    map.range = range_of(&keys);
    if !map.kit {
        return;
    }
    for key in &mut map.keys {
        key.sounds = Some(keys.contains(&key.key));
    }
    for &key in &keys {
        if !map.keys.iter().any(|k| k.key == key) {
            map.keys.push(AiKey {
                key,
                name: None,
                id: None,
                sounds: Some(true),
                source: AiKeySource::None,
            });
        }
    }
    map.keys.sort_by_key(|k| k.key);
}

/// Off the core thread (it reads a file): the zones of the SF2 preset of `map` from its file.
/// A kit's keys get the names of the samples they play, in place of the guess; an instrument
/// gets the keys it plays. When the file cannot be read now, the map stays with a note.
pub fn read_sf2(map: &mut AiKeyMap) {
    let (Some(path), InstrumentRef::Sf2(preset)) = (map.sf2_path.take(), &map.sound) else {
        return;
    };
    let zones = Sf2File::open(&path)
        .and_then(|file| file.key_names(preset.bank, u16::from(preset.program)));
    let Ok(zones) = zones else {
        map.notes.push(AiKeyNote::Sf2Unreadable);
        return;
    };
    map.range = range_of(&zones.iter().map(|(key, _)| *key).collect::<Vec<u8>>());
    if !map.kit {
        return;
    }
    map.keys = (zones.into_iter())
        .map(|(key, name)| match name {
            Some(name) => AiKey {
                key,
                name: Some(name),
                id: None,
                sounds: Some(true),
                source: AiKeySource::Sf2SampleName,
            },
            None => guessed(key, Some(true)),
        })
        .collect();
    let has = |source| map.keys.iter().any(|k| k.source == source);
    let (named, guessed) = (
        has(AiKeySource::Sf2SampleName),
        has(AiKeySource::GeneralMidiGuess),
    );
    map.source = match (named, guessed) {
        (true, _) => AiKeySource::Sf2SampleName,
        (false, true) => AiKeySource::GeneralMidiGuess,
        (false, false) => AiKeySource::None,
    };
    map.certain = !guessed;
    if named {
        map.notes.push(AiKeyNote::SampleNames);
    }
}

#[cfg(test)]
mod tests;
