//! The C door of `shim.cpp`, written by hand. Every function is `noexcept` on the other side.
//! The ones returning `c_int` give 0 for success and -1 when the library threw.

use std::ffi::{c_double, c_float, c_int, c_long};

#[repr(C)]
pub struct MeloetStretch {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    pub fn meloet_stretch_new(seed: c_long) -> *mut MeloetStretch;
    pub fn meloet_stretch_free(s: *mut MeloetStretch);
    pub fn meloet_stretch_preset_default(
        s: *mut MeloetStretch,
        channels: c_int,
        sample_rate: c_float,
    ) -> c_int;
    pub fn meloet_stretch_preset_cheaper(
        s: *mut MeloetStretch,
        channels: c_int,
        sample_rate: c_float,
    ) -> c_int;
    pub fn meloet_stretch_configure(
        s: *mut MeloetStretch,
        channels: c_int,
        block: c_int,
        interval: c_int,
    ) -> c_int;
    pub fn meloet_stretch_block_samples(s: *const MeloetStretch) -> c_int;
    pub fn meloet_stretch_interval_samples(s: *const MeloetStretch) -> c_int;
    pub fn meloet_stretch_input_latency(s: *const MeloetStretch) -> c_int;
    pub fn meloet_stretch_output_latency(s: *const MeloetStretch) -> c_int;
    pub fn meloet_stretch_seek_length(s: *const MeloetStretch) -> c_int;
    pub fn meloet_stretch_set_transpose_semitones(
        s: *mut MeloetStretch,
        semitones: c_float,
        tonality_limit: c_float,
    ) -> c_int;
    pub fn meloet_stretch_set_formant_factor(
        s: *mut MeloetStretch,
        multiplier: c_float,
        compensate_pitch: c_int,
    ) -> c_int;
    pub fn meloet_stretch_set_formant_base(s: *mut MeloetStretch, base: c_float) -> c_int;
    pub fn meloet_stretch_reset(s: *mut MeloetStretch) -> c_int;
    pub fn meloet_stretch_seek(
        s: *mut MeloetStretch,
        inputs: *const *const c_float,
        frames: c_int,
        playback_rate: c_double,
    ) -> c_int;
    pub fn meloet_stretch_output_seek(
        s: *mut MeloetStretch,
        inputs: *const *const c_float,
        frames: c_int,
    ) -> c_int;
    pub fn meloet_stretch_process(
        s: *mut MeloetStretch,
        inputs: *const *const c_float,
        in_frames: c_int,
        outputs: *const *mut c_float,
        out_frames: c_int,
    ) -> c_int;
    pub fn meloet_stretch_flush(
        s: *mut MeloetStretch,
        outputs: *const *mut c_float,
        frames: c_int,
        playback_rate: c_float,
    ) -> c_int;
}
