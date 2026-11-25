# Later: seen by fix-ui-keys (bug sweep, 2026-11-25)

- While a dialog is open, the "main" keys still act behind it: ↑ ↓ ← → move the selected notes, Delete deletes a chosen marker / points / clips, one-letter keys work (`handleKeydown` in `ui/src/keys/attach.ts`; only the output chooser, the shelf and the history band now wait for the dialog).
- `PresetMenu.svelte`: while the focus is inside the open preset list, its `onkeydown` stops every key, so Mod S, Mod Z and Space do nothing there.
