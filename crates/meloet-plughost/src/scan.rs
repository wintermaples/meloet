//! `scan` mode: looks into one bundle and writes what it finds, one plugin after another, as
//! frames of `meloet_plugin::wire::ScanOutput`. Each plugin's `Probing` is written before
//! the plugin is read, so when reading it takes this process down (a crash, a hang), meloet
//! knows which plugin it was and keeps the ones written before.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;

use meloet_plugin::wire::{ScanOutput, WireError, write_frame};
use meloet_plugin::{PluginFormat, PluginInfo};

#[cfg(any(test, feature = "dev-hooks"))]
use crate::formats::fake;
use crate::formats::{Candidate, lv2, vst3};

fn list(format: PluginFormat, bundle: &Path) -> Result<Vec<Candidate>, String> {
    #[cfg(any(test, feature = "dev-hooks"))]
    if let Some(listed) = fake::list(bundle) {
        return listed;
    }
    match format {
        PluginFormat::Vst3 => vst3::list(bundle),
        PluginFormat::Lv2 => lv2::list(bundle),
    }
}

fn probe(format: PluginFormat, bundle: &Path, id: &str) -> Result<PluginInfo, String> {
    #[cfg(any(test, feature = "dev-hooks"))]
    if let Some(probed) = fake::probe(bundle, id) {
        return probed;
    }
    match format {
        PluginFormat::Vst3 => vst3::probe(bundle, id),
        PluginFormat::Lv2 => lv2::probe(bundle, id),
    }
}

/// Writes every frame of the scan to `out`, the plugins whose ids are in `skip` left out.
pub fn scan(
    format: PluginFormat,
    bundle: &Path,
    skip: &[String],
    out: &mut File,
) -> Result<(), WireError> {
    let candidates = match list(format, bundle) {
        Ok(candidates) => candidates,
        Err(message) => return write_frame(out, &ScanOutput::Broken { message }, &[]),
    };
    for Candidate { id, name } in candidates {
        if skip.contains(&id) {
            continue;
        }
        write_frame(
            out,
            &ScanOutput::Probing {
                id: id.clone(),
                name,
            },
            &[],
        )?;
        let output = match probe(format, bundle, &id) {
            Ok(plugin) => ScanOutput::Found { plugin },
            Err(message) => ScanOutput::PluginBroken { id, message },
        };
        write_frame(out, &output, &[])?;
    }
    write_frame(out, &ScanOutput::Done, &[])
}

/// The process's exit code.
pub fn run(format: PluginFormat, bundle: &Path, skip: &[String], out: OwnedFd) -> i32 {
    match scan(format, bundle, skip, &mut File::from(out)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("meloet-plughost: {e}");
            2
        }
    }
}
