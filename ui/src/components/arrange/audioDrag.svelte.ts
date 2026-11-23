// Values while dragging the gain line or fade handles of an audio clip (only the screen moves first; release sends one setClipField or shiftClipGains).
// The handle and line parts (HTML) write them, and the waveform drawing (WebGL) reads them to change the waveform size and fade line in the same frame.

export type AudioDragValue = {
  /** ClipView.id */
  clip: number;
  gainDb?: number;
  /** Ticks */
  fadeIn?: number;
  /** Ticks */
  fadeOut?: number;
  /** While several gain lines move together: other clips (ClipView.id) also show their gain shifted by byDb (each stops at −60 / +12). */
  shift?: { clips: readonly number[]; byDb: number };
};

export const audioDrag = $state<{ now: AudioDragValue | null }>({ now: null });
