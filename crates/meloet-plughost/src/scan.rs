//! `scan` mode: looks into one bundle and writes what it found as one frame
//! (`meloet_plugin::wire::ScanOutput`). Whatever goes wrong in here takes only this process
//! with it; meloet sees a process that died or did not end in time.

use std::os::fd::OwnedFd;
use std::path::Path;

use meloet_plugin::PluginFormat;
use meloet_plugin::wire::{ScanOutput, write_frame};

use crate::formats::{fake, lv2, vst3};

pub fn scan(format: PluginFormat, bundle: &Path) -> ScanOutput {
    if let Some(output) = fake::scan(bundle) {
        return output;
    }
    match format {
        PluginFormat::Vst3 => vst3::scan(bundle),
        PluginFormat::Lv2 => lv2::scan(bundle),
    }
}

/// The process's exit code.
pub fn run(format: PluginFormat, bundle: &Path, out: OwnedFd) -> i32 {
    let output = scan(format, bundle);
    match write_frame(&mut std::fs::File::from(out), &output, &[]) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("meloet-plughost: {e}");
            2
        }
    }
}
