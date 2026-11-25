# UI-TEXT — 2 回目で直した点 (2026-11-25)

- ui/src/text/en.ts:1193,1573 — `clipParams.pitchValue` / `msg.pitchDragging` の "{semitones} semitones {cents} cents" には単数形がありません("+1 semitones · +1 cents" になります)。数が二つある文面なので、`n()` の組だけでは直せません。
