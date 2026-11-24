//! The names a plugin instrument gives its keys (LV2 midnam), as its plugin host tells them:
//! when it is loaded and each time they change. They are cleaned once and kept with the loaded
//! plugin; the AI link's answer (`play::keynames`) and the drum piano roll of the screen
//! (`TrackView::key_names`, `drum_keys`) both read them from here.

use std::sync::Arc;

use meloet_model::drums::gm_kit_rows;
use meloet_model::{InstrumentRef, TrackId};
use meloet_plugin::wire::KeyName;

use super::{Play, Spot};

/// The keys of `names` a keyboard can play that have a name, lowest first: none over 127, no
/// name that is only blanks, the name trimmed, one name for a key (the first given).
pub(crate) fn cleaned(names: Option<Vec<KeyName>>) -> Vec<KeyName> {
    let mut names: Vec<KeyName> = (names.unwrap_or_default().into_iter())
        .filter(|n| n.key <= 127 && !n.name.trim().is_empty())
        .map(|n| KeyName {
            key: n.key,
            name: n.name.trim().to_owned(),
        })
        .collect();
    names.sort_by_key(|n| n.key);
    names.dedup_by_key(|n| n.key);
    names
}

/// The rows of the drum piano roll for a plugin that names these keys, top row first: the
/// same order as the rows of an SF2 kit.
pub(crate) fn drum_rows(names: &[KeyName]) -> Vec<u8> {
    gm_kit_rows(&names.iter().map(|n| n.key).collect::<Vec<u8>>())
}

impl Play {
    /// `PluginSaid::KeyNames`: what the plugin names its keys from now on.
    pub(in crate::play) fn plug_key_names_said(
        &mut self,
        ticket: u64,
        names: Option<Vec<KeyName>>,
    ) {
        if let Some(plug) = self.plugs.of_mut(ticket) {
            let names = cleaned(names);
            plug.key_names = (!names.is_empty()).then(|| names.into());
        }
    }

    /// The keys the plugin that is the track's instrument names ([`cleaned`]); `None` while
    /// it is not loaded (yet), and when it names none.
    pub(crate) fn instrument_key_names(&self, track: TrackId) -> Option<Arc<[KeyName]>> {
        let InstrumentRef::Plugin(p) = &self.doc().track(track)?.instrument else {
            return None;
        };
        let plug = self.plugs.at(Spot::Seat(self.seats.of(track)?))?;
        (plug.plugin.key == p.key).then(|| plug.key_names.clone())?
    }

    /// The tracks whose instrument plugin names its keys now: kits, as the screen shows them
    /// and as a MIDI export writes them (channel 10).
    pub(crate) fn plugin_kits(&self) -> Vec<TrackId> {
        (self.doc().tracks.iter())
            .filter(|t| self.instrument_key_names(t.id).is_some())
            .map(|t| t.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(key: u8, name: &str) -> KeyName {
        KeyName {
            key,
            name: name.into(),
        }
    }

    #[test]
    fn only_playable_keys_with_names_are_kept_once_in_order() {
        let said = vec![
            named(38, " Snare Center "),
            named(36, "Kick Drum"),
            named(36, "Kick again"),
            named(200, "Too high"),
            named(128, "Too high"),
            named(40, "  "),
            named(0, "Lowest"),
            named(127, "Highest"),
        ];
        assert_eq!(
            cleaned(Some(said)),
            [
                named(0, "Lowest"),
                named(36, "Kick Drum"),
                named(38, "Snare Center"),
                named(127, "Highest")
            ]
        );
        assert!(cleaned(None).is_empty());
        assert!(cleaned(Some(vec![named(40, "")])).is_empty());
    }

    #[test]
    fn named_keys_are_rows_as_an_sf2_kit_has_them() {
        // Black Pearl's first keys: the cymbals and hats on top, the kick at the bottom, the
        // keys General MIDI orders by itself before the others from the lowest up.
        let names: Vec<KeyName> = [36, 38, 42, 49, 54, 60, 61]
            .into_iter()
            .map(|key| named(key, "x"))
            .collect();
        assert_eq!(drum_rows(&names), [49, 42, 38, 36, 54, 60, 61]);
        assert!(drum_rows(&[]).is_empty());
    }
}
