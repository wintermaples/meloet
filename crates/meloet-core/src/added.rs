//! SF2 presets and plugins of the song as the screen names them, and which of them this PC
//! does not have.

use std::path::Path;

use meloet_model::{BuiltinInstrument, Doc, Insert, Inserts, InstrumentRef, PluginRef};
use meloet_plugin::PluginKey;
use meloet_protocol::{
    EffectKind, EffectRef, GmFamily, INSERT_SLOTS, InsertEffectView, InsertView, LibraryView,
    MissingView, PluginFormat, PluginRole, PluginState, PluginView, SlotsView, SoundName, SoundRef,
    SoundView, StandIn,
};

use crate::instruments;
use crate::play::mixer::effect_kind;
use crate::plugins::shown_path;

/// The sound of meloet an instrument is made as. An SF2 preset and a plugin have none of
/// their own here: they sound as the sine wave.
#[cfg(test)]
pub(crate) fn builtin_or_sine(instrument: &InstrumentRef) -> BuiltinInstrument {
    match instrument {
        InstrumentRef::Builtin(b) => *b,
        InstrumentRef::Plugin(_) | InstrumentRef::Sf2(_) => BuiltinInstrument::Sine,
    }
}

pub(crate) fn family(family: meloet_model::GmFamily) -> GmFamily {
    use meloet_model::GmFamily as M;
    match family {
        M::Piano => GmFamily::Piano,
        M::Mallet => GmFamily::Mallet,
        M::Organ => GmFamily::Organ,
        M::Guitar => GmFamily::Guitar,
        M::Bass => GmFamily::Bass,
        M::Strings => GmFamily::Strings,
        M::Ensemble => GmFamily::Ensemble,
        M::Brass => GmFamily::Brass,
        M::Reed => GmFamily::Reed,
        M::Pipe => GmFamily::Pipe,
        M::SynthLead => GmFamily::SynthLead,
        M::SynthPad => GmFamily::SynthPad,
        M::SynthEffects => GmFamily::SynthEffects,
        M::Ethnic => GmFamily::Ethnic,
        M::Percussive => GmFamily::Percussive,
        M::SoundEffects => GmFamily::SoundEffects,
        M::DrumKit => GmFamily::DrumKit,
    }
}

/// `None` for a key of a format this meloet does not read.
pub(crate) fn format_of(key: &str) -> Option<PluginFormat> {
    let key: PluginKey = key.parse().ok()?;
    Some(match key.format() {
        meloet_plugin::PluginFormat::Vst3 => PluginFormat::Vst3,
        meloet_plugin::PluginFormat::Lv2 => PluginFormat::Lv2,
    })
}

pub(crate) fn plugin_view(plugin: &PluginRef, state: PluginState) -> PluginView {
    PluginView {
        key: plugin.key.to_string(),
        name: plugin.name.to_string(),
        format: format_of(&plugin.key),
        state,
    }
}

/// `plugin`: how the plugin is, when the instrument is one.
pub(crate) fn sound_view(instrument: &InstrumentRef, plugin: PluginState) -> SoundView {
    match instrument {
        InstrumentRef::Builtin(b) => SoundView::Builtin {
            instrument: instruments::kind_of(*b),
        },
        InstrumentRef::Plugin(p) => SoundView::Plugin(plugin_view(p, plugin)),
        InstrumentRef::Sf2(s) => SoundView::Sf2 {
            file: s.file.to_string(),
            bank: s.bank,
            program: s.program,
            name: s.name.to_string(),
            family: family(s.family()),
        },
    }
}

pub(crate) fn sound_ref(instrument: &InstrumentRef) -> SoundRef {
    match instrument {
        InstrumentRef::Builtin(b) => SoundRef::Builtin {
            instrument: instruments::kind_of(*b),
        },
        InstrumentRef::Plugin(p) => SoundRef::Plugin {
            key: p.key.to_string(),
        },
        InstrumentRef::Sf2(s) => SoundRef::Sf2 {
            file: s.file.to_string(),
            bank: s.bank,
            program: s.program,
        },
    }
}

pub(crate) fn sound_name(instrument: &InstrumentRef) -> SoundName {
    match instrument {
        InstrumentRef::Builtin(b) => SoundName::Builtin {
            instrument: instruments::kind_of(*b),
        },
        InstrumentRef::Plugin(p) => SoundName::Plugin {
            name: p.name.to_string(),
        },
        InstrumentRef::Sf2(s) => SoundName::Sf2 {
            name: s.name.to_string(),
            file: s.file.to_string(),
        },
    }
}

pub(crate) fn effect_ref(kind: EffectKind) -> EffectRef {
    EffectRef::Builtin { effect: kind }
}

/// `plugin`: how the plugin of a slot is, by the number of the slot.
pub(crate) fn slots_view(inserts: &Inserts, plugin: impl Fn(usize) -> PluginState) -> SlotsView {
    let mut view: SlotsView = [const { None }; INSERT_SLOTS];
    for (slot, (to, from)) in view.iter_mut().zip(inserts).enumerate() {
        *to = from.as_ref().map(|i| insert_view(i, plugin(slot)));
    }
    view
}

fn insert_view(insert: &Insert, plugin: PluginState) -> InsertView {
    let effect = match (insert.kind(), insert.plugin_ref()) {
        (Some(kind), _) => InsertEffectView::Builtin {
            effect: effect_kind(kind),
        },
        (None, Some(p)) => InsertEffectView::Plugin(plugin_view(p, plugin)),
        (None, None) => unreachable!("an insert is an effect of meloet or a plugin"),
    };
    InsertView {
        effect,
        bypassed: insert.bypassed,
    }
}

/// The exact key of every plugin the song holds, instruments and effects.
pub(crate) fn plugin_keys(doc: &Doc) -> Vec<&str> {
    fn of(inserts: &Inserts) -> impl Iterator<Item = &str> {
        (inserts.iter().flatten())
            .filter_map(Insert::plugin_ref)
            .map(|p| &*p.key)
    }
    let mut keys = Vec::new();
    for t in &doc.tracks {
        if let InstrumentRef::Plugin(p) = &t.instrument {
            keys.push(&*p.key);
        }
        keys.extend(of(&t.channel.inserts));
    }
    for r in &doc.returns {
        keys.extend(of(&r.channel.inserts));
    }
    keys.extend(of(&doc.master.inserts));
    keys
}

fn has_sf2(library: &LibraryView, file: &str, bank: u16, program: u8) -> bool {
    library.sf2.iter().any(|f| {
        f.file == file && (f.presets.iter()).any(|p| p.bank == bank && p.program == program)
    })
}

fn has_plugin(library: &LibraryView, key: &str) -> bool {
    (library.plugins.iter()).any(|row| row.formats.iter().any(|f| f.key == key))
}

/// Why a track with `instrument` sounds as the sine wave, as far as the library tells:
/// its SF2 file or its preset is not there, or its plugin is not. `sounds`: the sounds of
/// meloet that can sound on this PC.
pub(crate) fn stand_in(
    instrument: &InstrumentRef,
    library: &LibraryView,
    sounds: &[BuiltinInstrument],
) -> Option<StandIn> {
    match instrument {
        InstrumentRef::Builtin(b) => (!sounds.contains(b)).then_some(StandIn::PackMissing),
        InstrumentRef::Sf2(s) => {
            (!has_sf2(library, &s.file, s.bank, s.program)).then_some(StandIn::Sf2Missing)
        }
        InstrumentRef::Plugin(p) => {
            (!has_plugin(library, &p.key)).then_some(StandIn::PluginMissing)
        }
    }
}

/// What the song uses and the library does not have: SF2 files by their names first, in the
/// order of the tracks, then plugins, instruments before the effects of the same track, the
/// returns and the master last. Each once. An SF2 file that is there but lacks a preset the
/// song uses is not one of these: the file is found.
pub(crate) fn missing(doc: &Doc, library: &LibraryView, home: Option<&Path>) -> Vec<MissingView> {
    let mut files: Vec<(String, u32)> = Vec::new();
    let mut plugins: Vec<MissingView> = Vec::new();
    let mut plugin = |p: &PluginRef, role: PluginRole| {
        let known =
            |m: &MissingView| matches!(m, MissingView::Plugin { key, .. } if *key == *p.key);
        if has_plugin(library, &p.key) || plugins.iter().any(known) {
            return;
        }
        plugins.push(MissingView::Plugin {
            key: p.key.to_string(),
            name: p.name.to_string(),
            vendor: p.vendor.to_string(),
            format: format_of(&p.key),
            role,
            shown_path: (!p.path.is_empty()).then(|| shown_path(Path::new(&*p.path), home)),
        });
    };
    let effects = |inserts: &Inserts, plugin: &mut dyn FnMut(&PluginRef, PluginRole)| {
        for p in inserts.iter().flatten().filter_map(Insert::plugin_ref) {
            plugin(p, PluginRole::Effect);
        }
    };
    for t in &doc.tracks {
        match &t.instrument {
            InstrumentRef::Builtin(_) => {}
            InstrumentRef::Plugin(p) => plugin(p, PluginRole::Instrument),
            InstrumentRef::Sf2(s) if library.sf2.iter().any(|f| *f.file == *s.file) => {}
            InstrumentRef::Sf2(s) => match files.iter_mut().find(|(file, _)| *file == *s.file) {
                Some((_, tracks)) => *tracks += 1,
                None => files.push((s.file.to_string(), 1)),
            },
        }
        effects(&t.channel.inserts, &mut plugin);
    }
    for r in &doc.returns {
        effects(&r.channel.inserts, &mut plugin);
    }
    effects(&doc.master.inserts, &mut plugin);
    (files.into_iter())
        .map(|(file, tracks)| MissingView::Sf2 { file, tracks })
        .chain(plugins)
        .collect()
}

#[cfg(test)]
mod tests;
