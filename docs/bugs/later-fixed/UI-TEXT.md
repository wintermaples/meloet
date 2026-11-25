# UI-TEXT — fixed in the second pass (2026-11-25)

- ui/src/text/en.ts:1193,1573 — `clipParams.pitchValue` / `msg.pitchDragging` "{semitones} semitones {cents} cents" has no singular form ("+1 semitones · +1 cents"); two counts in one text, so an `n()` pair cannot fix it alone.
