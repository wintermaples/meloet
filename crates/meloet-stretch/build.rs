//! Builds the small C door (`src/shim.cpp`) over the vendored Signalsmith Stretch headers.

fn main() {
    let watched = [
        "src/shim.cpp",
        "vendor/signalsmith-stretch/signalsmith-stretch.h",
        "vendor/signalsmith-linear/stft.h",
        "vendor/signalsmith-linear/fft.h",
        "vendor/signalsmith-linear/include/signalsmith-linear/stft.h",
    ];
    for path in watched {
        println!("cargo:rerun-if-changed={path}");
    }
    // -O3 also in debug builds (the library is about ten times slower without optimisation).
    // No -ffast-math: NaN checks must keep their meaning, and the headers refuse it on one compiler.
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .opt_level(3)
        .include("vendor/signalsmith-stretch")
        .include("vendor/signalsmith-linear/include")
        .file("src/shim.cpp")
        .compile("meloet_stretch_shim");
}
