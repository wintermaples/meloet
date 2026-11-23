//! LV2 presets (`pset:Preset`, docs/research/2026-11-23-plugin-presets-state.md Q9): listed
//! from the plugin's own bundle and from bundles that only describe presets, turned into a
//! state of meloet's form without touching the instance, and written the way jalv writes
//! them (`lilv_state_save`).
//!
//! A preset's data is often in a file of its own (its `rdfs:seeAlso`), and its `lv2:appliesTo`
//! only in the manifest: so the data is read into the world for the time it takes to make the
//! state from the world, as jalv does, and let go of again.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use livi::Features;
use livi::lilv;
use livi::state::State;
use meloet_plugin::wire::PresetInfo;

use super::{RDFS, STATE_MAGIC, STATE_URI, World, bundle_uri, file_path};

const PSET: &str = "http://lv2plug.in/ns/ext/presets#";
/// No manifest longer than this is looked into.
const MAX_MANIFEST: u64 = 4 << 20;
/// Bundles of presets read for one ask, at most.
const MAX_BUNDLES: usize = 256;
/// Presets with no label in a manifest whose data is read for their label, in one listing.
const MAX_LABEL_READS: usize = 64;

/// Whether the manifest of `bundle` describes presets and no plugin. A bundle with a plugin
/// is not read into a world that runs one: lilv may let go of the plugin it has for another
/// of the same URI.
fn only_presets(bundle: &Path) -> bool {
    let manifest = bundle.join("manifest.ttl");
    let Ok(meta) = std::fs::symlink_metadata(&manifest) else {
        return false;
    };
    if !meta.is_file() || meta.len() > MAX_MANIFEST {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return false;
    };
    text.contains("Preset") && !text.contains(":Plugin") && !text.contains("#Plugin>")
}

/// Reads into `world` the bundles of `bundles` it has not read yet, once each.
fn read_bundles(world: &World, bundles: &[PathBuf]) {
    for bundle in bundles.iter().take(MAX_BUNDLES) {
        if !world.preset_bundles.borrow_mut().insert(bundle.clone()) || !only_presets(bundle) {
            continue;
        }
        if let Ok(uri) = bundle_uri(bundle) {
            world.world.raw().load_bundle(&world.uri(&uri));
        }
    }
}

/// The last part of a URI, for a preset or a bank that has no label.
fn tail(uri: &str) -> String {
    let tail = uri
        .rsplit(['#', '/'])
        .find(|s| !s.is_empty())
        .unwrap_or(uri);
    tail.strip_suffix(".ttl").unwrap_or(tail).to_owned()
}

/// Reads the data of the preset `node` for `read`, and lets go of it again unless it is
/// data of the plugin itself.
fn with_data<T>(
    world: &World,
    plugin: &lilv::plugin::Plugin,
    node: &lilv::node::Node,
    read: impl FnOnce() -> T,
) -> T {
    let raw = world.world.raw();
    let files = raw.find_nodes(Some(node), &world.uri(&format!("{RDFS}seeAlso")), None);
    let own = plugin.data_uris();
    let shared = files.iter().any(|file| own.contains(&file));
    let loaded = raw.load_resource(node).is_some_and(|n| n > 0);
    let value = read();
    drop((files, own));
    if loaded && !shared {
        // SAFETY: nothing read from those files is held: a state is a copy of its own, and
        // the nodes of the queries are gone.
        let _ = unsafe { raw.unload_resource(node) };
    }
    value
}

/// The presets of the plugin `plugin_uri`, with those of the preset bundles `bundles`.
pub(super) fn list(
    world: &World,
    plugin_uri: &str,
    bundles: &[PathBuf],
) -> Result<Vec<PresetInfo>, String> {
    read_bundles(world, bundles);
    let raw = world.world.raw();
    let plugin = (raw.plugins().plugin(&world.uri(plugin_uri)))
        .ok_or("the plugin's bundle does not describe it any more")?;
    let Some(found) = plugin.related(Some(&world.uri(&format!("{PSET}Preset")))) else {
        return Ok(Vec::new());
    };
    let (label, bank, see_also) = (
        format!("{RDFS}label"),
        format!("{PSET}bank"),
        format!("{RDFS}seeAlso"),
    );
    let mut reads = 0;
    let mut presets = Vec::new();
    for preset in found.iter() {
        let Some(uri) = preset.as_uri().map(str::to_owned) else {
            continue;
        };
        let text = |node: &lilv::node::Node| {
            (world.text(node, &label)).filter(|label| !label.trim().is_empty())
        };
        let mut name = text(&preset);
        if name.is_none() && reads < MAX_LABEL_READS {
            reads += 1;
            name = with_data(world, &plugin, &preset, || text(&preset));
        }
        let bank = (world.get(&preset, &bank))
            .and_then(|bank| text(&bank).or_else(|| bank.as_uri().map(tail)));
        let file = (world.get(&preset, &see_also)).and_then(|file| file_path(file.as_uri()?));
        presets.push(PresetInfo {
            label: name.unwrap_or_else(|| tail(&uri)),
            uri,
            bank,
            file,
        });
    }
    // lilv gives them in no fixed order.
    presets.sort_by(|a, b| (&a.bank, &a.label, &a.uri).cmp(&(&b.bank, &b.label, &b.uri)));
    Ok(presets)
}

/// The state names a plugin, and it is `plugin_uri`: a state that names none would stop
/// this process when it is put back.
fn applies(state: &State, plugin_uri: &str) -> Result<(), String> {
    // SAFETY: a question about a state of lilv's own; the answer is not kept.
    let named = unsafe { lilv::sys::lilv_state_get_plugin_uri(state.raw().as_ptr()) };
    if named.is_null() || state.plugin_uri() != plugin_uri {
        return Err("the preset is not one of this plugin".into());
    }
    Ok(())
}

/// The state of the preset `uri` of the plugin `plugin_uri`, in the form of
/// `Lv2Controller::save_state`, with no `patch` values (the plugin says them once it has it).
pub(super) fn state(
    world: &World,
    features: &Arc<Features>,
    plugin_uri: &str,
    uri: &str,
    bundles: &[PathBuf],
) -> Result<Vec<u8>, String> {
    if !list(world, plugin_uri, bundles)?
        .iter()
        .any(|p| p.uri == uri)
    {
        return Err(format!("no preset {uri} applies to this plugin"));
    }
    let raw = world.world.raw();
    let plugin = (raw.plugins().plugin(&world.uri(plugin_uri)))
        .ok_or("the plugin's bundle does not describe it any more")?;
    let node = world.uri(uri);
    let state = with_data(world, &plugin, &node, || {
        world.world.state_from_uri(features, uri)
    })
    .map_err(|e| format!("the preset cannot be read ({e:?})"))?;
    applies(&state, plugin_uri)?;
    let text = (state.to_string(STATE_URI)).map_err(|e| format!("state: {e:?}"))?;
    let mut bytes = Vec::with_capacity(8 + text.len());
    bytes.extend_from_slice(STATE_MAGIC);
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    Ok(bytes)
}

/// Whether a preset may be written as `file` of the bundle folder `dir`.
pub(super) fn place_ok(dir: &Path, file: &str) -> Result<(), String> {
    let plain = |c: &Component| matches!(c, Component::RootDir | Component::Normal(_));
    if !dir.is_absolute()
        || !dir.components().all(|c| plain(&c))
        || file.contains('/')
        || !file.ends_with(".ttl")
        || file.starts_with('.')
    {
        return Err("not a place a preset is written to".into());
    }
    Ok(())
}

/// Writes `state`, made from the instance, as the preset `label` of `plugin_uri`: the file
/// `file` in the bundle folder `dir`, and its line in the bundle's manifest. The preset as
/// [`list`] gives it from now on. (lilv 0.26.2 stops the process when it writes a state made
/// from a text.)
pub(super) fn save(
    world: &World,
    plugin_uri: &str,
    mut state: State,
    dir: &Path,
    file: &str,
    label: &str,
) -> Result<PresetInfo, String> {
    place_ok(dir, file)?;
    applies(&state, plugin_uri)?;
    state.set_label(label);
    (state.save(dir, file, None)).map_err(|e| format!("the preset was not written ({e:?})"))?;
    // Read again, so that it is listed from now on.
    let uri = bundle_uri(dir)?;
    world.world.raw().load_bundle(&world.uri(&uri));
    world.preset_bundles.borrow_mut().insert(dir.to_path_buf());
    let path = dir.join(file);
    let written = path.canonicalize().map_err(|e| e.to_string())?;
    (list(world, plugin_uri, &[])?.into_iter())
        .find(|p| (p.file.as_ref()).and_then(|f| f.canonicalize().ok()) == Some(written.clone()))
        .ok_or_else(|| format!("{} was written, but lilv does not list it", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_comes_from_the_end_of_a_uri() {
        assert_eq!(tail("urn:zamaudio:ZamComp#preset001"), "preset001");
        assert_eq!(
            tail("file:///home/me/.lv2/X_Soft.preset.lv2/Soft.ttl"),
            "Soft"
        );
        assert_eq!(tail("http://example.org/banks/"), "banks");
    }

    #[test]
    fn a_preset_is_written_only_into_a_bundle_folder() {
        assert!(place_ok(Path::new("/home/me/.lv2/X_Soft.preset.lv2"), "Soft.ttl").is_ok());
        for (dir, file) in [
            ("relative/X.preset.lv2", "Soft.ttl"),
            ("/home/me/.lv2/../X.preset.lv2", "Soft.ttl"),
            ("/home/me/.lv2/X.preset.lv2", "../Soft.ttl"),
            ("/home/me/.lv2/X.preset.lv2", ".ttl"),
            ("/home/me/.lv2/X.preset.lv2", "Soft.txt"),
        ] {
            assert!(place_ok(Path::new(dir), file).is_err(), "{dir} {file}");
        }
    }
}
