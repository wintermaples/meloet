// The C door to Signalsmith Stretch. Every function is noexcept and catches everything:
// a C++ exception must never cross into Rust. A function that can fail returns 0 for success, -1 for failure.

#include "signalsmith-stretch.h"

namespace {
using Stretch = signalsmith::stretch::SignalsmithStretch<float>;

template <class F>
int guarded(F &&f) noexcept {
	try {
		f();
		return 0;
	} catch (...) {
		return -1;
	}
}
} // namespace

extern "C" {

struct MeloetStretch {
	Stretch stretch;
	explicit MeloetStretch(long seed) : stretch(seed) {}
};

// A fixed seed: the same input gives the same output (the library seeds from std::random_device otherwise).
MeloetStretch *meloet_stretch_new(long seed) noexcept {
	try {
		return new MeloetStretch(seed);
	} catch (...) {
		return nullptr;
	}
}

void meloet_stretch_free(MeloetStretch *s) noexcept {
	delete s;
}

int meloet_stretch_preset_default(MeloetStretch *s, int channels, float sampleRate) noexcept {
	return guarded([&] { s->stretch.presetDefault(channels, sampleRate, false); });
}

int meloet_stretch_preset_cheaper(MeloetStretch *s, int channels, float sampleRate) noexcept {
	return guarded([&] { s->stretch.presetCheaper(channels, sampleRate, false); });
}

int meloet_stretch_configure(MeloetStretch *s, int channels, int blockSamples, int intervalSamples) noexcept {
	return guarded([&] { s->stretch.configure(channels, blockSamples, intervalSamples, false); });
}

int meloet_stretch_block_samples(const MeloetStretch *s) noexcept {
	return s->stretch.blockSamples();
}

int meloet_stretch_interval_samples(const MeloetStretch *s) noexcept {
	return s->stretch.intervalSamples();
}

int meloet_stretch_input_latency(const MeloetStretch *s) noexcept {
	return s->stretch.inputLatency();
}

int meloet_stretch_output_latency(const MeloetStretch *s) noexcept {
	return s->stretch.outputLatency();
}

int meloet_stretch_seek_length(const MeloetStretch *s) noexcept {
	return s->stretch.seekLength();
}

int meloet_stretch_set_transpose_semitones(MeloetStretch *s, float semitones, float tonalityLimit) noexcept {
	return guarded([&] { s->stretch.setTransposeSemitones(semitones, tonalityLimit); });
}

int meloet_stretch_set_formant_factor(MeloetStretch *s, float multiplier, int compensatePitch) noexcept {
	return guarded([&] { s->stretch.setFormantFactor(multiplier, compensatePitch != 0); });
}

int meloet_stretch_set_formant_base(MeloetStretch *s, float baseFreq) noexcept {
	return guarded([&] { s->stretch.setFormantBase(baseFreq); });
}

int meloet_stretch_reset(MeloetStretch *s) noexcept {
	return guarded([&] { s->stretch.reset(); });
}

int meloet_stretch_seek(MeloetStretch *s, const float *const *inputs, int inputSamples, double playbackRate) noexcept {
	return guarded([&] { s->stretch.seek(inputs, inputSamples, playbackRate); });
}

int meloet_stretch_output_seek(MeloetStretch *s, const float *const *inputs, int inputSamples) noexcept {
	return guarded([&] { s->stretch.outputSeek(inputs, inputSamples); });
}

int meloet_stretch_process(MeloetStretch *s, const float *const *inputs, int inputSamples, float *const *outputs, int outputSamples) noexcept {
	return guarded([&] { s->stretch.process(inputs, inputSamples, outputs, outputSamples); });
}

int meloet_stretch_flush(MeloetStretch *s, float *const *outputs, int outputSamples, float playbackRate) noexcept {
	return guarded([&] { s->stretch.flush(outputs, outputSamples, playbackRate); });
}

} // extern "C"
